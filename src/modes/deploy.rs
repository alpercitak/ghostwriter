use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::{Sim, Status, Tick};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Pulling,   // docker image pull
    Starting,  // pod scheduling
    Rolling,   // replica rollout with health checks
    Verifying, // readiness/liveness probes
    Done,
}

pub struct DeployMode {
    sim: Sim,
    phase: Phase,

    image_tag: String,
    namespace: String,
    deployment_name: String,
    total_replicas: u32,
    rolled_replicas: u32,
}

impl DeployMode {
    pub fn new(seed: &str) -> Self {
        let mut sim = Sim::new(seed);

        let image_tag = format!(
            "{}:{}.{}.{}",
            seed,
            sim.rng.random_range(1..4u8),
            sim.rng.random_range(0..20u8),
            sim.rng.random_range(0..99u8),
        );
        let namespace = NAMESPACES[sim.rng.random_range(0..NAMESPACES.len())].to_string();
        let deployment_name = format!(
            "{}-{}",
            seed,
            SUFFIXES[sim.rng.random_range(0..SUFFIXES.len())]
        );
        let total_replicas = sim.rng.random_range(2..6u32);

        let mut s = Self {
            sim,
            phase: Phase::Pulling,
            image_tag,
            namespace,
            deployment_name,
            total_replicas,
            rolled_replicas: 0,
        };

        s.sim.log.push(Line::from(vec![
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
        s.sim.log.push(Line::raw(""));

        s
    }
}

impl Tick for DeployMode {
    const TITLE: &'static str = "kubectl rollout";
    const SUCCESS_MESSAGE: &'static str = "rollout complete";

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
            Phase::Pulling => self.tick_pulling(),
            Phase::Starting => self.tick_starting(),
            Phase::Rolling => self.tick_rolling(),
            Phase::Verifying => self.tick_verifying(),
            Phase::Done => {}
        }
    }
}

impl DeployMode {
    fn tick_pulling(&mut self) {
        let image_tag = self.image_tag.clone();
        match self.sim.step {
            0 => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  Pulling  ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("image \"{image_tag}\"")),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(10..25);
            }
            1 => {
                let digest = random_sha(&mut self.sim.rng, 12);
                self.sim.log.push(Line::from(vec![
                    Span::styled("  Pulled   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("sha256:{digest}"),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(4..10);
            }
            _ => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  Created  ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("container from image \"{image_tag}\"")),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(2..6);
                self.sim.progress = 0.15;
            }
        }

        if self.sim.next_step(3) {
            self.sim.log.push(Line::raw(""));
            self.phase = Phase::Starting;
        }
    }

    fn tick_starting(&mut self) {
        let deployment_name = self.deployment_name.clone();
        let namespace = self.namespace.clone();
        match self.sim.step {
            0 => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  Scaled   ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!(
                        "deployment/{} to {} replicas",
                        deployment_name, self.total_replicas
                    )),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(5..15);
            }
            1 => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  Waiting  ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!(
                        "for rollout of deployment/{deployment_name} in {namespace}"
                    )),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(8..20);
            }
            _ => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  Scheduled ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("{} pods", self.total_replicas)),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(3..8);
                self.sim.progress = 0.3;
            }
        }

        if self.sim.next_step(3) {
            self.sim.log.push(Line::raw(""));
            self.phase = Phase::Rolling;
        }
    }

    fn tick_rolling(&mut self) {
        let pod_name = format!(
            "{}-{}-{}",
            self.deployment_name,
            random_hex(&mut self.sim.rng, 5),
            random_hex(&mut self.sim.rng, 5),
        );

        let slow = self.sim.rng.random_bool(0.2);

        self.sim.log.push(Line::from(vec![
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
        self.sim.progress = 0.3 + (self.rolled_replicas as f64 / self.total_replicas as f64) * 0.45;
        self.sim.wait_ticks = if slow {
            self.sim.rng.random_range(20..45)
        } else {
            self.sim.rng.random_range(6..18)
        };

        if self.sim.next_step(self.total_replicas) {
            self.sim.log.push(Line::raw(""));
            self.phase = Phase::Verifying;
        }
    }

    fn tick_verifying(&mut self) {
        let deployment_name = self.deployment_name.clone();
        match self.sim.step {
            0 => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  Probing  ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("liveness on deployment/{deployment_name}")),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(10..20);
                self.sim.progress = 0.82;
            }
            1 => {
                self.sim.log.push(Line::from(vec![
                    Span::styled("  Probing  ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format!("readiness on deployment/{deployment_name}")),
                ]));
                self.sim.wait_ticks = self.sim.rng.random_range(8..18);
                self.sim.progress = 0.91;
            }
            _ => {
                let elapsed = self.sim.elapsed().as_secs_f64();
                self.sim.log.push(Line::raw(""));
                self.sim.log.push(Line::from(vec![Span::styled(
                    format!(
                        "  deployment/{deployment_name} successfully rolled out  ({elapsed:.0}s)",
                    ),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )]));
                self.sim.progress = 1.0;
                self.sim.status = Status::Success;
                self.sim.wait_ticks = self.sim.rng.random_range(4..10);
            }
        }

        if self.sim.next_step(3) {
            self.phase = Phase::Done;
        }
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
