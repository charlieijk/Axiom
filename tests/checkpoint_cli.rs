use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn cli_saves_inspects_and_replays_a_checkpoint() {
    let checkpoint_path = temporary_checkpoint_path();
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

    fs::remove_file(checkpoint_path).expect("test checkpoint should be removable");
}

fn temporary_checkpoint_path() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should follow epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("axiom-cli-{unique}.json"))
}
