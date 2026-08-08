const THREE_MODULE_URL = "https://cdn.jsdelivr.net/npm/three@0.165.0/build/three.module.js";

const canvas = document.getElementById("graphics-canvas");
const ui = {
  controller: document.getElementById("controller"),
  task: document.getElementById("task"),
  playToggle: document.getElementById("play-toggle"),
  reroll: document.getElementById("reroll"),
  cameraMode: document.getElementById("camera-mode"),
  fxToggle: document.getElementById("fx-toggle"),
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

const state = {
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
  cameraZoom: 1,
  effectsEnabled: true,
  hintFaded: false,
  replayRequestId: 0,
  selectedGenerationIndex: null,
};

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

  const boxGeometry = new THREE.BoxGeometry(1, 1, 1);
  const jointGeometry = new THREE.CylinderGeometry(0.045, 0.045, 1, 16);
  const actuatorGeometry = new THREE.SphereGeometry(0.1, 18, 12);
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

  const bodyMeshes = [];
  const actuatorMeshes = [];
  const jointMeshes = [];
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
    for (const mesh of [...bodyMeshes, ...actuatorMeshes, ...jointMeshes]) {
      group.remove(mesh);
    }
    for (const mesh of ghostMeshes.flat()) {
      ghostGroup.remove(mesh);
    }
    bodyMeshes.length = 0;
    actuatorMeshes.length = 0;
    jointMeshes.length = 0;
    ghostMeshes.length = 0;
    ghostSnapshots.length = 0;
    ghostAccumulator = 0;
  }

  function rebuild(body) {
    clearMeshes();
    for (let index = 0; index < body.length; index += 1) {
      const mesh = new THREE.Mesh(boxGeometry, index === 0 ? rootMaterial : limbMaterial);
      mesh.castShadow = true;
      mesh.receiveShadow = true;
      group.add(mesh);
      bodyMeshes.push(mesh);

      const actuator = new THREE.Mesh(actuatorGeometry, actuatorMaterial);
      actuator.castShadow = true;
      group.add(actuator);
      actuatorMeshes.push(actuator);
    }

    for (let index = 0; index < Math.max(0, body.length - 1); index += 1) {
      const joint = new THREE.Mesh(jointGeometry, jointMaterial);
      joint.castShadow = true;
      group.add(joint);
      jointMeshes.push(joint);
    }

    for (let layer = 0; layer < ghostMaterials.length; layer += 1) {
      const layerMeshes = [];
      for (let index = 0; index < body.length; index += 1) {
        const ghost = new THREE.Mesh(boxGeometry, ghostMaterials[layer]);
        ghost.visible = false;
        ghostGroup.add(ghost);
        layerMeshes.push(ghost);
      }
      ghostMeshes.push(layerMeshes);
    }
  }

  function captureGhostSnapshot() {
    if (!state.effectsEnabled || bodyMeshes.length === 0) {
      return;
    }
    ghostSnapshots.push(
      bodyMeshes.map((mesh) => ({
        position: mesh.position.clone(),
        rotation: mesh.rotation.clone(),
        scale: mesh.scale.clone(),
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
      const mesh = bodyMeshes[index];
      mesh.position.set(center[0], center[1] + 0.46, 0);
      mesh.rotation.set(Math.sin(frame.time + index) * 0.035, 0, angle);
      mesh.scale.set(scaleX, scaleY, depth);

      const actuator = actuatorMeshes[index];
      actuator.visible = node.actuator > 0.01;
      if (actuator.visible) {
        scratchOffset.set(scaleX * 0.36, 0, depth * 0.56);
        scratchOffset.applyAxisAngle(scratchZAxis, angle);
        actuator.position.set(center[0] + scratchOffset.x, center[1] + 0.46 + scratchOffset.y, scratchOffset.z);
        actuator.scale.setScalar(0.75 + Math.sin(frame.time * 8 + index) * 0.12);
      }
    }

    for (let index = 0; index < jointMeshes.length; index += 1) {
      const joint = jointMeshes[index];
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
      joint.scale.set(1, length, 1);
      joint.quaternion.setFromUnitVectors(yAxis, scratchDirection.normalize());
    }

    ghostAccumulator += deltaSeconds;
    if (ghostAccumulator >= 0.08) {
      ghostAccumulator = 0;
      captureGhostSnapshot();
    }
    updateGhosts();

    group.position.z = Math.sin(frame.time * 1.3) * 0.035;
    group.rotation.y = THREE.MathUtils.lerp(group.rotation.y, Math.sin(frame.time * 0.7) * 0.025, deltaSeconds * 4);
    ghostGroup.position.copy(group.position);
    ghostGroup.rotation.copy(group.rotation);
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
  const radius = (
    state.cameraMode === "showcase" ? (portrait ? 11.2 : 8.1) : state.cameraMode === "orbit" ? (portrait ? 10.6 : 7.4) : portrait ? 10.2 : 6.4
  ) * state.cameraZoom;
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

function terrainHeight(task, x, z = 0) {
  const roughness =
    task === "flat" ? 0 : Math.sin(x * 1.7) * 0.12 + Math.cos(x * 0.47) * 0.08;
  return roughness + Math.sin(z * 0.95 + x * 0.18) * 0.035;
}

function seededNoise(value) {
  const raw = Math.sin(value * 12.9898) * 43758.5453;
  return raw - Math.floor(raw);
}

function labelFor(value) {
  if (value === "cpg") {
    return "CPG";
  }
  return value
    .split(/[-_]/)
    .filter(Boolean)
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join(" ");
}

function replayRunLabel(replay) {
  const generationLabel =
    replay.source === "evolved" ? `gen ${replay.generations}` : replay.source || "minimal";
  const cell = replay.archive?.selected_cell;
  const cellLabel = cell ? ` / cell ${cell[0]},${cell[1]}` : "";
  return `${labelFor(replay.controller)} / ${labelFor(replay.task)} / ${generationLabel}${cellLabel} / seed ${replay.seed}`;
}

function replayFieldNote(replay) {
  const bodyText = `${replay.body.length} body parts and ${Math.max(0, replay.body.length - 1)} joints`;
  if (replay.source !== "evolved") {
    return `${bodyText}, replaying a seed genome across ${labelFor(replay.task)} terrain.`;
  }
  const cell = replay.archive?.selected_cell;
  const selection = cell ? ` Archive cell ${cell[0]},${cell[1]}.` : "";
  return `Evolved ${replay.population} candidates over ${replay.generations} generations.${selection} Distance ${formatMetric(replay.best_distance, 2)}, stable distance ${formatMetric(replay.stable_distance, 2)}, stability ${formatMetric(replay.stability, 2)}; staged with follow-camera framing.`;
}

function setLoadingState(status) {
  const visible = status !== "ready";
  ui.loadingState.classList.toggle("is-visible", visible);
  ui.loadingState.classList.toggle("is-error", status === "error");
  ui.reroll.disabled = status === "loading";
}

function renderFieldJournal(replay) {
  const history = replay.evolution_history || [];
  const first = history[0];
  const last = history.at(-1);
  const fitnessGain = last && first
    ? last.best_fitness - first.best_fitness
    : 0;
  const averageScale = replay.body.length
    ? replay.body.reduce((sum, node) => sum + node.size[0] + node.size[1], 0) / (replay.body.length * 2)
    : 0;
  const terrain = labelFor(replay.task);
  const controller = labelFor(replay.controller);
  const selectedLineage = replay.lineage?.at(-1);
  const lineageDepth = replay.lineage?.length || 0;

  ui.generationTitle.textContent = `Generation ${replay.generations}`;
  ui.generationTerrain.textContent = terrain === "Rough" ? "Rough terrain" : terrain;
  ui.currentGenerationLabel.textContent = `GEN ${replay.generations} (CURRENT)`;
  ui.previousGenerationLabel.textContent = `GEN ${Math.max(1, replay.generations - 1)} (PREVIOUS)`;
  ui.changeNarrative.textContent = selectedLineage
    ? `Genome #${selectedLineage.genome_id} descends through ${lineageDepth} recorded ${lineageDepth === 1 ? "generation" : "generations"}. Its latest inherited change ${selectedLineage.mutation_summary}, while preserving ${formatMetric(replay.stability, 2)} stability on ${terrain.toLowerCase()} terrain.`
    : `Selection favored forward travel without surrendering stability on ${terrain.toLowerCase()} terrain. The current champion balances ${replay.body.length} body segments through a ${controller} controller, converting a stable distance of ${formatMetric(replay.stable_distance, 2)} into repeatable progress.`;
  ui.deltaStride.textContent = `${fitnessGain >= 0 ? "+" : ""}${formatMetric(fitnessGain, 2)} total fitness gain`;
  ui.deltaStrideNote.textContent = first
    ? `Improved from ${formatMetric(first.best_fitness, 2)} to ${formatMetric(last.best_fitness, 2)} across the active search.`
    : "Establishing the first comparable champion.";
  ui.deltaGeometry.textContent = `${replay.body.length}-part morphology selected`;
  ui.deltaGeometryNote.textContent = selectedLineage
    ? `Recorded mutation: ${selectedLineage.mutation_summary}. Mean body scale ${formatMetric(averageScale, 2)}.`
    : `Mean body scale ${formatMetric(averageScale, 2)} with ${Math.max(0, replay.body.length - 1)} articulated joints.`;
  ui.deltaControl.textContent = `${controller} stability ${formatMetric(replay.stability, 2)}`;
  ui.deltaControlNote.textContent = controllerBehaviorNote(replay.controller);
}

function renderArchiveLab(replay, fetchReplay) {
  const archive = replay.archive;
  if (!archive || !archive.cells?.length) {
    ui.archiveCaption.textContent = "No occupied cells";
    ui.archiveGrid.replaceChildren();
    ui.archiveDetail.textContent = "This replay does not include a MAP-Elites archive.";
    return;
  }

  const byCell = new Map(archive.cells.map((elite) => [`${elite.cell[0]}:${elite.cell[1]}`, elite]));
  const fitnesses = archive.cells.map((elite) => elite.fitness);
  const minFitness = Math.min(...fitnesses);
  const maxFitness = Math.max(...fitnesses);
  const fitnessRange = Math.max(0.001, maxFitness - minFitness);
  const selectedKey = archive.selected_cell.join(":");
  const fragment = document.createDocumentFragment();

  ui.archiveGrid.style.setProperty("--archive-width", String(archive.width));
  for (let y = archive.height - 1; y >= 0; y -= 1) {
    for (let x = 0; x < archive.width; x += 1) {
      const key = `${x}:${y}`;
      const elite = byCell.get(key);
      if (!elite) {
        const empty = document.createElement("span");
        empty.className = "archive-cell is-empty";
        empty.setAttribute("role", "presentation");
        fragment.appendChild(empty);
        continue;
      }

      const intensity = (elite.fitness - minFitness) / fitnessRange;
      const button = document.createElement("button");
      button.type = "button";
      button.className = "archive-cell is-occupied";
      button.style.setProperty("--cell-intensity", intensity.toFixed(3));
      button.dataset.cell = key;
      button.setAttribute("role", "gridcell");
      button.setAttribute("aria-label", `Replay genome ${elite.genome_id} from archive cell ${x}, ${y}; born generation ${elite.generation}; fitness ${formatMetric(elite.fitness, 2)}; stable distance ${formatMetric(elite.stable_distance, 2)}; ${Math.round(elite.body_count)} body parts`);
      button.setAttribute("aria-pressed", String(key === selectedKey));
      button.classList.toggle("is-selected", key === selectedKey);
      button.addEventListener("click", () => {
        if (key === archive.selected_cell.join(":")) {
          return;
        }
        replayRequest.cell_x = x;
        replayRequest.cell_y = y;
        fetchReplay().catch((error) => {
          ui.fieldNote.textContent = "Archive elite failed to load.";
          console.error(error);
        });
      });
      fragment.appendChild(button);
    }
  }

  ui.archiveGrid.replaceChildren(fragment);
  ui.archiveCaption.textContent = `${archive.cells.length} / ${archive.width * archive.height} occupied`;
  ui.archiveXAxis.textContent = `${labelFor(archive.x_axis)} →`;
  ui.archiveYAxis.textContent = `${labelFor(archive.y_axis)} ↑`;
  const selected = byCell.get(selectedKey);
  ui.archiveDetail.textContent = selected
    ? `Genome #${selected.genome_id} · born gen ${selected.generation} · cell ${selected.cell[0]},${selected.cell[1]} · fitness ${formatMetric(selected.fitness, 2)} · ${selected.mutation_summary}`
    : "Select an occupied cell to replay its elite.";
}

function renderGenerationTimeline(history, options = {}) {
  const { preserveButtons = false } = options;
  if (!history.length) {
    ui.generationRail.replaceChildren();
    drawTimelineChart(history);
    return;
  }

  if (!preserveButtons || ui.generationRail.childElementCount !== history.length) {
    const fragment = document.createDocumentFragment();
    for (let index = 0; index < history.length; index += 1) {
      const point = history[index];
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = String(point.generation);
      button.dataset.generationIndex = String(index);
      button.setAttribute("aria-label", `Inspect generation ${point.generation}`);
      button.addEventListener("click", () => selectGeneration(index));
      fragment.appendChild(button);
    }
    ui.generationRail.replaceChildren(fragment);
    ui.generationRail.style.setProperty("--generation-count", String(history.length));
  }

  const selected = clamp(
    state.selectedGenerationIndex ?? history.length - 1,
    0,
    history.length - 1,
  );
  state.selectedGenerationIndex = selected;
  const buttons = ui.generationRail.querySelectorAll("button");
  buttons.forEach((button, index) => {
    const isSelected = index === selected;
    button.classList.toggle("is-selected", isSelected);
    button.setAttribute("aria-pressed", String(isSelected));
  });
  updateSelectedGeneration(history[selected]);
  drawTimelineChart(history);
}

function selectGeneration(index) {
  const history = state.replay?.evolution_history || [];
  if (!history[index]) {
    return;
  }
  state.selectedGenerationIndex = index;
  renderGenerationTimeline(history, { preserveButtons: true });
}

function updateSelectedGeneration(point) {
  if (!point) {
    ui.selectedGeneration.textContent = "NO GENERATION";
    ui.selectedFitness.textContent = "Fitness --";
    ui.selectedCoverage.textContent = "Coverage --";
    return;
  }
  ui.selectedGeneration.textContent = `GEN ${point.generation}`;
  ui.selectedFitness.textContent = `Fitness ${formatMetric(point.best_fitness, 2)}`;
  ui.selectedCoverage.textContent = `Coverage ${Math.round(point.archive_coverage * 100)}%`;
}

function drawTimelineChart(history) {
  const { context, width, height } = prepareCanvas(ui.timelineChart);
  context.clearRect(0, 0, width, height);
  if (!history.length) {
    return;
  }

  const padding = { left: 8, right: 10, top: 10, bottom: 10 };
  const plotWidth = width - padding.left - padding.right;
  const plotHeight = height - padding.top - padding.bottom;
  const fitnessValues = history.map((point) => point.best_fitness);
  const minFitness = Math.min(...fitnessValues);
  const maxFitness = Math.max(...fitnessValues);
  const fitnessRange = Math.max(0.001, maxFitness - minFitness);
  const x = (index) => padding.left + (history.length === 1 ? plotWidth / 2 : (index / (history.length - 1)) * plotWidth);
  const fitnessY = (value) => padding.top + plotHeight * 0.43 - ((value - minFitness) / fitnessRange) * plotHeight * 0.31;
  const coverageY = (value) => padding.top + plotHeight - value * plotHeight * 0.78;

  context.strokeStyle = "rgba(204, 222, 196, .13)";
  context.lineWidth = 1;
  context.beginPath();
  context.moveTo(padding.left, height * 0.5);
  context.lineTo(width - padding.right, height * 0.5);
  context.stroke();
  drawEvolutionLine(context, history, x, (point) => fitnessY(point.best_fitness), "#f0b943", 1.8);
  drawEvolutionLine(context, history, x, (point) => coverageY(point.archive_coverage), "#77aee7", 1.5);

  const selected = clamp(state.selectedGenerationIndex ?? history.length - 1, 0, history.length - 1);
  const selectedX = x(selected);
  context.strokeStyle = "rgba(163, 232, 127, .72)";
  context.beginPath();
  context.moveTo(selectedX, 0);
  context.lineTo(selectedX, height);
  context.stroke();
  for (const [value, color] of [
    [fitnessY(history[selected].best_fitness), "#f0b943"],
    [coverageY(history[selected].archive_coverage), "#77aee7"],
  ]) {
    context.beginPath();
    context.arc(selectedX, value, 3.5, 0, Math.PI * 2);
    context.fillStyle = color;
    context.fill();
  }
}

function prepareCanvas(chart) {
  const context = chart.getContext("2d");
  const ratio = Math.min(window.devicePixelRatio || 1, 2);
  const width = Math.max(1, chart.clientWidth || Number(chart.getAttribute("width")) || 320);
  const height = Math.max(1, chart.clientHeight || Number(chart.getAttribute("height")) || 120);
  chart.width = Math.floor(width * ratio);
  chart.height = Math.floor(height * ratio);
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  return { context, width, height };
}

function renderEvolutionProgress(history) {
  const { context, width, height } = prepareCanvas(ui.evolutionChart);
  context.clearRect(0, 0, width, height);

  if (!history.length) {
    ui.evolutionSummary.textContent = "Seed genome · no search history";
    return;
  }

  const padding = { left: 10, right: 10, top: 10, bottom: 12 };
  const plotWidth = width - padding.left - padding.right;
  const plotHeight = height - padding.top - padding.bottom;
  const fitnessValues = history.flatMap((point) => [point.best_fitness, point.mean_fitness]);
  const minFitness = Math.min(...fitnessValues);
  const maxFitness = Math.max(...fitnessValues);
  const fitnessRange = Math.max(0.001, maxFitness - minFitness);
  const x = (index) => padding.left + (history.length === 1 ? plotWidth / 2 : (index / (history.length - 1)) * plotWidth);
  const fitnessY = (value) => padding.top + plotHeight - ((value - minFitness) / fitnessRange) * plotHeight;
  const coverageY = (value) => padding.top + plotHeight - value * plotHeight;

  context.strokeStyle = "rgba(220, 239, 226, .15)";
  context.lineWidth = 1;
  for (let row = 0; row <= 3; row += 1) {
    const y = padding.top + (row / 3) * plotHeight;
    context.beginPath(); context.moveTo(padding.left, y); context.lineTo(width - padding.right, y); context.stroke();
  }
  drawEvolutionLine(context, history, x, (point) => fitnessY(point.best_fitness), "#f2b84b", 2.2);
  drawEvolutionLine(context, history, x, (point) => fitnessY(point.mean_fitness), "#8eea9f", 1.6);
  drawEvolutionLine(context, history, x, (point) => coverageY(point.archive_coverage), "#80b7ea", 1.4);

  const first = history[0];
  const last = history.at(-1);
  const gain = last.best_fitness - first.best_fitness;
  ui.evolutionSummary.textContent = `Gen ${last.generation} · fitness ${formatMetric(last.best_fitness, 2)} · ${gain >= 0 ? "+" : ""}${formatMetric(gain, 2)} gain · ${Math.round(last.archive_coverage * 100)}% coverage`;
}

function drawEvolutionLine(context, history, x, y, color, lineWidth) {
  context.beginPath();
  history.forEach((point, index) => {
    const px = x(index);
    const py = y(point);
    if (index === 0) context.moveTo(px, py); else context.lineTo(px, py);
  });
  context.strokeStyle = color;
  context.lineWidth = lineWidth;
  context.lineJoin = "round";
  context.lineCap = "round";
  context.stroke();
}

function controllerBehaviorNote(controller) {
  if (controller === "feedforward") {
    return "Feedforward maps the current sensor vector directly to joint actuator commands.";
  }
  if (controller === "recurrent") {
    return "Recurrent maps sensors plus prior outputs to the next joint actuator commands.";
  }
  return "CPG adds an alternating gait wave to neural joint commands; physics still comes from the simulator.";
}

function formatMetric(value, digits) {
  return Number.isFinite(value) ? value.toFixed(digits) : "--";
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}
