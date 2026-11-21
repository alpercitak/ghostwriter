pub mod build;
pub mod deploy;
pub mod incident;
pub mod migrate;
pub mod review;
pub mod terraform;

use std::time::Duration;

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use ratatui::text::Line;

pub enum TickResult {
    Continue,
    Done,
}

pub struct RenderState<'a> {
    pub title: &'a str,
    /// Shown in the status line once the run finishes successfully.
    pub success_message: &'a str,
    pub log_lines: &'a [Line<'static>],
    pub progress: f64,
    pub status: Status,
    pub elapsed_secs: u64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Status {
    Running,
    Success,
}

/// Simulated time per tick at 1x speed. Playback speed only changes how fast
/// ticks are played, so simulated timings stay identical for a given seed.
pub const TICK: Duration = Duration::from_millis(100);

fn hash_seed(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Simulation state shared by every mode.
pub struct Sim {
    pub rng: ChaCha8Rng,
    pub log: Vec<Line<'static>>,
    pub progress: f64,
    pub status: Status,
    /// Ticks elapsed since the start, including idle ones.
    pub ticks: u32,
    /// Ticks to idle before the next step runs.
    pub wait_ticks: u32,
    /// Index of the current step within the current phase.
    pub step: u32,
}

impl Sim {
    pub fn new(seed: &str) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(hash_seed(seed)),
            log: Vec::new(),
            progress: 0.0,
            status: Status::Running,
            ticks: 0,
            wait_ticks: 0,
            step: 0,
        }
    }

    /// Simulated time since the start, independent of playback speed.
    pub fn elapsed(&self) -> Duration {
        TICK * self.ticks
    }

    /// Finish the current step. Returns true (and resets the step counter)
    /// once all `steps` steps of the current phase have run.
    pub fn next_step(&mut self, steps: u32) -> bool {
        self.step += 1;
        if self.step >= steps {
            self.step = 0;
            true
        } else {
            false
        }
    }
}

/// A simulated activity. Modes provide their content via `step`; pacing and
/// rendering state come from the provided methods.
pub trait Tick {
    const TITLE: &'static str;
    /// Shown in the status line once the run finishes successfully.
    const SUCCESS_MESSAGE: &'static str;

    fn sim(&self) -> &Sim;
    fn sim_mut(&mut self) -> &mut Sim;
    fn is_done(&self) -> bool;
    /// Emit the next step of the current phase.
    fn step(&mut self);

    fn tick(&mut self) -> TickResult {
        if self.is_done() {
            return TickResult::Done;
        }
        let sim = self.sim_mut();
        sim.ticks += 1;
        if sim.wait_ticks > 0 {
            sim.wait_ticks -= 1;
            return TickResult::Continue;
        }
        self.step();
        TickResult::Continue
    }

    fn state(&self) -> RenderState<'_> {
        let sim = self.sim();
        RenderState {
            title: Self::TITLE,
            success_message: Self::SUCCESS_MESSAGE,
            log_lines: &sim.log,
            progress: sim.progress,
            status: sim.status,
            elapsed_secs: sim.elapsed().as_secs(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Status, Tick, TickResult, build::BuildMode, deploy::DeployMode, incident::IncidentMode,
        migrate::MigrateMode, review::ReviewMode, terraform::TerraformMode,
    };

    /// Far more ticks than any mode needs; hitting it means a mode never finishes.
    const MAX_TICKS: usize = 100_000;

    /// Drive a mode to completion headlessly and return its log as plain text.
    fn run_to_done(mut mode: impl Tick) -> Vec<String> {
        for _ in 0..MAX_TICKS {
            if let TickResult::Done = mode.tick() {
                let state = mode.state();
                assert!(state.status == Status::Success, "finished without success");
                assert!(
                    (state.progress - 1.0).abs() < 1e-9,
                    "progress {}",
                    state.progress
                );
                return state
                    .log_lines
                    .iter()
                    .map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect())
                    .collect();
            }
        }
        panic!("mode did not finish within {MAX_TICKS} ticks");
    }

    fn run_all(seed: &str) -> Vec<Vec<String>> {
        vec![
            run_to_done(BuildMode::new(seed)),
            run_to_done(DeployMode::new(seed)),
            run_to_done(IncidentMode::new(seed)),
            run_to_done(ReviewMode::new(seed)),
            run_to_done(TerraformMode::new(seed)),
            run_to_done(MigrateMode::new(seed)),
        ]
    }

    #[test]
    fn every_mode_finishes_for_many_seeds() {
        // Debug builds panic on integer overflow, so this also guards the
        // per-phase step arithmetic.
        for i in 0..50 {
            run_all(&format!("seed-{i}"));
        }
    }

    #[test]
    fn same_seed_gives_same_output() {
        assert_eq!(run_all("payments-api"), run_all("payments-api"));
    }

    #[test]
    fn different_seeds_give_different_output() {
        let a = run_all("payments-api");
        let b = run_all("my-app");
        for (x, y) in a.iter().zip(&b) {
            assert_ne!(x, y);
        }
    }
}
