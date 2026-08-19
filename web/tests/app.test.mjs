// web/app.js is a classic script: it reads the DOM at top level and exports
// nothing, so it is loaded into a vm context with stubbed browser globals.
// Its top-level `function` declarations land on that context's global object,
// which is what lets these tests call fetchReplay/tick directly. Mutable state
// lives in a `const`, so it is observed through `window.render_game_to_text` --
// the same interface the Rust GUI smoke check in src/web.rs reads.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";

// The element stub already hands app.js a canvas context from its own
// getContext(), so this suite never needs createCanvasContext directly.
import { createDocumentStub, createWindowStub } from "./support/dom.mjs";

const source = await readFile(new URL("../app.js", import.meta.url), "utf8");

// Mirrors the shape src/web/replay.rs serializes (ReplayResponse /
// ReplayBodyNode / ReplayFrameResponse). Keeping the fixture faithful to the
// real schema is the point: a test that drew from an invented shape would pass
// while the page broke.
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
    genome_id: 1,
    controller: "cpg",
    task: "rough",
    seed: 19,
    source: "minimal",
    dt: 0.05,
    body: [
      { id: 0, parent: null, size: [0.4, 0.2], actuator: 0 },
      { id: 1, parent: 0, size: [0.3, 0.16], actuator: 0.5 },
    ],
    frames: [frameAt(0), frameAt(1), frameAt(2), frameAt(3)],
    ...overrides,
  };
}

function okResponse(replay = sampleReplay()) {
  return { ok: true, status: 200, json: async () => replay };
}

/** A promise whose resolution the test drives, so response ordering is explicit. */
function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/**
 * Load app.js into a fresh context.
 *
 * `respond` is the fetch stub: it receives the requested URL and returns the
 * Response-like object (or a promise for one) that the app should see.
 */
function loadApp({ controls = {}, respond } = {}) {
  const document = createDocumentStub();
  const defaults = {
    controller: "cpg",
    task: "rough",
    seed: "19",
    frames: "220",
    speed: "1",
  };
  for (const [id, value] of Object.entries({ ...defaults, ...controls })) {
    document.getElementById(id).value = value;
  }

  const requests = [];
  const window = createWindowStub();
  let now = 0;

  const context = {
    document,
    window,
    console: { error() {}, warn() {}, log() {} },
    performance: { now: () => now },
    requestAnimationFrame: () => 0,
    URLSearchParams,
    fetch(url) {
      requests.push(url);
      // `callIndex` is 1-based, and call 1 is always the request app.js makes
      // for itself at load; tests that care about ordering key off it.
      if (!respond) {
        return Promise.resolve(okResponse());
      }
      return respond(url, requests.length);
    },
  };
  context.globalThis = context;
  vm.createContext(context);
  vm.runInContext(source, context, { filename: "app.js" });

  return {
    context,
    document,
    requests,
    setNow(value) {
      now = value;
    },
    /** The app's own state report, parsed. */
    report: () => JSON.parse(window.render_game_to_text()),
    status: () => document.getElementById("status").textContent,
  };
}

test("fetchReplay builds the query from the current control values", async () => {
  const app = loadApp({ controls: { controller: "recurrent", task: "gap", seed: "77", frames: "150" } });
  await app.context.fetchReplay();

  // The load-time requestReplay fires one request; ours is the latest.
  const url = new URL(app.requests.at(-1), "http://127.0.0.1:8787");
  assert.equal(url.pathname, "/api/replay");
  assert.equal(url.searchParams.get("mode"), "minimal", "the 2D page always requests the minimal replay");
  assert.equal(url.searchParams.get("controller"), "recurrent");
  assert.equal(url.searchParams.get("task"), "gap");
  assert.equal(url.searchParams.get("seed"), "77");
  assert.equal(url.searchParams.get("frames"), "150");
});

test("a successful replay is applied to the page state", async () => {
  const app = loadApp();
  await app.context.fetchReplay();

  const report = app.report();
  assert.equal(report.ready, true);
  assert.equal(report.frames, 4);
  assert.equal(report.bodies, 2);
  assert.equal(report.controller, "cpg");
  assert.equal(app.status(), "Ready");
  assert.equal(app.document.getElementById("scrubber").max, 3, "scrubber spans frame 0..n-1");
  assert.equal(app.document.getElementById("joint-count").textContent, "1", "joints are bodies minus one");
});

test("a superseded in-flight replay is discarded rather than applied late", async () => {
  const stale = deferred();
  const fresh = deferred();
  const app = loadApp({
    respond: (_url, callIndex) => {
      // Call 1 is app.js loading itself; calls 2 and 3 are this test's, and it
      // decides when each of them lands.
      if (callIndex === 2) {
        return stale.promise;
      }
      if (callIndex === 3) {
        return fresh.promise;
      }
      return Promise.resolve(okResponse());
    },
  });

  const staleCall = app.context.fetchReplay();
  const freshCall = app.context.fetchReplay();

  // Resolve out of order: the newer request lands first, the superseded one after.
  fresh.resolve(okResponse(sampleReplay({ controller: "winner" })));
  await freshCall;
  stale.resolve(okResponse(sampleReplay({ controller: "loser" })));
  await staleCall;

  assert.equal(
    app.report().controller,
    "winner",
    "the late stale response must not overwrite the newer replay",
  );
});

test("a replay whose body parses late is discarded once superseded", async () => {
  // fetchReplay re-checks staleness twice: once when fetch() resolves and again
  // after response.json() does. This covers the second check -- the response
  // arrives while it is still current, but its body finishes parsing only after
  // a newer request has already landed.
  const staleBody = deferred();
  const app = loadApp({
    respond: (_url, callIndex) => {
      if (callIndex === 2) {
        return Promise.resolve({ ok: true, status: 200, json: () => staleBody.promise });
      }
      return Promise.resolve(okResponse(sampleReplay({ controller: "winner" })));
    },
  });

  const staleCall = app.context.fetchReplay();
  // Let the stale request get past the post-fetch check before superseding it.
  await new Promise((resolve) => setImmediate(resolve));
  await app.context.fetchReplay();

  staleBody.resolve(sampleReplay({ controller: "loser" }));
  await staleCall;

  assert.equal(
    app.report().controller,
    "winner",
    "a body that parses after a newer replay landed must not overwrite it",
  );
});

test("a failed replay request surfaces an error instead of stale data", async () => {
  const app = loadApp({ respond: () => Promise.resolve({ ok: false, status: 500, json: async () => ({}) }) });

  await assert.rejects(() => app.context.fetchReplay(), /Replay request failed: 500/);

  app.context.requestReplay();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(app.status(), "Error");
  assert.equal(app.report().ready, false, "a failed load must not report a replay as ready");
});

test("tick advances one frame per dt of playback time", async () => {
  const app = loadApp();
  await app.context.fetchReplay();
  assert.equal(app.report().frame, 0);

  // dt is 0.05s, so a frame is 50ms of playback at speed 1.
  app.setNow(0);
  app.context.tick(0);
  app.context.tick(50);
  assert.equal(app.report().frame, 1);

  app.context.tick(100);
  assert.equal(app.report().frame, 2);
});

test("tick wraps the frame index around the end of the replay", async () => {
  const app = loadApp();
  await app.context.fetchReplay();

  app.context.tick(0);
  // Four frames at 50ms each: 200ms of playback returns to frame 0. Feed it in
  // 100ms steps because a single tick clamps its delta to 100ms.
  app.context.tick(100);
  app.context.tick(200);
  assert.equal(app.report().frame, 0, "frame index is modulo the frame count");
});

test("tick clamps a long stall so a backgrounded tab does not fast-forward", async () => {
  const app = loadApp();
  await app.context.fetchReplay();

  app.context.tick(0);
  // Five seconds of wall clock must buy only the clamped 100ms of playback,
  // i.e. two frames -- not a hundred.
  app.context.tick(5000);
  assert.equal(app.report().frame, 2);
});

test("the replay is not advanced while playback is paused", async () => {
  const app = loadApp();
  await app.context.fetchReplay();

  app.document.getElementById("play").dispatch("click");
  app.context.tick(0);
  app.context.tick(100);

  assert.equal(app.report().frame, 0, "a paused replay holds its frame");
  assert.equal(app.document.getElementById("play").textContent, "Play");
});

test("changing a control only refetches when the request key actually changed", async () => {
  const app = loadApp();
  await app.context.fetchReplay();
  const before = app.requests.length;

  // Same values: the app must not re-request.
  app.document.getElementById("task").dispatch("change");
  assert.equal(app.requests.length, before, "an unchanged control must not trigger a request");

  app.document.getElementById("task").value = "flat";
  app.document.getElementById("task").dispatch("change");
  assert.equal(app.requests.length, before + 1, "a changed control must trigger exactly one request");
});
