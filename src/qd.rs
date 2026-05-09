use crate::fitness::{Evaluation, Metrics};
use crate::genome::Genome;
use crate::rng::Rng;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Axis {
    Distance,
    StableDistance,
    JumpHeight,
    Uprightness,
    Stability,
    BodyCount,
    ActuatorCount,
}

impl Axis {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Distance => "distance",
            Self::StableDistance => "stable-distance",
            Self::JumpHeight => "jump-height",
            Self::Uprightness => "uprightness",
            Self::Stability => "stability",
            Self::BodyCount => "body-count",
            Self::ActuatorCount => "actuator-count",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "distance" => Some(Self::Distance),
            "stable-distance" | "stable_distance" | "stable" => Some(Self::StableDistance),
            "jump-height" | "jump_height" | "jump" => Some(Self::JumpHeight),
            "uprightness" | "upright" => Some(Self::Uprightness),
            "stability" => Some(Self::Stability),
            "body-count" | "body_count" | "body" => Some(Self::BodyCount),
            "actuator-count" | "actuator_count" | "actuators" => Some(Self::ActuatorCount),
            _ => None,
        }
    }

    pub fn value(self, metrics: &Metrics) -> f32 {
        match self {
            Self::Distance => metrics.distance,
            Self::StableDistance => metrics.stable_distance,
            Self::JumpHeight => metrics.jump_height,
            Self::Uprightness => metrics.uprightness,
            Self::Stability => metrics.stability,
            Self::BodyCount => metrics.body_count,
            Self::ActuatorCount => metrics.actuator_count,
        }
    }

    pub fn range(self) -> (f32, f32) {
        match self {
            Self::Distance => (0.0, 12.0),
            Self::StableDistance => (0.0, 12.0),
            Self::JumpHeight => (0.0, 2.0),
            Self::Uprightness => (0.0, 1.0),
            Self::Stability => (0.0, 1.0),
            Self::BodyCount => (1.0, 10.0),
            Self::ActuatorCount => (1.0, 10.0),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Elite {
    pub genome_id: u64,
    pub parent_id: Option<u64>,
    pub generation: usize,
    pub mutation_summary: String,
    pub genome: Genome,
    pub evaluation: Evaluation,
    pub cell: (usize, usize),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Archive {
    pub x_axis: Axis,
    pub y_axis: Axis,
    pub width: usize,
    pub height: usize,
    cells: Vec<Option<Elite>>,
}

impl Archive {
    pub fn new(x_axis: Axis, y_axis: Axis, width: usize, height: usize) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        Self {
            x_axis,
            y_axis,
            width,
            height,
            cells: vec![None; width * height],
        }
    }

    pub fn insert(&mut self, genome: Genome, evaluation: Evaluation) -> bool {
        self.insert_tracked(genome, evaluation, 0, None, 0, "manual".to_string())
    }

    pub fn insert_tracked(
        &mut self,
        genome: Genome,
        evaluation: Evaluation,
        genome_id: u64,
        parent_id: Option<u64>,
        generation: usize,
        mutation_summary: String,
    ) -> bool {
        let cell = self.cell_for(&evaluation.metrics);
        let index = self.index(cell);
        let should_replace = self.cells[index]
            .as_ref()
            .map(|elite| evaluation.fitness > elite.evaluation.fitness)
            .unwrap_or(true);

        if should_replace {
            self.cells[index] = Some(Elite {
                genome_id,
                parent_id,
                generation,
                mutation_summary,
                genome,
                evaluation,
                cell,
            });
        }

        should_replace
    }

    pub fn coverage(&self) -> f32 {
        self.occupied_count() as f32 / self.cells.len().max(1) as f32
    }

    pub fn occupied_count(&self) -> usize {
        self.cells.iter().filter(|cell| cell.is_some()).count()
    }

    pub fn elites(&self) -> impl Iterator<Item = &Elite> {
        self.cells.iter().filter_map(Option::as_ref)
    }

    pub fn best(&self) -> Option<&Elite> {
        self.elites().max_by(|a, b| {
            a.evaluation
                .fitness
                .partial_cmp(&b.evaluation.fitness)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    pub fn sample_parent<'a>(&'a self, rng: &mut Rng) -> Option<&'a Genome> {
        self.sample_elite(rng).map(|elite| &elite.genome)
    }

    pub fn sample_elite<'a>(&'a self, rng: &mut Rng) -> Option<&'a Elite> {
        let elites: Vec<&Elite> = self.elites().collect();
        elites.get(rng.range_usize(elites.len())).copied()
    }

    pub fn sample_elite_tournament<'a>(
        &'a self,
        rng: &mut Rng,
        tournament_size: usize,
    ) -> Option<&'a Elite> {
        let elites: Vec<&Elite> = self.elites().collect();
        let mut best = None::<&Elite>;

        for _ in 0..tournament_size.max(1) {
            let candidate = elites.get(rng.range_usize(elites.len())).copied()?;
            if best
                .map(|elite| candidate.evaluation.fitness > elite.evaluation.fitness)
                .unwrap_or(true)
            {
                best = Some(candidate);
            }
        }

        best
    }

    pub fn elite_at(&self, cell: (usize, usize)) -> Option<&Elite> {
        if cell.0 >= self.width || cell.1 >= self.height {
            return None;
        }

        self.cells[self.index(cell)].as_ref()
    }

    pub fn cells(&self) -> &[Option<Elite>] {
        &self.cells
    }

    pub fn cell_for(&self, metrics: &Metrics) -> (usize, usize) {
        (
            axis_bin(self.x_axis, metrics, self.width),
            axis_bin(self.y_axis, metrics, self.height),
        )
    }

    fn index(&self, cell: (usize, usize)) -> usize {
        cell.1 * self.width + cell.0
    }
}

fn axis_bin(axis: Axis, metrics: &Metrics, bins: usize) -> usize {
    let (min, max) = axis.range();
    let normalized = ((axis.value(metrics) - min) / (max - min)).clamp(0.0, 0.999_999);
    (normalized * bins as f32) as usize
}

#[cfg(test)]
mod tests {
    use crate::{
        Evaluation, Genome,
        fitness::Metrics,
        policy::ControllerKind,
        qd::{Archive, Axis},
        rng::Rng,
    };

    #[test]
    fn archive_keeps_highest_fitness_per_cell() {
        let mut rng = Rng::new(2);
        let genome = Genome::minimal(ControllerKind::FeedForward, &mut rng);
        let mut archive = Archive::new(Axis::Distance, Axis::BodyCount, 4, 4);
        let metrics = Metrics {
            distance: 1.0,
            body_count: 4.0,
            ..Metrics::default()
        };

        assert!(archive.insert(
            genome.clone(),
            Evaluation {
                fitness: 1.0,
                metrics: metrics.clone(),
                steps: 10,
            },
        ));
        assert!(!archive.insert(
            genome.clone(),
            Evaluation {
                fitness: 0.5,
                metrics: metrics.clone(),
                steps: 10,
            },
        ));
        assert!(archive.insert(
            genome,
            Evaluation {
                fitness: 2.0,
                metrics,
                steps: 10,
            },
        ));

        assert_eq!(archive.occupied_count(), 1);
        assert_eq!(archive.best().unwrap().evaluation.fitness, 2.0);
    }

    #[test]
    fn archive_clamps_zero_dimensions_to_insertable_grid() {
        let mut rng = Rng::new(3);
        let genome = Genome::minimal(ControllerKind::FeedForward, &mut rng);
        let mut archive = Archive::new(Axis::Distance, Axis::BodyCount, 0, 0);

        assert_eq!(archive.width, 1);
        assert_eq!(archive.height, 1);
        assert!(archive.insert(
            genome,
            Evaluation {
                fitness: 1.0,
                metrics: Metrics::default(),
                steps: 1,
            },
        ));
        assert_eq!(archive.occupied_count(), 1);
    }

    #[test]
    fn tournament_sampling_handles_empty_and_single_elite_archives() {
        let mut rng = Rng::new(5);
        let mut archive = Archive::new(Axis::Distance, Axis::BodyCount, 4, 4);

        assert!(archive.sample_elite_tournament(&mut rng, 3).is_none());

        let genome = Genome::minimal(ControllerKind::FeedForward, &mut rng);
        assert!(archive.insert_tracked(
            genome,
            Evaluation {
                fitness: 2.5,
                metrics: Metrics {
                    distance: 1.0,
                    body_count: 4.0,
                    ..Metrics::default()
                },
                steps: 10,
            },
            42,
            None,
            0,
            "seed".to_string(),
        ));

        let elite = archive
            .sample_elite_tournament(&mut rng, 0)
            .expect("single occupied archive should sample its elite");
        assert_eq!(elite.genome_id, 42);
        assert_eq!(elite.evaluation.fitness, 2.5);
    }
}
