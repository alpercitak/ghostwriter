pub mod build;
pub mod deploy;
pub mod incident;
pub mod review;

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
    #[allow(dead_code)]
    Failed,
}

pub fn hash_seed(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub trait Tick {
    fn tick(&mut self) -> TickResult;
    fn state(&self) -> RenderState<'_>;
}

#[cfg(test)]
mod tests {
    use super::{
        Status, Tick, TickResult, build::BuildMode, deploy::DeployMode, incident::IncidentMode,
        review::ReviewMode,
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
                    .map(|line| {
                        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                        // Build mode reports real wall-clock milliseconds here.
                        if text.contains("compiled successfully in") {
                            text.replace(|c: char| c.is_ascii_digit(), "#")
                        } else {
                            text
                        }
                    })
                    .collect();
            }
        }
        panic!("mode did not finish within {MAX_TICKS} ticks");
    }

    fn run_all(seed: &str) -> [Vec<String>; 4] {
        [
            run_to_done(BuildMode::new(seed)),
            run_to_done(DeployMode::new(seed)),
            run_to_done(IncidentMode::new(seed)),
            run_to_done(ReviewMode::new(seed)),
        ]
    }

    #[test]
    fn every_mode_finishes_for_many_seeds() {
        // Debug builds panic on integer underflow, so this also guards the
        // `N - phase_lines_remaining` arithmetic in every phase.
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
