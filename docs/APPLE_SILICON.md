# Apple Silicon download and source build

Axiom runs natively on Apple Silicon as an `aarch64-apple-darwin` command-line
binary and opens its interactive interface through a loopback-only local web
server. It is not a conventional `.app` bundle.

## Download a tagged release

Each stable tag that completes the release workflow publishes a versioned
archive and a separate SHA-256 checksum on the
[GitHub Releases page](https://github.com/charlieijk/Axiom/releases). The tag,
crate version, and compiled binary version must agree before that workflow can
publish anything. The release page is the source of truth: a version is not
available as a prebuilt download until both files appear there.

For Axiom 0.1.1 on an Apple Silicon Mac with the GitHub CLI installed:

```sh
AXIOM_TAG=v0.1.1
AXIOM_ARCHIVE="axiom-${AXIOM_TAG}-aarch64-apple-darwin.tar.gz"
gh release download "$AXIOM_TAG" --repo charlieijk/Axiom \
  --pattern "$AXIOM_ARCHIVE" --pattern "$AXIOM_ARCHIVE.sha256"
shasum -a 256 -c "$AXIOM_ARCHIVE.sha256"
tar -xzf "$AXIOM_ARCHIVE"
"${AXIOM_ARCHIVE%.tar.gz}/axiom" --version
"${AXIOM_ARCHIVE%.tar.gz}/axiom" gui
```

Open the loopback address printed by the final command. The archive includes
the README, security policy, install guide, MIT license, and licenses for the
browser assets embedded in the executable.

The release binary is built on a native GitHub-hosted Arm64 runner and receives
an ad-hoc code signature. It is not signed with an Apple Developer ID and is
not notarized. Organizations that require Developer ID provenance should use
the source-build path below until a notarized release channel exists.

## Build and run

Prerequisites: Git and Rust 1.85 or newer on an Apple Silicon Mac.

```sh
git clone https://github.com/charlieijk/Axiom.git
cd Axiom
uname -m
cargo test --locked --test arm64_evidence
cargo run --release --locked --bin axiom -- gui
```

`uname -m` should print `arm64`. Open the printed address with `/3d` appended,
normally `http://127.0.0.1:8787/3d`. If port 8787 is occupied, Axiom probes the
next 19 ports and prints the one it selected. Press Control-C in the terminal
to stop the server.

The 2D interface and the Three.js-powered 3D Field Journal are embedded and
work offline. The 3D page requests cosmetic fonts and icons from public CDNs,
but its controls, text, simulation, and rendering do not depend on them.

## Evidence-oriented check

The full source-backed reviewer path, expected digests, raw allocation reports,
and limitations are documented in
[ARM64_OPTIMIZATION.md](ARM64_OPTIMIZATION.md#arm64-judge-quick-check). The
repository also includes the MIT license, the vendored Three.js license, and
the raw baseline, optimized, and comparison JSON under `docs/benchmarks/`.

The compiled executable embeds Axiom's Rust replay server and 2D/3D browser
clients. It does not bundle checkpoints, credentials, or user data.
