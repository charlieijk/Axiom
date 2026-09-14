import test from 'node:test';
import assert from 'node:assert/strict';
import { geometrySignature, replayIndex, parseWorkshop, DESIGN_FIELDS, validateDesign, jobFraction, designSummary } from '../workshop-model.js';
test('geometry changes rebuild equal-count bodies while movement preserves geometry', () => {
  const bodies = [{ shape: 'box', size: [0.1, 0.02, 0.05], kind: 'chassis', position: [0, 0, 0] }];
  assert.equal(geometrySignature(bodies), geometrySignature([{ ...bodies[0], position: [1, 2, 3] }]));
  assert.notEqual(geometrySignature(bodies), geometrySignature([{ ...bodies[0], size: [0.12, 0.02, 0.05] }]));
  assert.notEqual(geometrySignature(bodies), geometrySignature([{ ...bodies[0], kind: 'obstacle' }]));
});
test('scrubbing cannot leave the recorded frame range', () => {
  assert.equal(replayIndex(-4, 100), 0); assert.equal(replayIndex(500, 100), 99);
  assert.equal(replayIndex('3', 100), 3); assert.equal(replayIndex(NaN, 100), 0);
  assert.equal(replayIndex(5, 0), 0); assert.equal(replayIndex(5, 1), 0);
});
test('workshop import enforces bytes rather than character count and object shape', () => {
  assert.deepEqual(parseWorkshop('{"version":1}'), { version: 1 });
  assert.throws(() => parseWorkshop('[]'), /object/);
  assert.throws(() => parseWorkshop('null'), /object/);
  assert.throws(() => parseWorkshop('{broken'), SyntaxError);
  assert.throws(() => parseWorkshop(JSON.stringify({ label: 'é'.repeat(131072) })), /256 KiB/);
});
test('design boundaries reject non-finite and out-of-range physical values', () => {
  const design = Object.fromEntries(DESIGN_FIELDS.map(([key, , min]) => [key, min]));
  assert.equal(validateDesign(design), true);
  for (const [key, , min, max] of DESIGN_FIELDS) {
    assert.equal(validateDesign({ ...design, [key]: max }), true);
    assert.equal(validateDesign({ ...design, [key]: min - 0.001 }), false);
    assert.equal(validateDesign({ ...design, [key]: Infinity }), false);
  }
  assert.equal(validateDesign({}), false);
  assert.match(designSummary(design), /120 × 80 mm/);
});
test('job progress remains bounded for empty and completed batches', () => {
  assert.equal(jobFraction({ evaluated: 0, total: 0 }), 0);
  assert.equal(jobFraction({ evaluated: 6, total: 12 }), 0.5);
  assert.equal(jobFraction({ evaluated: 13, total: 12 }), 1);
});
