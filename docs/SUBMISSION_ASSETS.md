# Submission assets

These assets are the recommended judge-facing sequence for Axiom's Arm Create
submission. Captions are intentionally factual and can be pasted into Devpost
after registration and rule review.

The current UI captures were taken from the Rust-backed `/3d` interface on
August 13, 2026. Their integrity manifest is
[submission-assets.sha256](submission-assets.sha256).

Use [axiom-arm64-result.png](axiom-arm64-result.png) as the Devpost cover and
thumbnail because its measured result remains legible at card size. Use the
remaining items below as the gallery sequence.

## Screenshot sequence

| Order | Asset | Caption | What it proves |
| ---: | --- | --- | --- |
| 1 | [Arm64 result card](axiom-arm64-result.png) | On Apple M2 Arm64, Axiom's reusable rollout workspaces cut allocation and reallocation calls by 94.38% and requested allocation bytes by 89.73% across 2,048,400 deterministic steps, while the complete evolution report stayed identical. | A material, measured Arm64 optimization with an explicit correctness contract. |
| 2 | [Live Archive Lab](axiom-archive-lab-gen12.jpg) | A live CPG-controlled champion traverses rough terrain while the Field Journal connects its 7-part morphology, fitness 121.46, 15% archive coverage, recorded lineage, and selected MAP-Elites cell 6,5. | The complete Rust-backed product experience rather than a benchmark-only entry. |
| 3 | [Selected diversity elite](axiom-archive-elite-cell-0-7.jpg) | Selecting Archive Lab cell 0,7 replays Genome 210: a 9-part, 0.86-stability elite from generation 9. Its lower fitness but distinct morphology shows why Axiom preserves a repertoire instead of only one winner. | A real, interactive quality-diversity archive whose cells replay their own genomes and lineage. |
| 4 | [Core-loop map](axiom-core-loop.png) | Sensor observations feed neural-controller inference and actuator commands; deterministic simulation metrics return to fitness and MAP-Elites selection. Reusable rollout workspaces, measured on Arm64, optimize that loop without changing scalar operation order. | A readable judge-level view of the Physical AI loop and where the optimization lives. |
| 5 | [Next-generation state](axiom-archive-lab-gen13.jpg) | “Evolve next generation” advances the run to generation 13 and refreshes replay, progress chart, Archive Lab, journal, and timeline. The champion honestly remains unchanged in this deterministic run. | An end-to-end interaction backed by the Rust evolution engine, without implying every generation must improve. |

The older root-level Field Journal and mobile captures show the superseded
Search Lineage panel. Keep them as design-history evidence, but do not present
them as screenshots of the current Archive Lab interface. The five assets above
were selected against the current implementation.

## Demo video flow

Target length: 2 minutes 45 seconds. Record at 1440 × 1024 or larger and keep
terminal text at a readable scale.

| Time | Visual | Narration target |
| --- | --- | --- |
| 0:00–0:15 | Arm64 result card | “Axiom is a deterministic embodied-evolution sandbox. For the Arm challenge, I optimized the rollout path that evaluates thousands of candidate bodies and controllers.” |
| 0:15–0:40 | Live `/3d` Field Journal on rough terrain | Identify the evolving morphology, CPG controller, terrain, fitness, archive coverage, and lineage. Emphasize that the journal reflects Rust replay data. |
| 0:40–1:03 | Select Archive Lab cell 0,7 | Contrast its 9-part body and 0.86 stability with the champion in cell 6,5. Explain that quality-diversity keeps behaviorally distinct elites instead of collapsing to one winner. |
| 1:03–1:18 | Select generation 6, then 12 | Show verified fitness/coverage moving from 87.98/10% to 121.46/15%. |
| 1:18–1:35 | Choose **Evolve next generation** | Show the replay, chart, Archive Lab, journal, and timeline updating to generation 13 together. Note honestly that the champion is unchanged. |
| 1:35–1:52 | Core-loop map | Trace observation → brain → action → simulation → fitness → MAP-Elites. Point out that versioned checkpoints preserve the complete experiment record; keep the full authority map linked for deeper review. |
| 1:52–2:27 | `docs/ARM64_OPTIMIZATION.md` and the two allocation rows | Explain the reusable observation, network, action, and snapshot workspaces. State 94.38% fewer calls and 89.73% fewer requested bytes on Apple M2 Arm64 over 2,048,400 steps. |
| 2:27–2:40 | Correctness table in the same document | Show that the full 1,710,519-byte report—including champion, archive, history, and lineage—has the same SHA-256 before and after. Say explicitly that no runtime-speedup claim is being made. |
| 2:40–2:45 | Return to the moving creature | Close on the measured result: the rollout creates far less allocator traffic without changing the experiment's result. |

## Recording checklist

- Start the local interface with `cargo run --locked -- gui --port 8787`, then open
  `http://127.0.0.1:8787/3d`.
- Use the verified CPG, rough-terrain, seed-29 state shown in the screenshot set.
- Keep the pointer still while speaking; use it only for the timeline and
  **Evolve next generation** interaction.
- Avoid unsupported claims: this evidence measures allocation request volume,
  not peak RSS, energy use, or a defensible runtime speedup.
- Mention Codex concretely: it helped profile the hot path, implement reusable
  workspaces, build the isolated baseline/optimized harness, and strengthen the
  semantic-equivalence tests and CI evidence.
- Wait for `14 / 96 occupied`, then pause at the desired pose before capture.
- End before three minutes; the optional Devpost video limit is under three
  minutes.

## Evidence links

- [Arm64 optimization protocol](ARM64_OPTIMIZATION.md)
- [Baseline raw JSON](benchmarks/arm64-allocation-baseline.json)
- [Optimized raw JSON](benchmarks/arm64-allocation-optimized.json)
- [Machine-readable comparison](benchmarks/comparison.json)
- [Submission asset checksums](submission-assets.sha256)
- [Flagship local walkthrough](../DEMO.md)
