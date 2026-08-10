// Entry point for the 3D field journal: loads three.js, drives the render
// loop and replay fetching, and wires user input. Scene construction lives
// in graphics3d-scene.js; journal DOM rendering in graphics3d-journal.js.

import {
  CAMERA_LABELS,
  CAMERA_MODES,
  canvas,
  clamp,
  controllerBehaviorNote,
  labelFor,
  replayRequest,
  state,
  ui,
} from "./graphics3d-state.js";
import {
  createCreatureRenderer,
  createSceneParts,
  createTerrain,
  updateCamera,
} from "./graphics3d-scene.js";
import {
  renderArchiveLab,
  renderEvolutionProgress,
  renderFieldJournal,
  renderGenerationTimeline,
  replayFieldNote,
  replayRunLabel,
  setLoadingState,
} from "./graphics3d-journal.js";

const THREE_MODULE_URL = "https://cdn.jsdelivr.net/npm/three@0.165.0/build/three.module.js";

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
    const requestId = state.replayRequestId + 1;
    state.replayRequestId = requestId;
    const evolving = replayRequest.mode === "evolved";
    setLoadingState("loading");
    ui.loadingTitle.textContent = evolving
      ? `Evolving generation ${replayRequest.generations}`
      : "Preparing replay";
    ui.loadingDetail.textContent = evolving
      ? `Searching ${replayRequest.population} candidate bodies on ${labelFor(replayRequest.task)} terrain`
      : "Compiling the selected genome";
    ui.fieldNote.textContent = evolving
      ? `Evolving ${replayRequest.population} ${labelFor(replayRequest.controller)} candidates for ${replayRequest.generations} generations on ${labelFor(replayRequest.task)} terrain.`
      : "Syncing replay data.";
    ui.controllerNote.textContent = controllerBehaviorNote(replayRequest.controller);
    ui.reroll.disabled = true;
    try {
      const query = new URLSearchParams(replayRequest);
      const response = await fetch(`/api/replay?${query.toString()}`);
      if (requestId !== state.replayRequestId) {
        return;
      }
      if (!response.ok) {
        throw new Error(`Replay request failed: ${response.status}`);
      }

      const replay = await response.json();
      if (requestId !== state.replayRequestId) {
        return;
      }
      state.replay = replay;
      canvas.dataset.controller = state.replay.controller;
      canvas.dataset.task = state.replay.task;
      canvas.dataset.source = state.replay.source;
      canvas.dataset.generations = String(state.replay.generations);
      canvas.dataset.population = String(state.replay.population);
      state.frameIndex = 0;
      state.accumulator = 0;
      state.playing = !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      updateActionButtons();
      ui.runLabel.textContent = replayRunLabel(state.replay);
      ui.fieldNote.textContent = replayFieldNote(state.replay);
      renderEvolutionProgress(state.replay.evolution_history || []);
      renderFieldJournal(state.replay);
      state.selectedGenerationIndex = Math.max(0, (state.replay.evolution_history || []).length - 1);
      renderGenerationTimeline(state.replay.evolution_history || []);
      renderArchiveLab(state.replay, fetchReplay);
      setLoadingState("ready");

      scene.remove(terrain.mesh);
      terrain.dispose();
      terrain = createTerrain(THREE, state.replay.task);
      scene.add(terrain.mesh);
      creature.rebuild(state.replay.body);
      updateMetrics();
    } catch (error) {
      if (requestId === state.replayRequestId) {
        setLoadingState("error");
        ui.loadingTitle.textContent = "Evolution run interrupted";
        ui.loadingDetail.textContent = "Try the generation again or choose a lighter terrain task.";
        throw error;
      }
    } finally {
      if (requestId === state.replayRequestId) {
        ui.reroll.disabled = false;
      }
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
  window.addEventListener("resize", () => {
    if (!state.replay) {
      return;
    }
    renderEvolutionProgress(state.replay.evolution_history || []);
    renderGenerationTimeline(state.replay.evolution_history || [], { preserveButtons: true });
  });
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
    ui.fieldNote.textContent = "Replay data failed to load.";
    console.error(error);
  });
  renderer.setAnimationLoop(renderLoop);

  window.render_game_to_text = () =>
    JSON.stringify({
      ready: Boolean(window.__AXIOM_3D_READY__),
      frame: state.frameIndex,
      bodies: state.replay ? state.replay.body.length : 0,
      source: state.replay ? state.replay.source : null,
      controller: state.replay ? state.replay.controller : null,
      task: state.replay ? state.replay.task : null,
      requestController: replayRequest.controller,
      requestTask: replayRequest.task,
      generations: state.replay ? state.replay.generations : null,
      population: state.replay ? state.replay.population : null,
      fitness: state.replay ? state.replay.fitness : null,
      bestDistance: state.replay ? state.replay.best_distance : null,
      stableDistance: state.replay ? state.replay.stable_distance : null,
      stability: state.replay ? state.replay.stability : null,
      cameraMode: state.cameraMode,
      effectsEnabled: state.effectsEnabled,
      selectedGeneration: state.replay?.evolution_history?.[state.selectedGenerationIndex]?.generation ?? null,
      width: canvas.width,
      height: canvas.height,
    });

  window.axiomFieldJournalSnapshot = () => ({
    ready: Boolean(state.replay),
    generation: state.replay?.generations ?? null,
    selectedGeneration: state.replay?.evolution_history?.[state.selectedGenerationIndex]?.generation ?? null,
    controller: state.replay?.controller ?? null,
    terrain: state.replay?.task ?? null,
    fitness: state.replay?.fitness ?? null,
    archiveCoverage: state.replay?.evolution_history?.at(-1)?.archive_coverage ?? null,
    loading: ui.loadingState.classList.contains("is-visible"),
  });
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

  target.addEventListener("wheel", (event) => {
    event.preventDefault();
    state.cameraZoom = clamp(state.cameraZoom + event.deltaY * 0.0008, 0.68, 1.48);
    fadeHint();
  }, { passive: false });
}

function bindActions(fetchReplay) {
  ui.controller.value = replayRequest.controller;
  ui.task.value = replayRequest.task;
  ui.controllerNote.textContent = controllerBehaviorNote(replayRequest.controller);
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
    replayRequest.generations = Math.min(40, replayRequest.generations + 1);
    delete replayRequest.cell_x;
    delete replayRequest.cell_y;
    fetchReplay().catch((error) => {
      ui.fieldNote.textContent = "Replay data failed to load.";
      console.error(error);
    });
  });

  for (const control of [ui.controller, ui.task]) {
    control.addEventListener("change", () => {
      replayRequest.controller = ui.controller.value;
      replayRequest.task = ui.task.value;
      delete replayRequest.cell_x;
      delete replayRequest.cell_y;
      fetchReplay().catch((error) => {
        ui.fieldNote.textContent = "Replay data failed to load.";
        console.error(error);
      });
    });
  }

  window.addEventListener("keydown", (event) => {
    if (event.code !== "Space") {
      return;
    }
    event.preventDefault();
    state.playing = !state.playing;
    updateActionButtons();
  });
}

function updateActionButtons() {
  setActionButton(ui.playToggle, state.playing ? "Pause" : "Play", state.playing ? "ph-pause" : "ph-play");
  setActionButton(ui.cameraMode, CAMERA_LABELS[state.cameraMode], "ph-video-camera");
  setActionButton(ui.fxToggle, state.effectsEnabled ? "Full FX" : "Lite FX", "ph-sparkle");
  const evolveLabel = replayRequest.mode === "evolved" ? "Evolve next generation" : "Generate replay";
  const evolveText = ui.reroll.querySelector("span");
  if (evolveText) {
    evolveText.textContent = evolveLabel;
  }
  ui.cameraMode.classList.toggle("is-active", state.cameraMode !== "follow");
  ui.fxToggle.classList.toggle("is-active", state.effectsEnabled);
}

function setActionButton(button, label, iconClass) {
  const text = button.querySelector("span");
  const icon = button.querySelector("i");
  if (text) {
    text.textContent = label;
  }
  if (icon) {
    icon.className = `ph ${iconClass}`;
  }
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
