import test from 'node:test';
import assert from 'node:assert/strict';
import { REFERENCE_GAIT, progressPercent, isTerminal, stepDelay, outcomeCopy } from '../lab-model.js';
test('progress reflects signed forward distance and cannot exceed the challenge bounds', () => {
  assert.equal(progressPercent(-0.2, 1), 0);
  assert.equal(progressPercent(0.34, 1), 34);
  assert.equal(progressPercent(1.3, 1), 100);
  for (const [distance, goal] of [[NaN, 1], [1, 0], [Infinity, 1], [1, NaN]]) assert.equal(progressPercent(distance, goal), 0);
});
test('only authoritative complete, fallen and timeout outcomes terminate a run', () => {
  assert.equal(isTerminal('complete'), true);
  assert.equal(isTerminal('fallen'), true);
  assert.equal(isTerminal('timeout'), true);
  for (const outcome of ['ready', 'running', 'paused', undefined]) assert.equal(isTerminal(outcome), false);
});
test('step pacing uses simulation duration and never schedules a negative wait', () => {
  assert.equal(stepDelay({ step_seconds: 0.05 }, 12), 38);
  assert.equal(stepDelay({ step_seconds: 0.05 }, 150), 0);
  assert.equal(stepDelay({}, 10), 40);
  assert.equal(stepDelay({ step_seconds: -1 }, 10), 40);
});
test('pause messaging is distinct from ready, and success never comes from client distance', () => {
  assert.match(outcomeCopy('running', false)[0], /paused/);
  assert.match(outcomeCopy('running', true)[0], /running/);
  assert.match(outcomeCopy('complete', false)[0], /complete/);
  assert.match(outcomeCopy('fallen', false)[0], /lost/);
  assert.match(outcomeCopy('ready', false)[0], /challenge/);
  assert.deepEqual(REFERENCE_GAIT, { frequency_hz: 2, hip_amplitude: 0.15, knee_amplitude: 0.12 });
});
