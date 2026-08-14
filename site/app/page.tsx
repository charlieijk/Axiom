/* eslint-disable @next/next/no-img-element -- Local proof screenshots are bundled static assets. */

import { AxiomSimulator } from "./AxiomSimulator";

export default function Home() {
  return (
    <main className="site" style={{ "--accent": "#c9f24b", "--soft": "#e9f9a7" } as React.CSSProperties}>
      <header className="topbar">
        <div className="brand"><i>AX</i>Axiom</div>
        <div className="status">Live browser simulation · Deterministic</div>
      </header>
      <section className="hero">
        <div className="hero-copy">
          <p className="kicker">EMBODIED EVOLUTION / MAP-ELITES</p>
          <h1>Evolution you can interrogate.</h1>
          <p className="summary">Change a controller, terrain, or seed. Then run a source-aligned deterministic creature model that makes Axiom&apos;s morphology, motion, and fitness legible.</p>
          <div className="actions"><a href="#experience">Run the simulator ↘</a><a href="#connection">Inspect the source</a></div>
          <div className="metrics">
            <div className="metric"><strong>3 CONTROL MODES</strong><span>CPG · feedforward · recurrent</span></div>
            <div className="metric"><strong>SEEDED</strong><span>Repeatable browser trials</span></div>
            <div className="metric"><strong>RUST → WEB</strong><span>Source-aligned simulation loop</span></div>
          </div>
        </div>
        <AxiomSimulator />
      </section>
      <section className="connection" id="connection">
        <div className="connection-grid">
          <div className="connection-copy">
            <p className="kicker">ORIGINAL PROJECT RECORD</p>
            <h2>Live here. Rust remains the source of truth.</h2>
            <p>The simulator above runs entirely in the browser from a source-aligned model of Axiom&apos;s seeded replay loop. A canonical CPG / Rough / seed 19 replay is checked against Rust within explicit tolerances; the repository remains authoritative for evaluation, evolution, MAP-Elites archives, 3D playback, and saved experiments.</p>
            <div className="connection-links"><a href="https://github.com/charlieijk/Axiom" target="_blank" rel="noreferrer">Open source project ↗</a></div>
            <figure className="original-proof"><img src="/original-interface.jpg" alt="Original axiom project interface or design proof" loading="lazy" /><figcaption>Original project interface / design proof</figcaption></figure>
          </div>
          <dl className="connection-record">
            <div><dt>Source repository</dt><dd>https://github.com/charlieijk/Axiom</dd></div>
            <div><dt>Recorded snapshot</dt><dd>main · 2a375cc3</dd></div>
            <div><dt>Original frontend</dt><dd>Rust simulation + 2D/3D browser viewer</dd></div>
            <div><dt>Source boundary</dt><dd>web/index.html · web/graphics3d.html</dd></div>
            <div><dt>Launch locally</dt><dd>cargo run -- serve</dd></div>
          </dl>
        </div>
      </section>
      <section className="manifest" id="principles">
        <p className="kicker">ACTIVE PRODUCT PROOF / CURRENT BOUNDARY</p>
        <h2>One real loop. Three ways to interrogate it.</h2>
        <div className="manifest-grid">
          <article><span>01 / CHANGE</span><h3>Set the trial</h3><p>Choose controller behavior, terrain, and a repeatable seed at the boundary.</p></article>
          <article><span>02 / OBSERVE</span><h3>Replay the gait</h3><p>Pause, restart, change speed, or scrub through every generated frame.</p></article>
          <article><span>03 / COMPARE</span><h3>Read the outcome</h3><p>Use fitness, stability, distance, body count, and joint count to compare runs.</p></article>
        </div>
      </section>
      <section className="boundary"><b>SITES EDITION</b><p>The live trial is client-side and deterministic. Full MAP-Elites search, checkpoint persistence, native rendering, and local server capabilities remain inside the source project.</p></section>
      <footer><span>Charlie Cullen / Developer workspace</span><b>Axiom</b><span>Beta 1.0 · Sites edition</span></footer>
    </main>
  );
}
