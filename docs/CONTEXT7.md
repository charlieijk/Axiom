# Context7 documentation map

Use version-qualified Context7 questions for Axiom's deterministic Rust core and its small browser viewer.

## Pinned stack

- Rust edition 2024 with MSRV 1.85.
- Serde 1.0.228 and serde_json 1.0.149 are the only crate dependencies.
- The GUI server uses `std::net::TcpListener`; it is not a web-framework application.
- The 3D viewer imports a checked-in Three.js r165 ES module and has no JavaScript package-manager dependency.

## Library map

| Context7 library ID | Canonical authoritative documentation | Recommended query topics |
| --- | --- | --- |
| `/websites/serde_rs` | [Serde documentation](https://serde.rs/) and [serde_json 1.0.149 API](https://docs.rs/serde_json/1.0.149/serde_json/) | Stable replay JSON, enum representations, defaults and aliases, custom serialization, schema evolution, and deterministic output caveats. |
| `/mrdoob/three.js` | [Three.js r165 source and docs](https://github.com/mrdoob/three.js/tree/r165/docs) | Loading the vendored ES module, cameras and controls, render loops, resize behavior, geometry/material disposal, and viewer performance. |
| `/rust-lang/reference` | [Rust 1.85 Reference](https://doc.rust-lang.org/1.85.0/reference/) | Edition-2024 language semantics, patterns, traits, concurrency guarantees, and migration-sensitive syntax. |

## Version warnings

- `/mrdoob/three.js` follows current development. Always say `Three.js r165` and verify any proposed API against the r165 tag.
- The Rust Reference documents the language, not Cargo or `std`. Use the [Cargo Reference](https://doc.rust-lang.org/cargo/reference/) and [Rust 1.85 standard library](https://doc.rust-lang.org/1.85.0/std/) for workspace and `TcpListener` questions.
- Preserve the deterministic Rust simulation as authoritative; Three.js is only a projection of replay data.
