export const REFERENCE_GAIT = Object.freeze({ frequency_hz: 2, hip_amplitude: 0.15, knee_amplitude: 0.12 });
export function progressPercent(distance, goal) {
  return Number.isFinite(distance) && Number.isFinite(goal) && goal > 0
    ? Math.min(100, Math.max(0, distance / goal * 100)) : 0;
}
export function isTerminal(outcome) { return outcome === 'complete' || outcome === 'fallen' || outcome === 'timeout'; }
export function stepDelay(snapshot, requestMs) {
  const seconds = Number.isFinite(snapshot.step_seconds) && snapshot.step_seconds > 0 ? snapshot.step_seconds : 0.05;
  return Math.max(0, seconds * 1000 - Math.max(0, requestMs));
}
export function outcomeCopy(outcome, active) {
  if (outcome === 'complete') return ['Trial complete', 'Finish line crossed. Reset the trial or test the 6 mm rails.', '✓'];
  if (outcome === 'timeout') return ['Trial timed out', '60 seconds elapsed. Adjust the gait and reset the trial.', '↺'];
  if (outcome === 'fallen') return ['Trial failed · balance lost', 'Reduce the stride, reset the trial, and try again.', '↺'];
  if (active) return ['Trial running', 'Advance 1 m and stay upright. Gait changes apply to the next step.', '↗'];
  if (outcome === 'running') return ['Trial paused', 'Physics paused. Adjust the gait or resume the trial.', 'Ⅱ'];
  return ['The one-metre challenge', 'Objective: advance 1 m and stay upright. Tune the gait, then start.', '01'];
}
