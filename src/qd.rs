use crate::fitness::{Evaluation, Metrics};
use crate::genome::Genome;
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

/// A labelled, bounded descriptor axis.
///
/// Binning is data rather than a match arm, so a search that has nothing to do
/// with [`Metrics`] can describe its own behaviour space and still use [`Grid`].
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AxisSpec {
    pub label: String,
    pub min: f32,
    pub max: f32,
}

impl AxisSpec {
    /// Builds an axis over `[min, max)`.
    ///
    /// A degenerate or inverted span is widened to `min + 1.0` so binning stays
    /// total: callers get every value in bin zero instead of a `NaN` division.
    pub fn new(label: impl Into<String>, min: f32, max: f32) -> Self {
        let max = if max > min { max } else { min + 1.0 };
        Self {
            label: label.into(),
            min,
            max,
        }
    }

    /// Maps `value` onto `0..bins`, clamping anything outside the span.
    pub fn bin(&self, value: f32, bins: usize) -> usize {
        let bins = bins.max(1);
        let normalized = ((value - self.min) / (self.max - self.min)).clamp(0.0, 0.999_999);
        (normalized * bins as f32) as usize
    }

    /// Reports whether `value` lands inside the span instead of being clamped.
    ///
    /// Coverage over an axis whose range does not match the search is
    /// misleading, so callers can detect the clamp rather than infer it.
    pub fn contains(&self, value: f32) -> bool {
        value >= self.min && value < self.max
    }
}

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

    /// The label and range this axis contributes to a behaviour space.
    pub fn spec(self) -> AxisSpec {
        let (label, min, max) = match self {
            Self::Distance => ("distance", 0.0, 12.0),
            Self::StableDistance => ("stable distance", 0.0, 12.0),
            Self::JumpHeight => ("jump height", 0.0, 2.0),
            Self::Uprightness => ("uprightness", 0.0, 1.0),
            Self::Stability => ("stability", 0.0, 1.0),
            Self::BodyCount => ("body count", 1.0, 10.0),
            Self::ActuatorCount => ("actuator count", 1.0, 10.0),
        };
        AxisSpec::new(label, min, max)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Distance => "distance",
            Self::StableDistance => "stable distance",
            Self::JumpHeight => "jump height",
            Self::Uprightness => "uprightness",
            Self::Stability => "stability",
            Self::BodyCount => "body count",
            Self::ActuatorCount => "actuator count",
        }
    }

    pub fn range(self) -> (f32, f32) {
        let spec = self.spec();
        (spec.min, spec.max)
    }
}

/// A MAP-Elites grid that keeps the best entry per behaviour cell.
///
/// The grid knows nothing about genomes, metrics, or simulation: it stores
/// whatever payload the caller supplies and ranks it with a caller-supplied
/// score. [`Archive`] is the Axiom-specific instance of it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Grid<T> {
    pub width: usize,
    pub height: usize,
    cells: Vec<Option<T>>,
}

impl<T> Grid<T> {
    /// Builds a grid, clamping each dimension to at least one cell.
    pub fn new(width: usize, height: usize) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        Self {
            width,
            height,
            cells: (0..width * height).map(|_| None).collect(),
        }
    }

    /// Places `item` in `cell` when it outscores the current occupant.
    ///
    /// Returns whether the grid changed. An out-of-bounds cell is rejected
    /// rather than wrapped into a neighbour.
    pub fn insert_better(
        &mut self,
        cell: (usize, usize),
        item: T,
        score: impl Fn(&T) -> f32,
    ) -> bool {
        if cell.0 >= self.width || cell.1 >= self.height {
            return false;
        }

        let index = self.index(cell);
        let incoming = score(&item);
        let should_replace = self.cells[index]
            .as_ref()
            .map(|occupant| incoming > score(occupant))
            .unwrap_or(true);

        if should_replace {
            self.cells[index] = Some(item);
        }

        should_replace
    }

    pub fn coverage(&self) -> f32 {
        self.occupied_count() as f32 / self.cells.len().max(1) as f32
    }

    pub fn occupied_count(&self) -> usize {
        self.cells.iter().filter(|cell| cell.is_some()).count()
    }

    pub fn occupants(&self) -> impl Iterator<Item = &T> {
        self.cells.iter().filter_map(Option::as_ref)
    }

    pub fn at(&self, cell: (usize, usize)) -> Option<&T> {
        if cell.0 >= self.width || cell.1 >= self.height {
            return None;
        }

        self.cells[self.index(cell)].as_ref()
    }

    /// Bins a two-dimensional descriptor onto this grid's dimensions.
    pub fn cell_for(&self, axes: (&AxisSpec, &AxisSpec), descriptor: [f32; 2]) -> (usize, usize) {
        (
            axes.0.bin(descriptor[0], self.width),
            axes.1.bin(descriptor[1], self.height),
        )
    }

    pub fn sample<'a>(&'a self, rng: &mut Rng) -> Option<&'a T> {
        let occupants: Vec<&T> = self.occupants().collect();
        occupants.get(rng.range_usize(occupants.len())).copied()
    }

    fn index(&self, cell: (usize, usize)) -> usize {
        cell.1 * self.width + cell.0
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Elite {
    pub genome: Genome,
    pub evaluation: Evaluation,
    pub cell: (usize, usize),
    pub genome_id: u64,
    pub parent_id: Option<u64>,
    pub generation: usize,
    pub mutation_summary: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Archive {
    pub x_axis: Axis,
    pub y_axis: Axis,
    #[serde(flatten)]
    grid: Grid<Elite>,
}

impl Archive {
    pub fn new(x_axis: Axis, y_axis: Axis, width: usize, height: usize) -> Self {
        Self {
            x_axis,
            y_axis,
            grid: Grid::new(width, height),
        }
    }

    pub fn width(&self) -> usize {
        self.grid.width
    }

    pub fn height(&self) -> usize {
        self.grid.height
    }

    /// The behaviour space this archive bins into.
    pub fn axes(&self) -> (AxisSpec, AxisSpec) {
        (self.x_axis.spec(), self.y_axis.spec())
    }

    pub fn insert(&mut self, genome: Genome, evaluation: Evaluation) -> bool {
        self.insert_tracked(genome, evaluation, 0, None, 0, "untracked".to_string())
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
        self.grid.insert_better(
            cell,
            Elite {
                genome,
                evaluation,
                cell,
                genome_id,
                parent_id,
                generation,
                mutation_summary,
            },
            |elite| elite.evaluation.fitness,
        )
    }

    pub fn coverage(&self) -> f32 {
        self.grid.coverage()
    }

    pub fn occupied_count(&self) -> usize {
        self.grid.occupied_count()
    }

    pub fn elites(&self) -> impl Iterator<Item = &Elite> {
        self.grid.occupants()
    }

    pub fn elite_at(&self, cell: (usize, usize)) -> Option<&Elite> {
        self.grid.at(cell)
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
        self.grid.sample(rng)
    }

    pub fn cell_for(&self, metrics: &Metrics) -> (usize, usize) {
        let (x_axis, y_axis) = self.axes();
        self.grid.cell_for(
            (&x_axis, &y_axis),
            [self.x_axis.value(metrics), self.y_axis.value(metrics)],
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Evaluation, Genome,
        fitness::Metrics,
        policy::ControllerKind,
        qd::{Archive, Axis, AxisSpec, Grid},
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

        assert_eq!(archive.width(), 1);
        assert_eq!(archive.height(), 1);
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
    fn archive_exposes_elites_by_cell() {
        let mut rng = Rng::new(4);
        let genome = Genome::minimal(ControllerKind::FeedForward, &mut rng);
        let metrics = Metrics {
            distance: 2.0,
            body_count: 5.0,
            ..Metrics::default()
        };
        let mut archive = Archive::new(Axis::Distance, Axis::BodyCount, 4, 4);
        let cell = archive.cell_for(&metrics);

        archive.insert(
            genome,
            Evaluation {
                fitness: 3.0,
                metrics,
                steps: 10,
            },
        );

        assert_eq!(archive.elite_at(cell).unwrap().evaluation.fitness, 3.0);
        assert!(archive.elite_at((archive.width(), 0)).is_none());
    }

    #[test]
    fn archive_serializes_with_a_flat_checkpoint_shape() {
        // Checkpoints are versioned, so the archive's serialized keys are a
        // compatibility surface: the grid must stay inlined, not nested.
        let archive = Archive::new(Axis::StableDistance, Axis::BodyCount, 3, 2);
        let value: serde_json::Value =
            serde_json::to_value(&archive).expect("archive should serialize");
        let object = value.as_object().expect("archive serializes as an object");

        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["cells", "height", "width", "x_axis", "y_axis"]);
        assert_eq!(object["x_axis"], serde_json::json!("StableDistance"));
        assert_eq!(object["width"], serde_json::json!(3));
        assert_eq!(object["cells"].as_array().unwrap().len(), 6);

        let decoded: Archive = serde_json::from_value(value).expect("archive should round-trip");
        assert_eq!(decoded, archive);
    }

    #[test]
    fn axis_spec_bins_across_its_configured_range() {
        let axis = AxisSpec::new("speed", 0.0, 1.0);

        assert_eq!(axis.bin(0.0, 4), 0);
        assert_eq!(axis.bin(0.5, 4), 2);
        assert_eq!(axis.bin(0.99, 4), 3);
    }

    #[test]
    fn axis_spec_clamps_values_beyond_its_range() {
        // The archive silently folds out-of-range behaviour into the edge bins.
        // That is load-bearing for coverage numbers, so it is pinned here.
        let axis = AxisSpec::new("distance", 0.0, 12.0);

        assert_eq!(axis.bin(-5.0, 12), 0);
        assert_eq!(axis.bin(100.0, 12), 11);
        assert!(!axis.contains(-5.0));
        assert!(!axis.contains(100.0));
        assert!(axis.contains(6.0));
    }

    #[test]
    fn axis_spec_widens_a_degenerate_span_instead_of_dividing_by_zero() {
        let axis = AxisSpec::new("constant", 2.0, 2.0);

        assert_eq!(axis.max, 3.0);
        assert_eq!(axis.bin(2.0, 8), 0);
        assert_eq!(axis.bin(f32::MIN, 8), 0);
    }

    #[test]
    fn axis_spec_matches_the_axis_catalog() {
        for axis in [
            Axis::Distance,
            Axis::StableDistance,
            Axis::JumpHeight,
            Axis::Uprightness,
            Axis::Stability,
            Axis::BodyCount,
            Axis::ActuatorCount,
        ] {
            let spec = axis.spec();
            assert_eq!((spec.min, spec.max), axis.range());
            assert_eq!(spec.label, axis.label());
        }
    }

    #[test]
    fn grid_holds_a_payload_that_knows_nothing_about_axiom() {
        // The point of the generic grid: a search with its own behaviour space
        // and its own payload reuses MAP-Elites without Genome or Metrics.
        let speed = AxisSpec::new("forward speed", 0.0, 0.6);
        let cost = AxisSpec::new("cost of transport", 0.0, 40.0);
        let mut grid: Grid<(&str, f32)> = Grid::new(6, 4);

        let cell = grid.cell_for((&speed, &cost), [0.3, 20.0]);
        assert_eq!(cell, (3, 2));

        assert!(grid.insert_better(cell, ("crawl", 1.0), |entry| entry.1));
        assert!(!grid.insert_better(cell, ("slower crawl", 0.5), |entry| entry.1));
        assert!(grid.insert_better(cell, ("trot", 2.0), |entry| entry.1));

        assert_eq!(grid.occupied_count(), 1);
        assert_eq!(grid.at(cell).unwrap().0, "trot");
        assert!((grid.coverage() - 1.0 / 24.0).abs() < 1e-6);
    }

    #[test]
    fn grid_rejects_out_of_bounds_cells_instead_of_wrapping() {
        let mut grid: Grid<f32> = Grid::new(3, 3);

        assert!(!grid.insert_better((3, 0), 1.0, |value| *value));
        assert!(!grid.insert_better((0, 3), 1.0, |value| *value));
        assert_eq!(grid.occupied_count(), 0);
        assert!(grid.at((3, 0)).is_none());
    }
}
