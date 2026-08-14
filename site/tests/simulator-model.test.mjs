import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  normalizeTrialConfig,
  runDeterministicTrial,
} from "../app/simulator-model.ts";

function assertClose(actual, expected, tolerance, label) {
  assert.ok(
    Math.abs(actual - expected) <= tolerance,
    `${label}: expected ${actual} to be within ${tolerance} of ${expected}`,
  );
}

test("a trial is deterministic for the same controls and seed", () => {
  const config = {
    controller: "cpg",
    terrain: "rough",
    seed: 19,
    steps: 120,
    dt: 0.05,
  };

  assert.deepEqual(runDeterministicTrial(config), runDeterministicTrial(config));
});

test("a trial exposes the real creature replay contract", () => {
  const trial = runDeterministicTrial({
    controller: "recurrent",
    terrain: "flat",
    seed: 42,
    steps: 80,
    dt: 0.05,
  });

  assert.equal(trial.frames.length, 81);
  assert.equal(trial.body.length, 5);
  assert.equal(trial.frames[0].bodies.length, 5);
  assert.equal(trial.frames[0].joints.length, 4);
  assert.equal(trial.controller, "recurrent");
  assert.equal(trial.terrain, "flat");
  assert.ok(Number.isFinite(trial.metrics.fitness));
  assert.ok(Number.isFinite(trial.metrics.distance));
  assert.ok(trial.metrics.stability >= 0 && trial.metrics.stability <= 1);
});

test("controllers and terrain materially change the replay", () => {
  const base = { seed: 19, steps: 140, dt: 0.05 };
  const cpgRough = runDeterministicTrial({
    ...base,
    controller: "cpg",
    terrain: "rough",
  });
  const feedforwardRough = runDeterministicTrial({
    ...base,
    controller: "feedforward",
    terrain: "rough",
  });
  const cpgFlat = runDeterministicTrial({
    ...base,
    controller: "cpg",
    terrain: "flat",
  });

  assert.notEqual(
    cpgRough.frames.at(-1).root[0],
    feedforwardRough.frames.at(-1).root[0],
  );
  assert.notEqual(
    cpgRough.frames.at(-1).root[1],
    cpgFlat.frames.at(-1).root[1],
  );
});

test("trial controls are validated at the browser boundary", () => {
  assert.deepEqual(
    normalizeTrialConfig({
      controller: "unknown",
      terrain: "lava",
      seed: 0,
      steps: 10_000,
      dt: 2,
    }),
    {
      controller: "cpg",
      terrain: "rough",
      seed: 1,
      steps: 360,
      dt: 0.1,
    },
  );
});

test("the browser model stays aligned with a canonical Rust replay", async () => {
  const golden = JSON.parse(
    await readFile(new URL("./fixtures/axiom-rust-golden.json", import.meta.url), "utf8"),
  );
  const trial = runDeterministicTrial(golden.config);

  assertClose(trial.metrics.fitness, golden.metrics.fitness, 0.001, "fitness");
  assertClose(trial.metrics.distance, golden.metrics.distance, 0.001, "distance");
  assertClose(
    trial.metrics.stableDistance,
    golden.metrics.stableDistance,
    0.001,
    "stable distance",
  );
  assertClose(trial.metrics.stability, golden.metrics.stability, 0.001, "stability");

  for (const expected of golden.frames) {
    const actual = trial.frames[expected.index];
    assertClose(actual.time, expected.time, 0.0001, `frame ${expected.index} time`);
    assertClose(actual.root[0], expected.root[0], 0.0001, `frame ${expected.index} root x`);
    assertClose(actual.root[1], expected.root[1], 0.0001, `frame ${expected.index} root y`);
    assertClose(actual.tilt, expected.tilt, 0.0001, `frame ${expected.index} tilt`);
    expected.bodies.forEach((body, bodyIndex) => {
      assertClose(
        actual.bodies[bodyIndex][0],
        body[0],
        0.0001,
        `frame ${expected.index} body ${bodyIndex} x`,
      );
      assertClose(
        actual.bodies[bodyIndex][1],
        body[1],
        0.0001,
        `frame ${expected.index} body ${bodyIndex} y`,
      );
    });
  }
});
