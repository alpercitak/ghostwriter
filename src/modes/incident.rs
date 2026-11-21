use std::time::Instant;

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::{RenderState, Status, Tick, TickResult, hash_seed};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Detecting,  // error spike alert fires
    Triaging,   // pulling logs, identifying source
    Mitigating, // applying fix (rollback / scale / flag)
    Resolving,  // error rate dropping, monitors green
    Done,
}

impl Phase {
    fn next(self) -> Self {
        match self {
            Phase::Detecting => Phase::Triaging,
            Phase::Triaging => Phase::Mitigating,
            Phase::Mitigating => Phase::Resolving,
            Phase::Resolving => Phase::Done,
            Phase::Done => Phase::Done,
        }
    }
}

pub struct IncidentMode {
    rng: ChaCha8Rng,
    log: Vec<Line<'static>>,
    phase: Phase,
    progress: f64,
    status: Status,
    started_at: Instant,
    wait_ticks: u32,
    phase_lines_remaining: u32,

    service: String,
    error_code: u16,
    error_rate_start: u32, // req/min at spike peak
    runbook_id: String,
}

impl IncidentMode {
    pub fn new(seed: &str) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(hash_seed(seed));

        let service = format!("{}-{}", seed, SUFFIXES[rng.random_range(0..SUFFIXES.len())]);
        let error_code = ERROR_CODES[rng.random_range(0..ERROR_CODES.len())];
        let error_rate_start = rng.random_range(120..980u32);
        let runbook_id = format!("RB-{}", rng.random_range(1000..9999u32));

        let mut s = Self {
            rng,
            log: Vec::new(),
            phase: Phase::Detecting,
            progress: 0.0,
            status: Status::Running,
            started_at: Instant::now(),
            wait_ticks: 0,
            phase_lines_remaining: 4,
            service,
            error_code,
            error_rate_start,
            runbook_id,
        };

        // opening alert line — red, loud
        s.log.push(Line::from(vec![
            Span::styled(
                "  ✖ ALERT  ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("error rate spike on {}", s.service),
                Style::default().fg(Color::Red),
            ),
        ]));
        s.log.push(Line::raw(""));
        s
    }
}

impl Tick for IncidentMode {
    async fn tick(&mut self) -> TickResult {
        if self.phase == Phase::Done {
            return TickResult::Done;
        }

        if self.wait_ticks > 0 {
            self.wait_ticks -= 1;
            return TickResult::Continue;
        }

        match self.phase {
            Phase::Detecting => self.tick_detecting(),
            Phase::Triaging => self.tick_triaging(),
            Phase::Mitigating => self.tick_mitigating(),
            Phase::Resolving => self.tick_resolving(),
            Phase::Done => {}
        }

        TickResult::Continue
    }

    fn state(&self) -> RenderState<'_> {
        RenderState {
            title: "incident response",
            log_lines: &self.log,
            progress: self.progress,
            status: self.status,
            elapsed_secs: self.started_at.elapsed().as_secs(),
        }
    }
}

impl IncidentMode {
    fn tick_detecting(&mut self) {
        let service = self.service.clone();
        let runbook_id = self.runbook_id.clone();
        match 4 - self.phase_lines_remaining {
            0 => {
                self.log.push(Line::from(vec![
                    Span::styled("  severity  ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "SEV-2",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "  auto-paging on-call",
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                self.wait_ticks = self.rng.random_range(6..14);
            }
            1 => {
                self.log.push(Line::from(vec![
                    Span::styled("  runbook   ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("https://runbooks.internal/{runbook_id}")),
                ]));
                self.wait_ticks = self.rng.random_range(4..10);
            }
            2 => {
                self.log.push(Line::from(vec![
                    Span::styled("  rate      ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{} errors/min", self.error_rate_start),
                        Style::default().fg(Color::Red),
                    ),
                    Span::styled(
                        format!("  HTTP {}", self.error_code),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                self.wait_ticks = self.rng.random_range(8..18);
                self.progress = 0.1;
            }
            _ => {
                self.log.push(Line::from(vec![
                    Span::styled("  affects   ", Style::default().fg(Color::DarkGray)),
                    Span::raw(service.to_string()),
                ]));
                self.log.push(Line::raw(""));
                self.wait_ticks = self.rng.random_range(4..8);
                self.progress = 0.15;
            }
        }

        self.phase_lines_remaining -= 1;
        if self.phase_lines_remaining == 0 {
            self.advance_phase();
        }
    }

    fn tick_triaging(&mut self) {
        let service = self.service.clone();
        match 5 - self.phase_lines_remaining {
            0 => {
                self.log.push(Line::from(vec![
                    Span::styled("  ▶ triage  ", Style::default().fg(Color::Magenta)),
                    Span::raw(format!("pulling logs for {service}")),
                ]));
                self.wait_ticks = self.rng.random_range(10..22);
                self.progress = 0.22;
            }
            1 => {
                // fake a stack trace excerpt
                self.log.push(Line::from(Span::styled(
                    format!(
                        "    Error: upstream timeout in {}::{}",
                        service,
                        METHODS[self.rng.random_range(0..METHODS.len())]
                    ),
                    Style::default().fg(Color::Red),
                )));
                self.wait_ticks = self.rng.random_range(4..10);
            }
            2 => {
                let dep = DEPS[self.rng.random_range(0..DEPS.len())];
                self.log.push(Line::from(vec![
                    Span::styled("  source    ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("dependency: {dep}"),
                        Style::default().fg(Color::Yellow),
                    ),
                ]));
                self.wait_ticks = self.rng.random_range(8..16);
                self.progress = 0.38;
            }
            3 => {
                self.log.push(Line::from(vec![
                    Span::styled("  latency   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!(
                            "p99 {}ms  (baseline 40ms)",
                            self.rng.random_range(800..4200u32)
                        ),
                        Style::default().fg(Color::Red),
                    ),
                ]));
                self.wait_ticks = self.rng.random_range(6..12);
                self.progress = 0.44;
            }
            _ => {
                self.log.push(Line::raw(""));
                self.wait_ticks = self.rng.random_range(2..6);
            }
        }

        self.phase_lines_remaining -= 1;
        if self.phase_lines_remaining == 0 {
            self.advance_phase();
        }
    }

    fn tick_mitigating(&mut self) {
        let service = self.service.clone();
        match 4 - self.phase_lines_remaining {
            0 => {
                let action = MITIGATIONS[self.rng.random_range(0..MITIGATIONS.len())];
                self.log.push(Line::from(vec![
                    Span::styled("  ▶ action  ", Style::default().fg(Color::Magenta)),
                    Span::raw(action),
                ]));
                self.wait_ticks = self.rng.random_range(14..30);
                self.progress = 0.55;
            }
            1 => {
                self.log.push(Line::from(vec![
                    Span::styled("  applied   ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("to {service}")),
                ]));
                self.wait_ticks = self.rng.random_range(8..18);
                self.progress = 0.65;
            }
            2 => {
                // error rate dropping
                let reduced = self.error_rate_start / self.rng.random_range(4..10);
                self.log.push(Line::from(vec![
                    Span::styled("  rate      ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{reduced} errors/min"),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled("  ↓ dropping", Style::default().fg(Color::Green)),
                ]));
                self.wait_ticks = self.rng.random_range(10..20);
                self.progress = 0.75;
            }
            _ => {
                self.log.push(Line::raw(""));
                self.wait_ticks = self.rng.random_range(4..8);
            }
        }

        self.phase_lines_remaining -= 1;
        if self.phase_lines_remaining == 0 {
            self.advance_phase();
        }
    }

    fn tick_resolving(&mut self) {
        let service = self.service.clone();
        let runbook_id = self.runbook_id.clone();
        match 4 - self.phase_lines_remaining {
            0 => {
                self.log.push(Line::from(vec![
                    Span::styled("  monitors  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("all green", Style::default().fg(Color::Green)),
                ]));
                self.wait_ticks = self.rng.random_range(8..16);
                self.progress = 0.85;
            }
            1 => {
                self.log.push(Line::from(vec![
                    Span::styled("  rate      ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{} errors/min", self.rng.random_range(0..4u32)),
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled("  (normal)", Style::default().fg(Color::DarkGray)),
                ]));
                self.wait_ticks = self.rng.random_range(6..14);
                self.progress = 0.93;
            }
            2 => {
                let elapsed = self.started_at.elapsed().as_secs();
                self.log.push(Line::raw(""));
                self.log.push(Line::from(vec![Span::styled(
                    format!(
                        "  incident resolved  {}  TTR {}m{}s",
                        service,
                        elapsed / 60,
                        elapsed % 60,
                    ),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )]));
                self.log.push(Line::from(vec![Span::styled(
                    format!("  post-mortem  https://runbooks.internal/{runbook_id}/postmortem"),
                    Style::default().fg(Color::DarkGray),
                )]));
                self.progress = 1.0;
                self.status = Status::Success;
                self.wait_ticks = 0;
            }
            _ => {}
        }

        self.phase_lines_remaining -= 1;
        if self.phase_lines_remaining == 0 {
            self.advance_phase();
        }
    }

    fn advance_phase(&mut self) {
        self.phase = self.phase.next();
        self.phase_lines_remaining = match self.phase {
            Phase::Detecting => 4,
            Phase::Triaging => 5,
            Phase::Mitigating => 4,
            Phase::Resolving => 4,
            Phase::Done => 0,
        };
    }
}

static SUFFIXES: &[&str] = &["api", "worker", "gateway", "service", "backend"];
static ERROR_CODES: &[u16] = &[500, 502, 503, 504];
static METHODS: &[&str] = &[
    "handleRequest",
    "fetchUpstream",
    "processPayload",
    "validateAuth",
    "routeRequest",
    "forwardToQueue",
];
static DEPS: &[&str] = &[
    "postgres-primary",
    "redis-cluster",
    "kafka-broker",
    "payment-gateway",
    "auth-service",
    "object-storage",
];
static MITIGATIONS: &[&str] = &[
    "rolling back to previous deployment",
    "disabling feature flag: new-checkout-flow",
    "scaling up replica count  3 → 8",
    "switching to secondary upstream",
    "enabling circuit breaker on dependency",
];
