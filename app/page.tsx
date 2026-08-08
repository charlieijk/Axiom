"use client";

import { useState } from "react";

const views = [
  { title: "Creature Lab", body: "Run a compact creature simulation and watch a genome become movement instead of a row of opaque parameters." },
  { title: "Archive Map", body: "Browse occupied behavioral niches, compare elites, and see which experiments widened the search frontier." },
  { title: "Lineage", body: "Follow ancestry, checkpoints, and genome differences across the experiments that produced an evolved stride." }
];

export default function Home() {
  const [active, setActive] = useState(0);
  const selected = views[active];

  return (
    <main className="site" style={{ "--accent": "#c9f24b", "--soft": "#e9f9a7" } as React.CSSProperties}>
      <header className="topbar">
        <div className="brand"><i>AX</i>Axiom</div>
        <div className="status">Deterministic simulation</div>
      </header>
      <section className="hero">
        <div className="hero-copy">
          <p className="kicker">EMBODIED EVOLUTION / MAP-ELITES</p>
          <h1>Evolution you can interrogate.</h1>
          <p className="summary">An embodied evolution laboratory where morphology, controllers, fitness, and diversity search become visible through creature replays and an inspectable archive.</p>
          <div className="actions"><a href="#experience">Explore the product ↘</a><a href="#principles">Product principles</a></div>
          <div className="metrics">
            <div className="metric"><strong>2D + 3D</strong><span>Replay surfaces</span></div>
            <div className="metric"><strong>MAP-ELITES</strong><span>Diversity archive</span></div>
            <div className="metric"><strong>RUST</strong><span>Authoritative core</span></div>
          </div>
        </div>
        <div className="product-window" id="experience">
          <div className="window-bar"><i/><i/><i/><b>axiom / product surface</b></div>
          <div className="window-body">
            <aside className="rail"><i/><i/><i/><i/></aside>
            <div className="workspace">
              <div className="workspace-head"><span>Current workspace</span><b>Deterministic simulation</b></div>
              <div className="visual" data-kind="world" aria-hidden="true"><i className="orb" /><i className="orb" /><i className="orb" /><i className="orb" /><b className="beam" /><b className="beam" /><b className="beam" /></div>
              <div className="readout" aria-live="polite"><small>0{active + 1} / ACTIVE VIEW</small><h2>{selected.title}</h2><p>{selected.body}</p></div>
            </div>
          </div>
          <div className="tabs" role="tablist" aria-label="Axiom product views">
            {views.map((view,index)=><button key={view.title} role="tab" aria-selected={active===index} className={active===index?"active":""} onClick={()=>setActive(index)}>{String(index+1).padStart(2,"0")} / {view.title}</button>)}
          </div>
        </div>
      </section>
      <section className="manifest" id="principles">
        <p className="kicker">PRODUCT SURFACE / CURRENT BOUNDARY</p>
        <h2>Three views. One coherent operating model.</h2>
        <div className="manifest-grid">
          <article><span>01</span><h3>Creature Lab</h3><p>Run a compact creature simulation and watch a genome become movement instead of a row of opaque parameters.</p></article>
          <article><span>02</span><h3>Archive Map</h3><p>Browse occupied behavioral niches, compare elites, and see which experiments widened the search frontier.</p></article>
          <article><span>03</span><h3>Lineage</h3><p>Follow ancestry, checkpoints, and genome differences across the experiments that produced an evolved stride.</p></article>
        </div>
      </section>
      <section className="boundary"><b>SITES EDITION</b><p>This dedicated website presents the current frontend safely. Native capabilities, local credentials, private connectors, and production mutations remain outside the browser boundary.</p></section>
      <footer><span>Charlie Cullen / Developer workspace</span><b>Axiom</b><span>Private Sites edition</span></footer>
    </main>
  );
}

