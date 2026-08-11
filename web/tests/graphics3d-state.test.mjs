// The 3D field journal's shared state module owns the small pure helpers the
// rest of the page formats its labels with. It reads DOM handles at import
// time, so the document stub has to exist before the dynamic import runs.
import assert from "node:assert/strict";
import test from "node:test";

import { createDocumentStub, createWindowStub } from "./support/dom.mjs";

globalThis.document = createDocumentStub();
globalThis.window = createWindowStub();

const { CAMERA_MODES, clamp, controllerBehaviorNote, labelFor, replayRequest, state } =
  await import("../graphics3d-state.js");

test("labelFor keeps the CPG acronym uppercase", () => {
  assert.equal(labelFor("cpg"), "CPG");
});

test("labelFor title-cases hyphen and underscore separated names", () => {
  assert.equal(labelFor("feedforward"), "Feedforward");
  assert.equal(labelFor("rough-terrain"), "Rough Terrain");
  assert.equal(labelFor("rough_terrain"), "Rough Terrain");
  // Repeated separators must not produce empty words.
  assert.equal(labelFor("gap--crossing"), "Gap Crossing");
});

test("controllerBehaviorNote describes each controller distinctly", () => {
  const feedforward = controllerBehaviorNote("feedforward");
  const recurrent = controllerBehaviorNote("recurrent");
  const fallback = controllerBehaviorNote("cpg");

  assert.match(feedforward, /current sensor vector/);
  assert.match(recurrent, /prior outputs/);
  // The note is a claim about the simulator, so it must not promise that the
  // controller itself produces the physics.
  assert.match(fallback, /physics still comes from the simulator/);
  assert.equal(new Set([feedforward, recurrent, fallback]).size, 3);
});

test("controllerBehaviorNote falls back to the CPG note for unknown controllers", () => {
  assert.equal(controllerBehaviorNote("something-else"), controllerBehaviorNote("cpg"));
});

test("clamp bounds a value from both sides and passes through the middle", () => {
  assert.equal(clamp(5, 0, 10), 5);
  assert.equal(clamp(-3, 0, 10), 0);
  assert.equal(clamp(42, 0, 10), 10);
  assert.equal(clamp(0, 0, 0), 0);
});

test("the default replay request targets the evolved preview", () => {
  // The 3D page is the evolved view; a regression to "minimal" here would
  // silently strip the evolution history the whole journal renders from.
  assert.equal(replayRequest.mode, "evolved");
  assert.ok(CAMERA_MODES.includes(state.cameraMode), "default camera mode must be a known mode");
});

test("autoplay starts on by default but defers to prefers-reduced-motion", async () => {
  // The default stub reports no motion preference, so the journal animates.
  assert.equal(state.playing, true);

  // Re-import under a reduced-motion preference. The query string is only a
  // cache buster; it changes nothing about the module that loads.
  globalThis.window = createWindowStub({ reducedMotion: true });
  const reduced = await import("../graphics3d-state.js?prefers-reduced-motion");

  assert.equal(
    reduced.state.playing,
    false,
    "a reduced-motion preference must leave the journal paused rather than animating",
  );

  globalThis.window = createWindowStub();
});
