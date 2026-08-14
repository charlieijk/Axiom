export type ControllerKind = "cpg" | "feedforward" | "recurrent";
export type TerrainKind = "rough" | "flat" | "recovery";
export type Vec2 = [number, number];

export type TrialConfig = {
  controller: ControllerKind;
  terrain: TerrainKind;
  seed: number;
  steps: number;
  dt: number;
};

type TrialConfigInput = {
  controller?: string;
  terrain?: string;
  seed?: number;
  steps?: number;
  dt?: number;
};

export type BodyNode = {
  id: number;
  parent: number | null;
  attachment: "right" | "left" | "top" | "bottom";
  size: Vec2;
  actuator: number;
};

export type ReplayFrame = {
  time: number;
  root: Vec2;
  tilt: number;
  bodies: Vec2[];
  joints: [Vec2, Vec2][];
};

export type TrialMetrics = {
  fitness: number;
  distance: number;
  stableDistance: number;
  stability: number;
  energy: number;
};

export type DeterministicTrial = TrialConfig & {
  body: BodyNode[];
  frames: ReplayFrame[];
  metrics: TrialMetrics;
};

type JointState = {
  nodeId: number;
  attachment: BodyNode["attachment"];
  angle: number;
  angularVelocity: number;
};

type SimulationState = {
  time: number;
  root: Vec2;
  velocity: Vec2;
  tilt: number;
  angularVelocity: number;
  energy: number;
  joints: JointState[];
};

const DEFAULT_CONFIG: TrialConfig = {
  controller: "cpg",
  terrain: "rough",
  seed: 19,
  steps: 220,
  dt: 0.05,
};

const CONTROLLERS = new Set<ControllerKind>([
  "cpg",
  "feedforward",
  "recurrent",
]);
const TERRAINS = new Set<TerrainKind>(["rough", "flat", "recovery"]);

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function finiteNumber(value: number | undefined, fallback: number) {
  return Number.isFinite(value) ? Number(value) : fallback;
}

export function normalizeTrialConfig(input: TrialConfigInput): TrialConfig {
  const controller = CONTROLLERS.has(input.controller as ControllerKind)
    ? (input.controller as ControllerKind)
    : DEFAULT_CONFIG.controller;
  const terrain = TERRAINS.has(input.terrain as TerrainKind)
    ? (input.terrain as TerrainKind)
    : DEFAULT_CONFIG.terrain;

  return {
    controller,
    terrain,
    seed: clamp(Math.trunc(finiteNumber(input.seed, DEFAULT_CONFIG.seed)), 1, 999_999),
    steps: clamp(Math.trunc(finiteNumber(input.steps, DEFAULT_CONFIG.steps)), 30, 360),
    dt: clamp(finiteNumber(input.dt, DEFAULT_CONFIG.dt), 0.02, 0.1),
  };
}

class Rng {
  private state: bigint;

  constructor(seed: number) {
    this.state = BigInt(Math.max(1, Math.trunc(seed)));
  }

  nextU64() {
    let value = this.state;
    value = BigInt.asUintN(64, value ^ BigInt.asUintN(64, value << BigInt(13)));
    value = BigInt.asUintN(64, value ^ (value >> BigInt(7)));
    value = BigInt.asUintN(64, value ^ BigInt.asUintN(64, value << BigInt(17)));
    this.state = value;
    return value;
  }

  nextFloat() {
    return Number(this.nextU64() >> BigInt(40)) / 16_777_216;
  }

  range(min: number, max: number) {
    return min + (max - min) * this.nextFloat();
  }
}

function seedQuadruped(): BodyNode[] {
  return [
    { id: 0, parent: null, attachment: "right", size: [1.2, 0.55], actuator: 0 },
    { id: 1, parent: 0, attachment: "left", size: [0.45, 0.2], actuator: 0.9 },
    { id: 2, parent: 0, attachment: "right", size: [0.45, 0.2], actuator: 0.9 },
    { id: 3, parent: 0, attachment: "top", size: [0.22, 0.45], actuator: 0.5 },
    { id: 4, parent: 0, attachment: "bottom", size: [0.22, 0.55], actuator: 1 },
  ];
}

function sensorCount(body: BodyNode[]) {
  const actuatorCount = body.filter((node) => node.parent !== null && Math.abs(node.actuator) > 0.01).length;
  return 12 + actuatorCount * 2 + body.length * 2;
}

function createWeights(controller: ControllerKind, body: BodyNode[], rng: Rng) {
  const outputCount = body.filter((node) => node.parent !== null && Math.abs(node.actuator) > 0.01).length;
  const inputCount = sensorCount(body) + (controller === "recurrent" ? outputCount : 0);
  const weights = Array.from({ length: outputCount }, () => Array(inputCount + 1).fill(0));

  for (let input = 0; input <= inputCount; input += 1) {
    for (let output = 0; output < outputCount; output += 1) {
      weights[output][input] = rng.range(-1, 1);
    }
  }

  return { inputCount, outputCount, weights };
}

function terrainHeight(terrain: TerrainKind, x: number) {
  if (terrain === "flat") return 0;
  return Math.sin(x * 1.7) * 0.12 + Math.cos(x * 0.47) * 0.08;
}

function terrainSlope(terrain: TerrainKind, x: number) {
  return (terrainHeight(terrain, x + 0.05) - terrainHeight(terrain, x - 0.05)) / 0.1;
}

function normal(attachment: BodyNode["attachment"]): Vec2 {
  switch (attachment) {
    case "left":
      return [-1, 0];
    case "top":
      return [0, 1];
    case "bottom":
      return [0, -1];
    default:
      return [1, 0];
  }
}

function childOffset(parent: BodyNode, child: BodyNode): Vec2 {
  const direction = normal(child.attachment);
  return [
    direction[0] * (parent.size[0] + child.size[0]) * 0.5,
    direction[1] * (parent.size[1] + child.size[1]) * 0.5,
  ];
}

function rotate([x, y]: Vec2, radians: number): Vec2 {
  const sin = Math.sin(radians);
  const cos = Math.cos(radians);
  return [x * cos - y * sin, x * sin + y * cos];
}

function bodyPose(body: BodyNode[], state: SimulationState) {
  const centers: Vec2[] = body.map(() => [...state.root] as Vec2);
  const rotations = body.map(() => state.tilt);
  const jointAngles = body.map(() => 0);

  for (const joint of state.joints) jointAngles[joint.nodeId] = joint.angle;
  for (const node of body) {
    if (node.parent === null) continue;
    const parent = body[node.parent];
    const rotation = rotations[node.parent] + jointAngles[node.id];
    const offset = rotate(childOffset(parent, node), rotation);
    centers[node.id] = [
      centers[node.parent][0] + offset[0],
      centers[node.parent][1] + offset[1],
    ];
    rotations[node.id] = rotation;
  }

  return centers;
}

function replayFrame(body: BodyNode[], state: SimulationState): ReplayFrame {
  const bodies = bodyPose(body, state);
  const joints = body.flatMap<[Vec2, Vec2]>((node) =>
    node.parent === null ? [] : [[bodies[node.parent], bodies[node.id]]],
  );

  return {
    time: state.time,
    root: [...state.root] as Vec2,
    tilt: state.tilt,
    bodies,
    joints,
  };
}

function observations(body: BodyNode[], state: SimulationState, terrain: TerrainKind) {
  const actuatorCount = body.filter((node) => node.parent !== null && Math.abs(node.actuator) > 0.01).length;
  const values = [
    state.time,
    state.root[0],
    state.root[1],
    state.velocity[0],
    state.velocity[1],
    state.tilt,
    state.angularVelocity,
    state.energy,
    terrainHeight(terrain, state.root[0]),
    terrainSlope(terrain, state.root[0]),
    body.length / 10,
    actuatorCount / 10,
  ];

  for (const joint of state.joints) {
    const node = body[joint.nodeId];
    if (Math.abs(node.actuator) > 0.01) values.push(joint.angle, joint.angularVelocity);
  }
  for (const node of body) values.push(node.size[0], node.size[1]);
  while (values.length < sensorCount(body)) values.push(0);
  return values;
}

function networkForward(inputs: number[], weights: number[][], inputCount: number) {
  return weights.map((outputWeights) => {
    let sum = outputWeights[inputCount];
    for (let index = 0; index < inputCount; index += 1) {
      sum += (inputs[index] ?? 0) * outputWeights[index];
    }
    return Math.tanh(sum);
  });
}

function stepSimulation(
  body: BodyNode[],
  state: SimulationState,
  actions: number[],
  terrain: TerrainKind,
  dt: number,
) {
  let thrust = 0;
  let lift = 0;
  let torque = 0;
  let actionIndex = 0;

  for (const joint of state.joints) {
    const node = body[joint.nodeId];
    const actuated = Math.abs(node.actuator) > 0.01;
    const action = actuated ? clamp(actions[actionIndex++] ?? 0, -1, 1) : 0;
    const targetVelocity = action * node.actuator * 3;
    joint.angularVelocity += (targetVelocity - joint.angularVelocity) * 0.35;
    joint.angle = clamp(joint.angle + joint.angularVelocity * dt, -1.4, 1.4);

    const direction = normal(joint.attachment);
    thrust += action * node.actuator * Math.max(Math.abs(direction[0]), 0.35);
    lift += Math.abs(action) * node.actuator * Math.max(direction[1], 0) * 0.12;
    torque += action * node.actuator * direction[1] * 0.05;
    state.energy += Math.abs(action) * node.actuator * dt;
  }

  const gravity = 9.8;
  const friction = terrain === "flat" ? 0.14 : terrain === "recovery" ? 0.28 : 0.18;
  state.velocity[0] += (thrust / body.length) * dt;
  state.velocity[1] += lift * dt - gravity * 0.05 * dt;
  state.velocity[0] *= 1 - friction * dt;
  state.root[0] += state.velocity[0] * dt;
  state.root[1] += state.velocity[1] * dt;

  const floor = terrainHeight(terrain, state.root[0]) + 0.35;
  if (state.root[1] < floor) {
    state.root[1] = floor;
    state.velocity[1] = Math.max(0, state.velocity[1]);
  }

  state.angularVelocity += torque - terrainSlope(terrain, state.root[0]) * 0.02;
  state.angularVelocity *= 0.96;
  state.tilt = clamp(state.tilt + state.angularVelocity * dt, -1.5, 1.5);
  state.time += dt;
}

function tiltStability(tilt: number) {
  return clamp(1 - Math.abs(tilt) / 1.5, 0, 1);
}

export function runDeterministicTrial(input: TrialConfigInput): DeterministicTrial {
  const config = normalizeTrialConfig(input);
  const body = seedQuadruped();
  const rng = new Rng(config.seed);
  const network = createWeights(config.controller, body, rng);
  const state: SimulationState = {
    time: 0,
    root: [0, terrainHeight(config.terrain, 0) + 0.6],
    velocity: [0, 0],
    tilt: 0,
    angularVelocity: 0,
    energy: 0,
    joints: body.slice(1).map((node) => ({
      nodeId: node.id,
      attachment: node.attachment,
      angle: 0,
      angularVelocity: 0,
    })),
  };
  const recurrent = Array(network.outputCount).fill(0);
  let oscillatorPhase = 0;
  let maxHeight = 0;
  let uprightAccumulator = 0;
  let angularControlAccumulator = 0;
  const frames: ReplayFrame[] = [];

  for (let step = 0; step < config.steps; step += 1) {
    frames.push(replayFrame(body, state));
    const inputs = observations(body, state, config.terrain);
    if (config.controller === "recurrent") inputs.push(...recurrent);
    while (inputs.length < network.inputCount) inputs.push(0);
    const actions = networkForward(inputs, network.weights, network.inputCount);

    if (config.controller === "recurrent") recurrent.splice(0, recurrent.length, ...actions);
    if (config.controller === "cpg") {
      const wave = Math.sin(oscillatorPhase);
      oscillatorPhase += 0.18;
      for (let index = 0; index < actions.length; index += 1) {
        actions[index] = Math.tanh(actions[index] + (index % 2 === 0 ? wave : -wave) * 0.65);
      }
    }

    stepSimulation(body, state, actions, config.terrain, config.dt);
    maxHeight = Math.max(maxHeight, state.root[1] - terrainHeight(config.terrain, state.root[0]));
    uprightAccumulator += tiltStability(state.tilt);
    angularControlAccumulator += clamp(1 - Math.abs(state.angularVelocity) / 3, 0, 1);
  }
  frames.push(replayFrame(body, state));

  const distance = Math.max(state.root[0], 0);
  const uprightness = uprightAccumulator / Math.max(1, config.steps);
  const angularControl = angularControlAccumulator / Math.max(1, config.steps);
  const terminalStability = tiltStability(state.tilt);
  const stability = clamp(
    uprightness * 0.56 + terminalStability * 0.32 + angularControl * 0.12,
    0,
    1,
  );
  const stableDistance = distance * stability;
  const terrainMultiplier = config.terrain === "flat" ? 1 : config.terrain === "rough" ? 1.25 : 1.15;
  const controlledDistance = stableDistance * 11 * terrainMultiplier;
  const rawProgress = distance * 1.4 * terrainMultiplier;
  const postureScore = uprightness * 8 + terminalStability * 12 + stability * 10;
  const hopScore = clamp(maxHeight, 0, 0.85) * 0.7;
  const tumblePenalty = (1 - terminalStability) ** 2 * 18 + (1 - stability) ** 2 * 8;
  const fitness = controlledDistance + rawProgress + postureScore + hopScore - tumblePenalty - state.energy * 0.08 - body.length * 0.04;

  return {
    ...config,
    body,
    frames,
    metrics: {
      fitness,
      distance,
      stableDistance,
      stability,
      energy: state.energy,
    },
  };
}

export function terrainAt(terrain: TerrainKind, x: number) {
  return terrainHeight(terrain, x);
}
