//! Bounded workshop state. Search runs off the HTTP thread; the exact robot
//! design and all eight oscillators travel together through selection and saves.
use crate::{
    CpgGenome, FieldElite, HoldoutReport, RobotConfig, SearchConfig, SearchReport, Spread,
    lab::GaitInput,
    search::{GenerationRecord, run_search_observed},
};
use axiom::qd::Grid;
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, TryRecvError},
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RobotDesign {
    pub body_length_m: f32,
    pub body_width_m: f32,
    pub body_mass_kg: f32,
    pub upper_length_m: f32,
    pub lower_length_m: f32,
    pub friction: f32,
}
impl Default for RobotDesign {
    fn default() -> Self {
        let c = RobotConfig::nominal();
        Self {
            body_length_m: c.body.length_m,
            body_width_m: c.body.width_m,
            body_mass_kg: c.body.mass_kg,
            upper_length_m: c.leg.upper_length_m,
            lower_length_m: c.leg.lower_length_m,
            friction: c.contact.friction,
        }
    }
}
impl RobotDesign {
    pub fn robot(self) -> Result<RobotConfig, &'static str> {
        if !(0.12..=0.24).contains(&self.body_length_m)
            || !(0.08..=0.16).contains(&self.body_width_m)
            || !(0.15..=0.6).contains(&self.body_mass_kg)
            || !(0.035..=0.075).contains(&self.upper_length_m)
            || !(0.04..=0.09).contains(&self.lower_length_m)
            || !(0.3..=1.3).contains(&self.friction)
        {
            return Err("Robot design is outside workshop limits");
        }
        let mut c = RobotConfig::nominal();
        c.body.length_m = self.body_length_m;
        c.body.width_m = self.body_width_m;
        c.body.mass_kg = self.body_mass_kg;
        c.leg.upper_length_m = self.upper_length_m;
        c.leg.lower_length_m = self.lower_length_m;
        c.contact.friction = self.friction;
        c.meta.name = "axiom-workshop-quadruped".into();
        c.meta.provenance = "Uncalibrated Axiom workshop design".into();
        c.validate().map_err(|_| "Invalid physical robot design")?;
        Ok(c)
    }
}

pub fn valid_genome(g: &CpgGenome) -> bool {
    g.is_well_formed()
        && (0.25..=4.0).contains(&g.frequency_hz)
        && g.joints.iter().all(|j| {
            (0.0..=0.6).contains(&j.amplitude)
                && (0.0..1.0).contains(&j.phase)
                && (-0.4..=0.4).contains(&j.bias)
        })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    pub seed: u32,
    pub generations: usize,
    pub evaluated: usize,
    pub ticks: usize,
    pub worlds: usize,
    pub width: usize,
    pub height: usize,
    pub cells: Vec<FieldElite>,
    pub history: Vec<GenerationRecord>,
    pub source: String,
}
impl Archive {
    fn from_report(r: SearchReport) -> Self {
        Self {
            seed: r.seed as u32,
            generations: r.generations,
            evaluated: r.evaluated,
            ticks: r.ticks,
            worlds: r.worlds,
            width: r.archive.width,
            height: r.archive.height,
            cells: r.archive.occupants().cloned().collect(),
            history: r.history,
            source: "local".into(),
        }
    }
    fn valid(&self) -> bool {
        let mut seen = std::collections::HashSet::new();
        self.width == 12
            && self.height == 8
            && self.cells.len() <= 96
            && self.generations <= 24
            && self.evaluated <= 576
            && matches!(self.ticks, 120 | 160)
            && matches!(self.worlds, 3 | 4)
            && self.history.len() == self.generations
            && self.cells.iter().all(|e| {
                let s = e.score;
                e.cell.0 < 12
                    && e.cell.1 < 8
                    && seen.insert(e.cell)
                    && valid_genome(&e.genome)
                    && e.generation < self.generations
                    && s.worlds == self.worlds
                    && s.falls < s.worlds
                    && [s.fitness, s.worst_fitness, s.mean_effort_per_m]
                        .iter()
                        .all(|v| v.is_finite() && *v >= 0.0)
                    && s.mean_speed_m_s.is_finite()
                    && s.worst_fitness <= s.fitness + 1e-5
            })
            && self.history.iter().enumerate().all(|(i, h)| {
                h.generation == i
                    && h.best_fitness.is_finite()
                    && h.best_fitness >= 0.0
                    && (0.0..=1.0).contains(&h.coverage)
                    && h.occupied <= 96
                    && h.fell_somewhere <= 24
            })
    }
    fn report(&self, design: RobotDesign) -> SearchReport {
        // Reconstruct a bounded grid from validated cells; never deserialize grid storage.
        let mut grid = Grid::new(12, 8);
        for e in &self.cells {
            grid.insert_better(e.cell, e.clone(), |e| e.score.fitness);
        }
        SearchReport {
            robot: design.robot().expect("validated design").meta.name,
            calibrated: false,
            seed: self.seed as u64,
            generations: self.generations,
            evaluated: self.evaluated,
            ticks: self.ticks,
            worlds: self.worlds,
            archive: grid,
            history: self.history.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Save {
    pub format: String,
    pub version: u32,
    pub design: RobotDesign,
    pub gait: GaitInput,
    pub controller: Option<CpgGenome>,
    pub controller_name: String,
    pub archive: Option<Archive>,
}
impl Save {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.format != "axiom-workshop" || self.version != 1 {
            return Err("Unsupported Axiom workshop save version");
        }
        self.design.robot()?;
        if !self.gait.valid()
            || self.controller_name.len() > 80
            || self.controller_name.chars().any(char::is_control)
            || self.controller.as_ref().is_some_and(|c| !valid_genome(c))
            || self.archive.as_ref().is_some_and(|a| !a.valid())
        {
            return Err("Invalid controller or archive in workshop save");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct JobStatus {
    pub status: &'static str,
    pub kind: &'static str,
    pub evaluated: usize,
    pub total: usize,
    pub message: String,
}
impl Default for JobStatus {
    fn default() -> Self {
        Self {
            status: "idle",
            kind: "search",
            evaluated: 0,
            total: 0,
            message: String::new(),
        }
    }
}
enum Event {
    Progress(usize),
    Search(SearchReport),
    Holdout(HoldoutReport),
    Cancelled,
}
struct Worker {
    cancel: Arc<AtomicBool>,
    receiver: Receiver<Event>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchInput {
    pub seed: u32,
    pub generations: usize,
}

pub struct Workshop {
    pub design: RobotDesign,
    pub controller: Option<CpgGenome>,
    pub controller_name: String,
    pub archive: Option<Archive>,
    pub holdout: Option<HoldoutReport>,
    pub job: JobStatus,
    worker: Option<Worker>,
}
impl Default for Workshop {
    fn default() -> Self {
        Self {
            design: RobotDesign::default(),
            controller: None,
            controller_name: "Manual trot".into(),
            archive: None,
            holdout: None,
            job: JobStatus::default(),
            worker: None,
        }
    }
}
impl Workshop {
    /// Reuse the repository's recorded full-search repertoire with its nominal
    /// robot, instead of claiming its scores belong to the current custom design.
    pub fn bundled() -> Result<Self, &'static str> {
        let report = SearchReport::from_json(include_str!("../tests/golden/archive-seed1.json"))
            .map_err(|_| "Bundled archive could not be read")?;
        let best = report.best().ok_or("Bundled archive is empty")?.clone();
        let mut archive = Archive::from_report(report);
        archive.source = "bundled".into();
        if !archive.valid() {
            return Err("Bundled archive failed validation");
        }
        Ok(Self {
            controller: Some(best.genome),
            controller_name: format!("Archive gait {},{}", best.cell.0, best.cell.1),
            archive: Some(archive),
            ..Self::default()
        })
    }
    pub fn mode(&self) -> &'static str {
        if self.controller.is_some() {
            "archive"
        } else {
            "manual"
        }
    }
    pub fn busy(&self) -> bool {
        self.worker.is_some()
    }
    pub fn state(&self) -> serde_json::Value {
        serde_json::json!({"design":self.design,"controller_name":self.controller_name,
            "controller_mode":self.mode(),"archive":self.archive,"holdout":self.holdout,"job":self.job})
    }
    pub fn poll(&mut self) {
        let mut done = false;
        if let Some(worker) = &self.worker {
            loop {
                match worker.receiver.try_recv() {
                    Ok(Event::Progress(n)) => self.job.evaluated = n,
                    Ok(Event::Search(report)) => {
                        self.job.evaluated = report.evaluated;
                        self.archive = Some(Archive::from_report(report));
                        self.holdout = None;
                        self.job.status = "complete";
                        self.job.message = "Search complete".into();
                        done = true;
                        break;
                    }
                    Ok(Event::Holdout(report)) => {
                        self.job.evaluated = report.tested;
                        self.holdout = Some(report);
                        self.job.status = "complete";
                        self.job.message = "Unseen-world test complete".into();
                        done = true;
                        break;
                    }
                    Ok(Event::Cancelled) => {
                        self.job.status = "cancelled";
                        self.job.message = "Cancelled; previous archive kept".into();
                        done = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.job.status = "error";
                        self.job.message = "Worker stopped; previous archive kept".into();
                        done = true;
                        break;
                    }
                }
            }
        }
        if done {
            self.worker = None;
        }
    }
    pub fn cancel(&mut self) {
        if let Some(worker) = &self.worker {
            worker.cancel.store(true, Ordering::Relaxed);
            self.job.message = "Stopping after current evaluation".into();
        }
    }
    pub fn start_search(&mut self, input: SearchInput) -> Result<(), &'static str> {
        if self.busy() {
            return Err("A search or holdout test is already running");
        }
        if !(1..=12).contains(&input.generations) {
            return Err("Choose 1 to 12 generations");
        }
        let robot = self.design.robot()?;
        let config = SearchConfig {
            seed: input.seed as u64,
            generations: input.generations,
            batch: 12,
            ticks: 120,
            worlds: 3,
            mutation_scale: 0.10,
            ..SearchConfig::default()
        };
        let (tx, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        std::thread::Builder::new()
            .name("axiom-evolution".into())
            .spawn(move || {
                let report = run_search_observed(&config, &robot, |n| {
                    let _ = tx.send(Event::Progress(n));
                    !stop.load(Ordering::Relaxed)
                });
                let event = if stop.load(Ordering::Relaxed) {
                    Event::Cancelled
                } else {
                    Event::Search(report)
                };
                let _ = tx.send(event);
            })
            .map_err(|_| "Could not start search worker")?;
        self.worker = Some(Worker { cancel, receiver });
        self.job = JobStatus {
            status: "running",
            kind: "search",
            evaluated: 0,
            total: input.generations * 12,
            message: "Evaluating gaits across three perturbed flat-ground worlds".into(),
        };
        Ok(())
    }
    pub fn start_holdout(&mut self) -> Result<(), &'static str> {
        if self.busy() {
            return Err("A search or holdout test is already running");
        }
        let archive = self
            .archive
            .as_ref()
            .filter(|a| !a.cells.is_empty())
            .ok_or("Evolve or import an archive first")?;
        let report = archive.report(self.design);
        let robot = self.design.robot()?;
        let total = archive.cells.len();
        let (tx, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        std::thread::Builder::new()
            .name("axiom-holdout".into())
            .spawn(move || {
                // Distinct from training's seed XOR 0x5eed, including when seed is zero.
                let seed = report.seed ^ 0xbeef_1234;
                let mut combined = HoldoutReport {
                    tested: 0,
                    robust: 0,
                    retained_fraction: 0.0,
                    per_cell: Vec::new(),
                };
                let mut retained_count = 0;
                for elite in report.archive.occupants() {
                    if stop.load(Ordering::Relaxed) {
                        let _ = tx.send(Event::Cancelled);
                        return;
                    }
                    let mut single = report.clone();
                    single.archive = Grid::new(12, 8);
                    single
                        .archive
                        .insert_better(elite.cell, elite.clone(), |e| e.score.fitness);
                    let result = crate::holdout(&single, &robot, &Spread::default(), seed, 3);
                    combined.tested += result.tested;
                    combined.robust += result.robust;
                    if elite.score.fitness > 1e-3 {
                        combined.retained_fraction += result.retained_fraction;
                        retained_count += 1;
                    }
                    combined.per_cell.extend(result.per_cell);
                    let _ = tx.send(Event::Progress(combined.tested));
                }
                if retained_count > 0 {
                    combined.retained_fraction /= retained_count as f32;
                }
                let event = if stop.load(Ordering::Relaxed) {
                    Event::Cancelled
                } else {
                    Event::Holdout(combined)
                };
                let _ = tx.send(event);
            })
            .map_err(|_| "Could not start holdout worker")?;
        self.worker = Some(Worker { cancel, receiver });
        self.job = JobStatus {
            status: "running",
            kind: "holdout",
            evaluated: 0,
            total,
            message: "Testing three unseen parameter worlds on flat ground".into(),
        };
        Ok(())
    }
    pub fn save(&self, gait: GaitInput) -> Save {
        Save {
            format: "axiom-workshop".into(),
            version: 1,
            design: self.design,
            gait,
            controller: self.controller.clone(),
            controller_name: self.controller_name.clone(),
            archive: self.archive.clone(),
        }
    }
    pub fn restore(save: Save) -> Result<Self, &'static str> {
        save.validate()?;
        let mut archive = save.archive;
        if let Some(a) = &mut archive {
            a.source = "imported".into();
        }
        Ok(Self {
            design: save.design,
            controller: save.controller,
            controller_name: save.controller_name,
            archive,
            holdout: None,
            job: JobStatus::default(),
            worker: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Gait;
    #[test]
    fn design_changes_real_physical_parameters_and_rejects_nonfinite_values() {
        let design = RobotDesign {
            body_length_m: 0.22,
            lower_length_m: 0.085,
            ..RobotDesign::default()
        };
        let robot = design.robot().unwrap();
        assert_eq!(robot.body.length_m, 0.22);
        assert_eq!(robot.leg.lower_length_m, 0.085);
        assert!(!robot.meta.calibrated);
        assert!(
            RobotDesign {
                friction: f32::NAN,
                ..design
            }
            .robot()
            .is_err()
        );
        assert!(
            RobotDesign {
                body_mass_kg: 20.0,
                ..design
            }
            .robot()
            .is_err()
        );
    }
    #[test]
    fn controller_bounds_reject_malformed_imports() {
        let mut g = CpgGenome::from_gait(&Gait::default());
        assert!(valid_genome(&g));
        g.joints[7].phase = 1.0;
        assert!(!valid_genome(&g));
        g.joints[7].phase = 0.0;
        g.joints[0].amplitude = 4.0;
        assert!(!valid_genome(&g));
    }
    #[test]
    fn saves_preserve_design_and_full_controller_and_mark_imported_scores() {
        let mut w = Workshop::default();
        let mut g = CpgGenome::from_gait(&Gait::default());
        g.joints[3].bias = 0.12;
        w.controller = Some(g.clone());
        w.archive = Some(Archive::from_report(crate::run_search(
            &SearchConfig {
                generations: 1,
                batch: 12,
                ticks: 120,
                worlds: 3,
                ..SearchConfig::default()
            },
            &w.design.robot().unwrap(),
        )));
        let save: Save =
            serde_json::from_str(&serde_json::to_string(&w.save(GaitInput::default())).unwrap())
                .unwrap();
        let restored = Workshop::restore(save).unwrap();
        assert_eq!(restored.controller, Some(g));
        assert_eq!(restored.archive.as_ref().unwrap().source, "imported");
        let mut bad = restored.save(GaitInput::default());
        bad.version = 9;
        assert!(Workshop::restore(bad).is_err());
    }
    #[test]
    fn observed_search_preserves_results_and_honors_cancellation() {
        let c = SearchConfig {
            generations: 2,
            batch: 2,
            ticks: 10,
            worlds: 1,
            ..SearchConfig::default()
        };
        let robot = RobotConfig::nominal();
        let normal = crate::run_search(&c, &robot);
        let observed = run_search_observed(&c, &robot, |_| true);
        assert_eq!(normal.archive, observed.archive);
        let cancelled = run_search_observed(&c, &robot, |n| n < 1);
        assert_eq!(cancelled.evaluated, 1);
    }
    #[test]
    fn bundled_repertoire_restores_the_full_prior_search_with_its_robot() {
        let w = Workshop::bundled().unwrap();
        let archive = w.archive.as_ref().unwrap();
        assert_eq!(archive.cells.len(), 43);
        assert_eq!(archive.evaluated, 576);
        assert_eq!(w.design, RobotDesign::default());
        assert!(w.controller.is_some());
        let saved = w.save(GaitInput::default());
        assert!(Workshop::restore(saved).is_ok());
    }
}
