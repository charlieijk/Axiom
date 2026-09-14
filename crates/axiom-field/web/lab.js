import * as THREE from '/vendor/three.module.min.js';
import { setupWorkshop } from './workshop.js';
import { geometrySignature } from './workshop-model.js';
import { REFERENCE_GAIT, progressPercent, isTerminal, stepDelay, outcomeCopy } from './lab-model.js';

const $ = (id) => document.getElementById(id);
const canvas = $('world');
let snapshot = null;
let running = false;
let timer = null;
let inFlight = null;
let resetting = false;
let connected = false;
let replaying = false;
let bodySignature = "";
let workshop = null;
let scene, camera, renderer;
let follow = true;
let yaw = 0.82, pitch = 0.54, radius = 1.04;
const target = new THREE.Vector3(0.2, 0.055, 0);
const meshes = [];
const materials = {
  ground: new THREE.MeshStandardMaterial({ color: 0xc5cdbb, roughness: 0.94 }),
  chassis: new THREE.MeshStandardMaterial({ color: 0xdbe978, metalness: 0.32, roughness: 0.38 }),
  limb: new THREE.MeshStandardMaterial({ color: 0x34453e, metalness: 0.55, roughness: 0.42 }),
  obstacle: new THREE.MeshStandardMaterial({ color: 0xce8a44, metalness: 0.15, roughness: 0.68 }),
};

function planeMark(width, depth, x, z, color, opacity = 1) {
  const mesh = new THREE.Mesh(new THREE.PlaneGeometry(width, depth), new THREE.MeshBasicMaterial({ color, transparent: opacity < 1, opacity, depthWrite: false }));
  mesh.rotation.x = -Math.PI / 2;
  mesh.position.set(x, 0.001, z);
  scene.add(mesh);
  return mesh;
}
function floorLabel(text, x, z, width, color = '#52654b') {
  const plate = document.createElement('canvas');
  plate.width = 512; plate.height = 128;
  const ctx = plate.getContext('2d');
  ctx.fillStyle = color; ctx.font = '600 54px sans-serif'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.fillText(text, 256, 64);
  const texture = new THREE.CanvasTexture(plate);
  texture.colorSpace = THREE.SRGBColorSpace;
  const mesh = new THREE.Mesh(new THREE.PlaneGeometry(width, width / 4), new THREE.MeshBasicMaterial({ map: texture, transparent: true, depthWrite: false }));
  mesh.rotation.set(-Math.PI / 2, 0, -Math.PI / 2);
  mesh.position.set(x, 0.002, z);
  scene.add(mesh);
}
function initScene() {
  scene = new THREE.Scene();
  scene.background = new THREE.Color(0xdde5d8);
  scene.fog = new THREE.Fog(0xdde5d8, 2.3, 7);
  camera = new THREE.PerspectiveCamera(42, 1, 0.005, 30);
  renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: false });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.2;
  scene.add(new THREE.HemisphereLight(0xf5f8ee, 0x75826a, 2.6));
  const sun = new THREE.DirectionalLight(0xfff1d4, 3.4);
  sun.position.set(-1.5, 3, 1.3);
  sun.castShadow = true;
  sun.shadow.mapSize.set(2048, 2048);
  Object.assign(sun.shadow.camera, { left: -3, right: 3, top: 3, bottom: -3, near: 0.1, far: 10 });
  sun.shadow.bias = -0.00015;
  sun.shadow.normalBias = 0.001;
  scene.add(sun);
  // Painted markings have no collision geometry. The API supplies every physical body.
  planeMark(2.1, 0.68, 0.55, 0, 0xdfe3cc);
  for (const z of [-0.345, 0.345]) planeMark(2.1, 0.009, 0.55, z, 0x7d8c6c);
  planeMark(0.012, 0.68, 0, 0, 0x8a9979);
  for (let i = 0; i < 12; i++) planeMark(0.025, 0.056, 1 + (i % 2) * 0.025, -0.308 + i * 0.056, i % 2 ? 0xe8edc5 : 0x596b48);
  for (let i = -1; i <= 6; i++) {
    planeMark(0.006, 0.028, i * 0.25, -0.385, 0x82916f);
    floorLabel(`${(i * 0.25).toFixed(2)}`, i * 0.25, -0.43, 0.09);
  }
  floorLabel('START', -0.07, 0.22, 0.13);
  floorLabel('01 / FINISH', 1.13, 0, 0.26);
  floorLabel('AXIOM', 0.47, 0.56, 0.65);
  const grid = new THREE.GridHelper(8, 80, 0xb0bda5, 0xb0bda5);
  grid.position.y = 0.0005;
  grid.material.transparent = true; grid.material.opacity = 0.25;
  scene.add(grid);
  const resize = () => {
    const { width, height } = canvas.getBoundingClientRect();
    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
  };
  new ResizeObserver(resize).observe(canvas);
  resize();
  renderer.setAnimationLoop(() => {
    if (follow && snapshot) target.set(snapshot.sample.position_m[0] + 0.14, 0.055, snapshot.sample.position_m[2]);
    camera.position.set(target.x + Math.sin(yaw) * Math.cos(pitch) * radius, target.y + Math.sin(pitch) * radius, target.z + Math.cos(yaw) * Math.cos(pitch) * radius);
    camera.lookAt(target);
    renderer.render(scene, camera);
  });
}
function syncBodies(bodies) {
  const signature = geometrySignature(bodies);
  if (bodySignature !== signature) {
    bodySignature = signature;
    for (const mesh of meshes) { scene.remove(mesh); mesh.geometry.dispose(); }
    meshes.length = 0;
    for (const body of bodies) {
      const geometry = body.shape === 'capsule'
        ? new THREE.CapsuleGeometry(body.size[0], body.size[1] * 2, 8, 12)
        : new THREE.BoxGeometry(...body.size.map((half) => Math.min(half * 2, 10)));
      const mesh = new THREE.Mesh(geometry, materials[body.kind] || materials.limb);
      mesh.castShadow = body.kind !== 'ground'; mesh.receiveShadow = true;
      scene.add(mesh); meshes.push(mesh);
    }
  }
  for (let i = 0; i < bodies.length; i++) {
    meshes[i].position.fromArray(bodies[i].position);
    meshes[i].quaternion.fromArray(bodies[i].rotation);
  }
}
function renderState(next) {
  snapshot = next;
  if (isTerminal(next.outcome)) running = false;
  syncBodies(next.bodies);
  $('distance').textContent = next.distance_m.toFixed(2);
  $('goal').textContent = `${next.goal_m.toFixed(2)} m`;
  $('elapsed').innerHTML = `${next.elapsed_s.toFixed(1)} <small>s</small>`;
  $('tilt').innerHTML = `${(next.sample.tilt_rad * 180 / Math.PI).toFixed(1)} <small>°</small>`;
  $('tick').textContent = next.sample.tick;
  const percent = progressPercent(next.distance_m, next.goal_m);
  $('progress-fill').style.width = `${percent}%`;
  $('progress').setAttribute('aria-valuenow', String(Math.round(percent)));
  $('range-name').textContent = next.course === 'rails' ? 'RANGE 02 · LOW RAILS' : 'RANGE 01 · FLAT GROUND';
  $('trial-name').textContent = next.course === 'rails' ? '02 / Rail crossing' : '01 / Gait calibration';
  $('trial-description').textContent = next.course === 'rails' ? '6 mm rails · 1 m forward · stay upright' : 'Flat surface · 1 m forward · stay upright';
  document.querySelectorAll('[data-course]').forEach((button) => {
    const selected = button.dataset.course === next.course;
    button.classList.toggle('selected', selected); button.setAttribute('aria-pressed', String(selected));
  });
  renderControls();
  workshop?.onSnapshot(next, replaying);
}
function renderControls() {
  const terminal = snapshot && isTerminal(snapshot.outcome);
  $('start').disabled = !connected || resetting || replaying || terminal;
  $('reset').disabled = !connected || resetting || replaying;
  document.querySelectorAll('[data-course]').forEach((button) => { button.disabled = resetting || replaying || !connected; });
  $('start').innerHTML = running ? 'Pause trial <span>Ⅱ</span>' : 'Start trial <span>▶</span>';
  for (const id of ['frequency', 'hip', 'knee', 'reference']) $(id).disabled = replaying || resetting || snapshot?.controller_mode === 'archive';
  $('manual-tuning').hidden = snapshot?.controller_mode === 'archive';
  $('active-controller-settings').hidden = snapshot?.controller_mode !== 'archive';
  $('active-controller-settings').textContent = `Eight independent joint oscillators · ${Number(snapshot?.controller_frequency_hz || 0).toFixed(3)} Hz. Inspect the full controller in Evolve.`;
  $('controller-name').textContent = snapshot?.controller_name || 'Reference trot';
  $('manual-controller').hidden = snapshot?.controller_mode !== 'archive';
  $('manual-controller').disabled = replaying || resetting;
  $('state-pill').textContent = replaying ? 'REPLAY' : !connected ? 'OFFLINE' : resetting ? 'RESETTING' : terminal ? snapshot.outcome.toUpperCase() : running ? 'RUNNING' : snapshot?.outcome === 'ready' ? 'READY' : 'PAUSED';
  if (snapshot && connected) {
    const [title, copy, symbol] = replaying ? ['Recorded trial', 'Playback of saved physics states. Return to live to control the robot.', '▷'] : outcomeCopy(snapshot.outcome, running);
    $('banner-title').textContent = title; $('banner-copy').textContent = copy; $('banner-symbol').textContent = symbol;
    $('run-banner').className = `run-banner ${snapshot.outcome}`;
  }
}
function gait() {
  return { frequency_hz: Number($('frequency').value), hip_amplitude: Number($('hip').value), knee_amplitude: Number($('knee').value) };
}
function showGait() {
  $('frequency-value').textContent = `${Number($('frequency').value).toFixed(2)} Hz`;
  $('hip-value').textContent = Number($('hip').value).toFixed(2);
  $('knee-value').textContent = Number($('knee').value).toFixed(2);
}
function pause() { running = false; clearTimeout(timer); renderControls(); }
function fail(error) {
  pause();
  if (error.status && error.status < 500) {
    $('workshop-message').textContent = error.message;
    return;
  }
  connected = false; renderControls();
  $('connection').textContent = 'Simulator unavailable'; $('connection-dot').className = 'status-dot error';
  $('run-banner').className = 'run-banner error'; $('banner-title').textContent = 'Connection interrupted';
  $('banner-copy').textContent = `${error.message}. Reload this page after restarting the local simulator.`;
  $('banner-symbol').textContent = '!';
}
async function api(path, payload) {
  const response = await fetch(path, { method: payload ? 'POST' : 'GET', headers: { 'Content-Type': 'application/json' }, ...(payload ? { body: JSON.stringify(payload) } : {}), signal: AbortSignal.timeout(10000) });
  const value = await response.json();
  if (!response.ok) { const error = new Error(value.error || value.message || `Simulator returned HTTP ${response.status}`); error.status = response.status; throw error; }
  return value;
}
async function step() {
  if (!running || resetting || replaying || inFlight) return;
  const started = performance.now();
  try {
    inFlight = api('/api/step', gait());
    const next = await inFlight;
    renderState(next);
    if (running && !resetting) timer = setTimeout(step, stepDelay(next, performance.now() - started));
  } catch (error) { fail(error); }
  finally { inFlight = null; }
}
function toggleRun() {
  if (!connected || resetting || replaying || !snapshot || isTerminal(snapshot.outcome)) return;
  if (running) { pause(); return; }
  running = true; renderControls();
  // An existing step will schedule the next request after it settles.
  if (!inFlight) step();
}
async function mutate(operation) {
  if (resetting || replaying) throw new Error(replaying ? 'Return to live before changing the workshop.' : 'Wait for the current operation to finish.');
  pause(); resetting = true; renderControls();
  try {
    if (inFlight) await inFlight;
    const result = await operation();
    if (result?.sample) {
      if (result.gait) { $('frequency').value = result.gait.frequency_hz; $('hip').value = result.gait.hip_amplitude; $('knee').value = result.gait.knee_amplitude; showGait(); }
      renderState(result);
    }
    return result;
  } finally { resetting = false; renderControls(); }
}
async function reset(course = snapshot?.course || 'flat') {
  if (!connected || replaying) return;
  try { await mutate(() => api('/api/reset', { course })); } catch (error) { fail(error); }
}
$('start').addEventListener('click', toggleRun);
$('reset').addEventListener('click', () => reset());
document.querySelectorAll('[data-course]').forEach((button) => button.addEventListener('click', () => reset(button.dataset.course)));
for (const id of ['frequency', 'hip', 'knee']) $(id).addEventListener('input', showGait);
$('reference').addEventListener('click', () => {
  $('frequency').value = REFERENCE_GAIT.frequency_hz; $('hip').value = REFERENCE_GAIT.hip_amplitude; $('knee').value = REFERENCE_GAIT.knee_amplitude; showGait();
});
$('follow').addEventListener('click', () => { follow = !follow; $('follow').setAttribute('aria-pressed', String(follow)); });
$('camera-reset').addEventListener('click', () => { yaw = 0.82; pitch = 0.54; radius = 1.04; follow = true; $('follow').setAttribute('aria-pressed', 'true'); });
let drag = null;
canvas.addEventListener('pointerdown', (event) => { drag = { x: event.clientX, y: event.clientY }; canvas.setPointerCapture(event.pointerId); });
canvas.addEventListener('pointermove', (event) => {
  if (!drag) return;
  yaw -= (event.clientX - drag.x) * 0.006; pitch = THREE.MathUtils.clamp(pitch + (event.clientY - drag.y) * 0.006, 0.08, 1.45);
  drag = { x: event.clientX, y: event.clientY };
});
for (const name of ['pointerup', 'pointercancel', 'lostpointercapture']) canvas.addEventListener(name, () => { drag = null; });
canvas.addEventListener('wheel', (event) => { event.preventDefault(); radius = THREE.MathUtils.clamp(radius * Math.exp(event.deltaY * 0.001), 0.32, 4); }, { passive: false });
document.addEventListener('keydown', (event) => {
  if (['INPUT', 'BUTTON', 'A', 'TEXTAREA', 'SELECT'].includes(event.target.tagName) || event.repeat) return;
  if (event.code === 'Space') { event.preventDefault(); toggleRun(); }
  if (event.code === 'KeyR') reset();
});
document.addEventListener('visibilitychange', () => { if (document.hidden) { pause(); workshop?.pauseReplay(); } });
try {
  initScene();
  const initial = await api('/api/state');
  connected = true; $('connection').textContent = 'Local simulator connected'; $('connection-dot').className = 'status-dot connected';
  if (initial.gait) {
    $('frequency').value = initial.gait.frequency_hz; $('hip').value = initial.gait.hip_amplitude; $('knee').value = initial.gait.knee_amplitude;
  }
  showGait(); renderState(initial);
  workshop = setupWorkshop({ api, mutate, gait, snapshot: () => snapshot,
    async enterReplay(frame) { pause(); resetting = true; renderControls(); try { if (inFlight) await inFlight; replaying = true; renderState(frame); } finally { resetting = false; renderControls(); } },
    showFrame: (frame) => { if (replaying) renderState(frame); },
    async returnLive() { const state = await api('/api/state'); replaying = false; renderState(state); },
  });
  workshop.onSnapshot(initial, false);
} catch (error) { fail(error); }
