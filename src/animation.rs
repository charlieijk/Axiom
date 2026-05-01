use std::{
    io::{self, IsTerminal, Write},
    thread,
    time::Duration,
};

use crate::{
    fitness::{TaskKind, observation_vector},
    genome::{BodyGenome, Genome},
    math::{Vec2, child_center_offset},
    policy::Brain,
    simulation::{Simulation, Snapshot, World},
};

#[derive(Clone, Debug)]
pub struct TerminalAnimationConfig {
    pub steps: usize,
    pub dt: f32,
    pub width: usize,
    pub height: usize,
    pub fps: u32,
    pub clear_screen: bool,
}

impl Default for TerminalAnimationConfig {
    fn default() -> Self {
        Self {
            steps: 160,
            dt: 0.05,
            width: 96,
            height: 28,
            fps: 20,
            clear_screen: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReplayFrame {
    pub time: f32,
    pub root_position: Vec2,
    pub tilt: f32,
    pub body_centers: Vec<Vec2>,
    pub joint_segments: Vec<(Vec2, Vec2)>,
}

pub fn capture_replay(genome: &Genome, task: TaskKind, steps: usize, dt: f32) -> Vec<ReplayFrame> {
    let mut simulation = Simulation::new(task.world());
    simulation.spawn_creature(&genome.body);

    let brain = Brain::from_genome(genome);
    let mut brain_state = brain.reset_state();
    let mut frames = Vec::with_capacity(steps.saturating_add(1));

    for _ in 0..steps {
        let snapshot = simulation.snapshot();
        frames.push(replay_frame(&genome.body, &snapshot));

        let observations = observation_vector(&snapshot, &genome.body);
        let actions = brain.think(&observations, &mut brain_state);
        simulation.step(genome, &actions, dt);
    }

    frames.push(replay_frame(&genome.body, &simulation.snapshot()));
    frames
}

pub fn play_terminal_animation(
    genome: &Genome,
    task: TaskKind,
    config: TerminalAnimationConfig,
) -> io::Result<()> {
    let world = task.world();
    let frames = capture_replay(genome, task, config.steps, config.dt);
    let clear_screen = config.clear_screen && io::stdout().is_terminal();
    let delay = if config.fps == 0 {
        Duration::ZERO
    } else {
        Duration::from_secs_f32(1.0 / config.fps as f32)
    };

    let mut stdout = io::stdout().lock();
    for frame in frames {
        if clear_screen {
            stdout.write_all(b"\x1b[2J\x1b[H")?;
        }

        stdout.write_all(
            render_frame(&frame, &genome.body, &world, config.width, config.height).as_bytes(),
        )?;
        if !clear_screen {
            stdout.write_all(b"\n\n")?;
        }
        stdout.flush()?;

        if !delay.is_zero() {
            thread::sleep(delay);
        }
    }

    Ok(())
}

pub fn render_frame(
    frame: &ReplayFrame,
    body: &BodyGenome,
    world: &World,
    width: usize,
    height: usize,
) -> String {
    let width = width.max(40);
    let height = height.max(12);
    let scale = 0.16_f32;
    let view_width = width as f32 * scale;
    let view_height = height as f32 * scale;
    let left = frame.root_position.x - view_width * 0.35;
    let floor = world.terrain_height(frame.root_position.x);
    let bottom = floor - 0.55;
    let top = bottom + view_height;

    let mut grid = vec![vec![' '; width]; height];
    draw_terrain(&mut grid, world, left, top, scale);

    for (start, end) in &frame.joint_segments {
        draw_line(&mut grid, *start, *end, left, top, scale, '*');
    }

    for (index, center) in frame.body_centers.iter().copied().enumerate() {
        let node = &body.nodes[index];
        let symbol = if index == 0 { '@' } else { 'o' };
        draw_body(&mut grid, center, node.size, left, top, scale, symbol);
    }

    let mut output = format!(
        "time {:>5.2}s | x {:>6.2} | y {:>5.2} | tilt {:>5.2}\n",
        frame.time, frame.root_position.x, frame.root_position.y, frame.tilt
    );
    for row in grid {
        output.extend(row);
        output.push('\n');
    }
    output
}

fn replay_frame(body: &BodyGenome, snapshot: &Snapshot) -> ReplayFrame {
    let body_centers = body_pose(body, snapshot);
    let joint_segments = body
        .nodes
        .iter()
        .filter_map(|node| {
            node.parent
                .map(|parent_id| (body_centers[parent_id], body_centers[node.id]))
        })
        .collect();

    ReplayFrame {
        time: snapshot.time,
        root_position: snapshot.root_position,
        tilt: snapshot.tilt,
        body_centers,
        joint_segments,
    }
}

fn body_pose(body: &BodyGenome, snapshot: &Snapshot) -> Vec<Vec2> {
    let mut centers = vec![snapshot.root_position; body.nodes.len()];
    let mut rotations = vec![snapshot.tilt; body.nodes.len()];
    let mut joint_angles = vec![0.0; body.nodes.len()];

    for joint in &snapshot.joints {
        if joint.node_id < joint_angles.len() {
            joint_angles[joint.node_id] = joint.angle;
        }
    }

    for node in body.nodes.iter().filter(|node| node.parent.is_some()) {
        let parent_id = node.parent.expect("filtered to nodes with parents");
        let parent = &body.nodes[parent_id];
        let joint_angle = joint_angles[node.id];
        let rotation = rotations[parent_id] + joint_angle;
        let offset = child_center_offset(parent.size, node.size, node.attachment).rotate(rotation);

        centers[node.id] = centers[parent_id] + offset;
        rotations[node.id] = rotation;
    }

    centers
}

fn draw_terrain(grid: &mut [Vec<char>], world: &World, left: f32, top: f32, scale: f32) {
    let height = grid.len();
    let width = grid[0].len();

    for col in 0..width {
        let x = left + col as f32 * scale;
        let y = world.terrain_height(x);
        if let Some(row) = row_for_y(y, top, scale, height) {
            grid[row][col] = '#';
            for ground_row in row + 1..height {
                if grid[ground_row][col] == ' ' {
                    grid[ground_row][col] = '.';
                }
            }
        }
    }
}

fn draw_body(
    grid: &mut [Vec<char>],
    center: Vec2,
    size: Vec2,
    left: f32,
    top: f32,
    scale: f32,
    symbol: char,
) {
    let width = grid[0].len();
    let height = grid.len();
    let Some((center_col, center_row)) =
        point_for_position(center, left, top, scale, width, height)
    else {
        return;
    };

    let half_cols = ((size.x / scale) * 0.25).round().max(1.0) as isize;
    let half_rows = ((size.y / scale) * 0.25).round().max(1.0) as isize;
    for row in center_row - half_rows..=center_row + half_rows {
        for col in center_col - half_cols..=center_col + half_cols {
            plot(grid, col, row, symbol);
        }
    }
}

fn draw_line(
    grid: &mut [Vec<char>],
    start: Vec2,
    end: Vec2,
    left: f32,
    top: f32,
    scale: f32,
    symbol: char,
) {
    let width = grid[0].len();
    let height = grid.len();
    let Some((mut x0, mut y0)) = point_for_position(start, left, top, scale, width, height) else {
        return;
    };
    let Some((x1, y1)) = point_for_position(end, left, top, scale, width, height) else {
        return;
    };

    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut error = dx + dy;

    loop {
        plot(grid, x0, y0, symbol);
        if x0 == x1 && y0 == y1 {
            break;
        }

        let next_error = error * 2;
        if next_error >= dy {
            error += dy;
            x0 += sx;
        }
        if next_error <= dx {
            error += dx;
            y0 += sy;
        }
    }
}

fn point_for_position(
    position: Vec2,
    left: f32,
    top: f32,
    scale: f32,
    width: usize,
    height: usize,
) -> Option<(isize, isize)> {
    let col = ((position.x - left) / scale).round() as isize;
    let row = ((top - position.y) / scale).round() as isize;
    if col >= 0 && col < width as isize && row >= 0 && row < height as isize {
        Some((col, row))
    } else {
        None
    }
}

fn row_for_y(y: f32, top: f32, scale: f32, height: usize) -> Option<usize> {
    let row = ((top - y) / scale).round() as isize;
    if row >= 0 && row < height as isize {
        Some(row as usize)
    } else {
        None
    }
}

fn plot(grid: &mut [Vec<char>], col: isize, row: isize, symbol: char) {
    if row < 0 || row >= grid.len() as isize {
        return;
    }
    if col < 0 || col >= grid[0].len() as isize {
        return;
    }

    grid[row as usize][col as usize] = symbol;
}

#[cfg(test)]
mod tests {
    use crate::{Genome, policy::ControllerKind, rng::Rng};

    use super::{TerminalAnimationConfig, capture_replay, render_frame};

    #[test]
    fn replay_capture_includes_pose_for_each_body_node() {
        let mut rng = Rng::new(21);
        let genome = Genome::minimal(ControllerKind::Cpg, &mut rng);

        let frames = capture_replay(&genome, crate::TaskKind::RoughTerrain, 4, 0.05);

        assert_eq!(frames.len(), 5);
        assert_eq!(frames[0].body_centers.len(), genome.body.body_count());
        assert_eq!(frames[0].joint_segments.len(), genome.body.body_count() - 1);
    }

    #[test]
    fn frame_renderer_draws_creature_and_terrain() {
        let mut rng = Rng::new(22);
        let genome = Genome::minimal(ControllerKind::FeedForward, &mut rng);
        let frame = capture_replay(&genome, crate::TaskKind::RoughTerrain, 1, 0.05).remove(0);

        let rendered = render_frame(
            &frame,
            &genome.body,
            &crate::TaskKind::RoughTerrain.world(),
            TerminalAnimationConfig::default().width,
            TerminalAnimationConfig::default().height,
        );

        assert!(rendered.contains('@'));
        assert!(rendered.contains('#'));
    }
}
