pub mod build;
pub mod deploy;
pub mod incident;
pub mod review;

use ratatui::{
    text::{Line},
};

pub enum TickResult {
    Continue,
    Done,
}

pub struct RenderState<'a> {
    pub title: &'a str,
    pub log_lines: &'a [Line<'static>],
    pub progress: f64,
    pub status: Status,
    pub elapsed_secs: u64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Status {
    Running,
    Success,
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
    async fn tick(&mut self) -> TickResult;
    fn state(&self) -> RenderState<'_>;
}
