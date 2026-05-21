const canvas = document.getElementById("simulator");
const ctx = canvas.getContext("2d");

const controls = {
  controller: document.getElementById("controller"),
  task: document.getElementById("task"),
  seed: document.getElementById("seed"),
  frames: document.getElementById("frames"),
  generate: document.getElementById("generate"),
  play: document.getElementById("play"),
  restart: document.getElementById("restart"),
  speed: document.getElementById("speed"),
  speedValue: document.getElementById("speed-value"),
  scrubber: document.getElementById("scrubber"),
  frameValue: document.getElementById("frame-value"),
  status: document.getElementById("status"),
  bodyCount: document.getElementById("body-count"),
  jointCount: document.getElementById("joint-count"),
  time: document.getElementById("metric-time"),
  distance: document.getElementById("metric-distance"),
  tilt: document.getElementById("metric-tilt"),
};

const state = {
  replay: null,
  frameIndex: 0,
  playing: true,
  lastTick: performance.now(),
  accumulator: 0,
  replayRequestId: 0,
  lastRequestedReplayKey: "",
};

function resizeCanvas() {
  const rect = canvas.getBoundingClientRect();
  const pixelRatio = window.devicePixelRatio || 1;
  canvas.width = Math.max(640, Math.floor(rect.width * pixelRatio));
  canvas.height = Math.max(420, Math.floor(rect.height * pixelRatio));
  ctx.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
  draw();
}

async function fetchReplay() {
  const requestId = state.replayRequestId + 1;
  state.replayRequestId = requestId;
  controls.status.textContent = "Loading";
  const query = new URLSearchParams({
    mode: "minimal",
    controller: controls.controller.value,
    task: controls.task.value,
    seed: controls.seed.value,
    frames: controls.frames.value,
  });
  let response;
  let replay;
  try {
    response = await fetch(`/api/replay?${query.toString()}`);
    if (requestId !== state.replayRequestId) {
      return;
    }
    if (!response.ok) {
      throw new Error(`Replay request failed: ${response.status}`);
    }
    replay = await response.json();
  } catch (error) {
    if (requestId === state.replayRequestId) {
      throw error;
    }
    return;
  }
  if (requestId !== state.replayRequestId) {
    return;
  }
  state.replay = replay;
  canvas.dataset.controller = state.replay.controller;
  canvas.dataset.task = state.replay.task;
  canvas.dataset.source = state.replay.source;
  canvas.dataset.frames = String(state.replay.frames.length);
  state.frameIndex = 0;
  state.accumulator = 0;
  state.playing = true;
  controls.play.textContent = "Pause";
  controls.scrubber.max = Math.max(0, state.replay.frames.length - 1);
  controls.bodyCount.textContent = String(state.replay.body.length);
  controls.jointCount.textContent = String(Math.max(0, state.replay.body.length - 1));
  controls.status.textContent = "Ready";
  updateMetrics();
  draw();
}

function replayControlKey() {
  return [
    controls.controller.value,
    controls.task.value,
    controls.seed.value,
    controls.frames.value,
  ].join(":");
}

function requestReplay() {
  state.lastRequestedReplayKey = replayControlKey();
  fetchReplay().catch((error) => {
    controls.status.textContent = "Error";
    console.error(error);
  });
}

function requestReplayIfChanged() {
  if (replayControlKey() !== state.lastRequestedReplayKey) {
    requestReplay();
  }
}

function draw() {
  const width = canvas.clientWidth;
  const height = canvas.clientHeight;
  ctx.clearRect(0, 0, width, height);
  drawBackground(width, height);

  if (!state.replay) {
    return;
  }

  const frame = state.replay.frames[state.frameIndex];
  const floor = terrainHeight(state.replay.task, frame.root[0]);
  const scale = Math.min(width / 11.5, height / 4.8);
  const camera = {
    x: frame.root[0] - 3.6,
    y: floor - 1.05,
    scale,
  };

  drawWorldGrid(width, height, camera);
  drawTerrain(width, height, camera, state.replay.task);
  drawCreature(frame, state.replay.body, camera);
}

function drawBackground(width, height) {
  const gradient = ctx.createLinearGradient(0, 0, width, height);
  gradient.addColorStop(0, "#111610");
  gradient.addColorStop(0.55, "#0b0e0c");
  gradient.addColorStop(1, "#151512");
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, width, height);

  ctx.fillStyle = "rgba(107, 164, 216, 0.05)";
  for (let i = 0; i < 9; i += 1) {
    ctx.fillRect(i * 150 - 20, 0, 1, height);
  }
}

function drawWorldGrid(width, height, camera) {
  ctx.save();
  ctx.strokeStyle = "rgba(237, 243, 232, 0.06)";
  ctx.lineWidth = 1;
  for (let x = Math.floor(camera.x); x < camera.x + width / camera.scale + 1; x += 1) {
    const screenX = worldToScreen([x, camera.y], camera).x;
    ctx.beginPath();
    ctx.moveTo(screenX, 0);
    ctx.lineTo(screenX, height);
    ctx.stroke();
  }
  for (let y = Math.floor(camera.y); y < camera.y + height / camera.scale + 1; y += 1) {
    const screenY = worldToScreen([camera.x, y], camera).y;
    ctx.beginPath();
    ctx.moveTo(0, screenY);
    ctx.lineTo(width, screenY);
    ctx.stroke();
  }
  ctx.restore();
}

function drawTerrain(width, height, camera, task) {
  ctx.save();
  ctx.beginPath();
  ctx.moveTo(0, height);
  for (let px = 0; px <= width; px += 8) {
    const worldX = camera.x + px / camera.scale;
    const worldY = terrainHeight(task, worldX);
    const point = worldToScreen([worldX, worldY], camera);
    ctx.lineTo(px, point.y);
  }
  ctx.lineTo(width, height);
  ctx.closePath();

  const fill = ctx.createLinearGradient(0, height * 0.65, 0, height);
  fill.addColorStop(0, "#263223");
  fill.addColorStop(1, "#151812");
  ctx.fillStyle = fill;
  ctx.fill();

  ctx.beginPath();
  for (let px = 0; px <= width; px += 8) {
    const worldX = camera.x + px / camera.scale;
    const worldY = terrainHeight(task, worldX);
    const point = worldToScreen([worldX, worldY], camera);
    if (px === 0) {
      ctx.moveTo(px, point.y);
    } else {
      ctx.lineTo(px, point.y);
    }
  }
  ctx.strokeStyle = "#7bd88f";
  ctx.lineWidth = 2;
  ctx.stroke();
  ctx.restore();
}

function drawCreature(frame, body, camera) {
  ctx.save();
  ctx.lineCap = "round";
  ctx.lineJoin = "round";

  for (const joint of frame.joints) {
    const start = worldToScreen(joint[0], camera);
    const end = worldToScreen(joint[1], camera);
    ctx.strokeStyle = "rgba(226, 189, 103, 0.72)";
    ctx.lineWidth = 8;
    ctx.beginPath();
    ctx.moveTo(start.x, start.y);
    ctx.lineTo(end.x, end.y);
    ctx.stroke();
    ctx.strokeStyle = "rgba(16, 18, 16, 0.8)";
    ctx.lineWidth = 2;
    ctx.stroke();
  }

  for (let index = body.length - 1; index >= 0; index -= 1) {
    const node = body[index];
    const center = frame.bodies[index];
    const screen = worldToScreen(center, camera);
    const parent = node.parent;
    const angle =
      parent === null
        ? frame.tilt
        : Math.atan2(center[1] - frame.bodies[parent][1], center[0] - frame.bodies[parent][0]);
    const sizeX = Math.max(18, node.size[0] * camera.scale);
    const sizeY = Math.max(12, node.size[1] * camera.scale);

    ctx.save();
    ctx.translate(screen.x, screen.y);
    ctx.rotate(-angle);
    roundRect(ctx, -sizeX / 2, -sizeY / 2, sizeX, sizeY, Math.min(10, sizeY / 2));
    ctx.fillStyle = index === 0 ? "#dfeee0" : "#79aee0";
    ctx.fill();
    ctx.strokeStyle = index === 0 ? "#7bd88f" : "#101210";
    ctx.lineWidth = index === 0 ? 3 : 2;
    ctx.stroke();

    if (node.actuator > 0) {
      ctx.fillStyle = "#e2bd67";
      ctx.beginPath();
      ctx.arc(sizeX * 0.22, 0, Math.max(3, sizeY * 0.16), 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.restore();
  }

  ctx.restore();
}

function roundRect(context, x, y, width, height, radius) {
  context.beginPath();
  context.moveTo(x + radius, y);
  context.lineTo(x + width - radius, y);
  context.quadraticCurveTo(x + width, y, x + width, y + radius);
  context.lineTo(x + width, y + height - radius);
  context.quadraticCurveTo(x + width, y + height, x + width - radius, y + height);
  context.lineTo(x + radius, y + height);
  context.quadraticCurveTo(x, y + height, x, y + height - radius);
  context.lineTo(x, y + radius);
  context.quadraticCurveTo(x, y, x + radius, y);
  context.closePath();
}

function terrainHeight(task, x) {
  if (task === "flat") {
    return 0;
  }
  return Math.sin(x * 1.7) * 0.12 + Math.cos(x * 0.47) * 0.08;
}

function worldToScreen(point, camera) {
  return {
    x: (point[0] - camera.x) * camera.scale,
    y: canvas.clientHeight - (point[1] - camera.y) * camera.scale,
  };
}

function updateMetrics() {
  if (!state.replay) {
    return;
  }
  const frame = state.replay.frames[state.frameIndex];
  controls.time.textContent = `${frame.time.toFixed(2)}s`;
  controls.distance.textContent = frame.root[0].toFixed(2);
  controls.tilt.textContent = frame.tilt.toFixed(2);
  controls.scrubber.value = String(state.frameIndex);
  controls.frameValue.textContent = `${state.frameIndex}/${state.replay.frames.length - 1}`;
  controls.speedValue.textContent = `${Number(controls.speed.value).toFixed(2)}x`;
}

function tick(now) {
  const delta = Math.min(100, now - state.lastTick);
  state.lastTick = now;

  if (state.playing && state.replay) {
    state.accumulator += delta * Number(controls.speed.value);
    const frameMs = Math.max(10, state.replay.dt * 1000);
    while (state.accumulator >= frameMs) {
      state.accumulator -= frameMs;
      state.frameIndex = (state.frameIndex + 1) % state.replay.frames.length;
    }
    updateMetrics();
    draw();
  }

  requestAnimationFrame(tick);
}

controls.generate.addEventListener("click", requestReplay);

for (const control of [controls.controller, controls.task]) {
  control.addEventListener("change", requestReplayIfChanged);
}

for (const control of [controls.seed, controls.frames]) {
  control.addEventListener("change", requestReplayIfChanged);
  control.addEventListener("blur", requestReplayIfChanged);
  control.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      requestReplayIfChanged();
    }
  });
}

controls.play.addEventListener("click", () => {
  state.playing = !state.playing;
  controls.play.textContent = state.playing ? "Pause" : "Play";
});

controls.restart.addEventListener("click", () => {
  state.frameIndex = 0;
  state.accumulator = 0;
  state.playing = true;
  controls.play.textContent = "Pause";
  updateMetrics();
  draw();
});

controls.speed.addEventListener("input", updateMetrics);

controls.scrubber.addEventListener("input", () => {
  state.playing = false;
  controls.play.textContent = "Play";
  state.frameIndex = Number(controls.scrubber.value);
  updateMetrics();
  draw();
});

window.render_game_to_text = () =>
  JSON.stringify({
    ready: Boolean(state.replay),
    frame: state.frameIndex,
    selectedController: controls.controller.value,
    selectedTask: controls.task.value,
    controller: state.replay ? state.replay.controller : null,
    task: state.replay ? state.replay.task : null,
    source: state.replay ? state.replay.source : null,
    frames: state.replay ? state.replay.frames.length : 0,
    bodies: state.replay ? state.replay.body.length : 0,
  });

window.addEventListener("resize", resizeCanvas);
resizeCanvas();
requestReplay();
requestAnimationFrame(tick);
