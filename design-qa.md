# Axiom Evolution Field Journal — Design QA

## Evidence

- Source visual truth: `/Users/charlie/Developer/Axiom/design/axiom-field-journal-source.png`
- Browser-rendered implementation: `/Users/charlie/Developer/Axiom/axiom-field-journal-final.png`
- Full-view comparison: `/Users/charlie/Developer/Axiom/axiom-field-journal-comparison.png`
- Focused Field Journal comparison: `/Users/charlie/Developer/Axiom/axiom-field-journal-focus-comparison.png`
- Mobile implementation: `/Users/charlie/Developer/Axiom/axiom-field-journal-mobile.png`
- Post-evolution interaction state: `/Users/charlie/Developer/Axiom/axiom-field-journal-post-evolve.png`
- Local implementation URL: `http://127.0.0.1:8787/3d`
- Desktop viewport: 1440 × 1024
- Mobile viewport: 375 × 844
- Comparison state: CPG controller, rough terrain, seed 29, Generation 12 selected, evolution complete

## Findings

No actionable P0, P1, or P2 differences remain.

- [P3] The source captures a different live creature pose and a more heavily textured terrain moment. The implementation uses the real Axiom WebGL replay and therefore changes pose continuously while preserving the same subject, crop, color treatment, ghost trail, scan rings, and rough-terrain framing.
- [P3] The source lineage tree depicts a bespoke individual genealogy. Axiom’s current replay response exposes generation aggregates rather than parent IDs, so the implementation renders an honest search-lineage visualization from archive occupancy and generation history instead of fabricating individual ancestry.
- [P3] The source uses custom morphology pictograms and claims such as “rear limb angle widened.” The implementation uses a consistent Phosphor icon family and backend-derived body-part, joint, fitness, stability, and controller copy. This is a small illustration-fidelity tradeoff that materially improves product credibility.
- [P3] The implementation adds a small “Current observation” eyebrow and compact live replay metrics. Both use the source’s hierarchy and tokens and do not alter the primary composition.

## Required Fidelity Surfaces

- Fonts and typography: Newsreader Variable reproduces the source’s editorial scientific serif, while Manrope Variable handles compact controls, labels, and telemetry. Display scale, small caps, weights, line height, and wrapping closely match the source at 1440px. Mobile text remains readable without truncation.
- Spacing and layout rhythm: the 66/34 replay-to-journal split, full-width bottom timeline, upper progress chart, journal narrative sequence, lineage area, selected-generation summary, and primary action match the source’s major-region proportions. Desktop document size is 1440 × 1024 with no overflow. Mobile stacks the replay, journal, and timeline with no horizontal overflow.
- Colors and visual tokens: deep green-black surfaces, parchment copy, luminous green selection, amber fitness, blue coverage, fine low-contrast rules, and critical state contrast map directly to the chosen visual direction.
- Image quality and asset fidelity: the visual hero remains Axiom’s real antialiased WebGL creature and terrain renderer, not a screenshot or placeholder. Existing motion ghosts, terrain lighting, particle field, scan rings, and live camera are preserved. Icons come from Phosphor rather than custom SVG, CSS drawings, emoji, or text substitutes.
- Copy and content: all journal metrics are derived from the active replay response. Generation, fitness, archive coverage, occupied cells, body segments, joints, stable distance, stability, controller, terrain, and search gain remain truthful to the simulation.

## Primary Interactions Tested

- Selected Generation 6 from the timeline; the summary updated to `Fitness 87.98` and `Coverage 10%`.
- Ran “Evolve next generation”; the application advanced from Generation 12 to Generation 13, reran evolution, selected the new champion, and updated the progress chart, labels, lineage, journal, and timeline.
- Changed terrain from Rough to Flat; the application reran the search and updated the Field Journal terrain state to `Flat`.
- Pause/play, camera-mode, effects, 2D replay link, drag-to-orbit, and wheel-to-zoom remain live controls.
- Loading, ready, and error treatments exist for replay generation.
- Final browser console errors and warnings: none.

## Responsive And Accessibility Verification

- Desktop viewport: 1440 × 1024; document width 1440; document height 1024; no horizontal or vertical overflow.
- Mobile viewport: 375 × 844; document width 375; no horizontal overflow; the complete experience stacks to 1682px and remains scrollable.
- Core controls are native buttons, links, and labeled selects with visible focus treatments.
- Decorative icon glyphs are hidden from accessible names.
- Primary controls remain reachable and visible at 375px.
- Motion respects `prefers-reduced-motion`; live simulation playback starts paused when reduced motion is requested.

## Comparison History

### Pass 1

- [P2] The journal reported `+0.0% fitness gain` because only the final two generations were compared, making a successful evolution run appear stagnant.
- [P2] The controller label rendered as `Cpg`, drifting from the selected source’s `CPG` technical naming.
- [P2] Decorative icon-font glyphs leaked into button accessible names, creating noisy spoken labels.
- Fixes made: changed the primary delta to the real total search gain (`+97.34` for the comparison state), added explicit CPG capitalization, and hid decorative icons from accessibility APIs while retaining visible Phosphor artwork.
- Post-fix evidence: `axiom-field-journal-final.png`, `axiom-field-journal-comparison.png`, and `axiom-field-journal-focus-comparison.png`.

### Pass 2

- Side-by-side full-view and focused-region comparison found no remaining actionable P0, P1, or P2 mismatch.
- The final implementation preserved the selected composition while keeping live simulation data, responsive behavior, keyboard semantics, and the end-to-end evolution action functional.

## Verification

- `node --check web/graphics3d.js`: passed.
- `cargo test --workspace`: 31 passed, 0 failed.
- `cargo run -- gui --check`: passed.
- Browser interaction journey: passed.
- Browser console errors/warnings: none.
- Full-view and focused source/implementation comparisons: completed.

final result: passed
