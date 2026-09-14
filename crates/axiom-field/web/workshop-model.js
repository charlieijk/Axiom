export const DESIGN_FIELDS = Object.freeze([
  ['body_length_m', 'Body length', 0.12, 0.24, 0.005, 'm'],
  ['body_width_m', 'Body width', 0.08, 0.16, 0.005, 'm'],
  ['body_mass_kg', 'Body mass', 0.15, 0.6, 0.01, 'kg'],
  ['upper_length_m', 'Upper leg', 0.035, 0.075, 0.001, 'm'],
  ['lower_length_m', 'Lower leg', 0.04, 0.09, 0.001, 'm'],
  ['friction', 'Ground friction', 0.3, 1.3, 0.05, ''],
]);
export function geometrySignature(bodies) {
  return JSON.stringify(bodies.map(({ shape, size, kind }) => [shape, size, kind]));
}
export function replayIndex(value, count) {
  return Math.max(0, Math.min(Math.max(0, count - 1), Math.round(Number(value) || 0)));
}
export function parseWorkshop(text) {
  if (new TextEncoder().encode(text).byteLength > 256 * 1024) throw new Error('Workshop files must be 256 KiB or smaller.');
  const value = JSON.parse(text);
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Choose an exported Axiom workshop JSON object.');
  return value;
}
export function designSummary(design) {
  if (!design) return 'Design unavailable';
  return `${Math.round(design.body_length_m * 1000)} × ${Math.round(design.body_width_m * 1000)} mm · ${Number(design.body_mass_kg).toFixed(2)} kg`;
}
export function validateDesign(design) {
  return DESIGN_FIELDS.every(([key, , min, max]) => Number.isFinite(design[key]) && design[key] >= min && design[key] <= max);
}
export function jobFraction(job) {
  return job?.total > 0 ? Math.max(0, Math.min(1, job.evaluated / job.total)) : 0;
}
