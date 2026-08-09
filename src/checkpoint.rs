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
