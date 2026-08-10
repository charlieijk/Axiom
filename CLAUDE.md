# Axiom

## Running the UI

`cargo run -- gui` serves the replay browser on http://127.0.0.1:8787 (falls
forward to the next free port; pass `--port` to pick one). The 3D evolution
field journal lives at `/3d`; `cargo run -- gui --check` smoke-tests the
assets without serving.

## UI work checklist

Every UI change ends with looking at the rendered result — not with "the code looks right."

- **See it**: start the dev server (configured in `.claude/launch.json`), open the page, and verify the change visually. Finish with a screenshot.
- **Async states**: every async view has loading, empty, and error states — not just the happy path.
- **Keyboard**: interactive elements have visible focus styles; the primary flow works with Tab alone.
- **Responsive**: verify at 375px wide as well as desktop.
- **Motion**: respect `prefers-reduced-motion`; nothing essential is conveyed by animation alone.
- **Hierarchy**: real typographic scale — headings, body, and metadata are visually distinct, not uniform 14px gray.
- **Dark mode**: honor `prefers-color-scheme` where the app supports it.
