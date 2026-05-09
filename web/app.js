const canvas = document.getElementById("simulator");
const ctx = canvas.getContext("2d");

const controls = {
  task: document.getElementById("task"),
  seed: document.getElementById("seed"),
  generations: document.getElementById("generations"),
  population: document.getElementById("population"),
  steps: document.getElementById("steps"),
  archiveWidth: document.getElementById("archive-width"),
  archiveHeight: document.getElementById("archive-height"),
  xAxis: document.getElementById("x-axis"),
  yAxis: document.getElementById("y-axis"),
  runButton: document.getElementById("run-button"),
  checkpointList: document.getElementById("checkpoint-list"),
  loadCheckpoint: document.getElementById("load-checkpoint"),
  archiveGrid: document.getElementById("archive-grid"),
  archiveMeta: document.getElementById("archive-meta"),
  status: document.getElementById("status"),
  progressLabel: document.getElementById("progress-label"),
  progressFill: document.getElementById("progress-fill"),
  selectedCell: document.getElementById("selected-cell"),
  view3d: document.getElementById("view-3d"),
  play: document.getElementById("play"),
  restart: document.getElementById("restart"),
  speed: document.getElementById("speed"),
  speedValue: document.getElementById("speed-value"),
  scrubber: document.getElementById("scrubber"),
  frameValue: document.getElementById("frame-value"),
  time: document.getElementById("metric-time"),
  distance: document.getElementById("metric-distance"),
  tilt: document.getElementById("metric-tilt"),
  eliteFitness: document.getElementById("elite-fitness"),
  eliteDistance: document.getElementById("elite-distance"),
  eliteStability: document.getElementById("elite-stability"),
  eliteBody: document.getElementById("elite-body"),
  eliteGenome: document.getElementById("elite-genome"),
  eliteGeneration: document.getElementById("elite-generation"),
  eliteMutation: document.getElementById("elite-mutation"),
  lineageList: document.getElementById("lineage-list"),
};

const state = {
  run: null,
  archive: null,
  selected: null,
  replay: null,
  frameIndex: 0,
  playing: true,
  lastTick: performance.now(),
  accumulator: 0,
  pollTimer: null,
};

function readNumber(input, fallback) {
  const value = Number(input.value);
  return Number.isFinite(value) ? value : fallback;
}

function runConfig() {
  return {
    task: controls.task.value,
    seed: readNumber(controls.seed, 42),
    generations: readNumber(controls.generations, 16),
    population: readNumber(controls.population, 32),
    evaluation_steps: readNumber(controls.steps, 180),
    archive_width: readNumber(controls.archiveWidth, 12),
    archive_height: readNumber(controls.archiveHeight, 8),
    x_axis: controls.xAxis.value,
    y_axis: controls.yAxis.value,
    search_mode: "map-elites",
  };
}

async function apiJson(path, options = {}) {
  const response = await fetch(path, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });
  const payload = await response.json();
  if (!response.ok) {
    throw new Error(payload.error || `Request failed: ${response.status}`);
  }
  return payload;
}

async function startRun() {
  clearTimeout(state.pollTimer);
  controls.runButton.disabled = true;
  setStatus("Starting", 0);
  clearArchive();
  try {
    const started = await apiJson("/api/runs", {
      method: "POST",
      body: JSON.stringify(runConfig()),
    });
    state.run = { id: started.id, status: started.status };
    pollRun(started.id);
  } catch (error) {
    controls.runButton.disabled = false;
    setStatus("Error", 0);
    console.error(error);
  }
}

async function pollRun(runId) {
  try {
    const run = await apiJson(`/api/runs/${encodeURIComponent(runId)}`);
    state.run = run;
    updateRunSummary(run);
    if (run.status === "completed") {
      controls.runButton.disabled = false;
      await loadArchive(run.id);
      await refreshCheckpoints();
      return;
    }
    if (run.status === "failed") {
      controls.runButton.disabled = false;
      setStatus(run.error || "Failed", 1);
      return;
    }
    state.pollTimer = setTimeout(() => pollRun(runId), 500);
  } catch (error) {
    controls.runButton.disabled = false;
    setStatus("Error", 0);
    console.error(error);
  }
}

function updateRunSummary(run) {
  const percent = Math.round(run.progress * 100);
  setStatus(
    `${run.status} / gen ${run.generation}/${run.total_generations} / ${run.occupied_cells} cells`,
    run.progress,
  );
  controls.progressLabel.textContent = `${percent}%`;
}

function setStatus(text, progress) {
  controls.status.textContent = text;
  controls.progressFill.style.width = `${Math.round(progress * 100)}%`;
  controls.progressLabel.textContent = `${Math.round(progress * 100)}%`;
}

async function loadArchive(runId) {
  state.archive = await apiJson(`/api/runs/${encodeURIComponent(runId)}/archive`);
  controls.archiveMeta.textContent = `${state.archive.width} x ${state.archive.height} / ${state.archive.x_axis} by ${state.archive.y_axis}`;
  renderArchive();
  const best = bestOccupiedCell();
  if (best) {
    selectCell(best);
  }
}

function renderArchive() {
  const archive = state.archive;
  controls.archiveGrid.innerHTML = "";
  controls.archiveGrid.style.gridTemplateColumns = `repeat(${archive.width}, minmax(0, 1fr))`;
  const occupied = archive.cells.filter((cell) => cell.occupied);
  const minFitness = Math.min(...occupied.map((cell) => cell.fitness || 0));
  const maxFitness = Math.max(...occupied.map((cell) => cell.fitness || 0));

  for (const cell of archive.cells) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "archive-cell";
    button.dataset.x = String(cell.x);
    button.dataset.y = String(cell.y);
    button.disabled = !cell.occupied;
    button.setAttribute("role", "gridcell");
    button.setAttribute("aria-label", cell.occupied ? `Cell ${cell.x},${cell.y}` : "Empty cell");

    if (cell.occupied) {
      const normalized =
        maxFitness <= minFitness ? 0.7 : ((cell.fitness || 0) - minFitness) / (maxFitness - minFitness);
      button.style.setProperty("--heat", String(normalized));
      button.textContent = formatCompact(cell.fitness);
      button.addEventListener("click", () => selectCell(cell));
    }

    controls.archiveGrid.appendChild(button);
  }
}

function bestOccupiedCell() {
  if (!state.archive) {
    return null;
  }
  return state.archive.cells
    .filter((cell) => cell.occupied)
    .sort((a, b) => (b.fitness || 0) - (a.fitness || 0))[0];
}

async function selectCell(cell) {
  if (!state.run || !cell.occupied) {
    return;
  }
  state.selected = cell;
  for (const element of controls.archiveGrid.querySelectorAll(".archive-cell")) {
    element.classList.toggle(
      "selected",
      Number(element.dataset.x) === cell.x && Number(element.dataset.y) === cell.y,
    );
  }
  updateEliteDetails(cell);
  await fetchReplayForCell(cell);
}

function updateEliteDetails(cell) {
  controls.selectedCell.textContent = `Cell ${cell.x}, ${cell.y}`;
  controls.eliteFitness.textContent = formatNumber(cell.fitness);
  controls.eliteDistance.textContent = formatNumber(cell.distance);
  controls.eliteStability.textContent = formatNumber(cell.stability);
  controls.eliteBody.textContent = `${formatCompact(cell.body_count)} / ${formatCompact(cell.actuator_count)}`;
  controls.eliteGenome.textContent = String(cell.genome_id || "--");
  controls.eliteGeneration.textContent = String(cell.generation ?? "--");
  controls.eliteMutation.textContent = cell.mutation_summary || "No mutation summary.";
  controls.lineageList.innerHTML = "";
  for (const item of lineageForCell(cell)) {
    const row = document.createElement("li");
    row.textContent = item;
    controls.lineageList.appendChild(row);
  }

  const url = `/3d?run=${encodeURIComponent(state.run.id)}&cell=${cell.x},${cell.y}`;
  controls.view3d.href = url;
  controls.view3d.classList.remove("disabled");
  controls.view3d.setAttribute("aria-disabled", "false");
}

function lineageForCell(cell) {
  if (Array.isArray(cell.lineage) && cell.lineage.length > 0) {
    return cell.lineage.map(
      (step) =>
        `Genome ${step.genome_id} / gen ${step.generation} / ${labelFor(step.controller)}`,
    );
  }
  return [`Genome ${cell.genome_id} / gen ${cell.generation}`];
}

async function fetchReplayForCell(cell) {
  controls.status.textContent = "Loading replay";
  const query = new URLSearchParams({
    run: state.run.id,
    cell: `${cell.x},${cell.y}`,
    frames: "260",
  });
  state.replay = await apiJson(`/api/replay?${query.toString()}`);
  state.frameIndex = 0;
  state.accumulator = 0;
  state.playing = true;
  controls.play.textContent = "Pause";
  controls.scrubber.max = Math.max(0, state.replay.frames.length - 1);
  setStatus("Ready", state.run.progress || 1);
  updateMetrics();
  draw();
}

async function refreshCheckpoints() {
  try {
    const response = await apiJson("/api/checkpoints");
    controls.checkpointList.innerHTML = "";
    if (response.files.length === 0) {
      const option = document.createElement("option");
      option.value = "";
      option.textContent = "No checkpoints";
      controls.checkpointList.appendChild(option);
      return;
    }
    for (const file of response.files) {
      const option = document.createElement("option");
      option.value = file.name;
      option.textContent = `${file.name} (${Math.round(file.bytes / 1024)} KB)`;
      controls.checkpointList.appendChild(option);
    }
  } catch (error) {
    console.error(error);
  }
}

async function loadSelectedCheckpoint() {
  const name = controls.checkpointList.value;
  if (!name) {
    return;
  }
  clearTimeout(state.pollTimer);
  setStatus("Loading checkpoint", 0);
  try {
    const run = await apiJson(`/api/checkpoints/${encodeURIComponent(name)}`, { method: "POST" });
    state.run = run;
    updateRunSummary(run);
    await loadArchive(run.id);
  } catch (error) {
    setStatus("Error", 0);
    console.error(error);
  }
}

function clearArchive() {
  state.archive = null;
  state.selected = null;
  state.replay = null;
  controls.archiveGrid.innerHTML = "";
  controls.archiveMeta.textContent = "Run in progress";
  controls.selectedCell.textContent = "Select an occupied cell";
  controls.eliteFitness.textContent = "--";
  controls.eliteDistance.textContent = "--";
  controls.eliteStability.textContent = "--";
  controls.eliteBody.textContent = "--";
  controls.eliteGenome.textContent = "--";
  controls.eliteGeneration.textContent = "--";
  controls.eliteMutation.textContent = "No elite selected.";
  controls.lineageList.innerHTML = "";
  controls.view3d.href = "/3d";
  controls.view3d.classList.add("disabled");
  controls.view3d.setAttribute("aria-disabled", "true");
  draw();
}

function resizeCanvas() {
  const rect = canvas.getBoundingClientRect();
  const pixelRatio = window.devicePixelRatio || 1;
  canvas.width = Math.max(640, Math.floor(rect.width * pixelRatio));
  canvas.height = Math.max(420, Math.floor(rect.height * pixelRatio));
  ctx.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
  draw();
}

function draw() {
  const width = canvas.clientWidth;
  const height = canvas.clientHeight;
  ctx.clearRect(0, 0, width, height);
  drawBackground(width, height);

  if (!state.replay) {
    ctx.fillStyle = "rgba(237, 243, 232, 0.62)";
    ctx.font = "700 15px Inter, system-ui, sans-serif";
    ctx.fillText("Start a run or load a checkpoint, then select an occupied archive cell.", 28, 38);
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

function formatNumber(value) {
  return Number.isFinite(value) ? value.toFixed(2) : "--";
}

function formatCompact(value) {
  return Number.isFinite(value) ? Number(value).toFixed(0) : "";
}

function labelFor(value) {
  return String(value || "")
    .split(/[-_]/)
    .filter(Boolean)
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join(" ");
}

controls.runButton.addEventListener("click", startRun);
controls.loadCheckpoint.addEventListener("click", loadSelectedCheckpoint);
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
window.addEventListener("resize", resizeCanvas);

resizeCanvas();
refreshCheckpoints();
requestAnimationFrame(tick);
