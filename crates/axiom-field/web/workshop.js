import { DESIGN_FIELDS, replayIndex, parseWorkshop, designSummary, validateDesign, jobFraction } from './workshop-model.js';

const $ = (id) => document.getElementById(id);
const number = (value, digits = 3) => Number.isFinite(value) ? value.toFixed(digits) : '—';
function node(tag, text, className) {
  const element = document.createElement(tag);
  if (text !== undefined) element.textContent = text;
  if (className) element.className = className;
  return element;
}
function download(value, filename) {
  const url = URL.createObjectURL(new Blob([JSON.stringify(value, null, 2)], { type: 'application/json' }));
  const link = node('a'); link.href = url; link.download = filename; link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
export function setupWorkshop(app) {
  let workshop = null, selected = null, busy = false, pollTimer = null;
  let recording = null, replayTimer = null, playback = false, frameIndex = 0, replayMode = false;
  let designKey = '', terminalKey = '', workshopRead = 0;
  const message = (text) => { $('workshop-message').textContent = text; };
  function tab(name, focus = false) {
    for (const value of ['trial', 'build', 'evolve', 'replays']) {
      const active = value === name;
      $(`tab-${value}`).setAttribute('aria-selected', String(active));
      $(`tab-${value}`).tabIndex = active ? 0 : -1;
      $(`panel-${value}`).hidden = !active;
    }
    if (focus) $(`tab-${name}`).focus();
  }
  const tabs = ['trial', 'build', 'evolve', 'replays'];
  tabs.forEach((name, index) => {
    $(`tab-${name}`).addEventListener('click', () => tab(name));
    $(`tab-${name}`).addEventListener('keydown', (event) => {
      let target;
      if (event.key === 'ArrowRight') target = tabs[(index + 1) % tabs.length];
      if (event.key === 'ArrowLeft') target = tabs[(index + tabs.length - 1) % tabs.length];
      if (event.key === 'Home') target = tabs[0];
      if (event.key === 'End') target = tabs.at(-1);
      if (target) { event.preventDefault(); tab(target, true); }
    });
  });
  for (const [key, label, min, max, , unit] of DESIGN_FIELDS) {
    const field = node('label', undefined, 'design-field'); field.htmlFor = `design-${key}`;
    const heading = node('span', label); const output = node('output'); output.id = `design-${key}-value`;
    heading.append(output); field.append(heading);
    const input = node('input'); Object.assign(input, { type: 'range', min, max, step: 'any', id: `design-${key}`, name: key, value: min });
    input.addEventListener('input', () => { output.textContent = `${Number(input.value).toFixed(3)} ${unit}`; });
    field.append(input); $('design-fields').append(field);
  }
  function updateDesign(design) {
    const key = JSON.stringify(design);
    if (!design || key === designKey) return;
    designKey = key;
    for (const [name, , , , , unit] of DESIGN_FIELDS) {
      $(`design-${name}`).value = design[name];
      $(`design-${name}-value`).textContent = `${number(design[name])} ${unit}`;
    }
  }
  function locks() {
    const jobRunning = workshop?.job?.status === 'running';
    for (const form of ['design-form', 'search-form']) for (const input of $(form).elements) input.disabled = busy || replayMode || jobRunning;
    $('import-workshop').disabled = busy || replayMode || jobRunning;
    $('export-workshop').disabled = busy;
    $('refresh-recordings').disabled = busy;
    $('select-elite').disabled = busy || replayMode || jobRunning || !selected;
    $('load-bundled').disabled = busy || replayMode || jobRunning;
    $('holdout').disabled = busy || replayMode || jobRunning || !workshop?.archive?.cells?.length;
    $('cancel-job').hidden = !jobRunning;
    $('cancel-job').disabled = busy || replayMode;
    $('save-recording').disabled = busy || replayMode || !(app.snapshot()?.elapsed_s > 0);
    $('manual-controller').disabled = busy || replayMode || jobRunning;
    for (const button of $('recordings-list').querySelectorAll('button')) button.disabled = busy;
  }
  async function execute(operation, success) {
    if (busy) return;
    busy = true; message(''); locks();
    try { const result = await operation(); await success?.(result); }
    catch (error) { message(error.message); }
    finally { busy = false; locks(); }
  }
  function eliteDetail(elite) {
    selected = elite;
    $('elite-detail').replaceChildren();
    $('elite-detail').append(node('strong', `Cell ${elite.cell.join(', ')} · generation ${elite.generation}`));
    const details = [
      ['Fitness', number(elite.score.fitness)], ['Worst-world fitness', number(elite.score.worst_fitness)],
      ['Mean speed', `${number(elite.score.mean_speed_m_s)} m/s`], ['Mean effort / metre', number(elite.score.mean_effort_per_m)],
      ['Falls / worlds', `${elite.score.falls} / ${elite.score.worlds}`], ['CPG frequency', `${number(elite.genome.frequency_hz, 2)} Hz`],
    ];
    const list = node('dl');
    for (const [label, value] of details) { list.append(node('dt', label), node('dd', value)); }
    $('elite-detail').append(list, node('p', 'Selection uses the complete eight-joint genome, including each amplitude, phase and bias.', 'control-note'));
    const table = node('table', undefined, 'joint-table');
    const head = node('tr');
    for (const title of ['Joint', 'Amplitude', 'Phase', 'Bias']) head.append(node('th', title));
    table.append(head);
    const legs = ['Front R', 'Front L', 'Rear R', 'Rear L'];
    elite.genome.joints.forEach((joint, index) => {
      const row = node('tr');
      row.append(node('td', `${legs[Math.floor(index / 2)]} ${index % 2 ? 'knee' : 'hip'}`));
      for (const key of ['amplitude', 'phase', 'bias']) row.append(node('td', number(joint[key])));
      table.append(row);
    });
    $('elite-detail').append(table);
    for (const cell of $('archive-grid').children) cell.setAttribute('aria-pressed', String(cell.dataset.cell === elite.cell.join(',')));
    locks();
  }
  function renderArchive(archive) {
    $('archive-grid').replaceChildren(); selected = null;
    $('elite-detail').textContent = 'Choose an occupied cell to inspect its measured score.';
    if (!archive) { $('archive-summary').textContent = 'No archive yet. Run evolution to discover controllers.'; return; }
    $('archive-summary').textContent = `${archive.cells.length} occupied cells · ${archive.evaluated} evaluated · seed ${archive.seed} · ${archive.source === 'imported' ? 'Imported metrics; not re-verified locally' : archive.source === 'bundled' ? 'Bundled full-search metrics; not re-verified locally' : 'Locally evaluated'}`;
    const cells = new Map(archive.cells.map((elite) => [elite.cell.join(','), elite]));
    for (let y = archive.height - 1; y >= 0; y--) for (let x = 0; x < archive.width; x++) {
      const elite = cells.get(`${x},${y}`);
      const cell = node(elite ? 'button' : 'span', elite ? '•' : '');
      cell.className = elite ? 'archive-cell occupied' : 'archive-cell';
      if (elite) {
        cell.type = 'button'; cell.dataset.cell = `${x},${y}`; cell.setAttribute('aria-pressed', 'false');
        cell.setAttribute('aria-label', `Cell ${x}, ${y}: fitness ${number(elite.score.fitness)}, falls ${elite.score.falls}`);
        cell.title = `Cell ${x}, ${y} · fitness ${number(elite.score.fitness)}`;
        cell.addEventListener('click', () => eliteDetail(elite));
      } else cell.setAttribute('aria-hidden', 'true');
      $('archive-grid').append(cell);
    }
    if (archive.history?.length) {
      const last = archive.history.at(-1);
      $('archive-summary').append(document.createTextNode(` · generation ${last.generation}, best ${number(last.best_fitness)}`));
    }
  }
  let archiveKey = '';
  function renderWorkshop(next) {
    workshopRead++;
    workshop = next; updateDesign(next.design);
    const job = next.job;
    $('job-status').textContent = job.status === 'idle' ? 'No search running.' : `${job.kind}: ${job.status} · ${job.evaluated} / ${job.total}${job.message ? ` · ${job.message}` : ''}`;
    $('job-progress').value = jobFraction(job);
    const key = JSON.stringify(next.archive);
    if (key !== archiveKey) { archiveKey = key; renderArchive(next.archive); }
    if (next.holdout) $('holdout-result').textContent = `${next.holdout.robust} / ${next.holdout.tested} controllers stayed upright in every unseen world. Mean retained fitness: ${number(next.holdout.retained_fraction * 100, 1)}%. Software-only flat-ground test; no rail or hardware validation.`;
    else $('holdout-result').textContent = 'Holdout tests use flat ground with unseen parameter values. They do not validate rails or hardware.';
    clearTimeout(pollTimer);
    if (job.status === 'running') pollTimer = setTimeout(refreshWorkshop, 1000);
    locks();
  }
  async function refreshWorkshop() {
    const request = ++workshopRead;
    try { const next = await app.api('/api/workshop'); if (request === workshopRead) renderWorkshop(next); }
    catch (error) { message(`Workshop status unavailable: ${error.message}. Use Refresh status to retry.`); }
  }
  const refreshButton = node('button', 'Refresh status', 'text-button');
  refreshButton.type = 'button'; refreshButton.addEventListener('click', refreshWorkshop); $('job-status').after(refreshButton);
  $('design-form').addEventListener('submit', (event) => {
    event.preventDefault();
    const design = Object.fromEntries(DESIGN_FIELDS.map(([key]) => [key, Number($(`design-${key}`).value)]));
    if (!validateDesign(design)) { message('Every design value must be inside the displayed bounds.'); return; }
    execute(() => app.mutate(() => app.api('/api/design', design)), async () => { await refreshWorkshop(); message('Design applied. Trial and controller reset; archive cleared.'); tab('trial'); });
  });
  $('load-bundled').addEventListener('click', () => execute(() => app.mutate(() => app.api('/api/bundled', {})), async () => { await refreshWorkshop(); tab('trial'); message('Bundled archive loaded with the nominal robot and best controller.'); }));
  $('manual-controller').addEventListener('click', () => execute(() => app.mutate(() => app.api('/api/controller', { gait: app.gait() })), refreshWorkshop));
  $('search-form').addEventListener('submit', (event) => {
    event.preventDefault();
    const seed = Number($('search-seed').value), generations = Number($('search-generations').value);
    if (!Number.isInteger(seed) || seed < 0 || seed > 4294967295 || !Number.isInteger(generations) || generations < 1 || generations > 12) { message('Use an integer seed and 1–12 generations.'); return; }
    execute(() => app.mutate(() => app.api('/api/search', { seed, generations })), renderWorkshop);
  });
  $('cancel-job').addEventListener('click', () => execute(() => app.mutate(() => app.api('/api/cancel', {})), renderWorkshop));
  $('select-elite').addEventListener('click', () => {
    if (selected) execute(() => app.mutate(() => app.api('/api/controller', { cell: selected.cell })), async () => { await refreshWorkshop(); tab('trial'); message('Archived controller loaded. Start a trial to test it.'); });
  });
  $('holdout').addEventListener('click', () => execute(() => app.mutate(() => app.api('/api/holdout', {})), renderWorkshop));
  $('export-workshop').addEventListener('click', () => execute(() => app.api('/api/export'), (value) => download(value, 'Axiom-workshop.json')));
  $('import-workshop').addEventListener('change', () => execute(async () => {
    const file = $('import-workshop').files[0]; $('import-workshop').value = ''; if (!file) return;
    if (file.size > 256 * 1024) throw new Error('Workshop files must be 256 KiB or smaller.');
    const value = parseWorkshop(await file.text());
    await app.mutate(() => app.api('/api/import', value));
    await refreshWorkshop(); message('Workshop imported. Archive metrics are labelled as imported.');
  }, () => { $('import-workshop').value = ''; }));
  async function refreshRecordings() {
    try {
      const records = await app.api('/api/recordings');
      $('recordings-list').replaceChildren();
      if (!records.length) $('recordings-list').append(node('p', 'No recorded trials. Start a trial, then save it here. Completed trials are saved automatically.', 'empty-state'));
      for (const record of records) {
        const card = node('article', undefined, 'recording-card');
        card.append(node('strong', `${record.course === 'rails' ? 'Rail crossing' : 'Flat ground'} · ${record.outcome}`), node('p', `${record.controller_name} · ${designSummary(record.design)}`), node('p', `${number(record.distance_m, 2)} m · ${number(record.elapsed_s, 1)} s · ${record.frames} frames`));
        const open = node('button', 'Open replay', 'secondary');
        open.addEventListener('click', () => execute(async () => {
          pauseReplay();
          const value = replayMode ? await app.api(`/api/recording/${encodeURIComponent(record.id)}`) : await app.mutate(() => app.api(`/api/recording/${encodeURIComponent(record.id)}`));
          if (!value.frames?.length) throw new Error('This recording contains no frames.');
          await app.enterReplay(value.frames[0]); recording = value; replayMode = true;
          $('replay-controls').hidden = false; $('replay-frame').max = value.frames.length - 1;
          $('replay-label').textContent = `${value.course} · ${value.controller_name} · ${designSummary(value.design)}`;
          showFrame(0);
        }));
        card.append(open); $('recordings-list').append(card);
      }
      locks();
    } catch (error) { message(`Recordings unavailable: ${error.message}`); }
  }
  function showFrame(index) {
    if (!recording) return;
    frameIndex = replayIndex(index, recording.frames.length);
    $('replay-frame').value = frameIndex; $('replay-frame-label').textContent = `${frameIndex + 1} / ${recording.frames.length}`;
    app.showFrame(recording.frames[frameIndex]);
  }
  function pauseReplay() { playback = false; clearTimeout(replayTimer); $('play-replay').textContent = 'Play replay'; }
  function replayStep() {
    if (!playback || !recording) return;
    if (frameIndex >= recording.frames.length - 1) { pauseReplay(); return; }
    showFrame(frameIndex + 1);
    replayTimer = setTimeout(replayStep, Math.max(1, (recording.frames[frameIndex].step_seconds || 0.05) * 1000));
  }
  $('replay-frame').addEventListener('input', () => { pauseReplay(); showFrame($('replay-frame').value); });
  $('play-replay').addEventListener('click', () => {
    if (playback) { pauseReplay(); return; }
    if (!recording) return;
    if (frameIndex === recording.frames.length - 1) showFrame(0);
    playback = true; $('play-replay').textContent = 'Pause replay'; replayTimer = setTimeout(replayStep, 50);
  });
  $('return-live').addEventListener('click', () => execute(async () => { pauseReplay(); await app.returnLive(); replayMode = false; $('replay-controls').hidden = true; await refreshWorkshop(); }));
  $('download-replay').addEventListener('click', () => { if (recording) download(recording, `Axiom-recording-${recording.id}.json`); });
  $('save-recording').addEventListener('click', () => execute(() => app.mutate(() => app.api('/api/recordings/save', {})), refreshRecordings));
  $('refresh-recordings').addEventListener('click', refreshRecordings);
  refreshWorkshop(); refreshRecordings();
  return {
    pauseReplay,
    onSnapshot(state, replay) {
      replayMode = replay;
      if (!replay) updateDesign(state.design);
      const terminal = ['complete', 'fallen', 'timeout'].includes(state.outcome);
      const key = `${state.outcome}:${state.elapsed_s}:${state.controller_name}`;
      if (!replay && terminal && key !== terminalKey) { terminalKey = key; refreshRecordings(); }
      locks();
    },
  };
}
