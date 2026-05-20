use std::{fs, io, path::Path};

use serde::Serialize;

use crate::{
    EvolutionConfig, EvolutionReport, GenerationSummary, fitness::Metrics, qd::Elite,
    task_pack::TaskScenario,
};

const REPORT_VERSION: u32 = 1;
const TOP_ELITE_LIMIT: usize = 8;

#[derive(Clone, Debug, Serialize)]
struct ExperimentReportExport {
    report_version: u32,
    project: &'static str,
    evaluation: EvaluationExport,
    config: EvolutionConfig,
    best: BestExport,
    archive: ArchiveExport,
    generation_summaries: Vec<GenerationSummary>,
    top_elites: Vec<EliteExport>,
}

#[derive(Clone, Debug, Serialize)]
struct EvaluationExport {
    mode: String,
    task: String,
    task_pack: Option<String>,
    scenarios: Vec<TaskScenario>,
}

#[derive(Clone, Debug, Serialize)]
struct BestExport {
    fitness: f32,
    metrics: Metrics,
    controller: String,
    body_count: usize,
    actuator_count: usize,
}

#[derive(Clone, Debug, Serialize)]
struct ArchiveExport {
    width: usize,
    height: usize,
    x_axis: String,
    y_axis: String,
    occupied_cells: usize,
    coverage: f32,
}

#[derive(Clone, Debug, Serialize)]
struct EliteExport {
    rank: usize,
    cell: [usize; 2],
    genome_id: u64,
    parent_id: Option<u64>,
    generation: usize,
    mutation_summary: String,
    fitness: f32,
    metrics: Metrics,
    controller: String,
    body_count: usize,
    actuator_count: usize,
}

pub fn save_report_json(path: impl AsRef<Path>, report: &EvolutionReport) -> io::Result<()> {
    create_parent_dir(path.as_ref())?;
    let export = ExperimentReportExport::from_report(report);
    let json = serde_json::to_string_pretty(&export).map_err(io::Error::other)?;
    fs::write(path, json)
}

pub fn save_report_markdown(path: impl AsRef<Path>, report: &EvolutionReport) -> io::Result<()> {
    create_parent_dir(path.as_ref())?;
    fs::write(path, render_report_markdown(report))
}

pub fn render_report_markdown(report: &EvolutionReport) -> String {
    let evaluation = evaluation_export(report);
    let mut output = String::new();
    output.push_str("# Axiom Experiment Report\n\n");
    output.push_str("## Summary\n\n");
    output.push_str(&format!("- Evaluation: {}\n", evaluation.mode));
    output.push_str(&format!("- Task: {}\n", evaluation.task));
    if let Some(pack) = &evaluation.task_pack {
        output.push_str(&format!("- Task pack: {pack}\n"));
    }
    output.push_str(&format!("- Generations: {}\n", report.generations));
    output.push_str(&format!(
        "- Evaluated genomes: {}\n",
        report.evaluated_count
    ));
    output.push_str(&format!(
        "- Archive coverage: {:.1}% ({} of {} cells)\n",
        report.archive.coverage() * 100.0,
        report.archive.occupied_count(),
        report.archive.width * report.archive.height
    ));
    output.push_str(&format!(
        "- Best fitness: {:.3}\n",
        report.best_evaluation.fitness
    ));
    output.push_str(&format!(
        "- Best stable distance: {:.3}\n\n",
        report.best_evaluation.metrics.stable_distance
    ));

    output.push_str("## Evaluation Scenarios\n\n");
    output.push_str("| Scenario | Task | Steps | Weight |\n");
    output.push_str("| --- | --- | ---: | ---: |\n");
    for scenario in evaluation.scenarios {
        output.push_str(&format!(
            "| {} | {} | {} | {:.2} |\n",
            scenario.name,
            scenario.task.as_str(),
            scenario.steps,
            scenario.weight
        ));
    }

    output.push_str("\n## Morphology Constraints\n\n");
    output.push_str(&format!(
        "- Max body parts: {}\n",
        report.config.morphology_constraints.max_body_parts
    ));
    output.push_str(&format!(
        "- Max actuators: {}\n",
        report.config.morphology_constraints.max_actuators
    ));
    output.push_str(&format!(
        "- Max actuator strength: {:.2}\n\n",
        report.config.morphology_constraints.max_actuator_strength
    ));

    output.push_str("## Top Archive Elites\n\n");
    output
        .push_str("| Rank | Cell | Fitness | Distance | Stability | Body | Actuators | Genome |\n");
    output.push_str("| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for elite in top_elites(report) {
        output.push_str(&format!(
            "| {} | {},{} | {:.3} | {:.3} | {:.3} | {} | {} | {} |\n",
            elite.rank,
            elite.cell[0],
            elite.cell[1],
            elite.fitness,
            elite.metrics.distance,
            elite.metrics.stability,
            elite.body_count,
            elite.actuator_count,
            elite.genome_id
        ));
    }

    output.push_str("\n## Generation Summaries\n\n");
    output.push_str(
        "| Generation | Evaluated | Best Fitness | Best Distance | Coverage | Occupied |\n",
    );
    output.push_str("| ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for summary in &report.generation_summaries {
        output.push_str(&format!(
            "| {} | {} | {:.3} | {:.3} | {:.1}% | {} |\n",
            summary.generation,
            summary.evaluated_count,
            summary.best_fitness,
            summary.best_distance,
            summary.coverage * 100.0,
            summary.occupied_cells
        ));
    }

    output
}

impl ExperimentReportExport {
    fn from_report(report: &EvolutionReport) -> Self {
        Self {
            report_version: REPORT_VERSION,
            project: "Axiom",
            evaluation: evaluation_export(report),
            config: report.config.clone(),
            best: BestExport::from_report(report),
            archive: ArchiveExport::from_report(report),
            generation_summaries: report.generation_summaries.clone(),
            top_elites: top_elites(report),
        }
    }
}

impl BestExport {
    fn from_report(report: &EvolutionReport) -> Self {
        Self {
            fitness: report.best_evaluation.fitness,
            metrics: report.best_evaluation.metrics.clone(),
            controller: report.best_genome.controller.as_str().to_string(),
            body_count: report.best_genome.body.body_count(),
            actuator_count: report.best_genome.body.actuator_count(),
        }
    }
}

impl ArchiveExport {
    fn from_report(report: &EvolutionReport) -> Self {
        Self {
            width: report.archive.width,
            height: report.archive.height,
            x_axis: report.archive.x_axis.as_str().to_string(),
            y_axis: report.archive.y_axis.as_str().to_string(),
            occupied_cells: report.archive.occupied_count(),
            coverage: report.archive.coverage(),
        }
    }
}

impl EliteExport {
    fn from_elite(rank: usize, elite: &Elite) -> Self {
        Self {
            rank,
            cell: [elite.cell.0, elite.cell.1],
            genome_id: elite.genome_id,
            parent_id: elite.parent_id,
            generation: elite.generation,
            mutation_summary: elite.mutation_summary.clone(),
            fitness: elite.evaluation.fitness,
            metrics: elite.evaluation.metrics.clone(),
            controller: elite.genome.controller.as_str().to_string(),
            body_count: elite.genome.body.body_count(),
            actuator_count: elite.genome.body.actuator_count(),
        }
    }
}

fn evaluation_export(report: &EvolutionReport) -> EvaluationExport {
    match report.config.task_pack {
        Some(pack) => EvaluationExport {
            mode: pack.label().to_string(),
            task: report.config.task.as_str().to_string(),
            task_pack: Some(pack.as_str().to_string()),
            scenarios: pack.scenarios(report.config.evaluation_steps),
        },
        None => EvaluationExport {
            mode: "Single task".to_string(),
            task: report.config.task.as_str().to_string(),
            task_pack: None,
            scenarios: vec![TaskScenario::single_task(
                report.config.task,
                report.config.evaluation_steps,
            )],
        },
    }
}

fn top_elites(report: &EvolutionReport) -> Vec<EliteExport> {
    let mut elites = report.archive.elites().collect::<Vec<_>>();
    elites.sort_by(|a, b| {
        b.evaluation
            .fitness
            .partial_cmp(&a.evaluation.fitness)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    elites
        .into_iter()
        .take(TOP_ELITE_LIMIT)
        .enumerate()
        .map(|(index, elite)| EliteExport::from_elite(index + 1, elite))
        .collect()
}

fn create_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        EvolutionConfig, MorphologyConstraints, TaskPackKind,
        evolution::run_evolution,
        report::{render_report_markdown, save_report_json},
    };

    #[test]
    fn markdown_report_includes_pack_archive_and_elites() {
        let report = test_report();

        let markdown = render_report_markdown(&report);

        assert!(markdown.contains("Rough inspection"));
        assert!(markdown.contains("Archive coverage"));
        assert!(markdown.contains("step field"));
        assert!(markdown.contains("Top Archive Elites"));
    }

    #[test]
    fn json_report_exports_config_and_top_elites() {
        let report = test_report();
        let path = temp_path("axiom-report-export.json");

        save_report_json(&path, &report).expect("report should save");
        let json = fs::read_to_string(&path).expect("report should be readable");
        let _ = fs::remove_file(&path);
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("report should be valid JSON");

        assert_eq!(value["report_version"], 1);
        assert_eq!(
            value["evaluation"]["task_pack"],
            TaskPackKind::RoughInspection.as_str()
        );
        assert!(value["top_elites"].as_array().unwrap().len() > 0);
    }

    fn test_report() -> crate::EvolutionReport {
        run_evolution(EvolutionConfig {
            population_size: 5,
            generations: 1,
            evaluation_steps: 12,
            task_pack: Some(TaskPackKind::RoughInspection),
            morphology_constraints: MorphologyConstraints::rough_inspection(),
            ..EvolutionConfig::default()
        })
        .expect("test report should run")
    }

    fn temp_path(name: &str) -> PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_millis();
        std::env::temp_dir().join(format!("{}-{}-{name}", std::process::id(), millis))
    }

    use std::path::PathBuf;
}
