# Axiom architecture

> The Rust crates own simulation and experiment semantics; browser and Python surfaces expose or present those results without becoming a second source of truth.

The rendered submission asset is [axiom-architecture.png](axiom-architecture.png).
The large-type video/gallery version of the authoritative evolution path is
[axiom-core-loop.png](axiom-core-loop.png).

## System map

```mermaid
flowchart LR
    cli_user["CLI user"]
    local_user["Local browser user"]
    public_user["Public-site visitor"]

    subgraph loopback["Loopback trust boundary — unauthenticated, local-only by default"]
        flask["Flask adapter<br/>flask_app.py"]
        server["Rust GUI server<br/>src/web.rs"]
        web2d["Embedded 2D client<br/>web/index.html"]
        web3d["Embedded 3D client<br/>web/graphics3d.html"]
        api["GET /api/replay"]

        flask -->|"proxies requests and responses"| server
        server -->|"serves bundled assets"| web2d
        server -->|"serves bundled assets"| web3d
        web2d -->|"validated query"| api
        web3d -->|"validated query"| api
    end

    subgraph core["AUTHORITATIVE — Rust evolution core in src/"]
        request["Replay request parsing<br/>and interactive cost budget"]
        evolution["Evolution loop<br/>selection, mutation, evaluation"]
        brain["Genome and Brain<br/>feedforward, recurrent, CPG"]
        evaluation["Task evaluation loop"]
        simulation["Deterministic sandbox<br/>Simulation and World"]
        fitness["Metrics and task fitness"]
        archive["MAP-Elites archive<br/>history and lineage"]
        replay["Replay capture<br/>JSON response"]
        checkpoint["Versioned checkpoint JSON<br/>atomic save and explicit load errors"]

        request -->|"minimal genome"| evaluation
        request -->|"evolved request"| evolution
        evolution -->|"candidate genome"| evaluation
        evaluation -->|"observations"| brain
        brain -->|"actions"| simulation
        simulation -->|"snapshots"| evaluation
        evaluation --> fitness
        fitness -->|"score"| evolution
        evolution -->|"insert or replace elite"| archive
        archive -->|"sample parent"| evolution
        evaluation -->|"metrics"| replay
        archive -->|"selected elite and lineage"| replay
        replay -->|"selected genome"| simulation
        simulation -->|"frames"| replay
        evolution -->|"config and report when saved"| checkpoint
    end

    subgraph public_site["Public site boundary — site/"]
        site["React and Vinext product surface<br/>presentation and repository record"]
        site_boundary["No live Rust backend authority<br/>no checkpoint or evolution ownership"]
        site --> site_boundary
    end

    subgraph field["Field Lab boundary — crates/axiom-field/"]
        robot["robot.toml<br/>uncalibrated robot parameters"]
        controller["CPG genome and<br/>perturbed-world ensemble"]
        field_sim["Rapier rigid-body simulation<br/>links, joints, contact, servos"]
        field_eval["Ensemble evaluation"]
        field_search["Repertoire search<br/>and unseen-world holdout"]
        goldens["Committed trajectory and archive goldens<br/>software evidence, not hardware validation"]

        robot --> field_sim
        controller --> field_sim
        field_sim --> field_eval
        field_eval --> field_search
        field_search --> goldens
    end

    cli_user --> evolution
    cli_user --> evaluation
    cli_user -->|"save, inspect, replay"| checkpoint
    checkpoint -->|"loaded config and report"| cli_user
    local_user --> server
    local_user --> flask
    api --> request
    replay --> api
    public_user --> site
    archive -.-> field_search

    classDef authority fill:#173c32,stroke:#62b997,color:#ffffff;
    classDef adapter fill:#eef1ed,stroke:#75827c,color:#17211d;
    classDef artifact fill:#fff2cc,stroke:#b38b25,color:#302509;
    class evolution,brain,evaluation,simulation,fitness,archive,replay,request authority;
    class flask,server,web2d,web3d,api,site,site_boundary adapter;
    class checkpoint,goldens artifact;
```

Solid arrows are runtime control or data flow. The single dashed arrow is a code dependency: Field Lab reuses the generic `axiom::qd::Grid` mechanics, but owns its CPG payload, descriptors, evaluation, and search report.

## Boundaries that matter

### Authority

- `src/` is authoritative for the root sandbox's genome execution, task evaluation, fitness, evolution, MAP-Elites archive, checkpoint format, and replay JSON.
- The embedded clients render and request Rust replay data. They do not independently score genomes or mutate the authoritative archive.
- `flask_app.py` is a loopback reverse proxy. It does not reimplement replay generation or evolution.
- `site/` is a separate public product surface. It may explain or demonstrate Axiom, but it is not connected to the Rust replay server and does not own full evaluation, evolution, or persistence semantics.
- `crates/axiom-field/` is authoritative only for its fixed-robot rigid-body experiments. It is deliberately separate from the root sandbox's abstract locomotion model.

### Trust

- The Rust GUI and Flask adapter are unauthenticated single-user tools. Their safe default is loopback; exposing either beyond the host requires an external authenticated TLS boundary.
- `/api/replay` is read-only, but evolved requests consume compute. Request parsing rejects workloads above the interactive budget before running them.
- The public site remains outside the local simulation boundary and must not acquire local credentials, checkpoint writes, or production mutation authority by implication.

### Determinism and evidence

- Root evolution is tested for identical results from a fixed seed, and complete versioned checkpoints preserve the configuration, report, archive, history, and lineage needed to inspect a run. The shared MAP-Elites deserializer rejects zero, overflowing, or cell-count-mismatched grid dimensions before either root checkpoints or Field Lab reports can expose inconsistent archive evidence.
- Field Lab asserts bit-identical trajectories for identical inputs within one binary. Across x86_64 and aarch64, its committed goldens assert behavioral bands because rigid-body contact amplifies floating-point differences.
- Field Lab's bundled robot parameters are uncalibrated. Perturbed-world holdouts and committed artifacts are reproducible software evidence, not measured sim-to-real transfer or hardware validation.

## Judge paths

1. **Inspect an evolution:** CLI → evolution loop → task evaluation → MAP-Elites archive → versioned checkpoint.
2. **Watch an elite:** embedded browser client → `/api/replay` → validated Rust evolution/evaluation → replay JSON → 2D or 3D rendering.
3. **Inspect the physical-model claim:** Field Lab configuration → rigid-body simulation → perturbed-world ensemble → repertoire and holdout artifacts, with the uncalibrated boundary kept explicit.
