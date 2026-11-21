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
    Pulling,   // docker image pull
    Starting,  // pod scheduling
    Rolling,   // replica rollout with health checks
    Verifying, // readiness/liveness probes
    Done,
}

impl Phase {
    fn next(self) -> Self {
        match self {
            Phase::Pulling => Phase::Starting,
            Phase::Starting => Phase::Rolling,
            Phase::Rolling => Phase::Verifying,
            Phase::Verifying => Phase::Done,
            Phase::Done => Phase::Done,
        }
    }
}

pub struct DeployMode {
    rng: ChaCha8Rng,
    log: Vec<Line<'static>>,
    phase: Phase,
    progress: f64,
    status: Status,
    started_at: Instant,
    wait_ticks: u32,
    phase_lines_remaining: u32,

    image_tag: String,
    namespace: String,
    deployment_name: String,
    total_replicas: u32,
    rolled_replicas: u32,
}

impl DeployMode {
    pub fn new(seed: &str) -> Self {
        let seed_u64 = hash_seed(seed);
        let mut rng = ChaCha8Rng::seed_from_u64(seed_u64);

        let image_tag = format!(
            "{}:{}.{}.{}",
            seed,
            rng.random_range(1..4u8),
            rng.random_range(0..20u8),
            rng.random_range(0..99u8),
        );
        let namespace = NAMESPACES[rng.random_range(0..NAMESPACES.len())].to_string();
        let deployment_name = format!("{}-{}", seed, SUFFIXES[rng.random_range(0..SUFFIXES.len())]);
        let total_replicas = rng.random_range(2..6u32);

        let mut s = Self {
            rng,
            log: Vec::new(),
            phase: Phase::Pulling,
            progress: 0.0,
            status: Status::Running,
            started_at: Instant::now(),
            wait_ticks: 0,
            phase_lines_remaining: 3,
            image_tag,
            namespace,
            deployment_name,
            total_replicas,
            rolled_replicas: 0,
        };

        s.log.push(Line::from(vec![
            Span::styled("  deployment  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                s.deployment_name.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  namespace/{}", s.namespace),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        s.log.push(Line::raw(""));

        s
    }
}

impl Tick for DeployMode {
    async fn tick(&mut self) -> TickResult {
        if self.phase == Phase::Done {
            return TickResult::Done;
        }

        if self.wait_ticks > 0 {
            self.wait_ticks -= 1;
            return TickResult::Continue;
        }

        match self.phase {
            Phase::Pulling => self.tick_pulling(),
            Phase::Starting => self.tick_starting(),
            Phase::Rolling => self.tick_rolling(),
            Phase::Verifying => self.tick_verifying(),
            Phase::Done => {}
        }

        TickResult::Continue
    }

    fn state(&self) -> RenderState<'_> {
        RenderState {
            title: "kubectl rollout",
            success_message: "rollout complete",
            log_lines: &self.log,
            progress: self.progress,
            status: self.status,
            elapsed_secs: self.started_at.elapsed().as_secs(),
        }
    }
}

impl DeployMode {
    fn tick_pulling(&mut self) {
        let image_tag = self.image_tag.clone();
        match 3 - self.phase_lines_remaining {
            0 => {
                self.log.push(Line::from(vec![
                    Span::styled("  Pulling  ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("image \"{image_tag}\"")),
                ]));
                self.wait_ticks = self.rng.random_range(10..25);
            }
            1 => {
                let digest = random_sha(&mut self.rng, 12);
                self.log.push(Line::from(vec![
                    Span::styled("  Pulled   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("sha256:{digest}"),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                self.wait_ticks = self.rng.random_range(4..10);
            }
            _ => {
                self.log.push(Line::from(vec![
                    Span::styled("  Created  ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("container from image \"{image_tag}\"")),
                ]));
                self.wait_ticks = self.rng.random_range(2..6);
                self.progress = 0.15;
            }
        }

        self.phase_lines_remaining -= 1;
        if self.phase_lines_remaining == 0 {
            self.log.push(Line::raw(""));
            self.advance_phase();
        }
    }

    fn tick_starting(&mut self) {
        let deployment_name = self.deployment_name.clone();
        let namespace = self.namespace.clone();
        match 3 - self.phase_lines_remaining {
            0 => {
                self.log.push(Line::from(vec![
                    Span::styled("  Scaled   ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!(
                        "deployment/{} to {} replicas",
                        deployment_name, self.total_replicas
                    )),
                ]));
                self.wait_ticks = self.rng.random_range(5..15);
            }
            1 => {
                self.log.push(Line::from(vec![
                    Span::styled("  Waiting  ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!(
                        "for rollout of deployment/{deployment_name} in {namespace}"
                    )),
                ]));
                self.wait_ticks = self.rng.random_range(8..20);
            }
            _ => {
                self.log.push(Line::from(vec![
                    Span::styled("  Scheduled ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("{} pods", self.total_replicas)),
                ]));
                self.wait_ticks = self.rng.random_range(3..8);
                self.progress = 0.3;
            }
        }

        self.phase_lines_remaining -= 1;
        if self.phase_lines_remaining == 0 {
            self.log.push(Line::raw(""));
            self.advance_phase();
        }
    }

    fn tick_rolling(&mut self) {
        let pod_name = format!(
            "{}-{}-{}",
            self.deployment_name,
            random_hex(&mut self.rng, 5),
            random_hex(&mut self.rng, 5),
        );

        let slow = self.rng.random_bool(0.2);

        self.log.push(Line::from(vec![
            Span::styled("  ●  ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("pod/{pod_name}")),
            if slow {
                Span::styled(
                    "  waiting for readiness…",
                    Style::default().fg(Color::Yellow),
                )
            } else {
                Span::styled("  Running", Style::default().fg(Color::Green))
            },
        ]));

        self.rolled_replicas += 1;
        self.progress = 0.3 + (self.rolled_replicas as f64 / self.total_replicas as f64) * 0.45;
        self.wait_ticks = if slow {
            self.rng.random_range(20..45)
        } else {
            self.rng.random_range(6..18)
        };

        self.phase_lines_remaining -= 1;
        if self.phase_lines_remaining == 0 {
            self.log.push(Line::raw(""));
            self.advance_phase();
        }
    }

    fn tick_verifying(&mut self) {
        let deployment_name = self.deployment_name.clone();
        match 3 - self.phase_lines_remaining {
            0 => {
                self.log.push(Line::from(vec![
                    Span::styled("  Probing  ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("liveness on deployment/{deployment_name}")),
                ]));
                self.wait_ticks = self.rng.random_range(10..20);
                self.progress = 0.82;
            }
            1 => {
                self.log.push(Line::from(vec![
                    Span::styled("  Probing  ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("readiness on deployment/{deployment_name}")),
                ]));
                self.wait_ticks = self.rng.random_range(8..18);
                self.progress = 0.91;
            }
            _ => {
                let elapsed = self.started_at.elapsed().as_secs_f64();
                self.log.push(Line::raw(""));
                self.log.push(Line::from(vec![Span::styled(
                    format!(
                        "  deployment/{deployment_name} successfully rolled out  ({elapsed:.0}s)",
                    ),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )]));
                self.progress = 1.0;
                self.status = Status::Success;
                self.wait_ticks = self.rng.random_range(4..10);
            }
        }

        self.phase_lines_remaining -= 1;
        if self.phase_lines_remaining == 0 {
            self.advance_phase();
        }
    }

    fn advance_phase(&mut self) {
        self.phase = self.phase.next();
        self.phase_lines_remaining = match self.phase {
            Phase::Pulling => 3,
            Phase::Starting => 3,
            Phase::Rolling => self.total_replicas,
            Phase::Verifying => 3,
            Phase::Done => 0,
        };
    }
}

fn random_sha(rng: &mut ChaCha8Rng, len: usize) -> String {
    (0..len)
        .map(|_| format!("{:02x}", rng.random::<u8>()))
        .collect()
}

fn random_hex(rng: &mut ChaCha8Rng, len: usize) -> String {
    (0..len)
        .map(|_| {
            let c = b"abcdefghijklmnopqrstuvwxyz0123456789";
            c[rng.random_range(0..c.len())] as char
        })
        .collect()
}

static NAMESPACES: &[&str] = &["production", "staging", "default", "platform", "infra"];
static SUFFIXES: &[&str] = &[
    "api",
    "worker",
    "gateway",
    "service",
    "backend",
    "scheduler",
];
