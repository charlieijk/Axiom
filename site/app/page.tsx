"use client";

/* eslint-disable @next/next/no-img-element -- Local proof screenshots are bundled static assets. */

import { useState } from "react";

const experiments = [
  { title: "Creature Lab", body: "Run a compact creature simulation and watch a genome become movement instead of a row of opaque parameters." },
  { title: "Archive Map", body: "Browse occupied behavioral niches, compare elites, and see which experiments widened the search frontier." },
  { title: "Lineage", body: "Follow ancestry, checkpoints, and genome differences across the experiments that produced an evolved stride." }
];

// Deterministic elite placement for the archive-grid illustration.
const NICHES = 96;
const elites = new Set([3, 9, 14, 22, 27, 31, 38, 44, 45, 52, 58, 61, 67, 70, 76, 83, 89, 94]);

export default function Home() {
  const [active, setActive] = useState(0);
  const selected = experiments[active];

  return (
    <main className="journal">
      <header className="masthead">
        <div className="brand"><span className="seal">AX</span>Axiom</div>
        <p className="edition">Field Journal · Beta 1.0 · Deterministic simulation</p>
      </header>

      <section className="folio">
        <div className="folio-copy">
          <p className="entry-no">Entry 001 — Embodied evolution / MAP-Elites</p>
          <h1>Evolution you can interrogate.</h1>
          <p className="abstract">An embodied evolution laboratory where morphology, controllers, fitness, and diversity search become visible through creature replays and an inspectable archive.</p>
          <div className="actions"><a href="#experiments">Open the experiment log ↓</a><a href="#provenance">Provenance</a></div>
          <dl className="observations">
            <div><dt>2D + 3D</dt><dd>Replay surfaces</dd></div>
            <div><dt>MAP-Elites</dt><dd>Diversity archive</dd></div>
            <div><dt>Rust</dt><dd>Authoritative core</dd></div>
          </dl>
        </div>
        <figure className="archive-plate" aria-label="Illustration of the MAP-Elites behavioral archive">
          <figcaption>Fig. 1 — Occupied behavioral niches</figcaption>
          <div className="archive-grid" aria-hidden="true">
            {Array.from({ length: NICHES }, (_, i) => <i key={i} className={elites.has(i) ? "elite" : ""} />)}
          </div>
          <div className="axes"><span>← behavior descriptor 1 →</span><span>← behavior descriptor 2 →</span></div>
        </figure>
      </section>

      <section className="log" id="experiments">
        <p className="entry-no">Entries 002–004 — The experiment log</p>
        <h2>Three instruments. One archive.</h2>
        <div className="log-split">
          <div className="log-index" role="tablist" aria-label="Axiom experiment log">
            {experiments.map((exp, index) => (
              <button key={exp.title} role="tab" aria-selected={active === index} className={active === index ? "active" : ""} onClick={() => setActive(index)}>
                <span>{String(index + 2).padStart(3, "0")}</span>{exp.title}
              </button>
            ))}
          </div>
          <article className="log-plate" aria-live="polite">
            <p className="entry-no">Entry {String(active + 2).padStart(3, "0")} — active instrument</p>
            <h3>{selected.title}</h3>
            <p>{selected.body}</p>
          </article>
        </div>
      </section>

      <section className="provenance" id="provenance">
        <p className="entry-no">Appendix A — Provenance</p>
        <h2>This Site has a real source of truth.</h2>
        <div className="provenance-grid">
          <div>
            <p className="note">The presentation above is paired with the canonical project boundary below. Open the real live environment when one exists, inspect the source snapshot, or use the documented local launch path for backend and native capabilities.</p>
            <div className="links"><a href="https://github.com/charlieijk/Axiom" target="_blank" rel="noreferrer">Open source project ↗</a></div>
            <figure className="plate-photo"><img src="/original-interface.jpg" alt="Original axiom project interface or design proof" loading="lazy" /><figcaption>Plate I — Original project interface</figcaption></figure>
          </div>
          <dl className="specimen-label">
            <div><dt>Canonical source</dt><dd>https://github.com/charlieijk/Axiom</dd></div>
            <div><dt>Connected snapshot</dt><dd>main · 2a375cc3</dd></div>
            <div><dt>Original frontend</dt><dd>Rust simulation + 2D/3D browser viewer</dd></div>
            <div><dt>Source boundary</dt><dd>web/index.html · web/graphics3d.html</dd></div>
            <div><dt>Launch locally</dt><dd>cargo run -- serve</dd></div>
          </dl>
        </div>
      </section>

      <section className="colophon">
        <b>Sites edition</b>
        <p>This dedicated website presents the current frontend safely. Native capabilities, local credentials, private connectors, and production mutations remain outside the browser boundary.</p>
      </section>

      <footer><span>Charlie Cullen / Developer workspace</span><b>Axiom</b><span>Beta 1.0 · Sites edition</span></footer>
    </main>
  );
}
