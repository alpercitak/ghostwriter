use rand::prelude::*;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::{Sim, Status, Tick};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Detecting,  // error spike alert fires
    Triaging,   // pulling logs, identifying source
    Mitigating, // applying fix (rollback / scale / flag)
    Resolving,  // error rate dropping, monitors green
    Done,
}

pub struct IncidentMode {
    sim: Sim,
    phase: Phase,

    service: String,
    error_code: u16,
    error_rate_start: u32, // req/min at spike peak
    runbook_id: String,
}

impl IncidentMode {
    pub fn new(seed: &str) -> Self {
        let mut sim = Sim::new(seed);

        let service = format!(
            "{}-{}",
            seed,
            SUFFIXES[sim.rng.random_range(0..SUFFIXES.len())]
        );
        let error_code = ERROR_CODES[sim.rng.random_range(0..ERROR_CODES.len())];
        let error_rate_start = sim.rng.random_range(120..980u32);
        let runbook_id = format!("RB-{}", sim.rng.random_range(1000..9999u32));

        let mut s = Self {
            sim,
            phase: Phase::Detecting,
            service,
            error_code,
            error_rate_start,
            runbook_id,
        };

        // opening alert line — red, loud
        s.sim.log.push(Line::from(vec![
            Span::styled(
                "  ✖ ALERT  ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("error rate spike on {}", s.service),
                Style::default().fg(Color::Red),
            ),
        ]));
        s.sim.log.push(Line::raw(""));
        s
    }
}

impl Tick for IncidentMode {
    const TITLE: &'static str = "incident response";
    const SUCCESS_MESSAGE: &'static str = "incident resolved";

    fn sim(&self) -> &Sim {
        &self.sim
    }

    fn sim_mut(&mut self) -> &mut Sim {
        &mut self.sim
    }

    fn is_done(&self) -> bool {
        self.phase == Phase::Done
    }

    fn step(&mut self) {
        match self.phase {
            Phase::Detecting => self.tick_detecting(),
            Phase::Triaging => self.tick_triaging(),
            Phase::Mitigating => self.tick_mitigating(),
            Phase::Resolving => self.tick_resolving(),
            Phase::Done => {}
        }
    }
}

impl IncidentMode {
    fn tick_detecting(&mut self) {
        let service = self.service.clone();
        let runbook_id = self.runbook_id.clone();
        match self.sim.step {
            0 => {
                self.sim.log.push(Line::from(vec![
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
                self.sim.wait_ticks = self.sim.rng.random_range(6..14);
            }
            1 => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  runbook   ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("https://runbooks.internal/{runbook_id}")),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(4..10);
            }
            2 => {
                self.sim.log.push(Line::from(vec![
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
                self.sim.wait_ticks = self.sim.rng.random_range(8..18);
                self.sim.progress = 0.1;
            }
            _ => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  affects   ", Style::default().fg(Color::DarkGray)),
                    Span::raw(service.to_string()),
                ]));
                self.sim.log.push(Line::raw(""));
                self.sim.wait_ticks = self.sim.rng.random_range(4..8);
                self.sim.progress = 0.15;
            }
        }

        if self.sim.next_step(4) {
            self.phase = Phase::Triaging;
        }
    }

    fn tick_triaging(&mut self) {
        let service = self.service.clone();
        match self.sim.step {
            0 => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  ▶ triage  ", Style::default().fg(Color::Magenta)),
                    Span::raw(format!("pulling logs for {service}")),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(10..22);
                self.sim.progress = 0.22;
            }
            1 => {
                // fake a stack trace excerpt
                self.sim.log.push(Line::from(Span::styled(
                    format!(
                        "    Error: upstream timeout in {}::{}",
                        service,
                        METHODS[self.sim.rng.random_range(0..METHODS.len())]
                    ),
                    Style::default().fg(Color::Red),
                )));
                self.sim.wait_ticks = self.sim.rng.random_range(4..10);
            }
            2 => {
                let dep = DEPS[self.sim.rng.random_range(0..DEPS.len())];
                self.sim.log.push(Line::from(vec![
                    Span::styled("  source    ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("dependency: {dep}"),
                        Style::default().fg(Color::Yellow),
                    ),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(8..16);
                self.sim.progress = 0.38;
            }
            3 => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  latency   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!(
                            "p99 {}ms  (baseline 40ms)",
                            self.sim.rng.random_range(800..4200u32)
                        ),
                        Style::default().fg(Color::Red),
                    ),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(6..12);
                self.sim.progress = 0.44;
            }
            _ => {
                self.sim.log.push(Line::raw(""));
                self.sim.wait_ticks = self.sim.rng.random_range(2..6);
            }
        }

        if self.sim.next_step(5) {
            self.phase = Phase::Mitigating;
        }
    }

    fn tick_mitigating(&mut self) {
        let service = self.service.clone();
        match self.sim.step {
            0 => {
                let action = MITIGATIONS[self.sim.rng.random_range(0..MITIGATIONS.len())];
                self.sim.log.push(Line::from(vec![
                    Span::styled("  ▶ action  ", Style::default().fg(Color::Magenta)),
                    Span::raw(action),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(14..30);
                self.sim.progress = 0.55;
            }
            1 => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  applied   ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("to {service}")),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(8..18);
                self.sim.progress = 0.65;
            }
            2 => {
                // error rate dropping
                let reduced = self.error_rate_start / self.sim.rng.random_range(4..10);
                self.sim.log.push(Line::from(vec![
                    Span::styled("  rate      ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{reduced} errors/min"),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled("  ↓ dropping", Style::default().fg(Color::Green)),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(10..20);
                self.sim.progress = 0.75;
            }
            _ => {
                self.sim.log.push(Line::raw(""));
                self.sim.wait_ticks = self.sim.rng.random_range(4..8);
            }
        }

        if self.sim.next_step(4) {
            self.phase = Phase::Resolving;
        }
    }

    fn tick_resolving(&mut self) {
        let service = self.service.clone();
        let runbook_id = self.runbook_id.clone();
        match self.sim.step {
            0 => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  monitors  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("all green", Style::default().fg(Color::Green)),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(8..16);
                self.sim.progress = 0.85;
            }
            1 => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  rate      ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{} errors/min", self.sim.rng.random_range(0..4u32)),
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled("  (normal)", Style::default().fg(Color::DarkGray)),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(6..14);
                self.sim.progress = 0.93;
            }
            2 => {
                let elapsed = self.sim.started_at.elapsed().as_secs();
                self.sim.log.push(Line::raw(""));
                self.sim.log.push(Line::from(vec![Span::styled(
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
                self.sim.log.push(Line::from(vec![Span::styled(
                    format!("  post-mortem  https://runbooks.internal/{runbook_id}/postmortem"),
                    Style::default().fg(Color::DarkGray),
                )]));
                self.sim.progress = 1.0;
                self.sim.status = Status::Success;
                self.sim.wait_ticks = 0;
            }
            _ => {}
        }

        if self.sim.next_step(4) {
            self.phase = Phase::Done;
        }
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
