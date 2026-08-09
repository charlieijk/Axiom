# Field Lab Demo

## Promise

A controller that moves this robot has to actually walk it. Motion comes only
from joint torque through foot contact — there is no thrust term to exploit.

## Five-minute flow

### 1. Stand it up

```sh
cargo run -p axiom-field -- stand
```

The robot is released 5 mm above its own derived standing height and settles.
Expect roughly:

```
standing:   0.1277 m (derived)
rest:       0.1250 m
sag:        0.0077 m under its own weight
tilt:       0.0043 rad
```

The 7.7 mm sag is servo compliance under load — the position loop holding
against gravity with a finite torque budget. Note the standing height is
derived from the stance and link geometry, not written down anywhere.

### 2. Walk it

```sh
cargo run -p axiom-field -- gait --ticks 200
```

```
travel:     +0.9412 m forward
speed:      +0.0941 m/s
lateral:    +0.0115 m
UPRIGHT:    yes
```

Ten seconds of trot, 94 cm forward, about a centimetre of lateral drift.

### 3. Watch a bigger stride make it worse

```sh
cargo run -p axiom-field -- gait --ticks 200 --hip 0.45 --knee 0.35
```

```
travel:     +0.1575 m forward
tilt:       3.1408 rad
UPRIGHT:    NO — the robot fell; this travel is not walking
```

Amplitude is not throttle. The larger stride topples the robot, and it still
"travels" 16 cm — by sliding on its back. This is exactly the reading that a
distance-only fitness function would reward, which is why the CLI reports
whether the robot is still upright and why the golden test asserts it.

### 4. Reverse it

```sh
cargo run -p axiom-field -- gait --ticks 200 --knee-phase 0.25
```

The same robot walks backwards. Travel direction is set by the knee-to-hip
phase relationship, not by anything obvious in the geometry.

### 5. Verify the committed reference run

```sh
cargo run -p axiom-field -- verify crates/axiom-field/tests/golden/trot-nominal.json
```

```
verified:   21 samples match crates/axiom-field/tests/golden/trot-nominal.json
```

`verify` compares pointwise, so it is a same-machine check — it will fail on a
different architecture, because legged contact amplifies floating-point
differences rather than averaging them out. The test suite compares behaviour
instead, and keeps the sharp exact comparison for two runs from one binary.

Every command prints an `UNCALIBRATED` warning first. The bundled parameters
describe a robot nobody has built; the model is only as honest as `robot.toml`.

### 6. Evolve a repertoire

```sh
cargo run --release -p axiom-field -- evolve --seed 1 --out archive.json
```

```
reference:  trot walks 0.0669 m/s on these worlds (fell in 0 of 4)
archive:    43 of 96 cells (44.8% coverage)
best:       fitness 0.8367 m, speed 0.1175 m/s, cell (9, 0)
            worst world 0.6835 m, fell in 0 of 4 worlds
            +76% speed against the reference trot
```

Every genome is scored across four randomized worlds, and the hand-tuned trot
is scored on the same four — so the +76% is a measurement, not a claim. Note
the trot itself drops from 0.094 m/s in the nominal world to 0.067 m/s under
randomization: the reference gait is more brittle than the nominal number
suggests, which is the whole reason for scoring across an ensemble.

### 7. Check it on worlds it never trained on

```sh
cargo run --release -p axiom-field -- holdout archive.json --worlds 6
```

```
robust:     39 stood in all 6 unseen worlds (91%)
retained:   90% of training fitness on unseen worlds
```

This is the closest a software-only stage gets to a transfer test. It is a
proxy, not a measurement: it tests robustness to the perturbations this
simulator knows how to apply, which is not the same as robustness to being
real.

## Proof gate

```sh
cargo test -p axiom-field
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p axiom-field -- verify crates/axiom-field/tests/golden/trot-nominal.json
```
