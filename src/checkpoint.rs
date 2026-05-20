use std::{
    fs, io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{EvolutionConfig, EvolutionReport};

pub const CHECKPOINT_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EvolutionCheckpoint {
    pub version: u32,
    #[serde(default)]
    pub saved_at_unix_ms: Option<u64>,
    pub config: EvolutionConfig,
    pub report: EvolutionReport,
}

impl EvolutionCheckpoint {
    pub fn from_report(report: EvolutionReport) -> Self {
        Self {
            version: CHECKPOINT_VERSION,
            saved_at_unix_ms: unix_time_millis(SystemTime::now()),
            config: report.config.clone(),
            report,
        }
    }
}

pub fn save_checkpoint(path: impl AsRef<Path>, checkpoint: &EvolutionCheckpoint) -> io::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(checkpoint).map_err(io::Error::other)?;
    fs::write(path, json)
}

pub fn load_checkpoint(path: impl AsRef<Path>) -> io::Result<EvolutionCheckpoint> {
    let json = fs::read_to_string(path)?;
    serde_json::from_str(&json).map_err(io::Error::other)
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
        qd::Axis,
    };

    #[test]
    fn checkpoint_json_roundtrip_preserves_archive_and_lineage() {
        let report = run_evolution(EvolutionConfig {
            seed: 91,
            population_size: 6,
            generations: 2,
            evaluation_steps: 16,
            archive_width: 5,
            archive_height: 4,
            archive_x_axis: Axis::Distance,
            archive_y_axis: Axis::ActuatorCount,
            ..EvolutionConfig::default()
        })
        .expect("valid config should run");
        let checkpoint = EvolutionCheckpoint::from_report(report);

        let json = serde_json::to_string(&checkpoint).expect("checkpoint should serialize");
        let decoded: EvolutionCheckpoint =
            serde_json::from_str(&json).expect("checkpoint should deserialize");

        assert_eq!(decoded.version, CHECKPOINT_VERSION);
        assert_eq!(decoded.config.archive_width, 5);
        assert_eq!(decoded.config.archive_height, 4);
        assert_eq!(decoded.report.archive.x_axis, Axis::Distance);
        assert_eq!(decoded.report.archive.y_axis, Axis::ActuatorCount);
        assert_eq!(
            decoded.report.archive.occupied_count(),
            checkpoint.report.archive.occupied_count()
        );
        assert_eq!(
            decoded.report.lineage.len(),
            checkpoint.report.lineage.len()
        );
        assert!(
            decoded
                .report
                .archive
                .elites()
                .all(|elite| elite.genome_id > 0)
        );
    }
}
