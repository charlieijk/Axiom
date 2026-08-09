# Axiom Field Lab

A rigid-body simulator for one fixed, buildable robot: an 8-DOF hobby-servo
quadruped. It is the honest counterpart to Axiom's own sandbox.

Start with the reviewer-ready walkthrough in [DEMO.md](DEMO.md).

## Why this exists

Axiom's `simulation.rs` is an abstraction, not a physics model. A creature is a
single point mass, its limbs carry no dynamics, and forward motion comes
straight out of the action value:

```rust
thrust += action * node.actuator_strength * normal.x.abs().max(0.35);
```

There is no contact, no ground reaction, no limb-ground interaction. A constant
saturated output is close to optimal, and no gait is required. That is a fine
search toy and a useless model of hardware — a controller evolved against it has
no claim on a physical robot.

Here, forward motion can only come from joint torque acting through foot
contact. The robot has a chassis, four legs of two links each, eight revolute
joints, and one modelled servo per joint. If it moves, it walked.

## The robot

Fixed morphology, because evolving bodies would mean building a new robot per
generation. Frame is `x` forward, `y` up, `z` right. Each leg hinges only in its
own sagittal plane: a hip that swings the thigh fore and aft, and a knee that
folds the shank.

Legs are ordered front-right, front-left, rear-right, rear-left, and actuators
follow in the same order with the hip before its knee, so action `2 * leg` is a
hip and `2 * leg + 1` is its knee.

## Servos are position sources, not torque sources

A hobby servo accepts a *position* command, drives toward it with an internal
loop, cannot slew faster than its rated speed, and cannot exceed its stall
torque. All three are modelled, and the torque cap is handed to the solver as a
motor force limit.

Two things here are easy to get wrong and both are load-bearing:

- The joint motors use `MotorModel::ForceBased`, not rapier's
  `AccelerationBased` default. Only the force-based model interprets stiffness
  and the force cap in the N·m units a servo datasheet is written in. Under the
  default model `max_torque_nm` is decorative — it can be raised a hundredfold
  and the result does not move. `examples/motor_probe.rs` reproduces this in
  isolation.
- The neutral pose is a crouch, not straight-down legs. Straight legs are a
  singular configuration: the chassis is an inverted pendulum over the hinges
  and collapses under any perturbation. A zero action means "stand".

## Everything physical lives in robot.toml

No physical constant is hardcoded. Standing height is *derived* from the stance
and link geometry, including the capsule cap at the foot, so changing a leg
length cannot silently spawn the robot inside the floor. The parameter loader
rejects geometry the engine cannot represent rather than producing a
plausible-looking meaningless run.

**The bundled parameters are uncalibrated.** They are vendor figures and
estimates for an MG90S-class robot that has not been built. Every command says
so. When hardware exists, calibration is a diff of `robot.toml` and a re-record
of the golden trajectory — never a code change.

## Determinism, and its limit

Two claims, deliberately not conflated:

- **Within one binary**, identical inputs produce bit-identical output.
  Asserted exactly, no tolerance. This is the strong claim, and it is what
  makes an evolutionary search over this model reproducible.
- **Across architectures, they do not agree.** Legged contact is close to
  chaotic, so floating-point differences between x86_64 and aarch64 amplify
  rather than average out — the same commit diverges by about 10 mm within two
  seconds of walking. This was found by CI disagreeing with a laptop, not
  assumed.

So the committed golden anchors *behaviour* — it walks, forward, upright,
roughly this far — inside bands sized for solver noise. It cannot detect a
small parameter change, and does not pretend to. Sharp detection is a separate
test that records two runs in one binary and compares them exactly; that one
catches a 2% chassis-mass change.

`verify` uses the pointwise comparison, so it is a same-machine tool: use it to
check a replay on the hardware that produced the recording.

## What the reference gait does not exercise

Halving the rated slew rate or the stall torque changes the committed run *not
at all* — at these amplitudes the gait demands about 2.5 rad/s against a rated
5.2, and never approaches the torque cap. A test pins this.

It matters for transfer: the servo parameters most likely to be wrong on real
hardware are precisely the ones this reference run cannot discriminate. A
transfer study needs a faster or more heavily loaded gait to probe them.

## Usage

```sh
cargo run -p axiom-field -- stand
cargo run -p axiom-field -- gait --ticks 200
cargo run -p axiom-field -- gait --freq 2.0 --hip 0.15 --knee 0.12
cargo run -p axiom-field -- record --out tests/golden/trot-nominal.json --ticks 200 --stride 10
cargo run -p axiom-field -- verify crates/axiom-field/tests/golden/trot-nominal.json
```

Gait parameters are flags precisely so that exploring them never requires
editing source — which is how the committed defaults were chosen.

## What this is not

Not a transfer result. Nothing here has touched hardware, so the crate makes no
claim about sim-to-real accuracy. It is the model that a transfer measurement
would later be run against.

## Proof gate

```sh
cargo test -p axiom-field
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

## License

MIT — see the repository [LICENSE](../../LICENSE).
