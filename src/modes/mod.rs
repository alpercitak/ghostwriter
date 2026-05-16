pub mod build;
pub mod deploy;

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

pub trait Tick {
    async fn tick(&mut self) -> TickResult;
    fn state(&self) -> RenderState<'_>;
}
