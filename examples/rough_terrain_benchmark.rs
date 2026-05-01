use axiom::{EvolutionConfig, SearchMode, TaskKind, run_evolution};

fn main() {
    let report = run_evolution(EvolutionConfig {
        seed: 1337,
        population_size: 16,
        generations: 5,
        evaluation_steps: 160,
        task: TaskKind::RoughTerrain,
        search_mode: SearchMode::MapElites,
        ..EvolutionConfig::default()
    });

    println!("best fitness: {:.3}", report.best_evaluation.fitness);
    println!(
        "best distance: {:.3}",
        report.best_evaluation.metrics.distance
    );
    println!("archive cells: {}", report.archive.occupied_count());
    println!("coverage: {:.1}%", report.archive.coverage() * 100.0);
}
