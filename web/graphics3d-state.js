// Shared page state for the 3D field journal: DOM handles, the replay
// request, mutable view state, and small helpers used across modules.

export const canvas = document.getElementById("graphics-canvas");
export const ui = {
  controller: document.getElementById("controller"),
  task: document.getElementById("task"),
  playToggle: document.getElementById("play-toggle"),
  reroll: document.getElementById("reroll"),
  cameraMode: document.getElementById("camera-mode"),
  fxToggle: document.getElementById("fx-toggle"),
  bodyToggle: document.getElementById("body-toggle"),
  runLabel: document.getElementById("run-label"),
  fieldNote: document.getElementById("field-note"),
  controllerNote: document.getElementById("controller-note"),
  hint: document.getElementById("control-hint"),
  time: document.getElementById("metric-time"),
  distance: document.getElementById("metric-distance"),
  tilt: document.getElementById("metric-tilt"),
  evolutionChart: document.getElementById("evolution-chart"),
  evolutionSummary: document.getElementById("evolution-summary"),
  generationTitle: document.getElementById("generation-title"),
  generationTerrain: document.getElementById("generation-terrain"),
  changeNarrative: document.getElementById("change-narrative"),
  deltaStride: document.getElementById("delta-stride"),
  deltaStrideNote: document.getElementById("delta-stride-note"),
  deltaGeometry: document.getElementById("delta-geometry"),
  deltaGeometryNote: document.getElementById("delta-geometry-note"),
  deltaControl: document.getElementById("delta-control"),
  deltaControlNote: document.getElementById("delta-control-note"),
  archiveGrid: document.getElementById("archive-grid"),
  archiveCaption: document.getElementById("archive-caption"),
  archiveXAxis: document.getElementById("archive-x-axis"),
  archiveYAxis: document.getElementById("archive-y-axis"),
  archiveDetail: document.getElementById("archive-detail"),
  timelineChart: document.getElementById("timeline-chart"),
  generationRail: document.getElementById("generation-rail"),
  selectedGeneration: document.getElementById("selected-generation"),
  selectedFitness: document.getElementById("selected-fitness"),
  selectedCoverage: document.getElementById("selected-coverage"),
  currentGenerationLabel: document.getElementById("current-generation-label"),
  previousGenerationLabel: document.getElementById("previous-generation-label"),
  loadingState: document.getElementById("loading-state"),
  loadingTitle: document.getElementById("loading-title"),
  loadingDetail: document.getElementById("loading-detail"),
};

export const replayRequest = {
  mode: "evolved",
  controller: "cpg",
  task: "rough",
  seed: 29,
  frames: 520,
  generations: 12,
  population: 28,
  evaluation_steps: 180,
};

export const state = {
  replay: null,
  frameIndex: 0,
  accumulator: 0,
  playing: !window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  lastTime: performance.now(),
  orbitYaw: 0,
  orbitPitch: 0,
  dragging: false,
  dragStart: { x: 0, y: 0 },
  dragOrbit: { yaw: 0, pitch: 0 },
  cameraMode: "follow",
  // The box rig is the default because it is the honest one: the physics is
  // planar, and boxes at z = 0 claim nothing more. The character is opt-in.
  characterBody: false,
  cameraZoom: 1,
  effectsEnabled: true,
  hintFaded: false,
  replayRequestId: 0,
  selectedGenerationIndex: null,
};

export const Y_AXIS = { x: 0, y: 1, z: 0 };
export const CAMERA_MODES = ["follow", "orbit", "showcase"];
export const CAMERA_LABELS = {
  follow: "Follow",
  orbit: "Orbit",
  showcase: "Show",
};

export function labelFor(value) {
  if (value === "cpg") {
    return "CPG";
  }
  return value
    .split(/[-_]/)
    .filter(Boolean)
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join(" ");
}

export function controllerBehaviorNote(controller) {
  if (controller === "feedforward") {
    return "Feedforward maps the current sensor vector directly to joint actuator commands.";
  }
  if (controller === "recurrent") {
    return "Recurrent maps sensors plus prior outputs to the next joint actuator commands.";
  }
  return "CPG adds an alternating gait wave to neural joint commands; physics still comes from the simulator.";
}

export function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}
