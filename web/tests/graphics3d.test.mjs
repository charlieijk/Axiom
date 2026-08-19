// web/graphics3d.js is the 3D field journal's entry point: it boots three.js,
// wires the page, fetches replays and drives the render loop. Its -state,
// -scene and -journal siblings were already covered; this file covers the
// wiring itself.
//
// Two things make that possible without a browser:
//
//   * `start(THREE, scenery)` takes both three.js and the world builders as
//     parameters, so the scene can be built from recording stand-ins.
//   * the module boots itself on import, and under node the dynamic
//     `import("/vendor/three.module.min.js")` cannot resolve -- which is
//     exactly the failure path `threeReady` exists to make observable.
import assert from "node:assert/strict";
import test from "node:test";

import { createDocumentStub, createWindowStub } from "./support/dom.mjs";

globalThis.document = createDocumentStub();
globalThis.window = createWindowStub();

const { canvas, replayRequest, state, ui } = await import("../graphics3d-state.js");

// The action buttons carry an icon and a label element in graphics3d.html;
// setActionButton writes through those, so the stub needs them to exist before
// anything reads a label back.
for (const button of [ui.playToggle, ui.cameraMode, ui.fxToggle, ui.reroll]) {
  button.appendChild(document.createElement("i"));
  button.appendChild(document.createElement("span"));
}

/** Records what `fetch` was asked for and hands back the queued response. */
const requests = [];
let respond = () => okResponse();
globalThis.fetch = (url) => {
  requests.push(url);
  return Promise.resolve(respond(url, requests.length));
};

// The boot import runs before the first test does. Swallow its console.error so
// the expected ERR_MODULE_NOT_FOUND is not mistaken for a broken suite.
const consoleErrors = [];
const realConsoleError = console.error;
console.error = (...args) => consoleErrors.push(args);

const graphics3d = await import("../graphics3d.js");
await graphics3d.threeReady;

const {
  DEFAULT_SCENERY,
  bindPointerInput,
  fadeHint,
  reportThreeLoadFailure,
  setActionButton,
  start,
  updateMetrics,
} = graphics3d;

/**
 * Mirrors the shape src/web/replay/response.rs serializes for an evolved run.
 * A fixture invented from the page's own reads would pass while the real
 * response broke it, so every field name here comes from that struct.
 */
function sampleReplay(overrides = {}) {
  const frameAt = (index) => ({
    time: index * 0.05,
    root: [index * 0.1, 1],
    tilt: index * 0.01,
    bodies: [
      [index * 0.1, 1],
      [index * 0.1 + 0.4, 1],
    ],
    joints: [
      [
        [index * 0.1, 1],
        [index * 0.1 + 0.4, 1],
      ],
    ],
  });

  return {
    genome_id: 7,
    controller: "cpg",
    task: "rough",
    seed: 29,
    source: "evolved",
    generations: 12,
    population: 28,
    evaluation_steps: 180,
    fitness: 121.46,
    best_distance: 9.5,
    stable_distance: 8.25,
    uprightness: 0.91,
    stability: 0.87,
    terminal_tilt: 0.12,
    evolution_history: [
      { generation: 1, best_fitness: 40, mean_fitness: 20, archive_coverage: 0.05, occupied_cells: 2 },
      { generation: 2, best_fitness: 80, mean_fitness: 45, archive_coverage: 0.1, occupied_cells: 4 },
      { generation: 3, best_fitness: 121.46, mean_fitness: 70, archive_coverage: 0.15, occupied_cells: 6 },
    ],
    lineage: [
      {
        genome_id: 7,
        parent_id: 3,
        generation: 3,
        controller: "cpg",
        body_count: 3,
        actuator_count: 2,
        mutation_summary: "lengthened the trailing limb",
      },
    ],
    archive: {
      x_axis: "stable_distance",
      y_axis: "body_count",
      width: 2,
      height: 2,
      selected_cell: [1, 1],
      cells: [
        {
          cell: [0, 0],
          genome_id: 4,
          parent_id: null,
          generation: 1,
          mutation_summary: "seeded",
          fitness: 40,
          stable_distance: 3,
          body_count: 3,
          actuator_count: 2,
        },
        {
          cell: [1, 1],
          genome_id: 7,
          parent_id: 4,
          generation: 3,
          mutation_summary: "lengthened the trailing limb",
          fitness: 121.46,
          stable_distance: 8.25,
          body_count: 3,
          actuator_count: 2,
        },
      ],
    },
    dt: 0.05,
    body: [
      { id: 0, parent: null, size: [0.4, 0.2], actuator: 0 },
      { id: 1, parent: 0, size: [0.3, 0.16], actuator: 0.5 },
      { id: 2, parent: 1, size: [0.28, 0.14], actuator: 0.25 },
    ],
    frames: [frameAt(0), frameAt(1), frameAt(2), frameAt(3)],
    ...overrides,
  };
}

function okResponse(replay = sampleReplay()) {
  return { ok: true, status: 200, json: async () => replay };
}

/** The slice of three.js the entry point touches, recording what it is told. */
function fakeThree() {
  const record = { added: [], removed: [], renders: 0, animationLoop: null, sizes: [], pixelRatios: [] };

  class Vector3 {
    constructor(x = 0, y = 0, z = 0) {
      this.x = x;
      this.y = y;
      this.z = z;
    }
    set(x, y, z) {
      this.x = x;
      this.y = y;
      this.z = z;
      return this;
    }
  }

  class Scene {
    constructor() {
      this.background = null;
      this.fog = null;
    }
    add(object) {
      record.added.push(object);
    }
    remove(object) {
      record.removed.push(object);
    }
  }

  class Color {
    constructor(value) {
      this.value = value;
    }
  }

  class Fog {
    constructor(value, near, far) {
      Object.assign(this, { value, near, far });
    }
  }

  class PerspectiveCamera {
    constructor(fov, aspect, near, far) {
      Object.assign(this, { fov, aspect, near, far });
      this.position = new Vector3();
      this.projectionUpdates = 0;
    }
    updateProjectionMatrix() {
      this.projectionUpdates += 1;
    }
  }

  class WebGLRenderer {
    constructor(options) {
      this.options = options;
      this.outputColorSpace = null;
      this.shadowMap = { enabled: false, type: null };
    }
    setPixelRatio(ratio) {
      record.pixelRatios.push(ratio);
    }
    setSize(width, height, updateStyle) {
      record.sizes.push([width, height, updateStyle]);
    }
    render() {
      record.renders += 1;
    }
    setAnimationLoop(loop) {
      record.animationLoop = loop;
    }
  }

  class Clock {
    getDelta() {
      return 0.016;
    }
  }

  return {
    record,
    THREE: {
      Scene,
      Color,
      Fog,
      PerspectiveCamera,
      WebGLRenderer,
      Clock,
      Vector3,
      SRGBColorSpace: "srgb",
      PCFSoftShadowMap: "pcf-soft",
    },
  };
}

/** Scenery stand-ins that record the calls the render loop makes into them. */
function fakeScenery() {
  const record = { terrains: [], disposed: 0, sceneUpdates: [], creatureUpdates: [], rebuilds: [], cameraFrames: [] };
  return {
    record,
    scenery: {
      createSceneParts: () => ({
        update: (...args) => record.sceneUpdates.push(args),
      }),
      createCreatureRenderer: () => ({
        update: (...args) => record.creatureUpdates.push(args),
        rebuild: (body) => record.rebuilds.push(body),
      }),
      createTerrain: (_three, task) => {
        const terrain = { mesh: { task }, task, dispose: () => (record.disposed += 1) };
        record.terrains.push(terrain);
        return terrain;
      },
      updateCamera: (...args) => record.cameraFrames.push(args),
    },
  };
}

/** Put the shared page state back where a fresh load would leave it. */
function resetState() {
  Object.assign(state, {
    replay: null,
    frameIndex: 0,
    accumulator: 0,
    playing: true,
    lastTime: 0,
    orbitYaw: 0,
    orbitPitch: 0,
    dragging: false,
    cameraMode: "follow",
    cameraZoom: 1,
    effectsEnabled: true,
    hintFaded: false,
    selectedGenerationIndex: null,
  });
  state.dragStart.x = 0;
  state.dragStart.y = 0;
  state.dragOrbit.yaw = 0;
  state.dragOrbit.pitch = 0;
  replayRequest.controller = "cpg";
  replayRequest.task = "rough";
  replayRequest.generations = 12;
  delete replayRequest.cell_x;
  delete replayRequest.cell_y;
  requests.length = 0;
  respond = () => okResponse();
}

/**
 * Boot the page against stand-ins and wait for its opening fetch.
 *
 * `start` is called per test rather than once for the file because the boot
 * sequence is itself under test -- sharing one booted page would hide, for
 * example, a terrain that is built but never added to the scene.
 */
async function boot() {
  resetState();
  const three = fakeThree();
  const world = fakeScenery();
  const handles = start(three.THREE, world.scenery);
  await handles.ready;
  return { ...handles, three, world };
}

test("the three.js boot failure is reported on the page rather than swallowed", () => {
  // `threeReady` was awaited at load: under node the vendored module URL cannot
  // resolve, so the page took its own failure path before any test ran.
  assert.equal(ui.fieldNote.textContent, "Three.js could not load for this browser session.");
  assert.equal(ui.playToggle.disabled, true);
  assert.equal(ui.reroll.disabled, true);
  assert.equal(consoleErrors.length, 1, "the underlying load error must reach the console");
  assert.equal(consoleErrors[0][0].code, "ERR_MODULE_NOT_FOUND");

  // Calling it directly proves the three writes above belong to this function
  // and not to some other page code that happened to run first.
  ui.fieldNote.textContent = "";
  ui.playToggle.disabled = false;
  ui.reroll.disabled = false;
  reportThreeLoadFailure(new Error("boom"));
  assert.equal(ui.fieldNote.textContent, "Three.js could not load for this browser session.");
  assert.equal(ui.playToggle.disabled, true);
  assert.equal(ui.reroll.disabled, true);
  assert.equal(consoleErrors.length, 2);
});

test("DEFAULT_SCENERY wires the real scene builders", async () => {
  // The injection point must default to the shipped builders; a stray stub left
  // in the default would ship a page that renders nothing.
  const scene = await import("../graphics3d-scene.js");
  assert.equal(DEFAULT_SCENERY.createSceneParts, scene.createSceneParts);
  assert.equal(DEFAULT_SCENERY.createCreatureRenderer, scene.createCreatureRenderer);
  assert.equal(DEFAULT_SCENERY.createTerrain, scene.createTerrain);
  assert.equal(DEFAULT_SCENERY.updateCamera, scene.updateCamera);
});

test("start builds the world and puts the terrain in the scene", async () => {
  const { three, world } = await boot();

  assert.equal(world.record.terrains.length, 2, "one terrain at boot, one for the fetched replay");
  assert.deepEqual(
    three.record.added.map((object) => object.task),
    ["rough", "rough"],
    "both terrains must be added to the scene, not just built",
  );
  assert.equal(world.record.disposed, 1, "the boot terrain must be disposed when the replay lands");
  assert.equal(three.record.removed.length, 1);

  // Renderer configuration the scene depends on for correct colour and shadows.
  assert.equal(three.record.animationLoop !== null, true, "the render loop must be handed to three.js");
});

test("resize sizes the renderer from the canvas and reframes the camera", async () => {
  const { resize, three } = await boot();
  three.record.sizes.length = 0;
  three.record.pixelRatios.length = 0;

  canvas.clientWidth = 800;
  canvas.clientHeight = 400;
  globalThis.window.devicePixelRatio = 3;
  resize();

  assert.deepEqual(three.record.sizes.at(-1), [800, 400, false]);
  assert.equal(three.record.pixelRatios.at(-1), 2, "device pixel ratio must be clamped to 2");

  // A zero-height canvas (a collapsed flex parent) must not produce a NaN or
  // Infinity aspect, which would blank the scene.
  canvas.clientWidth = 0;
  canvas.clientHeight = 0;
  resize();
  assert.deepEqual(three.record.sizes.at(-1), [1, 1, false]);

  canvas.clientWidth = 960;
  canvas.clientHeight = 540;
  globalThis.window.devicePixelRatio = 1;
});

test("a successful fetch publishes the replay to the page and the scene", async () => {
  const { world } = await boot();

  assert.match(requests[0], /^\/api\/replay\?/);
  assert.match(requests[0], /mode=evolved/);
  assert.equal(state.replay.genome_id, 7);
  assert.equal(canvas.dataset.controller, "cpg");
  assert.equal(canvas.dataset.generations, "12");
  assert.equal(canvas.dataset.population, "28");
  assert.equal(ui.runLabel.textContent, "CPG / Rough / gen 12 / cell 1,1 / seed 29");
  assert.match(ui.fieldNote.textContent, /Evolved 28 candidates over 12 generations/);
  assert.equal(ui.loadingState.classList.contains("is-visible"), false, "the loading veil must clear");
  assert.equal(ui.reroll.disabled, false, "the evolve button must come back");
  assert.deepEqual(world.record.rebuilds.at(-1), state.replay.body, "the creature must be rebuilt");
  assert.equal(ui.time.textContent, "0.00s", "metrics must be written for the first frame");
  assert.equal(state.selectedGenerationIndex, 2, "the newest generation is selected");
});

test("a failed fetch shows the error state and re-enables the evolve button", async () => {
  resetState();
  respond = () => ({ ok: false, status: 503, json: async () => ({}) });
  const three = fakeThree();
  const world = fakeScenery();
  const handles = start(three.THREE, world.scenery);
  await handles.ready;

  assert.equal(state.replay, null, "a failed request must not publish a replay");
  assert.equal(ui.loadingState.classList.contains("is-visible"), true);
  assert.equal(ui.loadingState.classList.contains("is-error"), true);
  assert.equal(ui.loadingTitle.textContent, "Evolution run interrupted");
  assert.match(ui.loadingDetail.textContent, /lighter terrain task/);
  assert.equal(ui.reroll.disabled, false, "the user must be able to retry");
  assert.equal(ui.fieldNote.textContent, "Replay data failed to load.");
});

test("a superseded reply is discarded before its body is read", async () => {
  const { fetchReplay } = await boot();
  const stale = sampleReplay({ genome_id: 111, generations: 1 });
  const fresh = sampleReplay({ genome_id: 222, generations: 9 });

  let releaseStale;
  const staleHeaders = new Promise((resolve) => {
    releaseStale = resolve;
  });
  let staleBodyReads = 0;
  respond = (_url, callIndex) =>
    callIndex === 2
      ? staleHeaders.then(() => ({
          ok: true,
          status: 200,
          json: async () => {
            staleBodyReads += 1;
            return stale;
          },
        }))
      : okResponse(fresh);

  const first = fetchReplay();
  const second = fetchReplay();
  releaseStale();
  await Promise.all([first, second]);

  assert.equal(
    state.replay.genome_id,
    222,
    "the late reply to the superseded request must not clobber the current replay",
  );
  assert.equal(canvas.dataset.generations, "9");
  assert.equal(staleBodyReads, 0, "a superseded reply must be dropped before its body is parsed");
});

test("the loading view is on screen for as long as the request is in flight", async () => {
  const { fetchReplay } = await boot();
  let release;
  const inFlight = new Promise((resolve) => {
    release = resolve;
  });
  respond = () => inFlight.then(() => okResponse());

  ui.loadingState.classList.remove("is-visible", "is-error");
  ui.reroll.disabled = false;
  const pending = fetchReplay();

  assert.equal(ui.loadingState.classList.contains("is-visible"), true, "the veil must be up while loading");
  assert.equal(ui.loadingState.classList.contains("is-error"), false, "loading is not an error");
  assert.equal(ui.reroll.disabled, true, "the evolve button must be inert while a run is in flight");
  assert.equal(ui.loadingTitle.textContent, "Evolving generation 12");
  assert.match(ui.loadingDetail.textContent, /Searching 28 candidate bodies on Rough terrain/);
  assert.match(ui.fieldNote.textContent, /Evolving 28 CPG candidates for 12 generations/);

  release();
  await pending;
  assert.equal(ui.loadingState.classList.contains("is-visible"), false);
  assert.equal(ui.reroll.disabled, false);
});

test("the non-evolving mode gets its own loading copy", async () => {
  const { fetchReplay } = await boot();
  replayRequest.mode = "minimal";
  let release;
  const inFlight = new Promise((resolve) => {
    release = resolve;
  });
  respond = () => inFlight.then(() => okResponse(sampleReplay({ source: "minimal" })));
  const pending = fetchReplay();

  assert.equal(ui.loadingTitle.textContent, "Preparing replay");
  assert.equal(ui.loadingDetail.textContent, "Compiling the selected genome");
  assert.equal(ui.fieldNote.textContent, "Syncing replay data.");

  release();
  await pending;
  assert.match(ui.fieldNote.textContent, /replaying a seed genome across Rough terrain/);
  replayRequest.mode = "evolved";
});

test("a superseded reply is discarded after its body is read", async () => {
  // The slower half of the same hazard: the headers come back promptly and only
  // the body is slow, so the request is still in flight *after* the first
  // staleness check. Without the second check the parsed body lands on top of
  // the newer replay.
  const { fetchReplay } = await boot();
  const fresh = sampleReplay({ genome_id: 222, generations: 9 });

  let releaseBody;
  const staleBody = new Promise((resolve) => {
    releaseBody = resolve;
  });
  respond = (_url, callIndex) =>
    callIndex === 2
      ? { ok: true, status: 200, json: () => staleBody.then(() => sampleReplay({ genome_id: 111, generations: 1 })) }
      : okResponse(fresh);

  const first = fetchReplay();
  // Let the first request get as far as awaiting its body before superseding it.
  await new Promise((resolve) => setImmediate(resolve));
  const second = fetchReplay();
  await second;
  releaseBody();
  await first;

  assert.equal(state.replay.genome_id, 222, "a slow body must not overwrite the newer replay");
  assert.equal(canvas.dataset.generations, "9");
});

test("the render loop advances frames on a clock and stops when paused", async () => {
  const { renderLoop, three, world } = await boot();
  const frameMs = state.replay.dt * 1000;

  state.frameIndex = 0;
  state.accumulator = 0;
  state.lastTime = 0;
  state.playing = true;
  renderLoop(frameMs * 2);
  assert.equal(state.frameIndex, 2, "two frame periods must advance two frames");
  assert.equal(three.record.renders > 0, true);
  assert.equal(world.record.cameraFrames.length > 0, true, "the camera must be reframed each rendered frame");
  assert.equal(globalThis.window.__AXIOM_3D_READY__, true);

  const framesRendered = three.record.renders;
  state.playing = false;
  state.lastTime = frameMs * 2;
  renderLoop(frameMs * 10);
  assert.equal(state.frameIndex, 2, "a paused journal must hold its frame");
  assert.equal(three.record.renders, framesRendered + 1, "but it must still draw the held frame");

  // The frame index wraps rather than running off the end of the recording.
  state.playing = true;
  state.accumulator = 0;
  state.lastTime = 0;
  state.frameIndex = state.replay.frames.length - 1;
  renderLoop(frameMs);
  assert.equal(state.frameIndex, 0, "the last frame must wrap to the first, not index past the end");
});

test("the render loop clamps a long stall so the replay does not jump", async () => {
  const { renderLoop } = await boot();
  state.frameIndex = 0;
  state.accumulator = 0;
  state.lastTime = 0;
  state.playing = true;

  // A backgrounded tab returns with a huge delta. Only 100ms of it may count,
  // which at dt=0.05 is two frames -- not the forty the raw delta implies.
  renderLoop(2000);
  assert.equal(state.frameIndex, 2);
});

test("render_game_to_text reports the loaded replay for the Rust smoke check", async () => {
  const { renderLoop } = await boot();
  state.lastTime = 0;
  renderLoop(0);

  const snapshot = JSON.parse(globalThis.window.render_game_to_text());
  assert.equal(snapshot.ready, true);
  assert.equal(snapshot.bodies, 3);
  assert.equal(snapshot.source, "evolved");
  assert.equal(snapshot.controller, "cpg");
  assert.equal(snapshot.task, "rough");
  assert.equal(snapshot.generations, 12);
  assert.equal(snapshot.population, 28);
  assert.equal(snapshot.fitness, 121.46);
  assert.equal(snapshot.selectedGeneration, 3);
  assert.equal(snapshot.cameraMode, "follow");
});

test("the field journal snapshot reads the loading veil from the DOM", async () => {
  await boot();

  const ready = globalThis.window.axiomFieldJournalSnapshot();
  assert.equal(ready.ready, true);
  assert.equal(ready.loading, false);
  assert.equal(ready.generation, 12);
  assert.equal(ready.selectedGeneration, 3);
  assert.equal(ready.archiveCoverage, 0.15);

  ui.loadingState.classList.add("is-visible");
  assert.equal(
    globalThis.window.axiomFieldJournalSnapshot().loading,
    true,
    "the snapshot must follow the veil, not a separate flag that can drift from it",
  );
  ui.loadingState.classList.remove("is-visible");
});

test("dragging orbits the camera and takes it out of follow mode", async () => {
  await boot();
  const target = document.createElement("canvas");
  bindPointerInput(target);

  target.dispatch("pointerdown", { pointerId: 1, clientX: 100, clientY: 100 });
  assert.equal(state.dragging, true);
  assert.equal(state.cameraMode, "orbit", "a drag must leave follow mode so the camera obeys the pointer");
  assert.equal(state.hintFaded, true);

  target.dispatch("pointermove", { clientX: 150, clientY: 130 });
  assert.equal(Math.abs(state.orbitYaw - 0.3) < 1e-9, true);
  assert.equal(Math.abs(state.orbitPitch - 0.12) < 1e-9, true);

  // Far past the limits in both axes: the clamp is what keeps the camera from
  // ending up under the terrain or behind the creature.
  target.dispatch("pointermove", { clientX: 100000, clientY: 100000 });
  assert.equal(state.orbitYaw, 0.85);
  assert.equal(state.orbitPitch, 1.05);

  target.dispatch("pointerup", { pointerId: 1 });
  assert.equal(state.dragging, false);

  // A move after release must not keep orbiting.
  const yawAfterRelease = state.orbitYaw;
  target.dispatch("pointermove", { clientX: 0, clientY: 0 });
  assert.equal(state.orbitYaw, yawAfterRelease);
});

test("the wheel zooms within bounds and suppresses the page scroll", async () => {
  await boot();
  const target = document.createElement("canvas");
  bindPointerInput(target);

  let prevented = 0;
  const wheel = (deltaY) => target.dispatch("wheel", { deltaY, preventDefault: () => (prevented += 1) });

  wheel(100);
  assert.equal(Math.abs(state.cameraZoom - 1.08) < 1e-9, true);
  assert.equal(prevented, 1, "the wheel handler must stop the page from scrolling under the canvas");

  wheel(1e6);
  assert.equal(state.cameraZoom, 1.48);
  wheel(-1e6);
  assert.equal(state.cameraZoom, 0.68);
});

test("the action buttons drive playback, camera mode and effects", async () => {
  const { fetchReplay } = await boot();
  ui.playToggle.listeners.clear();
  ui.cameraMode.listeners.clear();
  ui.fxToggle.listeners.clear();
  ui.reroll.listeners.clear();
  ui.controller.listeners.clear();
  ui.task.listeners.clear();
  globalThis.window.listeners.delete("keydown");
  graphics3d.bindActions(fetchReplay);

  assert.equal(ui.playToggle.querySelector("span").textContent, "Pause");
  ui.playToggle.dispatch("click");
  assert.equal(state.playing, false);
  assert.equal(ui.playToggle.querySelector("span").textContent, "Play");
  assert.equal(ui.playToggle.querySelector("i").className, "ph ph-play");

  ui.cameraMode.dispatch("click");
  assert.equal(state.cameraMode, "orbit");
  ui.cameraMode.dispatch("click");
  assert.equal(state.cameraMode, "showcase");
  ui.cameraMode.dispatch("click");
  assert.equal(state.cameraMode, "follow", "the camera modes must cycle rather than run off the end");

  ui.fxToggle.dispatch("click");
  assert.equal(state.effectsEnabled, false);
  assert.equal(ui.fxToggle.querySelector("span").textContent, "Lite FX");

  // Space is the playback shortcut, and must not also scroll the page.
  let prevented = 0;
  globalThis.window.dispatch("keydown", { code: "Space", preventDefault: () => (prevented += 1) });
  assert.equal(state.playing, true);
  assert.equal(prevented, 1);
  globalThis.window.dispatch("keydown", { code: "KeyK", preventDefault: () => (prevented += 1) });
  assert.equal(state.playing, true, "other keys must be left to the page");
  assert.equal(prevented, 1);
});

test("evolving again advances the generation and drops the archive selection", async () => {
  const { fetchReplay } = await boot();
  ui.reroll.listeners.clear();
  ui.controller.listeners.clear();
  ui.task.listeners.clear();
  graphics3d.bindActions(fetchReplay);
  replayRequest.cell_x = 1;
  replayRequest.cell_y = 1;
  requests.length = 0;

  ui.reroll.dispatch("click");
  await new Promise((resolve) => setImmediate(resolve));

  assert.equal(replayRequest.generations, 13);
  assert.equal("cell_x" in replayRequest, false, "evolving forward must not pin the previous archive cell");
  assert.equal(requests.length, 1);
  assert.doesNotMatch(requests[0], /cell_x/);

  // The generation budget is capped so the button cannot queue an unbounded run.
  replayRequest.generations = 40;
  ui.reroll.dispatch("click");
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(replayRequest.generations, 40);
});

test("changing a control refetches with the new controller and terrain", async () => {
  const { fetchReplay } = await boot();
  ui.reroll.listeners.clear();
  ui.controller.listeners.clear();
  ui.task.listeners.clear();
  graphics3d.bindActions(fetchReplay);
  requests.length = 0;

  ui.controller.value = "recurrent";
  ui.task.value = "flat";
  ui.task.dispatch("change");
  await new Promise((resolve) => setImmediate(resolve));

  assert.equal(replayRequest.controller, "recurrent");
  assert.equal(replayRequest.task, "flat");
  assert.match(requests[0], /controller=recurrent/);
  assert.match(requests[0], /task=flat/);
});

test("the control hint fades exactly once, on a timer", async () => {
  await boot();
  const timers = globalThis.window.timeouts;
  timers.length = 0;
  ui.hint.classList.remove("is-faded");

  fadeHint();
  assert.equal(timers.length, 1);
  fadeHint();
  assert.equal(timers.length, 1, "a second interaction must not queue a second fade");

  assert.equal(ui.hint.classList.contains("is-faded"), false, "the hint must survive until the timer runs");
  timers[0]();
  assert.equal(ui.hint.classList.contains("is-faded"), true);
});

test("setActionButton leaves a button without a label or icon alone", async () => {
  await boot();
  const bare = document.createElement("button");
  setActionButton(bare, "Pause", "ph-pause");
  assert.equal(bare.childElementCount, 0, "a bare button must not gain markup or throw");
});

test("updateMetrics is a no-op until a replay has landed", async () => {
  await boot();
  ui.time.textContent = "untouched";
  state.replay = null;
  updateMetrics();
  assert.equal(ui.time.textContent, "untouched");
});

test("autoplay defers to prefers-reduced-motion when a replay lands", async () => {
  resetState();
  globalThis.window = createWindowStub({ reducedMotion: true });
  const three = fakeThree();
  const world = fakeScenery();
  const handles = start(three.THREE, world.scenery);
  await handles.ready;

  assert.equal(
    state.playing,
    false,
    "a reduced-motion preference must leave the freshly loaded replay paused",
  );
  assert.equal(ui.playToggle.querySelector("span").textContent, "Play");

  globalThis.window = createWindowStub();
});

test.after(() => {
  console.error = realConsoleError;
});
