const THREE_MODULE_URL = "https://cdn.jsdelivr.net/npm/three@0.165.0/build/three.module.js";

const canvas = document.getElementById("graphics-canvas");
const ui = {
  playToggle: document.getElementById("play-toggle"),
  reroll: document.getElementById("reroll"),
  cameraMode: document.getElementById("camera-mode"),
  fxToggle: document.getElementById("fx-toggle"),
  objectiveTitle: document.getElementById("objective-title"),
  runLabel: document.getElementById("run-label"),
  fieldNote: document.getElementById("field-note"),
  returnLink: document.getElementById("return-link"),
  hint: document.getElementById("control-hint"),
  time: document.getElementById("metric-time"),
  distance: document.getElementById("metric-distance"),
  tilt: document.getElementById("metric-tilt"),
};

const replayRequest = {
  mode: "evolved",
  controller: "cpg",
  task: "rough",
  seed: 29,
  frames: 520,
  generations: 12,
  population: 28,
  evaluation_steps: 180,
};

const pageParams = new URLSearchParams(window.location.search);
if (pageParams.has("run") && pageParams.has("cell")) {
  replayRequest.run = pageParams.get("run");
  replayRequest.cell = pageParams.get("cell");
  replayRequest.frames = Number(pageParams.get("frames") || replayRequest.frames);
}

applyInitialReplayContext();

const state = {
  replay: null,
  frameIndex: 0,
  accumulator: 0,
  loading: false,
  playing: !window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  lastTime: performance.now(),
  orbitYaw: 0,
  orbitPitch: 0,
  dragging: false,
  dragStart: { x: 0, y: 0 },
  dragOrbit: { yaw: 0, pitch: 0 },
  cameraMode: "follow",
  effectsEnabled: true,
  hintFaded: false,
};

window.__AXIOM_3D_READY__ = false;
window.render_game_to_text = () => renderReplayDebugState();

const Y_AXIS = { x: 0, y: 1, z: 0 };
const CAMERA_MODES = ["follow", "orbit", "showcase"];
const CAMERA_LABELS = {
  follow: "Follow",
  orbit: "Orbit",
  showcase: "Show",
};

import(THREE_MODULE_URL)
  .then((THREE) => start(THREE))
  .catch((error) => {
    ui.fieldNote.textContent = "Three.js could not load for this browser session.";
    ui.playToggle.disabled = true;
    ui.reroll.disabled = true;
    console.error(error);
  });

function start(THREE) {
  const scene = new THREE.Scene();
  scene.background = new THREE.Color("#08110f");
  scene.fog = new THREE.Fog("#08110f", 11, 34);

  const camera = new THREE.PerspectiveCamera(54, 1, 0.08, 120);
  camera.position.set(-3.2, 2.3, 6.6);

  const renderer = new THREE.WebGLRenderer({
    canvas,
    antialias: true,
    powerPreference: "high-performance",
    preserveDrawingBuffer: true,
  });
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;

  const clock = new THREE.Clock();
  const renderParts = createSceneParts(THREE, scene);
  const creature = createCreatureRenderer(THREE, scene);
  const cameraScratch = {
    target: new THREE.Vector3(),
    offset: new THREE.Vector3(),
    desired: new THREE.Vector3(),
  };
  let terrain = createTerrain(THREE, replayRequest.task);
  scene.add(terrain.mesh);

  function resize() {
    const { clientWidth, clientHeight } = canvas;
    const width = Math.max(1, clientWidth);
    const height = Math.max(1, clientHeight);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
  }

  async function fetchReplay() {
    state.loading = true;
    ui.fieldNote.textContent = loadingFieldNote();
    updateActionButtons();
    try {
      const query = new URLSearchParams(replayRequest);
      const response = await fetch(`/api/replay?${query.toString()}`);
      if (!response.ok) {
        let message = `Replay request failed: ${response.status}`;
        try {
          const error = await response.json();
          if (error && error.error) {
            message = error.error;
          }
        } catch (_) {
          // Keep the status-based fallback when the server does not return JSON.
        }
        throw new Error(message);
      }

      state.replay = await response.json();
      window.__AXIOM_3D_READY__ = true;
      state.frameIndex = 0;
      state.accumulator = 0;
      state.playing = !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      applyLoadedReplayContext(state.replay);

      scene.remove(terrain.mesh);
      terrain.dispose();
      terrain = createTerrain(THREE, state.replay.task);
      scene.add(terrain.mesh);
      creature.rebuild(state.replay.body);
      updateMetrics();
    } finally {
      state.loading = false;
      updateActionButtons();
    }
  }

  function frameStep(deltaMs) {
    if (!state.playing || !state.replay) {
      return;
    }
    state.accumulator += deltaMs;
    const frameMs = Math.max(10, state.replay.dt * 1000);
    while (state.accumulator >= frameMs) {
      state.accumulator -= frameMs;
      state.frameIndex = (state.frameIndex + 1) % state.replay.frames.length;
    }
  }

  function renderLoop(now) {
    const deltaMs = Math.min(100, now - state.lastTime);
    state.lastTime = now;
    const deltaSeconds = Math.min(0.08, clock.getDelta());

    frameStep(deltaMs);

    if (state.replay) {
      const frame = state.replay.frames[state.frameIndex];
      creature.update(frame, state.replay.body, deltaSeconds);
      renderParts.update(frame, state.replay.task, now * 0.001);
      updateCamera(camera, frame, state.replay.task, deltaSeconds, cameraScratch);
      updateMetrics();
    }

    renderer.render(scene, camera);
    window.__AXIOM_3D_READY__ = Boolean(state.replay);
  }

  window.addEventListener("resize", resize);
  canvas.addEventListener("webglcontextlost", (event) => {
    event.preventDefault();
    ui.fieldNote.textContent = "WebGL context lost.";
  });
  canvas.addEventListener("webglcontextrestored", () => {
    window.location.reload();
  });

  bindPointerInput(canvas);
  bindActions(fetchReplay);
  resize();
  fetchReplay().catch((error) => {
    ui.fieldNote.textContent = replayRequestIsArchive()
      ? `Archive replay failed for run ${replayRequest.run}, cell ${formatCell(replayRequest.cell)}: ${error.message}`
      : "Replay data failed to load.";
    console.error(error);
  });
  renderer.setAnimationLoop(renderLoop);

  window.render_game_to_text = () => renderReplayDebugState();
}

function renderReplayDebugState() {
  return JSON.stringify({
    ready: Boolean(window.__AXIOM_3D_READY__),
    frame: state.frameIndex,
    bodies: state.replay ? state.replay.body.length : 0,
    source: state.replay ? state.replay.source : null,
    fitness: state.replay ? state.replay.fitness : null,
    bestDistance: state.replay ? state.replay.best_distance : null,
    stableDistance: state.replay ? state.replay.stable_distance : null,
    stability: state.replay ? state.replay.stability : null,
    cameraMode: state.cameraMode,
    effectsEnabled: state.effectsEnabled,
    width: canvas.width,
    height: canvas.height,
  });
}

function createSceneParts(THREE, scene) {
  const skyDome = createSkyDome(THREE);
  scene.add(skyDome);
  scene.add(new THREE.HemisphereLight("#cfeede", "#17251d", 0.82));

  const keyLight = new THREE.DirectionalLight("#fff4d2", 2.45);
  keyLight.position.set(-4.5, 8.5, 5.5);
  keyLight.castShadow = true;
  keyLight.shadow.mapSize.set(2048, 2048);
  keyLight.shadow.camera.left = -12;
  keyLight.shadow.camera.right = 12;
  keyLight.shadow.camera.top = 10;
  keyLight.shadow.camera.bottom = -7;
  scene.add(keyLight);
  scene.add(keyLight.target);

  const rimLight = new THREE.DirectionalLight("#80b7ea", 1.35);
  rimLight.position.set(6, 4, -5);
  scene.add(rimLight);

  const creatureLight = new THREE.PointLight("#8eea9f", 1.8, 7, 1.8);
  scene.add(creatureLight);

  const grid = new THREE.GridHelper(128, 128, "#4f725c", "#26372f");
  grid.position.y = 0.018;
  grid.material.transparent = true;
  grid.material.opacity = 0.28;
  scene.add(grid);

  const beaconMaterial = new THREE.MeshStandardMaterial({
    color: "#e8c46f",
    emissive: "#8f5e14",
    emissiveIntensity: 0.55,
    metalness: 0.18,
    roughness: 0.38,
  });
  const beaconGeometry = new THREE.TorusGeometry(0.34, 0.014, 8, 48);
  const beacons = [];
  for (let index = 0; index < 8; index += 1) {
    const beacon = new THREE.Mesh(beaconGeometry, beaconMaterial);
    beacon.rotation.x = Math.PI / 2;
    beacon.position.set(index * 4.25 + 2.5, 0.54, -1.35);
    beacon.castShadow = false;
    scene.add(beacon);
    beacons.push(beacon);
  }

  const markerMaterial = new THREE.MeshStandardMaterial({
    color: "#31533f",
    emissive: "#14321f",
    emissiveIntensity: 0.28,
    roughness: 0.72,
    metalness: 0.12,
  });
  const markerGeometry = new THREE.ConeGeometry(0.16, 0.72, 4);
  const markers = [];
  for (let index = 0; index < 18; index += 1) {
    const marker = new THREE.Mesh(markerGeometry, markerMaterial);
    const lane = index % 2 === 0 ? -1 : 1;
    const offset = seededNoise(index + 22) * 2.3;
    marker.position.set(index * 3.2 - 4, 0.34, lane * (3.1 + offset));
    marker.rotation.y = Math.PI * 0.25 + seededNoise(index + 45) * Math.PI;
    marker.castShadow = true;
    scene.add(marker);
    markers.push(marker);
  }

  const rockGeometry = new THREE.DodecahedronGeometry(0.18, 0);
  const rockMaterial = new THREE.MeshStandardMaterial({
    color: "#344338",
    roughness: 0.94,
    metalness: 0.02,
  });
  const rocks = new THREE.InstancedMesh(rockGeometry, rockMaterial, 90);
  const rockObject = new THREE.Object3D();
  for (let index = 0; index < rocks.count; index += 1) {
    const x = seededNoise(index + 1) * 66 - 6;
    const zSign = seededNoise(index + 7) > 0.5 ? 1 : -1;
    const z = zSign * (1.9 + seededNoise(index + 11) * 5.8);
    const scale = 0.45 + seededNoise(index + 17) * 1.35;
    rockObject.position.set(x, terrainHeight("rough", x, z) + 0.07, z);
    rockObject.rotation.set(
      seededNoise(index + 31) * Math.PI,
      seededNoise(index + 37) * Math.PI,
      seededNoise(index + 41) * Math.PI,
    );
    rockObject.scale.set(scale, scale * (0.42 + seededNoise(index + 19) * 0.52), scale);
    rockObject.updateMatrix();
    rocks.setMatrixAt(index, rockObject.matrix);
  }
  rocks.castShadow = true;
  rocks.receiveShadow = true;
  scene.add(rocks);

  const particleGeometry = new THREE.BufferGeometry();
  const particleCount = 180;
  const particlePositions = new Float32Array(particleCount * 3);
  for (let index = 0; index < particleCount; index += 1) {
    particlePositions[index * 3] = seededNoise(index + 101) * 54 - 9;
    particlePositions[index * 3 + 1] = seededNoise(index + 211) * 4.1 + 0.3;
    particlePositions[index * 3 + 2] = seededNoise(index + 307) * 12 - 6;
  }
  particleGeometry.setAttribute("position", new THREE.BufferAttribute(particlePositions, 3));
  const particles = new THREE.Points(
    particleGeometry,
    new THREE.PointsMaterial({
      color: "#8eea9f",
      size: 0.025,
      transparent: true,
      opacity: 0.52,
      depthWrite: false,
    }),
  );
  scene.add(particles);

  const scanRings = [];
  for (let index = 0; index < 3; index += 1) {
    const ring = new THREE.Mesh(
      new THREE.TorusGeometry(0.68, 0.012, 8, 72),
      new THREE.MeshBasicMaterial({
        color: "#8eea9f",
        transparent: true,
        opacity: 0.34,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
      }),
    );
    ring.rotation.x = Math.PI / 2;
    scene.add(ring);
    scanRings.push(ring);
  }

  const wave = new THREE.Mesh(
    new THREE.TorusGeometry(1.3, 0.009, 8, 88),
    new THREE.MeshBasicMaterial({
      color: "#80b7ea",
      transparent: true,
      opacity: 0.18,
      depthWrite: false,
      blending: THREE.AdditiveBlending,
    }),
  );
  wave.rotation.x = Math.PI / 2;
  scene.add(wave);

  const trail = new THREE.Line(
    new THREE.BufferGeometry(),
    new THREE.LineBasicMaterial({
      color: "#80b7ea",
      transparent: true,
      opacity: 0.72,
    }),
  );
  trail.frustumCulled = false;
  scene.add(trail);

  return {
    update(frame, task, elapsed) {
      skyDome.position.set(frame.root[0], 0, 0);
      for (let index = 0; index < beacons.length; index += 1) {
        const beacon = beacons[index];
        const pulse = 1 + Math.sin(elapsed * 2.2 + index * 0.7) * 0.08;
        beacon.scale.setScalar(pulse);
        beacon.position.y = terrainHeight(task, beacon.position.x, beacon.position.z) + 0.54;
        beacon.visible = state.effectsEnabled || index % 2 === 0;
      }
      for (let index = 0; index < markers.length; index += 1) {
        const marker = markers[index];
        marker.position.y = terrainHeight(task, marker.position.x, marker.position.z) + 0.36;
        marker.scale.y = 1 + Math.sin(elapsed * 1.6 + index) * 0.08;
        marker.visible = state.effectsEnabled;
      }

      particles.visible = state.effectsEnabled;
      particles.rotation.y = Math.sin(elapsed * 0.22) * 0.05;
      particles.position.x = frame.root[0] * 0.06;
      rocks.visible = state.effectsEnabled;
      creatureLight.visible = state.effectsEnabled;
      creatureLight.position.set(frame.root[0], frame.root[1] + 1.1, 1.15);
      creatureLight.intensity = 1.45 + Math.sin(elapsed * 5.2) * 0.35;

      const groundY = terrainHeight(task, frame.root[0], 0) + 0.08;
      for (let index = 0; index < scanRings.length; index += 1) {
        const phase = (elapsed * 0.44 + index / scanRings.length) % 1;
        const ring = scanRings[index];
        ring.visible = state.effectsEnabled;
        ring.position.set(frame.root[0], groundY, 0);
        ring.scale.setScalar(0.65 + phase * 2.45);
        ring.material.opacity = (1 - phase) * 0.28;
      }
      const wavePhase = (elapsed * 0.28) % 1;
      wave.visible = state.effectsEnabled;
      wave.position.set(frame.root[0] - 0.15, groundY + 0.01, 0);
      wave.scale.setScalar(0.8 + wavePhase * 3.5);
      wave.material.opacity = (1 - wavePhase) * 0.16;

      const points = [];
      const replay = state.replay;
      const start = Math.max(0, state.frameIndex - 90);
      for (let index = start; replay && index <= state.frameIndex; index += 3) {
        const root = replay.frames[index].root;
        points.push(new THREE.Vector3(root[0], terrainHeight(task, root[0], -0.6) + 0.08, -0.62));
      }
      if (points.length > 1) {
        trail.geometry.setFromPoints(points);
        trail.visible = true;
      } else {
        trail.visible = false;
      }
      keyLight.target.position.set(frame.root[0], frame.root[1] + 0.4, 0);
    },
  };
}

function createSkyDome(THREE) {
  const skyCanvas = document.createElement("canvas");
  skyCanvas.width = 16;
  skyCanvas.height = 256;
  const context = skyCanvas.getContext("2d");
  const gradient = context.createLinearGradient(0, 0, 0, skyCanvas.height);
  gradient.addColorStop(0, "#143128");
  gradient.addColorStop(0.48, "#081310");
  gradient.addColorStop(1, "#020806");
  context.fillStyle = gradient;
  context.fillRect(0, 0, skyCanvas.width, skyCanvas.height);

  const texture = new THREE.CanvasTexture(skyCanvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  const material = new THREE.MeshBasicMaterial({
    map: texture,
    side: THREE.BackSide,
    depthWrite: false,
  });
  const dome = new THREE.Mesh(new THREE.SphereGeometry(80, 32, 16), material);
  dome.renderOrder = -10;
  return dome;
}

function createTerrain(THREE, task) {
  const width = 150;
  const depth = 20;
  const xSegments = 240;
  const zSegments = 28;
  const xStart = -34;
  const positions = [];
  const indices = [];

  for (let zi = 0; zi <= zSegments; zi += 1) {
    const z = (zi / zSegments - 0.5) * depth;
    for (let xi = 0; xi <= xSegments; xi += 1) {
      const x = xStart + (xi / xSegments) * width;
      positions.push(x, terrainHeight(task, x, z), z);
    }
  }

  for (let zi = 0; zi < zSegments; zi += 1) {
    for (let xi = 0; xi < xSegments; xi += 1) {
      const a = zi * (xSegments + 1) + xi;
      const b = a + 1;
      const c = a + xSegments + 1;
      const d = c + 1;
      indices.push(a, c, b, b, c, d);
    }
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();

  const material = new THREE.MeshStandardMaterial({
    color: "#2d3b2e",
    metalness: 0.04,
    roughness: 0.92,
  });
  const mesh = new THREE.Mesh(geometry, material);
  mesh.receiveShadow = true;

  return {
    mesh,
    dispose() {
      geometry.dispose();
      material.dispose();
    },
  };
}

function createCreatureRenderer(THREE, scene) {
  const group = new THREE.Group();
  const ghostGroup = new THREE.Group();
  scene.add(group);
  scene.add(ghostGroup);

  const hullGeometry = new THREE.SphereGeometry(0.5, 32, 18);
  const plateGeometry = new THREE.BoxGeometry(0.58, 0.17, 0.08);
  const railGeometry = new THREE.CylinderGeometry(0.018, 0.018, 1, 10);
  const sensorGeometry = new THREE.SphereGeometry(0.045, 14, 10);
  const jointStrutGeometry = new THREE.CylinderGeometry(0.035, 0.035, 1, 14);
  const jointRingGeometry = new THREE.TorusGeometry(0.13, 0.017, 8, 36);
  const jointPivotGeometry = new THREE.SphereGeometry(0.052, 14, 10);
  const actuatorPodGeometry = new THREE.SphereGeometry(0.13, 20, 14);
  const actuatorRingGeometry = new THREE.TorusGeometry(0.18, 0.014, 8, 36);
  const actuatorCoilGeometry = new THREE.CylinderGeometry(0.013, 0.013, 0.34, 8);
  const rootMaterial = new THREE.MeshStandardMaterial({
    color: "#edf5e9",
    emissive: "#3f6d46",
    emissiveIntensity: 0.16,
    roughness: 0.42,
    metalness: 0.18,
  });
  const limbMaterial = new THREE.MeshStandardMaterial({
    color: "#80b7ea",
    emissive: "#193754",
    emissiveIntensity: 0.2,
    roughness: 0.46,
    metalness: 0.24,
  });
  const plateMaterial = new THREE.MeshStandardMaterial({
    color: "#d7e8d5",
    emissive: "#1e4528",
    emissiveIntensity: 0.08,
    roughness: 0.5,
    metalness: 0.34,
  });
  const armorMaterial = new THREE.MeshStandardMaterial({
    color: "#18231c",
    emissive: "#0b1a10",
    emissiveIntensity: 0.16,
    roughness: 0.56,
    metalness: 0.46,
  });
  const jointMaterial = new THREE.MeshStandardMaterial({
    color: "#e8c46f",
    emissive: "#5d3e0c",
    emissiveIntensity: 0.2,
    roughness: 0.38,
    metalness: 0.24,
  });
  const actuatorMaterial = new THREE.MeshStandardMaterial({
    color: "#8eea9f",
    emissive: "#3f9f56",
    emissiveIntensity: 0.9,
    roughness: 0.3,
    metalness: 0.1,
  });
  const sensorMaterial = new THREE.MeshStandardMaterial({
    color: "#a9f1b5",
    emissive: "#52d86b",
    emissiveIntensity: 1.2,
    roughness: 0.22,
    metalness: 0.1,
  });
  const ghostMaterials = [0.18, 0.11, 0.06].map(
    (opacity) =>
      new THREE.MeshBasicMaterial({
        color: "#80b7ea",
        transparent: true,
        opacity,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
      }),
  );

  const bodyModules = [];
  const actuatorModules = [];
  const jointModules = [];
  const ghostMeshes = [];
  const ghostSnapshots = [];
  let ghostAccumulator = 0;
  const yAxis = new THREE.Vector3(Y_AXIS.x, Y_AXIS.y, Y_AXIS.z);
  const scratchOffset = new THREE.Vector3();
  const scratchZAxis = new THREE.Vector3(0, 0, 1);
  const scratchStart = new THREE.Vector3();
  const scratchEnd = new THREE.Vector3();
  const scratchDirection = new THREE.Vector3();

  function clearMeshes() {
    for (const module of [...bodyModules, ...actuatorModules, ...jointModules]) {
      group.remove(module);
    }
    for (const mesh of ghostMeshes.flat()) {
      ghostGroup.remove(mesh);
    }
    bodyModules.length = 0;
    actuatorModules.length = 0;
    jointModules.length = 0;
    ghostMeshes.length = 0;
    ghostSnapshots.length = 0;
    ghostAccumulator = 0;
  }

  function rebuild(body) {
    clearMeshes();
    for (let index = 0; index < body.length; index += 1) {
      const module = createBioMechBodyModule(index);
      group.add(module);
      bodyModules.push(module);

      const actuator = createBioMechActuatorModule();
      group.add(actuator);
      actuatorModules.push(actuator);
    }

    for (let index = 0; index < Math.max(0, body.length - 1); index += 1) {
      const joint = createBioMechJointModule();
      group.add(joint);
      jointModules.push(joint);
    }

    for (let layer = 0; layer < ghostMaterials.length; layer += 1) {
      const layerMeshes = [];
      for (let index = 0; index < body.length; index += 1) {
        const ghost = new THREE.Mesh(hullGeometry, ghostMaterials[layer]);
        ghost.visible = false;
        ghostGroup.add(ghost);
        layerMeshes.push(ghost);
      }
      ghostMeshes.push(layerMeshes);
    }
  }

  function captureGhostSnapshot() {
    if (!state.effectsEnabled || bodyModules.length === 0) {
      return;
    }
    ghostSnapshots.push(
      bodyModules.map((module) => ({
        position: module.position.clone(),
        rotation: module.rotation.clone(),
        scale: module.scale.clone(),
      })),
    );
    while (ghostSnapshots.length > 12) {
      ghostSnapshots.shift();
    }
  }

  function updateGhosts() {
    for (let layer = 0; layer < ghostMeshes.length; layer += 1) {
      const snapshot = state.effectsEnabled
        ? ghostSnapshots[ghostSnapshots.length - 1 - layer * 3]
        : null;
      for (let index = 0; index < ghostMeshes[layer].length; index += 1) {
        const ghost = ghostMeshes[layer][index];
        const source = snapshot ? snapshot[index] : null;
        ghost.visible = Boolean(source);
        if (!source) {
          continue;
        }
        ghost.position.copy(source.position);
        ghost.rotation.copy(source.rotation);
        ghost.scale.copy(source.scale).multiplyScalar(1 + layer * 0.04);
      }
    }
  }

  function update(frame, body, deltaSeconds) {
    for (let index = 0; index < body.length; index += 1) {
      const node = body[index];
      const center = frame.bodies[index];
      const parent = node.parent;
      const angle =
        parent === null
          ? frame.tilt
          : Math.atan2(center[1] - frame.bodies[parent][1], center[0] - frame.bodies[parent][0]);
      const depth = index === 0 ? 0.58 : 0.42;
      const scaleX = Math.max(0.34, node.size[0] * 1.25);
      const scaleY = Math.max(0.22, node.size[1] * 1.25);
      const module = bodyModules[index];
      module.position.set(center[0], center[1] + 0.46, 0);
      module.rotation.set(Math.sin(frame.time + index) * 0.035, 0, angle);
      module.scale.set(scaleX, scaleY, depth);
      updateBioMechBodyModule(module, index, frame.time);

      const actuator = actuatorModules[index];
      actuator.visible = node.actuator > 0.01;
      if (actuator.visible) {
        const pulse = 0.86 + Math.sin(frame.time * 8 + index * 0.7) * 0.16;
        scratchOffset.set(scaleX * 0.38, 0, depth * 0.7);
        scratchOffset.applyAxisAngle(scratchZAxis, angle);
        actuator.position.set(
          center[0] + scratchOffset.x,
          center[1] + 0.46 + scratchOffset.y,
          scratchOffset.z,
        );
        actuator.rotation.set(0, 0, angle);
        actuator.scale.setScalar(Math.max(0.78, scaleY * 1.1) * pulse);
        updateBioMechActuatorModule(actuator, pulse);
      }
    }

    for (let index = 0; index < jointModules.length; index += 1) {
      const joint = jointModules[index];
      const segment = frame.joints[index];
      if (!segment) {
        joint.visible = false;
        continue;
      }
      scratchStart.set(segment[0][0], segment[0][1] + 0.46, 0);
      scratchEnd.set(segment[1][0], segment[1][1] + 0.46, 0);
      scratchDirection.copy(scratchEnd).sub(scratchStart);
      const length = scratchDirection.length();
      joint.visible = length > 0.001;
      if (!joint.visible) {
        continue;
      }
      joint.position.copy(scratchStart).add(scratchEnd).multiplyScalar(0.5);
      joint.quaternion.setFromUnitVectors(yAxis, scratchDirection.normalize());
      updateBioMechJointModule(joint, length, frame.time, index);
    }

    ghostAccumulator += deltaSeconds;
    if (ghostAccumulator >= 0.08) {
      ghostAccumulator = 0;
      captureGhostSnapshot();
    }
    updateGhosts();

    group.position.z = Math.sin(frame.time * 1.3) * 0.035;
    group.rotation.y = THREE.MathUtils.lerp(
      group.rotation.y,
      Math.sin(frame.time * 0.7) * 0.025,
      deltaSeconds * 4,
    );
    ghostGroup.position.copy(group.position);
    ghostGroup.rotation.copy(group.rotation);
  }

  function createBioMechBodyModule(index) {
    const root = index === 0;
    const module = new THREE.Group();
    const hull = new THREE.Mesh(hullGeometry, root ? rootMaterial : limbMaterial);
    hull.castShadow = true;
    hull.receiveShadow = true;
    module.add(hull);

    const topPlate = new THREE.Mesh(plateGeometry, root ? plateMaterial : armorMaterial);
    topPlate.position.set(root ? 0 : -0.02, -0.02, 0.5);
    topPlate.scale.set(root ? 0.95 : 0.76, root ? 0.9 : 0.72, 0.82);
    topPlate.castShadow = true;
    module.add(topPlate);

    const rails = [];
    for (const side of [-1, 1]) {
      const rail = new THREE.Mesh(railGeometry, jointMaterial);
      rail.position.set(0, side * 0.33, 0.42);
      rail.rotation.z = Math.PI / 2;
      rail.scale.set(0.82, 1, 0.82);
      rail.castShadow = true;
      module.add(rail);
      rails.push(rail);
    }

    const sensors = [];
    const sensorCount = root ? 3 : 2;
    for (let sensorIndex = 0; sensorIndex < sensorCount; sensorIndex += 1) {
      const sensor = new THREE.Mesh(sensorGeometry, sensorMaterial);
      const centered = sensorIndex - (sensorCount - 1) / 2;
      sensor.position.set(centered * 0.22, -0.24, 0.52);
      sensor.castShadow = false;
      module.add(sensor);
      sensors.push(sensor);
    }

    module.userData = { hull, rails, sensors, topPlate, root };
    return module;
  }

  function createBioMechActuatorModule() {
    const module = new THREE.Group();
    const pod = new THREE.Mesh(actuatorPodGeometry, actuatorMaterial);
    pod.castShadow = true;
    module.add(pod);

    const ring = new THREE.Mesh(actuatorRingGeometry, sensorMaterial);
    ring.rotation.y = Math.PI / 2;
    module.add(ring);

    const coils = [];
    for (const offset of [-0.08, 0.08]) {
      const coil = new THREE.Mesh(actuatorCoilGeometry, jointMaterial);
      coil.position.set(offset, 0, 0.12);
      coil.rotation.z = Math.PI * 0.18;
      coil.castShadow = true;
      module.add(coil);
      coils.push(coil);
    }

    module.visible = false;
    module.userData = { pod, ring, coils };
    return module;
  }

  function createBioMechJointModule() {
    const module = new THREE.Group();
    const strut = new THREE.Mesh(jointStrutGeometry, jointMaterial);
    strut.castShadow = true;
    module.add(strut);

    const ringStart = new THREE.Mesh(jointRingGeometry, jointMaterial);
    const ringEnd = new THREE.Mesh(jointRingGeometry, jointMaterial);
    module.add(ringStart);
    module.add(ringEnd);

    const pivot = new THREE.Mesh(jointPivotGeometry, sensorMaterial);
    module.add(pivot);

    module.userData = { strut, ringStart, ringEnd, pivot };
    return module;
  }

  function updateBioMechBodyModule(module, index, time) {
    const pulse = 1 + Math.sin(time * 8 + index * 0.6) * 0.08;
    for (let sensorIndex = 0; sensorIndex < module.userData.sensors.length; sensorIndex += 1) {
      const sensor = module.userData.sensors[sensorIndex];
      sensor.scale.setScalar(pulse + sensorIndex * 0.04);
      sensor.visible = state.effectsEnabled || module.userData.root;
    }
    for (const rail of module.userData.rails) {
      rail.visible = state.effectsEnabled || module.userData.root;
    }
    module.userData.topPlate.visible = true;
  }

  function updateBioMechActuatorModule(module, pulse) {
    module.userData.pod.scale.setScalar(0.95 + pulse * 0.12);
    module.userData.ring.visible = state.effectsEnabled;
    module.userData.ring.scale.setScalar(0.85 + pulse * 0.22);
    for (let index = 0; index < module.userData.coils.length; index += 1) {
      const coil = module.userData.coils[index];
      coil.visible = state.effectsEnabled;
      coil.rotation.x = pulse * 0.4 + index * 0.8;
    }
  }

  function updateBioMechJointModule(module, length, time, index) {
    const pulse = 1 + Math.sin(time * 7 + index * 0.5) * 0.08;
    module.userData.strut.scale.set(1, length, 1);
    module.userData.ringStart.position.y = -length / 2;
    module.userData.ringEnd.position.y = length / 2;
    module.userData.ringStart.scale.setScalar(pulse);
    module.userData.ringEnd.scale.setScalar(pulse * 0.92);
    module.userData.pivot.scale.setScalar(1.1 + pulse * 0.18);
    module.userData.pivot.visible = state.effectsEnabled;
  }

  return { rebuild, update };
}

function updateCamera(camera, frame, task, deltaSeconds, scratch) {
  const terrainY = terrainHeight(task, frame.root[0], 0);
  const portrait = camera.aspect < 0.72;
  const showcaseYaw = Math.sin(frame.time * 0.33) * 0.72;
  const yaw = state.cameraMode === "showcase" ? showcaseYaw : state.orbitYaw;
  const targetLead =
    state.cameraMode === "orbit" ? 0.15 : portrait ? 0.22 : state.cameraMode === "showcase" ? 0.45 : 1.15;
  const radius =
    state.cameraMode === "showcase" ? (portrait ? 11.2 : 8.1) : state.cameraMode === "orbit" ? (portrait ? 10.6 : 7.4) : portrait ? 10.2 : 6.4;
  const sideOffset =
    state.cameraMode === "orbit" ? 0 : state.cameraMode === "showcase" ? Math.sin(frame.time * 0.21) * 1.4 : portrait ? -0.78 : -3.15;
  const cameraHeight =
    (portrait ? 3.35 : 2.25) + (state.cameraMode === "showcase" ? 0.72 : 0);
  scratch.target.set(frame.root[0] + targetLead, Math.max(terrainY + 0.7, frame.root[1] + 0.38), 0);
  scratch.offset.set(
    sideOffset + Math.sin(yaw) * (portrait ? 0.9 : 1.5),
    cameraHeight + state.orbitPitch,
    Math.cos(yaw) * radius,
  );
  scratch.desired.copy(scratch.target).add(scratch.offset);
  const blend = 1 - Math.pow(0.0008, Math.max(0.001, deltaSeconds));
  camera.position.lerp(scratch.desired, blend);
  camera.lookAt(scratch.target);
}

function bindPointerInput(target) {
  target.addEventListener("pointerdown", (event) => {
    target.setPointerCapture(event.pointerId);
    state.dragging = true;
    state.dragStart.x = event.clientX;
    state.dragStart.y = event.clientY;
    state.dragOrbit.yaw = state.orbitYaw;
    state.dragOrbit.pitch = state.orbitPitch;
    if (state.cameraMode === "follow") {
      state.cameraMode = "orbit";
      updateActionButtons();
    }
    fadeHint();
  });

  target.addEventListener("pointermove", (event) => {
    if (!state.dragging) {
      return;
    }
    const dx = event.clientX - state.dragStart.x;
    const dy = event.clientY - state.dragStart.y;
    state.orbitYaw = clamp(state.dragOrbit.yaw + dx * 0.006, -0.85, 0.85);
    state.orbitPitch = clamp(state.dragOrbit.pitch + dy * 0.004, -0.55, 1.05);
  });

  target.addEventListener("pointerup", (event) => {
    state.dragging = false;
    target.releasePointerCapture(event.pointerId);
  });

  target.addEventListener("pointercancel", () => {
    state.dragging = false;
  });
}

function bindActions(fetchReplay) {
  updateActionButtons();
  ui.playToggle.addEventListener("click", () => {
    state.playing = !state.playing;
    updateActionButtons();
    fadeHint();
  });

  ui.cameraMode.addEventListener("click", () => {
    const current = CAMERA_MODES.indexOf(state.cameraMode);
    state.cameraMode = CAMERA_MODES[(current + 1) % CAMERA_MODES.length];
    updateActionButtons();
    fadeHint();
  });

  ui.fxToggle.addEventListener("click", () => {
    state.effectsEnabled = !state.effectsEnabled;
    updateActionButtons();
  });

  ui.reroll.addEventListener("click", () => {
    if (!replayRequest.run) {
      replayRequest.seed += 1;
    }
    fetchReplay().catch((error) => {
      ui.fieldNote.textContent = replayRequestIsArchive()
        ? `Archive replay failed for run ${replayRequest.run}, cell ${formatCell(replayRequest.cell)}: ${error.message}`
        : "Replay data failed to load.";
      console.error(error);
    });
  });

  window.addEventListener("keydown", (event) => {
    if (event.code !== "Space") {
      return;
    }
    if (state.loading || !state.replay) {
      return;
    }
    event.preventDefault();
    state.playing = !state.playing;
    updateActionButtons();
  });
}

function updateActionButtons() {
  ui.playToggle.disabled = state.loading || !state.replay;
  ui.reroll.disabled = state.loading;
  ui.playToggle.textContent = state.loading ? "Loading" : state.playing ? "Pause" : "Play";
  if (state.loading) {
    ui.reroll.textContent = replayRequestIsArchive() ? "Loading Cell" : "Loading";
  } else if (replayRequestIsArchive()) {
    ui.reroll.textContent = "Reload Cell";
  } else {
    ui.reroll.textContent = replayRequest.mode === "evolved" ? "Evolve" : "Reroll";
  }
  updateActionContextLabels();
  ui.cameraMode.textContent = CAMERA_LABELS[state.cameraMode];
  ui.fxToggle.textContent = state.effectsEnabled ? "Full FX" : "Lite FX";
  ui.cameraMode.classList.toggle("is-active", state.cameraMode !== "follow");
  ui.fxToggle.classList.toggle("is-active", state.effectsEnabled);
}

function updateActionContextLabels() {
  if (!replayRequestIsArchive()) {
    ui.reroll.setAttribute("aria-label", "Generate a new 3D replay");
    ui.reroll.removeAttribute("title");
    return;
  }
  const cell = formatCell(replayRequest.cell);
  const label = state.loading
    ? `Loading archive cell ${cell} from run ${replayRequest.run}`
    : `Reload archive cell ${cell} from run ${replayRequest.run}`;
  ui.reroll.setAttribute("aria-label", label);
  ui.reroll.title = label;
}

function updateMetrics() {
  if (!state.replay) {
    return;
  }
  const frame = state.replay.frames[state.frameIndex];
  ui.time.textContent = `${frame.time.toFixed(2)}s`;
  ui.distance.textContent = frame.root[0].toFixed(2);
  ui.tilt.textContent = frame.tilt.toFixed(2);
}

function fadeHint() {
  if (state.hintFaded) {
    return;
  }
  state.hintFaded = true;
  window.setTimeout(() => ui.hint.classList.add("is-faded"), 850);
}

function terrainHeight(task, x, z = 0) {
  void z;
  if (task === "flat") {
    return 0;
  }
  if (task === "steps") {
    return Math.floor(x / 1.5) * 0.05;
  }
  return Math.sin(x * 1.7) * 0.12 + Math.cos(x * 0.47) * 0.08;
}

function seededNoise(value) {
  const raw = Math.sin(value * 12.9898) * 43758.5453;
  return raw - Math.floor(raw);
}

function labelFor(value) {
  return value
    .split(/[-_]/)
    .filter(Boolean)
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join(" ");
}

function replayRequestIsArchive() {
  return Boolean(replayRequest.run && replayRequest.cell);
}

function applyInitialReplayContext() {
  if (!replayRequestIsArchive()) {
    ui.objectiveTitle.textContent = "Test the evolved stride";
    ui.runLabel.textContent = `Evolving / ${labelFor(replayRequest.task)} / seed ${replayRequest.seed}`;
    ui.fieldNote.textContent = "Evolving a compact population before replay.";
    ui.reroll.textContent = replayRequest.mode === "evolved" ? "Evolve" : "Reroll";
    ui.returnLink.textContent = "Archive";
    ui.returnLink.setAttribute("aria-label", "Return to archive browser");
    return;
  }

  const cell = formatCell(replayRequest.cell);
  ui.objectiveTitle.textContent = "Inspect archive elite";
  ui.runLabel.textContent = `Run ${replayRequest.run} / Cell ${cell}`;
  ui.fieldNote.textContent = `Preparing archived cell ${cell} from run ${replayRequest.run}.`;
  ui.reroll.textContent = "Load Cell";
  ui.returnLink.textContent = "Archive";
  ui.returnLink.setAttribute(
    "aria-label",
    `Return to archive browser from run ${replayRequest.run}, cell ${cell}`,
  );
}

function applyLoadedReplayContext(replay) {
  if (replay.source === "archive") {
    ui.objectiveTitle.textContent = "Inspect archive elite";
  } else {
    ui.objectiveTitle.textContent =
      replay.source === "evolved" ? "Test the evolved stride" : "Replay seed body plan";
  }
  ui.runLabel.textContent = replayRunLabel(replay);
  ui.fieldNote.textContent = replayFieldNote(replay);
}

function loadingFieldNote() {
  if (replayRequestIsArchive()) {
    return `Loading archived cell ${formatCell(replayRequest.cell)} from run ${replayRequest.run}.`;
  }
  if (replayRequest.mode === "evolved") {
    return `Evolving ${replayRequest.population} candidates for ${replayRequest.generations} generations.`;
  }
  return "Syncing replay data.";
}

function replayRunLabel(replay) {
  if (replay.source === "archive") {
    return `${labelFor(replay.controller)} / ${labelFor(replay.task)} / run ${replay.run_id || replayRequest.run || "--"} / cell ${replayCellLabel(replay)}`;
  }
  const generationLabel =
    replay.source === "evolved" ? `gen ${replay.generations}` : replay.source || "minimal";
  return `${labelFor(replay.controller)} / ${labelFor(replay.task)} / ${generationLabel} / seed ${replay.seed}`;
}

function replayFieldNote(replay) {
  const bodyText = `${replay.body.length} body parts and ${Math.max(0, replay.body.length - 1)} joints`;
  if (replay.source === "archive") {
    return `${bodyText}, replaying archived genome ${replay.genome_id || "--"} from run ${replay.run_id || replayRequest.run || "--"}, cell ${replayCellLabel(replay)}. Distance ${formatMetric(replay.best_distance, 2)}, stable distance ${formatMetric(replay.stable_distance, 2)}, stability ${formatMetric(replay.stability, 2)}.`;
  }
  if (replay.source !== "evolved") {
    return `${bodyText}, replaying a seed genome across rough terrain.`;
  }
  return `Evolved ${replay.population} candidates over ${replay.generations} generations. Distance ${formatMetric(replay.best_distance, 2)}, stable distance ${formatMetric(replay.stable_distance, 2)}, stability ${formatMetric(replay.stability, 2)}; staged with follow-camera framing.`;
}

function replayCellLabel(replay) {
  return replay.cell ? replay.cell.join(",") : formatCell(replayRequest.cell);
}

function formatCell(cell) {
  if (Array.isArray(cell)) {
    return cell.join(",");
  }
  return typeof cell === "string" && cell.length > 0 ? cell : "--";
}

function formatMetric(value, digits) {
  return Number.isFinite(value) ? value.toFixed(digits) : "--";
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}
