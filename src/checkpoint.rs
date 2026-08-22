use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{EvolutionConfig, EvolutionReport};

pub const CHECKPOINT_VERSION: u32 = 1;

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

    let temporary_path = temporary_path_for(path);
    let result = (|| {
        let mut file = File::create(&temporary_path)?;
        file.write_all(checkpoint.to_json()?.as_bytes())?;
        file.sync_all()?;
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

fn temporary_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("axiom-checkpoint");
    let unique = unix_time_millis(SystemTime::now()).unwrap_or_default();
    path.with_file_name(format!(".{file_name}.{unique}.tmp"))
}

fn unix_time_millis(time: SystemTime) -> Option<u64> {
    let millis = time.duration_since(UNIX_EPOCH).ok()?.as_millis();
    u64::try_from(millis).ok()
}

#[cfg(test)]
mod tests {
    use crate::{
        EvolutionConfig,
        checkpoint::{CHECKPOINT_VERSION, EvolutionCheckpoint},
        evolution::run_evolution,
    };

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
}
