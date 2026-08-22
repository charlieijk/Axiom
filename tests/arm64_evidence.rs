use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::Value;

const BASELINE: &str = include_str!("../docs/benchmarks/arm64-allocation-baseline.json");
const OPTIMIZED: &str = include_str!("../docs/benchmarks/arm64-allocation-optimized.json");
const COMPARISON: &str = include_str!("../docs/benchmarks/comparison.json");
const RESULT_CARD: &str = include_str!("../docs/axiom-arm64-result.svg");
const ASSET_HASHES: &str = include_str!("../docs/submission-assets.sha256");
const BASELINE_REVISION: &str = "71d3897dc04f90a4903a14fd737af1ad680cef7c";

#[test]
fn checked_in_arm64_comparison_matches_raw_artifacts() {
    let baseline = parse(BASELINE);
    let optimized = parse(OPTIMIZED);
    let comparison = parse(COMPARISON);

    assert_eq!(baseline["schema_version"], 2);
    assert_eq!(optimized["schema_version"], 2);
    assert_eq!(baseline["authoritative_arm64"], true);
    assert_eq!(optimized["authoritative_arm64"], true);
    assert_eq!(baseline["workload"], optimized["workload"]);
    assert_eq!(
        baseline["environment"]["benchmark_source_sha256"],
        optimized["environment"]["benchmark_source_sha256"]
    );
    assert_eq!(
        optimized["environment"]["benchmark_source_sha256"],
        sha256(Path::new("examples/arm64_optimization_benchmark.rs"))
    );
    assert_eq!(
        baseline["environment"]["cargo_lock_sha256"],
        optimized["environment"]["cargo_lock_sha256"]
    );
    if repository_checkout() {
        let baseline_lock = format!("{BASELINE_REVISION}:Cargo.lock");
        let bytes = git_output(&["show", &baseline_lock])
            .unwrap_or_else(|| panic!("baseline Git object should exist: {baseline_lock}"));
        assert_eq!(
            optimized["environment"]["cargo_lock_sha256"],
            sha256_bytes(&bytes),
            "checked-in evidence does not match the recorded baseline lockfile"
        );
    }

    let baseline_digest = &baseline["deterministic_outcome"]["full_report_digest"];
    let optimized_digest = &optimized["deterministic_outcome"]["full_report_digest"];
    assert_eq!(baseline_digest, optimized_digest);
    assert_eq!(baseline_digest, "be0a71bc9f057db4");
    assert_eq!(
        baseline["deterministic_outcome"]["full_report_sha256"],
        optimized["deterministic_outcome"]["full_report_sha256"]
    );
    assert_eq!(
        comparison["results"]["semantic_equivalence"]["fnv1a64"],
        *baseline_digest
    );
    assert_eq!(
        comparison["results"]["semantic_equivalence"]["sha256"],
        baseline["deterministic_outcome"]["full_report_sha256"]
    );

    assert_metric(
        &baseline,
        &optimized,
        &comparison,
        "calls",
        "allocation_calls",
    );
    assert_metric(
        &baseline,
        &optimized,
        &comparison,
        "requested_bytes",
        "requested_bytes",
    );

    assert_recorded_check(
        &comparison,
        "authoritative_arm64",
        baseline["authoritative_arm64"] == true && optimized["authoritative_arm64"] == true,
    );
    for (check, field) in [
        ("same_hardware_model", "hardware_model"),
        ("same_cpu_brand", "cpu_brand"),
        ("same_os_version", "os_version"),
        ("same_rustc", "rustc"),
        ("same_cargo_lock_sha256", "cargo_lock_sha256"),
        ("same_benchmark_source_sha256", "benchmark_source_sha256"),
    ] {
        assert_recorded_check(
            &comparison,
            check,
            baseline["environment"][field] == optimized["environment"][field],
        );
    }
    assert_recorded_check(
        &comparison,
        "same_workload",
        baseline["workload"] == optimized["workload"],
    );
    assert_recorded_check(
        &comparison,
        "exact_full_report_digest",
        baseline["deterministic_outcome"]["full_report_digest"]
            == optimized["deterministic_outcome"]["full_report_digest"],
    );
    assert_recorded_check(
        &comparison,
        "exact_full_report_sha256",
        baseline["deterministic_outcome"]["full_report_sha256"]
            == optimized["deterministic_outcome"]["full_report_sha256"],
    );

    for source in ["fitness", "network", "policy", "simulation"] {
        let path = format!("src/{source}.rs");
        assert_eq!(
            optimized["environment"]["optimization_sources_sha256"][source],
            sha256(Path::new(&path)),
            "optimized evidence is stale for {path}"
        );
    }
}

#[test]
fn baseline_source_hashes_match_the_recorded_git_revision() {
    let baseline = parse(BASELINE);
    assert_eq!(
        baseline["environment"]["source_revision"],
        BASELINE_REVISION
    );

    if !repository_checkout() {
        eprintln!("baseline Git-object check skipped for a packaged source tree");
        return;
    }
    let baseline_commit = format!("{BASELINE_REVISION}^{{commit}}");
    assert!(
        git_output(&["cat-file", "-e", &baseline_commit]).is_some(),
        "repository checkout is missing baseline commit {BASELINE_REVISION}; fetch full history"
    );

    for source in ["fitness", "network", "policy", "simulation"] {
        let path = format!("src/{source}.rs");
        let spec = format!("{BASELINE_REVISION}:{path}");
        let bytes = git_output(&["show", &spec])
            .unwrap_or_else(|| panic!("baseline Git object should exist: {spec}"));
        assert_eq!(
            baseline["environment"]["optimization_sources_sha256"][source],
            sha256_bytes(&bytes),
            "recorded baseline hash does not match {spec}"
        );
    }
}

#[test]
fn judge_assets_match_the_checked_in_arm64_evidence() {
    let comparison = parse(COMPARISON);
    let allocation_calls = &comparison["results"]["allocation_calls"];
    let requested_bytes = &comparison["results"]["requested_bytes"];
    let equivalence = &comparison["results"]["semantic_equivalence"];

    assert_eq!(equivalence["serialized_report_bytes"], 1_710_519);
    assert_eq!(equivalence["lineage_entries"], 3_872);

    for expected in [
        format!("{}% FEWER", allocation_calls["reduction_percent"]),
        "13,000,529".to_string(),
        "730,613".to_string(),
        format!("{}% FEWER", requested_bytes["reduction_percent"]),
        "2.146 GB".to_string(),
        "220.5 MB".to_string(),
        "1,710,519 bytes".to_string(),
        "3,872 lineage records".to_string(),
    ] {
        assert!(
            RESULT_CARD.contains(&expected),
            "result card is missing evidence value {expected}"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for asset in [
        "docs/axiom-arm64-result.png",
        "docs/axiom-archive-lab-gen12.jpg",
        "docs/axiom-archive-elite-cell-0-7.jpg",
        "docs/axiom-archive-lab-gen13.jpg",
        "docs/axiom-core-loop.png",
        "docs/axiom-architecture.png",
    ] {
        assert!(
            root.join(asset).is_file(),
            "judge asset is missing: {asset}"
        );
    }

    for line in ASSET_HASHES.lines() {
        let (expected, path) = line
            .split_once("  ")
            .expect("asset checksum lines should use sha256sum format");
        assert_eq!(
            sha256(root.join(path).as_path()),
            expected,
            "judge asset checksum changed: {path}"
        );
    }
}

fn parse(source: &str) -> Value {
    serde_json::from_str(source).expect("checked-in benchmark evidence should be valid JSON")
}

fn assert_recorded_check(comparison: &Value, name: &str, actual: bool) {
    assert!(actual, "derived compatibility check failed: {name}");
    assert_eq!(
        comparison["compatibility_checks"][name], true,
        "recorded compatibility flag disagrees with derived result: {name}"
    );
}

fn assert_metric(
    baseline: &Value,
    optimized: &Value,
    comparison: &Value,
    raw_name: &str,
    comparison_name: &str,
) {
    let baseline_value = baseline["allocations"][raw_name]
        .as_u64()
        .expect("baseline metric should be an unsigned integer");
    let optimized_value = optimized["allocations"][raw_name]
        .as_u64()
        .expect("optimized metric should be an unsigned integer");
    let result = &comparison["results"][comparison_name];

    assert_eq!(result["baseline"], baseline_value);
    assert_eq!(result["optimized"], optimized_value);
    assert_eq!(result["removed"], baseline_value - optimized_value);

    let recorded_reduction = result["reduction_percent"]
        .as_f64()
        .expect("reduction should be numeric");
    let actual_reduction = 100.0 * (1.0 - optimized_value as f64 / baseline_value as f64);
    assert!((recorded_reduction - actual_reduction).abs() < 0.01);
}

fn sha256(path: &Path) -> String {
    let path = path
        .to_str()
        .expect("evidence paths should contain valid UTF-8");
    let commands = [
        ("shasum", ["-a", "256", path]),
        ("sha256sum", [path, "", ""]),
    ];

    for (program, arguments) in commands {
        let arguments: Vec<_> = arguments
            .into_iter()
            .filter(|argument| !argument.is_empty())
            .collect();
        if let Ok(output) = Command::new(program).args(arguments).output() {
            if output.status.success() {
                return String::from_utf8(output.stdout)
                    .expect("checksum output should be UTF-8")
                    .split_whitespace()
                    .next()
                    .expect("checksum output should contain a digest")
                    .to_string();
            }
        }
    }

    panic!("shasum or sha256sum is required to validate benchmark evidence");
}

fn sha256_bytes(bytes: &[u8]) -> String {
    for (program, arguments) in [("shasum", vec!["-a", "256"]), ("sha256sum", vec![])] {
        let mut child = match Command::new(program)
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => continue,
        };
        child
            .stdin
            .take()
            .expect("digest stdin should be piped")
            .write_all(bytes)
            .expect("digest input should be writable");
        let output = child
            .wait_with_output()
            .expect("digest process should finish");
        if output.status.success() {
            return String::from_utf8(output.stdout)
                .expect("checksum output should be UTF-8")
                .split_whitespace()
                .next()
                .expect("checksum output should contain a digest")
                .to_string();
        }
    }
    panic!("shasum or sha256sum is required to validate benchmark evidence");
}

fn repository_checkout() -> bool {
    Path::new(".git").exists()
}

fn git_output(arguments: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git").args(arguments).output().ok()?;
    output.status.success().then_some(output.stdout)
}
