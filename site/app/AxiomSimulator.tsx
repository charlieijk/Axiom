"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent } from "react";

import {
  runDeterministicTrial,
  terrainAt,
  type ControllerKind,
  type DeterministicTrial,
  type ReplayFrame,
  type TerrainKind,
  type Vec2,
} from "./simulator-model";

declare global {
  interface Window {
    render_axiom_simulator_to_text?: () => string;
  }
}

const DEFAULT_CONTROLS = {
  controller: "cpg" as ControllerKind,
  terrain: "rough" as TerrainKind,
  seed: 19,
};

function screenPoint(point: Vec2, camera: { x: number; y: number; scale: number }, height: number) {
  return {
    x: (point[0] - camera.x) * camera.scale,
    y: height - (point[1] - camera.y) * camera.scale,
  };
}

function drawTerrain(
  context: CanvasRenderingContext2D,
  trial: DeterministicTrial,
  camera: { x: number; y: number; scale: number },
  width: number,
  height: number,
) {
  context.save();
  context.beginPath();
  context.moveTo(0, height);
  for (let pixel = 0; pixel <= width; pixel += 7) {
    const worldX = camera.x + pixel / camera.scale;
    const point = screenPoint([worldX, terrainAt(trial.terrain, worldX)], camera, height);
    context.lineTo(pixel, point.y);
  }
  context.lineTo(width, height);
  context.closePath();
  const ground = context.createLinearGradient(0, height * 0.66, 0, height);
  ground.addColorStop(0, "#263223");
  ground.addColorStop(1, "#11140f");
  context.fillStyle = ground;
  context.fill();

  context.beginPath();
  for (let pixel = 0; pixel <= width; pixel += 7) {
    const worldX = camera.x + pixel / camera.scale;
    const point = screenPoint([worldX, terrainAt(trial.terrain, worldX)], camera, height);
    if (pixel === 0) context.moveTo(pixel, point.y);
    else context.lineTo(pixel, point.y);
  }
  context.strokeStyle = "#b9ed82";
  context.lineWidth = 2;
  context.stroke();
  context.restore();
}

function drawFrame(canvas: HTMLCanvasElement, trial: DeterministicTrial, frame: ReplayFrame) {
  const context = canvas.getContext("2d");
  if (!context) return;

  const width = canvas.clientWidth;
  const height = canvas.clientHeight;
  const ratio = window.devicePixelRatio || 1;
  const targetWidth = Math.max(1, Math.floor(width * ratio));
  const targetHeight = Math.max(1, Math.floor(height * ratio));
  if (canvas.width !== targetWidth || canvas.height !== targetHeight) {
    canvas.width = targetWidth;
    canvas.height = targetHeight;
  }
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  context.clearRect(0, 0, width, height);

  const background = context.createLinearGradient(0, 0, width, height);
  background.addColorStop(0, "#10150f");
  background.addColorStop(0.58, "#090c0a");
  background.addColorStop(1, "#171712");
  context.fillStyle = background;
  context.fillRect(0, 0, width, height);

  const floor = terrainAt(trial.terrain, frame.root[0]);
  const scale = Math.min(width / 10.5, height / 4.2);
  const camera = { x: frame.root[0] - 3.5, y: floor - 0.92, scale };

  context.save();
  context.strokeStyle = "rgba(240, 238, 230, 0.055)";
  context.lineWidth = 1;
  for (let x = Math.floor(camera.x); x < camera.x + width / scale + 1; x += 1) {
    const point = screenPoint([x, camera.y], camera, height);
    context.beginPath();
    context.moveTo(point.x, 0);
    context.lineTo(point.x, height);
    context.stroke();
  }
  for (let y = Math.floor(camera.y); y < camera.y + height / scale + 1; y += 1) {
    const point = screenPoint([camera.x, y], camera, height);
    context.beginPath();
    context.moveTo(0, point.y);
    context.lineTo(width, point.y);
    context.stroke();
  }
  context.restore();

  drawTerrain(context, trial, camera, width, height);

  context.save();
  context.lineCap = "round";
  for (const [startPoint, endPoint] of frame.joints) {
    const start = screenPoint(startPoint, camera, height);
    const end = screenPoint(endPoint, camera, height);
    context.strokeStyle = "rgba(226, 189, 103, 0.78)";
    context.lineWidth = 7;
    context.beginPath();
    context.moveTo(start.x, start.y);
    context.lineTo(end.x, end.y);
    context.stroke();
    context.strokeStyle = "rgba(10, 12, 10, 0.82)";
    context.lineWidth = 2;
    context.stroke();
  }

  for (let index = trial.body.length - 1; index >= 0; index -= 1) {
    const node = trial.body[index];
    const center = screenPoint(frame.bodies[index], camera, height);
    const parent = node.parent;
    const angle = parent === null
      ? frame.tilt
      : Math.atan2(
          frame.bodies[index][1] - frame.bodies[parent][1],
          frame.bodies[index][0] - frame.bodies[parent][0],
        );
    const sizeX = Math.max(18, node.size[0] * scale);
    const sizeY = Math.max(12, node.size[1] * scale);

    context.save();
    context.translate(center.x, center.y);
    context.rotate(-angle);
    context.beginPath();
    context.roundRect(-sizeX / 2, -sizeY / 2, sizeX, sizeY, Math.min(9, sizeY / 2));
    context.fillStyle = index === 0 ? "#e6f3df" : "#79aee0";
    context.fill();
    context.strokeStyle = index === 0 ? "#b9ed82" : "#0c100c";
    context.lineWidth = index === 0 ? 3 : 2;
    context.stroke();
    if (node.actuator > 0) {
      context.fillStyle = "#e2bd67";
      context.beginPath();
      context.arc(sizeX * 0.22, 0, Math.max(3, sizeY * 0.16), 0, Math.PI * 2);
      context.fill();
    }
    context.restore();
  }
  context.restore();
}

export function AxiomSimulator() {
  const [prefersReducedMotion, setPrefersReducedMotion] = useState(false);
  const [controls, setControls] = useState(DEFAULT_CONTROLS);
  const [trial, setTrial] = useState(() => runDeterministicTrial({ ...DEFAULT_CONTROLS, steps: 220 }));
  const [frameIndex, setFrameIndex] = useState(0);
  const [playing, setPlaying] = useState(true);
  const [speed, setSpeed] = useState(1);
  const [announcement, setAnnouncement] = useState("Seed 19 trial ready and playing.");
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const accumulatedTime = useRef(0);
  const previousTime = useRef<number | null>(null);
  const frame = trial.frames[frameIndex] ?? trial.frames[0];
  const currentTrial = useRef(trial);
  const currentFrame = useRef(frame);

  const runTrial = useCallback((nextControls = controls) => {
    const nextTrial = runDeterministicTrial({ ...nextControls, steps: 220 });
    setTrial(nextTrial);
    setControls({
      controller: nextTrial.controller,
      terrain: nextTrial.terrain,
      seed: nextTrial.seed,
    });
    setFrameIndex(0);
    accumulatedTime.current = 0;
    previousTime.current = null;
    const shouldPlay = !prefersReducedMotion;
    setPlaying(shouldPlay);
    setAnnouncement(
      `Seed ${nextTrial.seed} ${nextTrial.controller} trial on ${nextTrial.terrain} terrain is ready and ${shouldPlay ? "playing" : "paused"}.`,
    );
  }, [controls, prefersReducedMotion]);

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    runTrial();
  };

  useEffect(() => {
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    const updatePreference = () => {
      setPrefersReducedMotion(query.matches);
      if (query.matches) setPlaying(false);
    };
    updatePreference();
    query.addEventListener("change", updatePreference);
    return () => query.removeEventListener("change", updatePreference);
  }, []);

  useEffect(() => {
    if (!playing) {
      previousTime.current = null;
      return;
    }

    let animationFrame = 0;
    const animate = (now: number) => {
      const previous = previousTime.current ?? now;
      previousTime.current = now;
      accumulatedTime.current += Math.min(100, now - previous) * speed;
      const frameDuration = trial.dt * 1000;
      const advance = Math.floor(accumulatedTime.current / frameDuration);
      if (advance > 0) {
        accumulatedTime.current -= advance * frameDuration;
        setFrameIndex((current) => (current + advance) % trial.frames.length);
      }
      animationFrame = window.requestAnimationFrame(animate);
    };

    animationFrame = window.requestAnimationFrame(animate);
    return () => window.cancelAnimationFrame(animationFrame);
  }, [playing, speed, trial.dt, trial.frames.length]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    currentTrial.current = trial;
    currentFrame.current = frame;
    drawFrame(canvas, trial, frame);
  }, [frame, trial]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const stage = canvas?.parentElement;
    if (!canvas || !stage) return;

    let resizeFrame = 0;
    const observer = new ResizeObserver(() => {
      window.cancelAnimationFrame(resizeFrame);
      resizeFrame = window.requestAnimationFrame(() => {
        drawFrame(canvas, currentTrial.current, currentFrame.current);
      });
    });
    observer.observe(stage);
    return () => {
      observer.disconnect();
      window.cancelAnimationFrame(resizeFrame);
    };
  }, []);

  useEffect(() => {
    window.render_axiom_simulator_to_text = () => JSON.stringify({
      ready: true,
      playing,
      controller: trial.controller,
      terrain: trial.terrain,
      seed: trial.seed,
      frame: frameIndex,
      frames: trial.frames.length,
      bodyCount: trial.body.length,
      jointCount: frame.joints.length,
      time: frame.time,
      distance: frame.root[0],
      tilt: frame.tilt,
      fitness: trial.metrics.fitness,
    });
    return () => {
      delete window.render_axiom_simulator_to_text;
    };
  }, [frame, frameIndex, playing, trial]);

  const statusLabel = useMemo(() => playing ? "Running" : "Paused", [playing]);

  return (
    <section
      className="simulator-window"
      id="experience"
      aria-labelledby="simulator-title"
      data-simulator-state={playing ? "running" : "paused"}
      data-controller={trial.controller}
      data-terrain={trial.terrain}
      data-seed={trial.seed}
    >
      <div className="simulator-bar">
        <span className="simulator-lights" aria-hidden="true"><i/><i/><i/></span>
        <b>Source simulator / live browser runtime</b>
        <span className="live-indicator"><i aria-hidden="true"/>Live</span>
      </div>
      <div className="simulator-layout">
        <div className="simulator-stage">
          <canvas
            ref={canvasRef}
            width="960"
            height="560"
            role="img"
            aria-label={`Live deterministic creature replay using the ${trial.controller} controller on ${trial.terrain} terrain.`}
          >
            A deterministic creature replay. Use the adjacent controls to change the controller, terrain, seed, speed, or replay position.
          </canvas>
          <dl className="simulator-hud" aria-label="Current replay metrics">
            <div><dt>Time</dt><dd>{frame.time.toFixed(2)}s</dd></div>
            <div><dt>Distance</dt><dd>{frame.root[0].toFixed(2)}</dd></div>
            <div><dt>Tilt</dt><dd>{frame.tilt.toFixed(2)}</dd></div>
          </dl>
          <span className="frame-status">{statusLabel} · {frameIndex}/{trial.frames.length - 1}</span>
        </div>
        <form className="simulator-controls" onSubmit={handleSubmit}>
          <header>
            <p className="control-eyebrow">Axiom / Creature Lab</p>
            <h2 id="simulator-title">Run a deterministic trial.</h2>
          </header>
          <div className="control-grid">
            <label>
              <span>Controller</span>
              <select
                value={controls.controller}
                onChange={(event) => setControls((current) => ({
                  ...current,
                  controller: event.target.value as ControllerKind,
                }))}
              >
                <option value="cpg">CPG</option>
                <option value="feedforward">Feedforward</option>
                <option value="recurrent">Recurrent</option>
              </select>
            </label>
            <label>
              <span>Terrain</span>
              <select
                value={controls.terrain}
                onChange={(event) => setControls((current) => ({
                  ...current,
                  terrain: event.target.value as TerrainKind,
                }))}
              >
                <option value="rough">Rough</option>
                <option value="flat">Flat</option>
                <option value="recovery">Recovery</option>
              </select>
            </label>
            <label>
              <span>Seed</span>
              <input
                type="number"
                min="1"
                max="999999"
                step="1"
                value={controls.seed}
                onChange={(event) => setControls((current) => ({
                  ...current,
                  seed: Number(event.target.value),
                }))}
              />
            </label>
            <button className="run-trial" type="submit">Run trial</button>
          </div>
          <div className="playback-buttons" aria-label="Replay controls">
            <button
              type="button"
              onClick={() => {
                setPlaying((current) => !current);
                setAnnouncement(playing ? "Replay paused." : "Replay playing.");
              }}
            >
              {playing ? "Pause" : "Play"}
            </button>
            <button
              type="button"
              onClick={() => {
                setFrameIndex(0);
                accumulatedTime.current = 0;
                setPlaying(!prefersReducedMotion);
                setAnnouncement(`Replay restarted from frame zero and ${prefersReducedMotion ? "paused" : "playing"}.`);
              }}
            >
              Restart
            </button>
          </div>
          <label className="range-control">
            <span>Speed <output htmlFor="simulation-speed">{speed.toFixed(2)}×</output></span>
            <input
              id="simulation-speed"
              aria-label="Playback speed"
              type="range"
              min="0.25"
              max="3"
              step="0.25"
              value={speed}
              onChange={(event) => setSpeed(Number(event.target.value))}
            />
          </label>
          <label className="range-control">
            <span>Replay <output htmlFor="simulation-replay">{frameIndex}/{trial.frames.length - 1}</output></span>
            <input
              id="simulation-replay"
              aria-label="Replay frame"
              type="range"
              min="0"
              max={trial.frames.length - 1}
              step="1"
              value={frameIndex}
              onChange={(event) => {
                setPlaying(false);
                setFrameIndex(Number(event.target.value));
                setAnnouncement(`Replay moved to frame ${event.target.value}.`);
              }}
            />
          </label>
          <dl className="trial-outcome" aria-label="Trial outcome">
            <div><dt>Fitness</dt><dd>{trial.metrics.fitness.toFixed(2)}</dd></div>
            <div><dt>Stability</dt><dd>{Math.round(trial.metrics.stability * 100)}%</dd></div>
            <div><dt>Morphology</dt><dd>{trial.body.length} / {frame.joints.length}</dd></div>
          </dl>
          <p className="sr-only" aria-live="polite">{announcement}</p>
        </form>
      </div>
    </section>
  );
}
