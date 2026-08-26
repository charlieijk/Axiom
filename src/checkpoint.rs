use std::{
    collections::hash_map::RandomState,
    fs::{self, File, OpenOptions},
    hash::{BuildHasher, Hasher},
    io::{self, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use serde::{Deserialize, Serialize};

use crate::{EvolutionConfig, EvolutionReport};

pub const CHECKPOINT_VERSION: u32 = 1;
const TEMPORARY_FILE_ATTEMPTS: usize = 32;
static TEMPORARY_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EvolutionCheckpoint {
    pub version: u32,
    pub saved_at_unix_ms: Option<u64>,
    pub config: EvolutionConfig,
    pub report: EvolutionReport,
}

impl EvolutionCheckpoint {
    pub fn from_report(config: EvolutionConfig, report: EvolutionReport) -> Self {
        Self {
            version: CHECKPOINT_VERSION,
            saved_at_unix_ms: unix_time_millis(SystemTime::now()),
            config,
            report,
        }
    }

    pub fn to_json(&self) -> io::Result<String> {
        serde_json::to_string_pretty(self).map_err(io::Error::other)
    }

    pub fn from_json(json: &str) -> io::Result<Self> {
        #[derive(Deserialize)]
        struct VersionHeader {
            version: u32,
        }

        let header: VersionHeader = serde_json::from_str(json).map_err(io::Error::other)?;
        if header.version != CHECKPOINT_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported checkpoint version {}", header.version),
            ));
        }

        serde_json::from_str(json).map_err(io::Error::other)
    }
}

pub fn save_checkpoint(path: impl AsRef<Path>, checkpoint: &EvolutionCheckpoint) -> io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let (temporary_path, mut file) = create_temporary_file(path)?;
    let result = (|| {
        file.write_all(checkpoint.to_json()?.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary_path, path)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

pub fn load_checkpoint(path: impl AsRef<Path>) -> io::Result<EvolutionCheckpoint> {
    let json = fs::read_to_string(path)?;
    EvolutionCheckpoint::from_json(&json)
}

fn create_temporary_file(path: &Path) -> io::Result<(PathBuf, File)> {
    for _ in 0..TEMPORARY_FILE_ATTEMPTS {
        let temporary_path = temporary_path_for(path);
        match open_new_temporary_file(&temporary_path) {
            Ok(file) => return Ok((temporary_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "could not reserve a unique temporary checkpoint beside {}",
            path.display()
        ),
    ))
}

fn open_new_temporary_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    options.open(path)
}

fn temporary_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("axiom-checkpoint");
    let counter = TEMPORARY_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let random = operating_system_nonce().unwrap_or_else(fallback_nonce);
    path.with_file_name(format!(".{file_name}.{random:032x}-{counter:016x}.tmp"))
}

#[cfg(unix)]
fn operating_system_nonce() -> Option<u128> {
    let mut bytes = [0_u8; 16];
    let mut random = File::open("/dev/urandom").ok()?;
    std::io::Read::read_exact(&mut random, &mut bytes).ok()?;
    Some(u128::from_ne_bytes(bytes))
}

#[cfg(not(unix))]
fn operating_system_nonce() -> Option<u128> {
    None
}

fn fallback_nonce() -> u128 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u128(now);
    hasher.write_u32(process::id());
    hasher.write_u64(TEMPORARY_FILE_COUNTER.load(Ordering::Relaxed));
    let high = u128::from(hasher.finish()) << 64;
    let mut low_hasher = RandomState::new().build_hasher();
    low_hasher.write_u128(now);
    low_hasher.write_u32(process::id());
    high | u128::from(low_hasher.finish())
}

fn unix_time_millis(time: SystemTime) -> Option<u64> {
    let millis = time.duration_since(UNIX_EPOCH).ok()?.as_millis();
    u64::try_from(millis).ok()
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::atomic::AtomicU64};

    use crate::{
        EvolutionConfig,
        checkpoint::{
            CHECKPOINT_VERSION, EvolutionCheckpoint, load_checkpoint, open_new_temporary_file,
            save_checkpoint,
        },
        evolution::run_evolution,
    };

    static TEST_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn test_path(label: &str) -> std::path::PathBuf {
        let counter = TEST_PATH_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "axiom-checkpoint-test-{label}-{}-{counter}",
            std::process::id()
        ))
    }

    fn sample_checkpoint() -> EvolutionCheckpoint {
        let config = EvolutionConfig {
            seed: 91,
            population_size: 6,
            generations: 2,
            evaluation_steps: 16,
            archive_width: 5,
            archive_height: 4,
            ..EvolutionConfig::default()
        };
        let report = run_evolution(config.clone()).expect("valid config should run");
        EvolutionCheckpoint::from_report(config, report)
    }

    #[test]
    fn checkpoint_json_roundtrip_preserves_the_complete_run() {
        let config = EvolutionConfig {
            seed: 91,
            population_size: 6,
            generations: 2,
            evaluation_steps: 16,
            archive_width: 5,
            archive_height: 4,
            ..EvolutionConfig::default()
        };
        let report = run_evolution(config.clone()).expect("valid config should run");
        let checkpoint = EvolutionCheckpoint::from_report(config, report);

        let json = checkpoint.to_json().expect("checkpoint should serialize");
        let decoded = EvolutionCheckpoint::from_json(&json).expect("checkpoint should load");

        assert_eq!(decoded.version, CHECKPOINT_VERSION);
        assert_eq!(decoded.config.archive_width, 5);
        assert_eq!(decoded.config.archive_height, 4);
        assert_eq!(
            decoded.report.archive.occupied_count(),
            checkpoint.report.archive.occupied_count()
        );
        assert_eq!(decoded.report.history, checkpoint.report.history);
        assert_eq!(decoded.report.lineage, checkpoint.report.lineage);
        assert_eq!(decoded.report.best_genome, checkpoint.report.best_genome);
    }

    /// Pins the serialized field names the Parquet export reads.
    ///
    /// `script/export_checkpoints_parquet.py` and the recipes in
    /// `docs/duckdb.md` address these by name through DuckDB's JSON reader,
    /// which resolves them at query time: a rename here would surface as a
    /// "Could not find key" binder error during an export run rather than at
    /// compile time. This test moves that break back into `cargo test`.
    ///
    /// If a field is renamed deliberately, update the query in the exporter and
    /// the documented recipes, then update this list.
    #[test]
    fn checkpoint_json_exposes_the_columns_the_parquet_export_reads() {
        let config = EvolutionConfig {
            seed: 7,
            population_size: 6,
            generations: 2,
            evaluation_steps: 16,
            archive_width: 4,
            archive_height: 3,
            ..EvolutionConfig::default()
        };
        let report = run_evolution(config.clone()).expect("valid config should run");
        let checkpoint = EvolutionCheckpoint::from_report(config, report);
        let json: serde_json::Value =
            serde_json::from_str(&checkpoint.to_json().expect("checkpoint should serialize"))
                .expect("checkpoint JSON should parse");

        for pointer in [
            "/version",
            "/saved_at_unix_ms",
            "/config/seed",
            "/config/generations",
            "/config/population_size",
            "/config/evaluation_steps",
            "/config/task",
            "/config/search_mode",
            "/config/archive_width",
            "/config/archive_height",
            "/report/evaluated_count",
            "/report/history/0/generation",
            "/report/history/0/best_fitness",
            "/report/history/0/mean_fitness",
            "/report/history/0/archive_coverage",
            "/report/history/0/occupied_cells",
            "/report/archive/x_axis",
            "/report/archive/y_axis",
            "/report/archive/width",
            "/report/archive/height",
            "/report/archive/cells",
        ] {
            assert!(
                json.pointer(pointer).is_some(),
                "checkpoint JSON lost {pointer}, which script/export_checkpoints_parquet.py \
                 and docs/duckdb.md read by name"
            );
        }

        let elite = json
            .pointer("/report/archive/cells")
            .and_then(serde_json::Value::as_array)
            .expect("archive cells should serialize as an array")
            .iter()
            .find(|cell| !cell.is_null())
            .expect("a completed run should occupy at least one archive cell");

        for pointer in [
            "/cell/0",
            "/cell/1",
            "/genome_id",
            "/generation",
            "/mutation_summary",
            "/genome/controller",
            "/evaluation/fitness",
            "/evaluation/steps",
            "/evaluation/metrics/distance",
            "/evaluation/metrics/stable_distance",
            "/evaluation/metrics/jump_height",
            "/evaluation/metrics/uprightness",
            "/evaluation/metrics/stability",
            "/evaluation/metrics/body_count",
            "/evaluation/metrics/actuator_count",
            "/evaluation/metrics/energy",
        ] {
            assert!(
                elite.pointer(pointer).is_some(),
                "an archive elite lost {pointer}, which the Parquet export reads by name"
            );
        }

        // `parent_id` is null for a seed genome, so its presence is asserted as a
        // key rather than as a non-null value.
        assert!(
            elite
                .as_object()
                .expect("an elite should serialize as an object")
                .contains_key("parent_id"),
            "an archive elite lost parent_id, which the Parquet export reads by name"
        );
    }

    /// Guards the QD-score claim in `docs/duckdb.md`.
    ///
    /// The documented run-level QD-score is `SUM(fitness)` over the occupied
    /// cells of the exported archive. That is only equal to the search's own
    /// archive if empty cells serialize as nulls inside a flat `cells` array of
    /// exactly `width * height` entries, which is what the export's
    /// `UNNEST(...) WHERE elite IS NOT NULL` relies on.
    #[test]
    fn archive_cells_serialize_as_a_dense_nullable_grid() {
        let config = EvolutionConfig {
            seed: 11,
            population_size: 6,
            generations: 2,
            evaluation_steps: 16,
            archive_width: 5,
            archive_height: 4,
            ..EvolutionConfig::default()
        };
        let report = run_evolution(config.clone()).expect("valid config should run");
        let occupied = report.archive.occupied_count();
        let checkpoint = EvolutionCheckpoint::from_report(config, report);
        let json: serde_json::Value =
            serde_json::from_str(&checkpoint.to_json().expect("checkpoint should serialize"))
                .expect("checkpoint JSON should parse");

        let cells = json
            .pointer("/report/archive/cells")
            .and_then(serde_json::Value::as_array)
            .expect("archive cells should serialize as an array");

        assert_eq!(
            cells.len(),
            5 * 4,
            "the archive must serialize every cell so that empty cells stay addressable"
        );
        assert_eq!(
            cells.iter().filter(|cell| !cell.is_null()).count(),
            occupied,
            "the count of non-null serialized cells is the QD-score's summation domain"
        );
    }

    #[test]
    fn checkpoint_rejects_an_unknown_version() {
        let error = EvolutionCheckpoint::from_json(
            r#"{"version":999,"saved_at_unix_ms":null,"config":{},"report":{}}"#,
        )
        .expect_err("unknown checkpoint versions must be rejected");

        assert!(
            error
                .to_string()
                .contains("unsupported checkpoint version 999")
        );
    }

    #[test]
    fn checkpoint_save_uses_a_private_temporary_file_and_roundtrips() {
        let directory = test_path("roundtrip");
        let path = directory.join("run.json");
        let checkpoint = sample_checkpoint();

        save_checkpoint(&path, &checkpoint).expect("checkpoint should save atomically");
        let loaded = load_checkpoint(&path).expect("saved checkpoint should load");

        assert_eq!(loaded, checkpoint);
        let leftovers = fs::read_dir(&directory)
            .expect("checkpoint directory should exist")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);

        fs::remove_dir_all(directory).expect("test directory should be removable");
    }

    #[cfg(unix)]
    #[test]
    fn temporary_checkpoint_creation_refuses_a_preexisting_symlink() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let directory = test_path("symlink");
        fs::create_dir_all(&directory).expect("test directory should be created");
        let victim = directory.join("victim.json");
        let candidate = directory.join("candidate.tmp");
        fs::write(&victim, "do not replace").expect("victim should be created");
        symlink(&victim, &candidate).expect("test symlink should be created");

        let error = open_new_temporary_file(&candidate)
            .expect_err("exclusive temporary creation must reject a symlink");

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read_to_string(&victim).expect("victim should remain readable"),
            "do not replace"
        );

        fs::remove_file(&candidate).expect("test symlink should be removable");
        let private_candidate = directory.join("private.tmp");
        let file = open_new_temporary_file(&private_candidate)
            .expect("a fresh temporary path should be reserved");
        drop(file);
        assert_eq!(
            fs::metadata(&private_candidate)
                .expect("temporary file should exist")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        fs::remove_dir_all(directory).expect("test directory should be removable");
    }
}
