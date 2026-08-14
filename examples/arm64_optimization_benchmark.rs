//! Reproducible benchmark for the allocation-reduced rollout hot path.
//!
//! Timing and allocation counting deliberately use different binaries. A
//! normal release build uses Rust's default allocator with no instrumentation:
//!
//! ```text
//! cargo run --release --example arm64_optimization_benchmark -- --full
//! ```
//!
//! Allocation counting is opt-in and does not report timings:
//!
//! ```text
//! cargo run --release --features allocation-counting \
//!   --example arm64_optimization_benchmark -- --full --allocations
//! ```

#[cfg(feature = "allocation-counting")]
use std::alloc::{GlobalAlloc, Layout, System};
#[cfg(feature = "allocation-counting")]
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::{
    env,
    hint::black_box,
    io::Write,
    path::Path,
    process::{self, Command, Stdio},
    time::Instant,
};

use axiom::{EvolutionConfig, EvolutionReport, SearchMode, TaskKind, run_evolution};
use serde_json::{Value, json};

#[cfg(feature = "allocation-counting")]
struct TrackingAllocator;

#[cfg(feature = "allocation-counting")]
static TRACK_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "allocation-counting")]
static ALLOCATION_CALLS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "allocation-counting")]
static REQUESTED_BYTES: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "allocation-counting")]
#[global_allocator]
static GLOBAL_ALLOCATOR: TrackingAllocator = TrackingAllocator;

#[cfg(feature = "allocation-counting")]
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarding the exact layout to the process system allocator.
        let pointer = unsafe { System.alloc(layout) };
        record_allocation(pointer, layout.size());
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarding the exact layout to the process system allocator.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        record_allocation(pointer, layout.size());
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the pointer and layout came from the same system allocator.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwarding the allocation and requested size unchanged.
        let resized = unsafe { System.realloc(pointer, layout, new_size) };
        record_allocation(resized, new_size);
        resized
    }
}

#[cfg(feature = "allocation-counting")]
fn record_allocation(pointer: *mut u8, bytes: usize) {
    if !pointer.is_null() && TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
        ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
        REQUESTED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Timing,
    Allocations,
}

#[derive(Clone, Copy)]
struct Workload {
    name: &'static str,
    population: usize,
    generations: usize,
    steps: usize,
    warmups: usize,
    iterations: usize,
}

impl Workload {
    fn full() -> Self {
        Self {
            name: "full",
            population: 128,
            generations: 40,
            steps: 400,
            warmups: 2,
            iterations: 7,
        }
    }

    fn quick() -> Self {
        Self {
            name: "quick",
            population: 32,
            generations: 8,
            steps: 160,
            warmups: 1,
            iterations: 3,
        }
    }

    fn evolution_config(self) -> EvolutionConfig {
        EvolutionConfig {
            seed: 1337,
            population_size: self.population,
            generations: self.generations,
            evaluation_steps: self.steps,
            task: TaskKind::RoughTerrain,
            controller: None,
            search_mode: SearchMode::MapElites,
            archive_width: 12,
            archive_height: 8,
        }
    }
}

struct Arguments {
    workload: Workload,
    mode: Mode,
    allow_non_arm64: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Outcome {
    best_fitness_bits: u32,
    stable_distance_bits: u32,
    archive_cells: usize,
    evaluated_genomes: usize,
}

impl Outcome {
    fn from_report(report: &EvolutionReport) -> Self {
        Self {
            best_fitness_bits: report.best_evaluation.fitness.to_bits(),
            stable_distance_bits: report.best_evaluation.metrics.stable_distance.to_bits(),
            archive_cells: report.archive.occupied_count(),
            evaluated_genomes: report.evaluated_count,
        }
    }
}

fn main() {
    let arguments = parse_arguments();
    validate_environment(&arguments);
    let config = arguments.workload.evolution_config();
    let (report, timing, allocations) = match arguments.mode {
        Mode::Timing => run_timing(arguments.workload, &config),
        Mode::Allocations => run_allocation_count(&config),
    };
    let outcome = Outcome::from_report(&report);
    assert_full_workload_golden(arguments.workload, &report);
    let serialized_report = serde_json::to_vec(&report).expect("evolution report should serialize");

    let report_json = json!({
        "schema_version": 2,
        "benchmark": "axiom-arm64-evolution",
        "mode": match arguments.mode { Mode::Timing => "timing", Mode::Allocations => "allocations" },
        "authoritative_arm64": env::consts::ARCH == "aarch64",
        "environment": environment_metadata(),
        "workload": workload_metadata(arguments.workload, &config),
        "timing": timing,
        "allocations": allocations,
        "deterministic_outcome": {
            "full_report_digest_algorithm": "fnv1a64-over-serde-json",
            "full_report_digest": format!("{:016x}", semantic_digest(&report)),
            "full_report_sha256": sha256_bytes(&serialized_report),
            "serialized_report_bytes": serialized_report.len(),
            "best_fitness_bits": outcome.best_fitness_bits,
            "stable_distance_bits": outcome.stable_distance_bits,
            "archive_cells": outcome.archive_cells,
            "evaluated_genomes": outcome.evaluated_genomes,
            "history_entries": report.history.len(),
            "lineage_entries": report.lineage.len(),
        },
        "reproduction": {
            "timing": "cargo run --release --example arm64_optimization_benchmark -- --full",
            "allocations": "cargo run --release --features allocation-counting --example arm64_optimization_benchmark -- --full --allocations",
            "actual_argv": env::args().collect::<Vec<_>>()
        }
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&report_json).expect("benchmark report should serialize")
    );
}

fn run_timing(workload: Workload, config: &EvolutionConfig) -> (EvolutionReport, Value, Value) {
    for _ in 0..workload.warmups {
        black_box(run_once(config));
    }

    let mut samples_ms = Vec::with_capacity(workload.iterations);
    let mut expected_report = None;
    for _ in 0..workload.iterations {
        let started = Instant::now();
        let report = run_once(config);
        let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
        assert_reproducible(&mut expected_report, report);
        samples_ms.push(elapsed_ms);
    }

    let mut ordered_samples = samples_ms.clone();
    ordered_samples.sort_by(f64::total_cmp);
    let mean_ms = samples_ms.iter().sum::<f64>() / samples_ms.len() as f64;
    let timing = json!({
        "instrumented_allocator": false,
        "samples_ms": samples_ms,
        "median_ms": ordered_samples[ordered_samples.len() / 2],
        "mean_ms": mean_ms,
        "min_ms": ordered_samples[0],
        "max_ms": ordered_samples[ordered_samples.len() - 1]
    });
    (
        expected_report.expect("at least one measured iteration"),
        timing,
        Value::Null,
    )
}

#[cfg(feature = "allocation-counting")]
fn run_allocation_count(config: &EvolutionConfig) -> (EvolutionReport, Value, Value) {
    ALLOCATION_CALLS.store(0, Ordering::Relaxed);
    REQUESTED_BYTES.store(0, Ordering::Relaxed);
    TRACK_ALLOCATIONS.store(true, Ordering::Relaxed);
    let report = run_once(config);
    TRACK_ALLOCATIONS.store(false, Ordering::Relaxed);
    let allocations = json!({
        "instrumented_allocator": true,
        "calls": ALLOCATION_CALLS.load(Ordering::Relaxed),
        "requested_bytes": REQUESTED_BYTES.load(Ordering::Relaxed)
    });
    (report, Value::Null, allocations)
}

#[cfg(not(feature = "allocation-counting"))]
fn run_allocation_count(_config: &EvolutionConfig) -> (EvolutionReport, Value, Value) {
    eprintln!(
        "--allocations requires a separately instrumented build; rerun with \
         --features allocation-counting"
    );
    process::exit(2);
}

fn parse_arguments() -> Arguments {
    let mut arguments = Arguments {
        workload: Workload::full(),
        mode: Mode::Timing,
        allow_non_arm64: false,
    };
    let mut values = env::args().skip(1);
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--quick" => arguments.workload = Workload::quick(),
            "--full" => arguments.workload = Workload::full(),
            "--allocations" => arguments.mode = Mode::Allocations,
            "--allow-non-arm64" => arguments.allow_non_arm64 = true,
            "--warmups" => arguments.workload.warmups = parse_count("--warmups", values.next()),
            "--iterations" => {
                arguments.workload.iterations = parse_count("--iterations", values.next())
            }
            "--help" | "-h" => {
                println!(
                    "Usage: cargo run --release --example arm64_optimization_benchmark -- \
                     [--quick|--full] [--warmups N] [--iterations N] \
                     [--allocations] [--allow-non-arm64]"
                );
                process::exit(0);
            }
            other => {
                eprintln!("unknown benchmark option: {other}");
                process::exit(2);
            }
        }
    }
    arguments
}

fn parse_count(option: &str, value: Option<String>) -> usize {
    let Some(value) = value else {
        eprintln!("{option} requires a positive integer");
        process::exit(2);
    };
    match value.parse::<usize>() {
        Ok(value) if value > 0 || option == "--warmups" => value,
        _ => {
            eprintln!("{option} requires a positive integer");
            process::exit(2);
        }
    }
}

fn validate_environment(arguments: &Arguments) {
    if cfg!(debug_assertions) {
        eprintln!("benchmark must be compiled with --release");
        process::exit(2);
    }
    if env::consts::ARCH != "aarch64" && !arguments.allow_non_arm64 {
        eprintln!(
            "authoritative evidence requires aarch64; use --allow-non-arm64 only for portability checks"
        );
        process::exit(2);
    }
    if cfg!(feature = "allocation-counting") && arguments.mode == Mode::Timing {
        eprintln!(
            "timing is disabled in allocation-counting builds because allocator instrumentation \
             perturbs the hot path"
        );
        process::exit(2);
    }
}

fn run_once(config: &EvolutionConfig) -> EvolutionReport {
    black_box(run_evolution(config.clone()).expect("benchmark config should be valid"))
}

fn assert_reproducible(expected: &mut Option<EvolutionReport>, actual: EvolutionReport) {
    match expected {
        Some(expected) => assert_eq!(
            expected, &actual,
            "complete fixed-seed evolution report changed between iterations"
        ),
        None => *expected = Some(actual),
    }
}

fn assert_full_workload_golden(workload: Workload, report: &EvolutionReport) {
    if workload.name != "full" {
        return;
    }
    let actual = Outcome::from_report(report);
    let baseline = Outcome {
        best_fitness_bits: 1_141_730_189,
        stable_distance_bits: 1_108_109_660,
        archive_cells: 60,
        evaluated_genomes: 5_120,
    };
    assert_eq!(
        actual, baseline,
        "full benchmark no longer matches the pre-optimization semantic baseline"
    );
    assert_eq!(
        semantic_digest(report),
        0xbe0a71bc9f057db4,
        "complete evolution report no longer matches the pre-optimization baseline"
    );
}

fn semantic_digest(report: &EvolutionReport) -> u64 {
    let serialized = serde_json::to_vec(report).expect("evolution report should serialize");
    serialized
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}

fn workload_metadata(workload: Workload, config: &EvolutionConfig) -> Value {
    json!({
        "name": workload.name,
        "seed": config.seed,
        "population": config.population_size,
        "generations": config.generations,
        "evaluation_steps": config.evaluation_steps,
        "reported_evaluated_genomes": config.population_size * config.generations,
        "simulation_calls": config.population_size * config.generations + 1,
        "rollout_steps": (config.population_size * config.generations + 1) * config.evaluation_steps,
        "timing_warmups": workload.warmups,
        "timing_iterations": workload.iterations,
        "threads": 1,
        "task": "rough-terrain",
        "controller": "mixed-feedforward-recurrent-cpg",
        "search": "map-elites",
        "archive": {
            "width": config.archive_width,
            "height": config.archive_height,
            "x_axis": "stable-distance",
            "y_axis": "body-count"
        }
    })
}

fn environment_metadata() -> Value {
    let executable = env::current_exe().ok();
    json!({
        "arch": env::consts::ARCH,
        "os": env::consts::OS,
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "package_version": env!("CARGO_PKG_VERSION"),
        "build_label": option_env!("AXIOM_BENCHMARK_VARIANT").unwrap_or("unspecified"),
        "source_revision": option_env!("AXIOM_SOURCE_REVISION"),
        "allocation_counting_feature": cfg!(feature = "allocation-counting"),
        "target_features": {
            "neon": cfg!(target_feature = "neon")
        },
        "hardware_model": command_output("sysctl", &["-n", "hw.model"]),
        "cpu_brand": command_output("sysctl", &["-n", "machdep.cpu.brand_string"]),
        "logical_cpus": command_output("sysctl", &["-n", "hw.logicalcpu"]),
        "physical_cpus": command_output("sysctl", &["-n", "hw.physicalcpu"]),
        "memory_bytes": command_output("sysctl", &["-n", "hw.memsize"]),
        "os_version": command_output("sw_vers", &["-productVersion"]),
        "kernel": command_output("uname", &["-srv"]),
        "rustc": command_output("rustc", &["-Vv"]),
        "cargo": command_output("cargo", &["-V"]),
        "git_commit": command_output("git", &["rev-parse", "HEAD"]),
        "git_dirty": command_output("git", &["status", "--porcelain"])
            .map(|status| !status.is_empty()),
        "cargo_lock_sha256": sha256(Path::new("Cargo.lock")),
        "benchmark_source_sha256": sha256(Path::new("examples/arm64_optimization_benchmark.rs")),
        "optimization_sources_sha256": {
            "fitness": sha256(Path::new("src/fitness.rs")),
            "network": sha256(Path::new("src/network.rs")),
            "policy": sha256(Path::new("src/policy.rs")),
            "simulation": sha256(Path::new("src/simulation.rs"))
        },
        "executable_sha256": executable.as_deref().and_then(sha256),
        "rustflags": env::var("RUSTFLAGS").ok(),
        "timestamp_utc": command_output("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"])
    })
}

fn command_output(program: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(program).args(arguments).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn sha256(path: &Path) -> Option<String> {
    let path = path.to_str()?;
    command_output("shasum", &["-a", "256", path])
        .or_else(|| command_output("sha256sum", &[path]))
        .and_then(|output| output.split_whitespace().next().map(str::to_string))
}

fn sha256_bytes(bytes: &[u8]) -> Option<String> {
    digest_bytes("shasum", &["-a", "256"], bytes).or_else(|| digest_bytes("sha256sum", &[], bytes))
}

fn digest_bytes(program: &str, arguments: &[&str], bytes: &[u8]) -> Option<String> {
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(bytes).ok()?;
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .map(str::to_string)
}
