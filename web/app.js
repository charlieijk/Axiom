const canvas = document.getElementById("simulator");
const ctx = canvas.getContext("2d");
const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

const controls = {
  pack: document.getElementById("task-pack"),
  task: document.getElementById("task"),
  seed: document.getElementById("seed"),
  generations: document.getElementById("generations"),
  population: document.getElementById("population"),
  steps: document.getElementById("steps"),
  maxBodyParts: document.getElementById("max-body-parts"),
  maxActuators: document.getElementById("max-actuators"),
  archiveWidth: document.getElementById("archive-width"),
  archiveHeight: document.getElementById("archive-height"),
  xAxis: document.getElementById("x-axis"),
  yAxis: document.getElementById("y-axis"),
  runButton: document.getElementById("run-button"),
  checkpointList: document.getElementById("checkpoint-list"),
  checkpointMeta: document.getElementById("checkpoint-meta"),
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
  checkpoints: [],
  frameIndex: 0,
  playing: true,
  lastTick: performance.now(),
  accumulator: 0,
  pollTimer: null,
};

function readNumber(input, fallback) {
  if (!input) {
    return fallback;
  }
  const value = Number(input.value);
  return Number.isFinite(value) ? value : fallback;
}

function runConfig() {
  const taskPack = !controls.pack || controls.pack.value === "single-task" ? null : controls.pack.value;
  return {
    task: controls.task.value,
    task_pack: taskPack,
    seed: readNumber(controls.seed, 42),
    generations: readNumber(controls.generations, 16),
    population: readNumber(controls.population, 32),
    evaluation_steps: readNumber(controls.steps, 180),
    max_body_parts: readNumber(controls.maxBodyParts, 8),
    max_actuators: readNumber(controls.maxActuators, 6),
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
  controls.archiveMeta.textContent = `${state.archive.width} x ${state.archive.height} / ${labelFor(state.archive.x_axis)} by ${labelFor(state.archive.y_axis)}`;
  renderArchive();
  const best = bestOccupiedCell();
  if (best) {
    await selectCell(best);
  } else {
    resetSelectionDetails(
      "No occupied cells",
      "This archive loaded, but it does not contain an occupied cell yet.",
    );
    setStatus("Archive loaded without occupied cells", state.run ? state.run.progress || 1 : 1);
    draw();
  }
}

function renderArchive() {
  const archive = state.archive;
  controls.archiveGrid.innerHTML = "";
  controls.archiveGrid.style.gridTemplateColumns = `repeat(${archive.width}, minmax(0, 1fr))`;
  const occupied = archive.cells.filter((cell) => cell.occupied);
  const fitnessValues = occupied.map(fitnessValue);
  const minFitness = occupied.length > 0 ? Math.min(...fitnessValues) : 0;
  const maxFitness = occupied.length > 0 ? Math.max(...fitnessValues) : 0;

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
        maxFitness <= minFitness ? 0.7 : (fitnessValue(cell) - minFitness) / (maxFitness - minFitness);
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
    .reduce(
      (best, cell) => (best === null || fitnessValue(cell) > fitnessValue(best) ? cell : best),
      null,
    );
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
  setReplayControlsEnabled(true);
  controls.play.textContent = "Pause";
  controls.scrubber.max = Math.max(0, state.replay.frames.length - 1);
  setStatus("Ready", state.run.progress || 1);
  updateMetrics();
  draw();
}

async function refreshCheckpoints() {
  try {
    const response = await apiJson("/api/checkpoints");
    const files = Array.isArray(response.files)
      ? [...response.files].sort(compareCheckpointsNewestFirst)
      : [];
    state.checkpoints = files;
    renderCheckpointOptions(files, response.directory);
    return files;
  } catch (error) {
    state.checkpoints = [];
    controls.checkpointMeta.textContent = "Checkpoint list unavailable.";
    controls.loadCheckpoint.disabled = true;
    console.error(error);
    return [];
  }
}

function renderCheckpointOptions(files, directory) {
  controls.checkpointList.innerHTML = "";
  if (files.length === 0) {
    const option = document.createElement("option");
    option.value = "";
    option.textContent = "No checkpoints found";
    controls.checkpointList.appendChild(option);
    controls.checkpointMeta.textContent = `No checkpoints in ${directory || "checkpoint directory"}.`;
    controls.loadCheckpoint.disabled = true;
    return;
  }
  files.forEach((file, index) => {
    const option = document.createElement("option");
    option.value = file.name;
    option.textContent = checkpointLabel(file, index);
    option.title = checkpointSummaryText(file);
    controls.checkpointList.appendChild(option);
  });
  controls.checkpointList.value = files[0].name;
  updateCheckpointMeta();
  controls.loadCheckpoint.disabled = false;
}

async function loadSelectedCheckpoint() {
  await loadCheckpointByName(controls.checkpointList.value, "Loading checkpoint");
}

async function loadCheckpointByName(name, loadingText) {
  if (!name) {
    return null;
  }
  clearTimeout(state.pollTimer);
  setStatus(loadingText, 0);
  controls.loadCheckpoint.disabled = true;
  try {
    const run = await apiJson(`/api/checkpoints/${encodeURIComponent(name)}`, { method: "POST" });
    state.run = run;
    updateRunSummary(run);
    await loadArchive(run.id);
    return run;
  } catch (error) {
    setStatus("Checkpoint load failed", 0);
    console.error(error);
    return null;
  } finally {
    controls.loadCheckpoint.disabled = !controls.checkpointList.value;
  }
}

function clearArchive() {
  state.archive = null;
  controls.archiveGrid.innerHTML = "";
  controls.archiveGrid.style.gridTemplateColumns = "";
  controls.archiveMeta.textContent = "Evolving archive";
  resetSelectionDetails(
    "Waiting for occupied cells",
    "Evolution is running. A checkpoint will appear when it completes.",
  );
  draw();
}

function showEmptyCheckpointState() {
  clearTimeout(state.pollTimer);
  state.run = null;
  state.archive = null;
  controls.runButton.disabled = false;
  controls.archiveGrid.innerHTML = "";
  controls.archiveGrid.style.gridTemplateColumns = "";
  controls.archiveMeta.textContent = "No saved checkpoints yet";
  resetSelectionDetails("No archive loaded", "Evolve an archive to create a local checkpoint.");
  setStatus("No local checkpoints", 0);
  draw();
}

function resetSelectionDetails(selectedCellText, mutationText) {
  state.selected = null;
  state.replay = null;
  state.frameIndex = 0;
  state.accumulator = 0;
  state.playing = false;
  controls.selectedCell.textContent = selectedCellText;
  controls.eliteFitness.textContent = "--";
  controls.eliteDistance.textContent = "--";
  controls.eliteStability.textContent = "--";
  controls.eliteBody.textContent = "--";
  controls.eliteGenome.textContent = "--";
  controls.eliteGeneration.textContent = "--";
  controls.eliteMutation.textContent = mutationText;
  controls.lineageList.innerHTML = "";
  controls.view3d.href = "/3d";
  controls.view3d.classList.add("disabled");
  controls.view3d.setAttribute("aria-disabled", "true");
  controls.time.textContent = "0.00s";
  controls.distance.textContent = "0.00";
  controls.tilt.textContent = "0.00";
  controls.scrubber.max = "1";
  controls.scrubber.value = "0";
  controls.frameValue.textContent = "0";
  controls.play.textContent = "Play";
  controls.speedValue.textContent = `${Number(controls.speed.value).toFixed(2)}x`;
  setReplayControlsEnabled(false);
}

function setReplayControlsEnabled(enabled) {
  controls.play.disabled = !enabled;
  controls.restart.disabled = !enabled;
  controls.speed.disabled = !enabled;
  controls.scrubber.disabled = !enabled;
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
    ctx.fillText("Load or evolve an archive.", 28, 150);
    ctx.fillStyle = "rgba(237, 243, 232, 0.48)";
    ctx.font = "700 13px Inter, system-ui, sans-serif";
    ctx.fillText("Select a cell to replay.", 28, 174);
    return;
  }

  const frame = state.replay.frames[state.frameIndex];
  const floor = terrainHeight(state.replay.task, frame.root[0]);
  const scale = Math.min(width / 11.5, height / 4.8);
  const camera = {
    x: frame.root[0] - 3.6,
    y: floor - 2.1,
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

  drawBioMechTrail(camera);
  drawBioMechShadow(frame, body, camera);

  for (let index = 0; index < frame.joints.length; index += 1) {
    drawBioMechJoint(frame.joints[index], camera, index, frame.time);
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

    drawBioMechBody(node, index, screen, angle, sizeX, sizeY, frame.time);
  }

  ctx.restore();
}

function drawBioMechTrail(camera) {
  if (!state.replay || !state.replay.frames.length) {
    return;
  }

  const trailLength = prefersReducedMotion ? 3 : 11;
  const stride = prefersReducedMotion ? 4 : 2;
  for (let trail = trailLength; trail >= 1; trail -= 1) {
    const frame = state.replay.frames[state.frameIndex - trail * stride];
    if (!frame) {
      continue;
    }
    const alpha = (1 - trail / (trailLength + 1)) * (prefersReducedMotion ? 0.13 : 0.26);
    ctx.save();
    ctx.globalAlpha = alpha;
    ctx.strokeStyle = "rgba(107, 164, 216, 0.72)";
    ctx.fillStyle = "rgba(123, 216, 143, 0.42)";
    ctx.lineWidth = 1.5;
    for (let index = 0; index < frame.bodies.length; index += 1) {
      const point = worldToScreen(frame.bodies[index], camera);
      const radius = index === 0 ? 10 : 5;
      ctx.beginPath();
      ctx.arc(point.x, point.y, radius, 0, Math.PI * 2);
      ctx.fill();
      if (index > 0) {
        ctx.stroke();
      }
    }
    ctx.restore();
  }
}

function drawBioMechShadow(frame, body, camera) {
  const floor = terrainHeight(state.replay.task, frame.root[0]);
  const floorPoint = worldToScreen([frame.root[0], floor + 0.04], camera);
  const spread = Math.max(
    46,
    body.reduce((total, node) => total + node.size[0], 0) * camera.scale * 0.42,
  );

  ctx.save();
  const gradient = ctx.createRadialGradient(
    floorPoint.x,
    floorPoint.y,
    2,
    floorPoint.x,
    floorPoint.y,
    spread,
  );
  gradient.addColorStop(0, "rgba(5, 8, 6, 0.34)");
  gradient.addColorStop(1, "rgba(5, 8, 6, 0)");
  ctx.fillStyle = gradient;
  ctx.beginPath();
  ctx.ellipse(floorPoint.x, floorPoint.y + 8, spread, 13, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}

function drawBioMechJoint(joint, camera, index, time) {
  const start = worldToScreen(joint[0], camera);
  const end = worldToScreen(joint[1], camera);
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const length = Math.hypot(dx, dy);
  if (length < 1) {
    return;
  }

  const angle = Math.atan2(dy, dx);
  const midX = (start.x + end.x) / 2;
  const midY = (start.y + end.y) / 2;
  const pulse = 0.65 + Math.sin(time * 8 + index * 0.7) * 0.18;
  ctx.save();
  ctx.translate(midX, midY);
  ctx.rotate(angle);
  ctx.strokeStyle = "rgba(9, 13, 10, 0.82)";
  ctx.lineWidth = 11;
  ctx.beginPath();
  ctx.moveTo(-length / 2, 0);
  ctx.lineTo(length / 2, 0);
  ctx.stroke();
  ctx.strokeStyle = "rgba(226, 189, 103, 0.78)";
  ctx.lineWidth = 5;
  ctx.beginPath();
  ctx.moveTo(-length / 2 + 8, 0);
  ctx.lineTo(length / 2 - 8, 0);
  ctx.stroke();
  ctx.strokeStyle = `rgba(123, 216, 143, ${0.18 + pulse * 0.16})`;
  ctx.lineWidth = 1.5;
  for (let mark = -1; mark <= 1; mark += 1) {
    const x = mark * length * 0.18;
    ctx.beginPath();
    ctx.moveTo(x, -5);
    ctx.lineTo(x + 5, 5);
    ctx.stroke();
  }
  ctx.restore();

  drawBioMechJointRing(start, pulse);
  drawBioMechJointRing(end, pulse * 0.9);
}

function drawBioMechJointRing(point, pulse) {
  ctx.save();
  ctx.translate(point.x, point.y);
  ctx.strokeStyle = "rgba(226, 189, 103, 0.88)";
  ctx.lineWidth = 2.4;
  ctx.beginPath();
  ctx.arc(0, 0, 7 + pulse, 0, Math.PI * 2);
  ctx.stroke();
  ctx.fillStyle = "rgba(11, 14, 12, 0.92)";
  ctx.beginPath();
  ctx.arc(0, 0, 3.3, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = "rgba(123, 216, 143, 0.64)";
  ctx.beginPath();
  ctx.arc(0, 0, 1.4 + pulse * 0.22, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}

function drawBioMechBody(node, index, screen, angle, sizeX, sizeY, time) {
  const root = index === 0;
  const radius = Math.min(root ? 18 : 13, sizeY * 0.5);
  const pulse = 0.72 + Math.sin(time * 8.5 + index * 0.85) * 0.2;
  ctx.save();
  ctx.translate(screen.x, screen.y);
  ctx.rotate(-angle);

  ctx.fillStyle = "rgba(5, 8, 6, 0.24)";
  roundRect(ctx, -sizeX / 2 + 3, -sizeY / 2 + 4, sizeX, sizeY, radius);
  ctx.fill();

  const shell = ctx.createLinearGradient(-sizeX / 2, -sizeY / 2, sizeX / 2, sizeY / 2);
  if (root) {
    shell.addColorStop(0, "#f4faef");
    shell.addColorStop(0.55, "#c7dfc8");
    shell.addColorStop(1, "#88b794");
  } else {
    shell.addColorStop(0, "#9cc9ef");
    shell.addColorStop(0.58, "#5f94c5");
    shell.addColorStop(1, "#28485c");
  }
  roundRect(ctx, -sizeX / 2, -sizeY / 2, sizeX, sizeY, radius);
  ctx.fillStyle = shell;
  ctx.fill();
  ctx.strokeStyle = root ? "rgba(123, 216, 143, 0.96)" : "rgba(14, 18, 15, 0.92)";
  ctx.lineWidth = root ? 3 : 2;
  ctx.stroke();

  ctx.strokeStyle = root ? "rgba(28, 67, 42, 0.42)" : "rgba(218, 238, 247, 0.34)";
  ctx.lineWidth = Math.max(1, Math.min(3, sizeY * 0.08));
  roundRect(
    ctx,
    -sizeX * 0.34,
    -sizeY * 0.28,
    sizeX * 0.68,
    sizeY * 0.56,
    Math.max(5, radius * 0.55),
  );
  ctx.stroke();

  ctx.strokeStyle = root ? "rgba(16, 37, 24, 0.42)" : "rgba(10, 16, 16, 0.45)";
  ctx.lineWidth = 1;
  for (let slot = -1; slot <= 1; slot += 1) {
    const x = slot * sizeX * 0.2;
    ctx.beginPath();
    ctx.moveTo(x, -sizeY * 0.28);
    ctx.lineTo(x + sizeX * 0.05, sizeY * 0.28);
    ctx.stroke();
  }

  drawBioMechSensors(sizeX, sizeY, root, pulse);
  if (node.actuator > 0.01) {
    drawBioMechActuator(sizeX, sizeY, pulse);
  }

  ctx.restore();
}

function drawBioMechSensors(sizeX, sizeY, root, pulse) {
  const count = root ? 3 : 2;
  const start = -(count - 1) * 0.5;
  for (let index = 0; index < count; index += 1) {
    const x = (start + index) * sizeX * 0.18;
    const y = -sizeY * 0.2;
    ctx.fillStyle = `rgba(123, 216, 143, ${0.52 + pulse * 0.32})`;
    ctx.beginPath();
    ctx.arc(x, y, Math.max(2.2, sizeY * 0.07), 0, Math.PI * 2);
    ctx.fill();
    ctx.strokeStyle = "rgba(10, 18, 12, 0.78)";
    ctx.lineWidth = 1;
    ctx.stroke();
  }
}

function drawBioMechActuator(sizeX, sizeY, pulse) {
  const podX = sizeX * 0.32;
  const radius = Math.max(4, sizeY * 0.18);
  ctx.fillStyle = "rgba(12, 22, 16, 0.88)";
  ctx.beginPath();
  ctx.arc(podX, 0, radius * 1.35, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = `rgba(123, 216, 143, ${0.62 + pulse * 0.22})`;
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.arc(podX, 0, radius * (1.1 + pulse * 0.15), 0, Math.PI * 2);
  ctx.stroke();
  ctx.fillStyle = "rgba(226, 189, 103, 0.92)";
  ctx.beginPath();
  ctx.arc(podX, 0, radius * 0.46, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = "rgba(123, 216, 143, 0.72)";
  ctx.lineWidth = 1.4;
  for (let coil = -1; coil <= 1; coil += 1) {
    const x = podX - radius * 1.15 + coil * radius * 0.78;
    ctx.beginPath();
    ctx.moveTo(x, -radius * 1.05);
    ctx.lineTo(x + radius * 0.42, radius * 1.05);
    ctx.stroke();
  }
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
  if (task === "steps") {
    return Math.floor(x / 1.5) * 0.05;
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

function fitnessValue(cell) {
  const value = Number(cell.fitness);
  return Number.isFinite(value) ? value : Number.NEGATIVE_INFINITY;
}

function compareCheckpointsNewestFirst(a, b) {
  const aTime = checkpointTimestampValue(a);
  const bTime = checkpointTimestampValue(b);
  if (aTime !== bTime) {
    return bTime > aTime ? 1 : -1;
  }
  return String(a.name || "").localeCompare(String(b.name || ""));
}

function checkpointTimestampValue(file) {
  const value = Number(file?.timestamp_unix_ms ?? file?.saved_unix_ms ?? file?.modified_unix_ms);
  return Number.isFinite(value) ? value : Number.NEGATIVE_INFINITY;
}

function checkpointLabel(file, index) {
  const bytes = Number(file.bytes);
  const sizeText = Number.isFinite(bytes) ? `${Math.max(1, Math.round(bytes / 1024))} KB` : "unknown size";
  const task = file.task ? labelFor(file.task) : "Unknown task";
  const timestamp = checkpointTimestampValue(file);
  const age = index === 0 ? "Latest" : checkpointAgeText(timestamp);
  return [
    age,
    formatTimestamp(timestamp, "short"),
    task,
    checkpointDimensionsText(file),
    checkpointOccupancyText(file, "compact"),
    `${file.name} (${sizeText})`,
  ].join(" - ");
}

function selectedCheckpointFile() {
  const name = controls.checkpointList.value;
  return state.checkpoints.find((file) => file.name === name) || null;
}

function updateCheckpointMeta() {
  const file = selectedCheckpointFile();
  controls.checkpointMeta.title = checkpointSummaryText(file);
  controls.checkpointMeta.replaceChildren(
    ...checkpointSummaryParts(file).map((part) => {
      const token = document.createElement("span");
      token.className = "checkpoint-token";
      token.textContent = part;
      return token;
    }),
  );
}

function checkpointSummaryText(file) {
  if (!file) {
    return "No checkpoint selected.";
  }
  const timestamp = formatTimestamp(checkpointTimestampValue(file), "long");
  if (file.metadata_error) {
    return `${file.name} / metadata unavailable / ${timestamp}`;
  }
  const task = file.task ? `${labelFor(file.task)} task` : "Unknown task";
  return `${task} / ${checkpointAxesText(file)} / ${checkpointOccupancyText(file)} / ${timestamp}`;
}

function checkpointSummaryParts(file) {
  if (!file) {
    return ["No checkpoint selected"];
  }
  const timestamp = formatTimestamp(checkpointTimestampValue(file), "long");
  if (file.metadata_error) {
    return [file.name, "metadata unavailable", timestamp].filter(Boolean);
  }
  return [
    file.task ? `${labelFor(file.task)} task` : "Unknown task",
    `${checkpointDimensionsText(file)} archive`,
    checkpointOccupancyText(file),
    timestamp,
    file.name,
  ];
}

function checkpointAxesText(file) {
  const axes =
    file.archive_x_axis && file.archive_y_axis
      ? `${labelFor(file.archive_x_axis)} by ${labelFor(file.archive_y_axis)}`
      : "unknown axes";
  return `${checkpointDimensionsText(file)} archive, ${axes}`;
}

function checkpointDimensionsText(file) {
  return Number.isFinite(file.archive_width) && Number.isFinite(file.archive_height)
    ? `${file.archive_width} x ${file.archive_height}`
    : "unknown size";
}

function checkpointOccupancyText(file, mode = "full") {
  const occupied = Number.isFinite(file.occupied_cells) ? Number(file.occupied_cells) : null;
  const total =
    Number.isFinite(file.archive_width) && Number.isFinite(file.archive_height)
      ? Number(file.archive_width) * Number(file.archive_height)
      : null;
  const coverage = Number.isFinite(file.coverage) ? `${Math.round(Number(file.coverage) * 100)}%` : null;
  if (occupied !== null && total !== null && coverage) {
    return mode === "compact"
      ? `${occupied}/${total} cells, ${coverage}`
      : `${occupied}/${total} cells (${coverage} coverage)`;
  }
  if (occupied !== null && coverage) {
    return mode === "compact" ? `${occupied} cells, ${coverage}` : `${occupied} cells (${coverage} coverage)`;
  }
  if (occupied !== null) {
    return `${occupied} cells`;
  }
  return coverage ? `${coverage} coverage` : "unknown coverage";
}

function checkpointAgeText(value) {
  const timestamp = checkpointTimestampValue({ timestamp_unix_ms: value });
  if (!Number.isFinite(timestamp)) {
    return "Undated";
  }
  const elapsedMs = Date.now() - timestamp;
  const future = elapsedMs < 0;
  const absoluteMs = Math.abs(elapsedMs);
  const minute = 60 * 1000;
  const hour = 60 * minute;
  const day = 24 * hour;
  const month = 30 * day;
  const year = 365 * day;
  let label = "just now";
  if (absoluteMs >= year) {
    label = `${Math.floor(absoluteMs / year)}y`;
  } else if (absoluteMs >= month) {
    label = `${Math.floor(absoluteMs / month)}mo`;
  } else if (absoluteMs >= 2 * day) {
    label = `${Math.floor(absoluteMs / day)}d`;
  } else if (absoluteMs >= day) {
    label = "1d";
  } else if (absoluteMs >= hour) {
    label = `${Math.floor(absoluteMs / hour)}h`;
  } else if (absoluteMs >= minute) {
    label = `${Math.floor(absoluteMs / minute)}m`;
  }
  if (label === "just now") {
    return label;
  }
  return future ? `in ${label}` : `${label} ago`;
}

function formatTimestamp(value, length = "long") {
  if (!Number.isFinite(value)) {
    return "unknown time";
  }
  const date = new Date(Number(value));
  if (Number.isNaN(date.getTime())) {
    return "unknown time";
  }
  if (length === "short") {
    return new Intl.DateTimeFormat(undefined, {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
    }).format(date);
  }
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function labelFor(value) {
  return String(value || "")
    .split(/[-_]/)
    .filter(Boolean)
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join(" ");
}

controls.runButton.addEventListener("click", startRun);
controls.checkpointList.addEventListener("change", updateCheckpointMeta);
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

async function bootArchiveBrowser() {
  resizeCanvas();
  const files = await refreshCheckpoints();
  if (files.length === 0) {
    showEmptyCheckpointState();
  } else {
    const latest = files[0];
    controls.checkpointList.value = latest.name;
    const loaded = await loadCheckpointByName(latest.name, "Loading latest checkpoint");
    if (!loaded) {
      resetSelectionDetails(
        "No archive loaded",
        "Select another checkpoint or evolve a new archive.",
      );
      controls.archiveMeta.textContent = "Latest checkpoint failed to load";
      draw();
    }
  }
  requestAnimationFrame(tick);
}

bootArchiveBrowser();
