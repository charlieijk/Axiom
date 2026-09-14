use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMPORARY_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn cli_saves_inspects_and_replays_a_checkpoint() {
    let checkpoint_path = temporary_checkpoint_path();
    let handoff_path = checkpoint_path.with_extension("shortlist.json");
    let binary = env!("CARGO_BIN_EXE_axiom");

    let evolve = Command::new(binary)
        .args([
            "evolve",
            "--generations",
            "2",
            "--population",
            "6",
            "--steps",
            "16",
            "--seed",
            "71",
            "--save",
        ])
        .arg(&checkpoint_path)
        .output()
        .expect("evolve command should start");
    assert!(
        evolve.status.success(),
        "evolve failed: {}",
        String::from_utf8_lossy(&evolve.stderr)
    );
    assert!(checkpoint_path.is_file());

    let inspect = Command::new(binary)
        .arg("inspect")
        .arg(&checkpoint_path)
        .output()
        .expect("inspect command should start");
    assert!(inspect.status.success());
    let inspection = String::from_utf8_lossy(&inspect.stdout);
    assert!(inspection.contains("checkpoint version: 1"));
    assert!(inspection.contains("lineage records:"));

    let replay = Command::new(binary)
        .args(["animate", "--checkpoint"])
        .arg(&checkpoint_path)
        .args(["--frames", "1", "--fps", "0", "--no-clear"])
        .output()
        .expect("checkpoint replay should start");
    assert!(
        replay.status.success(),
        "replay failed: {}",
        String::from_utf8_lossy(&replay.stderr)
    );
    assert!(String::from_utf8_lossy(&replay.stdout).contains("time"));

    let handoff = Command::new(binary)
        .arg("handoff")
        .arg(&checkpoint_path)
        .arg("--output")
        .arg(&handoff_path)
        .output()
        .expect("candidate handoff should start");
    assert!(
        handoff.status.success(),
        "handoff failed: {}",
        String::from_utf8_lossy(&handoff.stderr)
    );
    let handoff_json = fs::read_to_string(&handoff_path).expect("handoff JSON should exist");
    let handoff_value: serde_json::Value =
        serde_json::from_str(&handoff_json).expect("handoff should be valid JSON");
    assert_eq!(
        handoff_value["schema_version"],
        "axiom.candidate-shortlist.v1"
    );
    assert_eq!(handoff_value["source"]["seed"], 71);
    let candidates = handoff_value["candidates"]
        .as_array()
        .expect("handoff should contain candidates");
    assert!(!candidates.is_empty());
    assert!(
        candidates[0]["roles"]
            .as_array()
            .is_some_and(|roles| roles.iter().any(|role| role == "champion"))
    );
    assert!(candidates[0]["genome"]["body"]["nodes"].is_array());
    assert!(
        handoff_value["limitations"]
            .as_array()
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item.as_str().is_some_and(|text| text.contains("hardware")))
            })
    );

    let destructive_handoff = Command::new(binary)
        .arg("handoff")
        .arg(&checkpoint_path)
        .arg("--output")
        .arg(&checkpoint_path)
        .output()
        .expect("same-path handoff should start");
    assert!(!destructive_handoff.status.success());
    assert!(
        String::from_utf8_lossy(&destructive_handoff.stderr)
            .contains("must differ from the checkpoint")
    );

    let inspect_after_refusal = Command::new(binary)
        .arg("inspect")
        .arg(&checkpoint_path)
        .output()
        .expect("checkpoint should remain readable after refused handoff");
    assert!(inspect_after_refusal.status.success());

    fs::remove_file(checkpoint_path).expect("test checkpoint should be removable");
    fs::remove_file(handoff_path).expect("test handoff should be removable");
}

#[test]
fn cli_rejects_a_checkpoint_with_an_inconsistent_archive_shape() {
    let checkpoint_path = temporary_checkpoint_path();
    let binary = env!("CARGO_BIN_EXE_axiom");
    let evolve = Command::new(binary)
        .args([
            "evolve",
            "--generations",
            "1",
            "--population",
            "4",
            "--steps",
            "12",
            "--seed",
            "73",
            "--save",
        ])
        .arg(&checkpoint_path)
        .output()
        .expect("evolve command should start");
    assert!(
        evolve.status.success(),
        "evolve failed: {}",
        String::from_utf8_lossy(&evolve.stderr)
    );

    let mut checkpoint: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&checkpoint_path).expect("checkpoint JSON should exist"),
    )
    .expect("checkpoint should begin as valid JSON");
    let width = checkpoint["report"]["archive"]["width"]
        .as_u64()
        .expect("archive width should be an integer");
    checkpoint["report"]["archive"]["width"] = serde_json::Value::from(width + 1);
    fs::write(
        &checkpoint_path,
        serde_json::to_vec_pretty(&checkpoint).expect("corrupt checkpoint should serialize"),
    )
    .expect("corrupt checkpoint should be written");

    let inspection = Command::new(binary)
        .arg("inspect")
        .arg(&checkpoint_path)
        .output()
        .expect("inspect command should start");
    let stderr = String::from_utf8_lossy(&inspection.stderr).into_owned();
    fs::remove_file(checkpoint_path).expect("test checkpoint should be removable");

    assert!(!inspection.status.success());
    assert!(
        stderr.contains("archive grid") && stderr.contains("does not match"),
        "unexpected checkpoint error: {stderr}"
    );
}

#[test]
fn cli_exports_rough_inspection_reports() {
    let json_path = temporary_checkpoint_path().with_extension("report.json");
    let markdown_path = temporary_checkpoint_path().with_extension("report.md");
    let binary = env!("CARGO_BIN_EXE_axiom");

    let evolve = Command::new(binary)
        .args([
            "evolve",
            "--pack",
            "rough-inspection",
            "--generations",
            "1",
            "--population",
            "4",
            "--steps",
            "12",
            "--report-json",
        ])
        .arg(&json_path)
        .arg("--report-md")
        .arg(&markdown_path)
        .output()
        .expect("reporting evolution should start");
    assert!(
        evolve.status.success(),
        "evolve failed: {}",
        String::from_utf8_lossy(&evolve.stderr)
    );

    let json = fs::read_to_string(&json_path).expect("JSON report should exist");
    let value: serde_json::Value = serde_json::from_str(&json).expect("report is valid JSON");
    assert_eq!(value["report_version"], 2);
    assert_eq!(value["evaluation"]["task_pack"], "rough-inspection");
    assert!(
        value["history"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );

    let markdown = fs::read_to_string(&markdown_path).expect("Markdown report should exist");
    assert!(markdown.contains("Rough inspection"));
    assert!(markdown.contains("step field"));

    fs::remove_file(json_path).expect("JSON report should be removable");
    fs::remove_file(markdown_path).expect("Markdown report should be removable");
}

fn temporary_checkpoint_path() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should follow epoch")
        .as_nanos();
    let counter = TEMPORARY_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "axiom-cli-{}-{unique}-{counter}.json",
        std::process::id()
    ))
}
