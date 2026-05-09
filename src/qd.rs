use crate::fitness::{Evaluation, Metrics};
use crate::genome::Genome;
use crate::rng::Rng;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug)]
pub struct Elite {
    pub genome: Genome,
    pub evaluation: Evaluation,
    pub cell: (usize, usize),
}

#[derive(Clone, Debug)]
pub struct Archive {
    pub x_axis: Axis,
    pub y_axis: Axis,
    pub width: usize,
    pub height: usize,
    cells: Vec<Option<Elite>>,
}

impl Archive {
    pub fn new(x_axis: Axis, y_axis: Axis, width: usize, height: usize) -> Self {
        Self {
            x_axis,
            y_axis,
            width,
            height,
            cells: vec![None; width * height],
        }
    }

    pub fn insert(&mut self, genome: Genome, evaluation: Evaluation) -> bool {
        let cell = self.cell_for(&evaluation.metrics);
        let index = self.index(cell);
        let should_replace = self.cells[index]
            .as_ref()
            .map(|elite| evaluation.fitness > elite.evaluation.fitness)
            .unwrap_or(true);

        if should_replace {
            self.cells[index] = Some(Elite {
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
        let elites: Vec<&Elite> = self.elites().collect();
        elites
            .get(rng.range_usize(elites.len()))
            .map(|elite| &elite.genome)
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
}
