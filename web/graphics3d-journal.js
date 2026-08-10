// Field-journal DOM rendering: narrative panels, the MAP-Elites archive
// grid, generation timeline, and the evolution progress charts.

import {
  clamp,
  controllerBehaviorNote,
  labelFor,
  replayRequest,
  state,
  ui,
} from "./graphics3d-state.js";

export function replayRunLabel(replay) {
  const generationLabel =
    replay.source === "evolved" ? `gen ${replay.generations}` : replay.source || "minimal";
  const cell = replay.archive?.selected_cell;
  const cellLabel = cell ? ` / cell ${cell[0]},${cell[1]}` : "";
  return `${labelFor(replay.controller)} / ${labelFor(replay.task)} / ${generationLabel}${cellLabel} / seed ${replay.seed}`;
}

export function replayFieldNote(replay) {
  const bodyText = `${replay.body.length} body parts and ${Math.max(0, replay.body.length - 1)} joints`;
  if (replay.source !== "evolved") {
    return `${bodyText}, replaying a seed genome across ${labelFor(replay.task)} terrain.`;
  }
  const cell = replay.archive?.selected_cell;
  const selection = cell ? ` Archive cell ${cell[0]},${cell[1]}.` : "";
  return `Evolved ${replay.population} candidates over ${replay.generations} generations.${selection} Distance ${formatMetric(replay.best_distance, 2)}, stable distance ${formatMetric(replay.stable_distance, 2)}, stability ${formatMetric(replay.stability, 2)}; staged with follow-camera framing.`;
}

export function setLoadingState(status) {
  const visible = status !== "ready";
  ui.loadingState.classList.toggle("is-visible", visible);
  ui.loadingState.classList.toggle("is-error", status === "error");
  ui.reroll.disabled = status === "loading";
}

export function renderFieldJournal(replay) {
  const history = replay.evolution_history || [];
  const first = history[0];
  const last = history.at(-1);
  const fitnessGain = last && first
    ? last.best_fitness - first.best_fitness
    : 0;
  const averageScale = replay.body.length
    ? replay.body.reduce((sum, node) => sum + node.size[0] + node.size[1], 0) / (replay.body.length * 2)
    : 0;
  const terrain = labelFor(replay.task);
  const controller = labelFor(replay.controller);
  const selectedLineage = replay.lineage?.at(-1);
  const lineageDepth = replay.lineage?.length || 0;

  ui.generationTitle.textContent = `Generation ${replay.generations}`;
  ui.generationTerrain.textContent = terrain === "Rough" ? "Rough terrain" : terrain;
  ui.currentGenerationLabel.textContent = `GEN ${replay.generations} (CURRENT)`;
  ui.previousGenerationLabel.textContent = `GEN ${Math.max(1, replay.generations - 1)} (PREVIOUS)`;
  ui.changeNarrative.textContent = selectedLineage
    ? `Genome #${selectedLineage.genome_id} descends through ${lineageDepth} recorded ${lineageDepth === 1 ? "generation" : "generations"}. Its latest inherited change ${selectedLineage.mutation_summary}, while preserving ${formatMetric(replay.stability, 2)} stability on ${terrain.toLowerCase()} terrain.`
    : `Selection favored forward travel without surrendering stability on ${terrain.toLowerCase()} terrain. The current champion balances ${replay.body.length} body segments through a ${controller} controller, converting a stable distance of ${formatMetric(replay.stable_distance, 2)} into repeatable progress.`;
  ui.deltaStride.textContent = `${fitnessGain >= 0 ? "+" : ""}${formatMetric(fitnessGain, 2)} total fitness gain`;
  ui.deltaStrideNote.textContent = first
    ? `Improved from ${formatMetric(first.best_fitness, 2)} to ${formatMetric(last.best_fitness, 2)} across the active search.`
    : "Establishing the first comparable champion.";
  ui.deltaGeometry.textContent = `${replay.body.length}-part morphology selected`;
  ui.deltaGeometryNote.textContent = selectedLineage
    ? `Recorded mutation: ${selectedLineage.mutation_summary}. Mean body scale ${formatMetric(averageScale, 2)}.`
    : `Mean body scale ${formatMetric(averageScale, 2)} with ${Math.max(0, replay.body.length - 1)} articulated joints.`;
  ui.deltaControl.textContent = `${controller} stability ${formatMetric(replay.stability, 2)}`;
  ui.deltaControlNote.textContent = controllerBehaviorNote(replay.controller);
}

export function renderArchiveLab(replay, fetchReplay) {
  const archive = replay.archive;
  if (!archive || !archive.cells?.length) {
    ui.archiveCaption.textContent = "No occupied cells";
    ui.archiveGrid.replaceChildren();
    ui.archiveDetail.textContent = "This replay does not include a MAP-Elites archive.";
    return;
  }

  const byCell = new Map(archive.cells.map((elite) => [`${elite.cell[0]}:${elite.cell[1]}`, elite]));
  const fitnesses = archive.cells.map((elite) => elite.fitness);
  const minFitness = Math.min(...fitnesses);
  const maxFitness = Math.max(...fitnesses);
  const fitnessRange = Math.max(0.001, maxFitness - minFitness);
  const selectedKey = archive.selected_cell.join(":");
  const fragment = document.createDocumentFragment();

  ui.archiveGrid.style.setProperty("--archive-width", String(archive.width));
  for (let y = archive.height - 1; y >= 0; y -= 1) {
    for (let x = 0; x < archive.width; x += 1) {
      const key = `${x}:${y}`;
      const elite = byCell.get(key);
      if (!elite) {
        const empty = document.createElement("span");
        empty.className = "archive-cell is-empty";
        empty.setAttribute("role", "presentation");
        fragment.appendChild(empty);
        continue;
      }

      const intensity = (elite.fitness - minFitness) / fitnessRange;
      const button = document.createElement("button");
      button.type = "button";
      button.className = "archive-cell is-occupied";
      button.style.setProperty("--cell-intensity", intensity.toFixed(3));
      button.dataset.cell = key;
      button.setAttribute("role", "gridcell");
      button.setAttribute("aria-label", `Replay genome ${elite.genome_id} from archive cell ${x}, ${y}; born generation ${elite.generation}; fitness ${formatMetric(elite.fitness, 2)}; stable distance ${formatMetric(elite.stable_distance, 2)}; ${Math.round(elite.body_count)} body parts`);
      button.setAttribute("aria-pressed", String(key === selectedKey));
      button.classList.toggle("is-selected", key === selectedKey);
      button.addEventListener("click", () => {
        if (key === archive.selected_cell.join(":")) {
          return;
        }
        replayRequest.cell_x = x;
        replayRequest.cell_y = y;
        fetchReplay().catch((error) => {
          ui.fieldNote.textContent = "Archive elite failed to load.";
          console.error(error);
        });
      });
      fragment.appendChild(button);
    }
  }

  ui.archiveGrid.replaceChildren(fragment);
  ui.archiveCaption.textContent = `${archive.cells.length} / ${archive.width * archive.height} occupied`;
  ui.archiveXAxis.textContent = `${labelFor(archive.x_axis)} →`;
  ui.archiveYAxis.textContent = `${labelFor(archive.y_axis)} ↑`;
  const selected = byCell.get(selectedKey);
  ui.archiveDetail.textContent = selected
    ? `Genome #${selected.genome_id} · born gen ${selected.generation} · cell ${selected.cell[0]},${selected.cell[1]} · fitness ${formatMetric(selected.fitness, 2)} · ${selected.mutation_summary}`
    : "Select an occupied cell to replay its elite.";
}

export function renderGenerationTimeline(history, options = {}) {
  const { preserveButtons = false } = options;
  if (!history.length) {
    ui.generationRail.replaceChildren();
    drawTimelineChart(history);
    return;
  }

  if (!preserveButtons || ui.generationRail.childElementCount !== history.length) {
    const fragment = document.createDocumentFragment();
    for (let index = 0; index < history.length; index += 1) {
      const point = history[index];
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = String(point.generation);
      button.dataset.generationIndex = String(index);
      button.setAttribute("aria-label", `Inspect generation ${point.generation}`);
      button.addEventListener("click", () => selectGeneration(index));
      fragment.appendChild(button);
    }
    ui.generationRail.replaceChildren(fragment);
    ui.generationRail.style.setProperty("--generation-count", String(history.length));
  }

  const selected = clamp(
    state.selectedGenerationIndex ?? history.length - 1,
    0,
    history.length - 1,
  );
  state.selectedGenerationIndex = selected;
  const buttons = ui.generationRail.querySelectorAll("button");
  buttons.forEach((button, index) => {
    const isSelected = index === selected;
    button.classList.toggle("is-selected", isSelected);
    button.setAttribute("aria-pressed", String(isSelected));
  });
  updateSelectedGeneration(history[selected]);
  drawTimelineChart(history);
}

function selectGeneration(index) {
  const history = state.replay?.evolution_history || [];
  if (!history[index]) {
    return;
  }
  state.selectedGenerationIndex = index;
  renderGenerationTimeline(history, { preserveButtons: true });
}

function updateSelectedGeneration(point) {
  if (!point) {
    ui.selectedGeneration.textContent = "NO GENERATION";
    ui.selectedFitness.textContent = "Fitness --";
    ui.selectedCoverage.textContent = "Coverage --";
    return;
  }
  ui.selectedGeneration.textContent = `GEN ${point.generation}`;
  ui.selectedFitness.textContent = `Fitness ${formatMetric(point.best_fitness, 2)}`;
  ui.selectedCoverage.textContent = `Coverage ${Math.round(point.archive_coverage * 100)}%`;
}

function drawTimelineChart(history) {
  const { context, width, height } = prepareCanvas(ui.timelineChart);
  context.clearRect(0, 0, width, height);
  if (!history.length) {
    return;
  }

  const padding = { left: 8, right: 10, top: 10, bottom: 10 };
  const plotWidth = width - padding.left - padding.right;
  const plotHeight = height - padding.top - padding.bottom;
  const fitnessValues = history.map((point) => point.best_fitness);
  const minFitness = Math.min(...fitnessValues);
  const maxFitness = Math.max(...fitnessValues);
  const fitnessRange = Math.max(0.001, maxFitness - minFitness);
  const x = (index) => padding.left + (history.length === 1 ? plotWidth / 2 : (index / (history.length - 1)) * plotWidth);
  const fitnessY = (value) => padding.top + plotHeight * 0.43 - ((value - minFitness) / fitnessRange) * plotHeight * 0.31;
  const coverageY = (value) => padding.top + plotHeight - value * plotHeight * 0.78;

  context.strokeStyle = "rgba(204, 222, 196, .13)";
  context.lineWidth = 1;
  context.beginPath();
  context.moveTo(padding.left, height * 0.5);
  context.lineTo(width - padding.right, height * 0.5);
  context.stroke();
  drawEvolutionLine(context, history, x, (point) => fitnessY(point.best_fitness), "#f0b943", 1.8);
  drawEvolutionLine(context, history, x, (point) => coverageY(point.archive_coverage), "#77aee7", 1.5);

  const selected = clamp(state.selectedGenerationIndex ?? history.length - 1, 0, history.length - 1);
  const selectedX = x(selected);
  context.strokeStyle = "rgba(163, 232, 127, .72)";
  context.beginPath();
  context.moveTo(selectedX, 0);
  context.lineTo(selectedX, height);
  context.stroke();
  for (const [value, color] of [
    [fitnessY(history[selected].best_fitness), "#f0b943"],
    [coverageY(history[selected].archive_coverage), "#77aee7"],
  ]) {
    context.beginPath();
    context.arc(selectedX, value, 3.5, 0, Math.PI * 2);
    context.fillStyle = color;
    context.fill();
  }
}

function prepareCanvas(chart) {
  const context = chart.getContext("2d");
  const ratio = Math.min(window.devicePixelRatio || 1, 2);
  const width = Math.max(1, chart.clientWidth || Number(chart.getAttribute("width")) || 320);
  const height = Math.max(1, chart.clientHeight || Number(chart.getAttribute("height")) || 120);
  chart.width = Math.floor(width * ratio);
  chart.height = Math.floor(height * ratio);
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  return { context, width, height };
}

export function renderEvolutionProgress(history) {
  const { context, width, height } = prepareCanvas(ui.evolutionChart);
  context.clearRect(0, 0, width, height);

  if (!history.length) {
    ui.evolutionSummary.textContent = "Seed genome · no search history";
    return;
  }

  const padding = { left: 10, right: 10, top: 10, bottom: 12 };
  const plotWidth = width - padding.left - padding.right;
  const plotHeight = height - padding.top - padding.bottom;
  const fitnessValues = history.flatMap((point) => [point.best_fitness, point.mean_fitness]);
  const minFitness = Math.min(...fitnessValues);
  const maxFitness = Math.max(...fitnessValues);
  const fitnessRange = Math.max(0.001, maxFitness - minFitness);
  const x = (index) => padding.left + (history.length === 1 ? plotWidth / 2 : (index / (history.length - 1)) * plotWidth);
  const fitnessY = (value) => padding.top + plotHeight - ((value - minFitness) / fitnessRange) * plotHeight;
  const coverageY = (value) => padding.top + plotHeight - value * plotHeight;

  context.strokeStyle = "rgba(220, 239, 226, .15)";
  context.lineWidth = 1;
  for (let row = 0; row <= 3; row += 1) {
    const y = padding.top + (row / 3) * plotHeight;
    context.beginPath(); context.moveTo(padding.left, y); context.lineTo(width - padding.right, y); context.stroke();
  }
  drawEvolutionLine(context, history, x, (point) => fitnessY(point.best_fitness), "#f2b84b", 2.2);
  drawEvolutionLine(context, history, x, (point) => fitnessY(point.mean_fitness), "#8eea9f", 1.6);
  drawEvolutionLine(context, history, x, (point) => coverageY(point.archive_coverage), "#80b7ea", 1.4);

  const first = history[0];
  const last = history.at(-1);
  const gain = last.best_fitness - first.best_fitness;
  ui.evolutionSummary.textContent = `Gen ${last.generation} · fitness ${formatMetric(last.best_fitness, 2)} · ${gain >= 0 ? "+" : ""}${formatMetric(gain, 2)} gain · ${Math.round(last.archive_coverage * 100)}% coverage`;
}

function drawEvolutionLine(context, history, x, y, color, lineWidth) {
  context.beginPath();
  history.forEach((point, index) => {
    const px = x(index);
    const py = y(point);
    if (index === 0) context.moveTo(px, py); else context.lineTo(px, py);
  });
  context.strokeStyle = color;
  context.lineWidth = lineWidth;
  context.lineJoin = "round";
  context.lineCap = "round";
  context.stroke();
}

function formatMetric(value, digits) {
  return Number.isFinite(value) ? value.toFixed(digits) : "--";
}
