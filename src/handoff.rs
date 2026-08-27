use std::{
    cmp::Ordering,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

use crate::{Evaluation, EvolutionCheckpoint, Genome, qd::Elite};

pub const CANDIDATE_SHORTLIST_SCHEMA: &str = "axiom.candidate-shortlist.v1";

#[derive(Clone, Debug, Serialize)]
pub struct CandidateShortlist {
    pub schema_version: &'static str,
    pub project: &'static str,
    pub source: ShortlistSource,
    pub selection_policy: Vec<&'static str>,
    pub candidates: Vec<CandidateExport>,
    pub limitations: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ShortlistSource {
    pub checkpoint_version: u32,
    pub seed: u64,
    pub generations: usize,
    pub evaluated_genomes: usize,
    pub archive_coverage: f32,
    pub archive_axes: [String; 2],
}

#[derive(Clone, Debug, Serialize)]
pub struct CandidateExport {
    pub roles: Vec<&'static str>,
    pub selection_reasons: Vec<&'static str>,
    pub genome_id: u64,
    pub parent_id: Option<u64>,
    pub generation: usize,
    pub archive_cell: [usize; 2],
    pub mutation_summary: String,
    pub evaluation: Evaluation,
    pub genome: Genome,
}

impl CandidateShortlist {
    pub fn from_checkpoint(checkpoint: &EvolutionCheckpoint) -> io::Result<Self> {
        let elites = checkpoint.report.archive.elites().collect::<Vec<_>>();
        if elites.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "checkpoint archive has no candidates to hand off",
            ));
        }

        let champion = elites
            .iter()
            .copied()
            .max_by(|left, right| compare_f32(left.evaluation.fitness, right.evaluation.fitness))
            .expect("non-empty archive has a champion");
        let stable = elites
            .iter()
            .copied()
            .max_by(|left, right| {
                compare_f32(
                    left.evaluation.metrics.stable_distance,
                    right.evaluation.metrics.stable_distance,
                )
                .then_with(|| compare_f32(left.evaluation.fitness, right.evaluation.fitness))
            })
            .expect("non-empty archive has a stable candidate");
        let explorer = elites
            .iter()
            .copied()
            .max_by(|left, right| {
                compare_f32(
                    left.evaluation.metrics.distance,
                    right.evaluation.metrics.distance,
                )
                .then_with(|| compare_f32(left.evaluation.fitness, right.evaluation.fitness))
            })
            .expect("non-empty archive has an explorer");
        let leanest = elites
            .iter()
            .copied()
            .min_by(|left, right| {
                left.genome
                    .body
                    .actuator_count()
                    .cmp(&right.genome.body.actuator_count())
                    .then_with(|| {
                        left.genome
                            .body
                            .body_count()
                            .cmp(&right.genome.body.body_count())
                    })
                    .then_with(|| compare_f32(right.evaluation.fitness, left.evaluation.fitness))
            })
            .expect("non-empty archive has a lean candidate");

        let mut candidates = Vec::new();
        add_role(
            &mut candidates,
            champion,
            "champion",
            "Highest task fitness in the archive.",
        );
        add_role(
            &mut candidates,
            stable,
            "stable",
            "Highest stable distance, with fitness as the tie-breaker.",
        );
        add_role(
            &mut candidates,
            explorer,
            "explorer",
            "Greatest raw distance, with fitness as the tie-breaker.",
        );
        add_role(
            &mut candidates,
            leanest,
            "lean",
            "Fewest actuators and body parts, with fitness as the tie-breaker.",
        );

        Ok(Self {
            schema_version: CANDIDATE_SHORTLIST_SCHEMA,
            project: "Axiom",
            source: ShortlistSource {
                checkpoint_version: checkpoint.version,
                seed: checkpoint.config.seed,
                generations: checkpoint.report.generations,
                evaluated_genomes: checkpoint.report.evaluated_count,
                archive_coverage: checkpoint.report.archive.coverage(),
                archive_axes: [
                    checkpoint.report.archive.x_axis.label().to_string(),
                    checkpoint.report.archive.y_axis.label().to_string(),
                ],
            },
            selection_policy: vec![
                "champion: highest fitness",
                "stable: highest stable distance",
                "explorer: greatest raw distance",
                "lean: fewest actuators, then fewest body parts",
            ],
            candidates,
            limitations: vec![
                "Candidates were evaluated in Axiom's 2D software simulation.",
                "This handoff is not hardware validation or evidence of sim-to-real transfer.",
                "Downstream runtimes must preserve the schema version and controller semantics.",
            ],
        })
    }

    pub fn to_json(&self) -> io::Result<String> {
        let mut json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        json.push('\n');
        Ok(json)
    }
}

pub fn save_candidate_shortlist(
    path: impl AsRef<Path>,
    shortlist: &CandidateShortlist,
) -> io::Result<()> {
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
        file.write_all(shortlist.to_json()?.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary_path, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn add_role(
    candidates: &mut Vec<CandidateExport>,
    elite: &Elite,
    role: &'static str,
    reason: &'static str,
) {
    if let Some(candidate) = candidates
        .iter_mut()
        .find(|candidate| candidate.genome_id == elite.genome_id)
    {
        candidate.roles.push(role);
        candidate.selection_reasons.push(reason);
        return;
    }
    candidates.push(CandidateExport {
        roles: vec![role],
        selection_reasons: vec![reason],
        genome_id: elite.genome_id,
        parent_id: elite.parent_id,
        generation: elite.generation,
        archive_cell: [elite.cell.0, elite.cell.1],
        mutation_summary: elite.mutation_summary.clone(),
        evaluation: elite.evaluation.clone(),
        genome: elite.genome.clone(),
    });
}

fn compare_f32(left: f32, right: f32) -> Ordering {
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

fn temporary_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("axiom-shortlist");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    path.with_file_name(format!(".{file_name}.{unique}.tmp"))
}

#[cfg(test)]
mod tests {
    use crate::{
        EvolutionCheckpoint, EvolutionConfig,
        evolution::run_evolution,
        handoff::{CANDIDATE_SHORTLIST_SCHEMA, CandidateShortlist},
    };

    #[test]
    fn shortlist_is_deterministic_and_preserves_importable_genomes() {
        let config = EvolutionConfig {
            population_size: 6,
            generations: 2,
            evaluation_steps: 16,
            seed: 17,
            ..EvolutionConfig::default()
        };
        let report = run_evolution(config.clone()).expect("evolution should run");
        let checkpoint = EvolutionCheckpoint::from_report(config, report);

        let first =
            CandidateShortlist::from_checkpoint(&checkpoint).expect("shortlist should build");
        let second =
            CandidateShortlist::from_checkpoint(&checkpoint).expect("shortlist should rebuild");

        assert_eq!(first.schema_version, CANDIDATE_SHORTLIST_SCHEMA);
        assert_eq!(first.to_json().unwrap(), second.to_json().unwrap());
        assert!(!first.candidates.is_empty());
        assert!(first.candidates[0].roles.contains(&"champion"));
        assert!(!first.candidates[0].genome.body.nodes.is_empty());
        assert!(
            first
                .limitations
                .iter()
                .any(|limitation| limitation.contains("hardware"))
        );
    }
}
