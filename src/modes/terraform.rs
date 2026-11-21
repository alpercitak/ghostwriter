use std::collections::HashSet;

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::{Sim, Status, Tick};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Init,    // backend + provider plugins
    Refresh, // refreshing state of existing resources
    Plan,    // execution plan with +/~/- changes
    Apply,   // creating / modifying / destroying, one resource at a time
    Done,
}

#[derive(Clone, Copy, PartialEq)]
enum Action {
    Create,
    Update,
    Destroy,
}

struct Resource {
    address: String,
    id: String,
    action: Action,
}

pub struct TerraformMode {
    sim: Sim,
    phase: Phase,

    provider_version: String,
    resources: Vec<Resource>,
    /// Indices of resources that already exist (and so get refreshed).
    existing: Vec<usize>,
    /// Ticks waited on the resource currently being applied.
    apply_wait: u32,
}

impl TerraformMode {
    pub fn new(seed: &str) -> Self {
        let mut sim = Sim::new(seed);
        let rng = &mut sim.rng;

        let provider_version = format!(
            "v5.{}.{}",
            rng.random_range(40..90u8),
            rng.random_range(0..10u8)
        );
        let prefix = seed.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
        let count = rng.random_range(4..9usize);
        let mut resources: Vec<Resource> = (0..count)
            .map(|_| {
                let (kind, id_prefix) = KINDS[rng.random_range(0..KINDS.len())];
                let name = NAMES[rng.random_range(0..NAMES.len())];
                let action = match rng.random_range(0..10u8) {
                    0..6 => Action::Create,
                    6..9 => Action::Update,
                    _ => Action::Destroy,
                };
                Resource {
                    address: format!("{kind}.{prefix}_{name}"),
                    id: format!("{id_prefix}{}", random_hex(rng, 12)),
                    action,
                }
            })
            .collect();
        // Addresses are unique in real Terraform state.
        let mut seen = HashSet::new();
        resources.retain(|r| seen.insert(r.address.clone()));
        // Always have something to refresh.
        if resources.iter().all(|r| r.action == Action::Create) {
            resources[0].action = Action::Update;
        }
        let existing = (0..resources.len())
            .filter(|&i| resources[i].action != Action::Create)
            .collect();

        let mut s = Self {
            sim,
            phase: Phase::Init,
            provider_version,
            resources,
            existing,
            apply_wait: 0,
        };

        let workspace = WORKSPACES[s.sim.rng.random_range(0..WORKSPACES.len())];
        s.sim.log.push(Line::from(vec![
            Span::styled("  $ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "terraform apply",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  workspace: {workspace}"),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        s.sim.log.push(Line::raw(""));
        s
    }

    fn count(&self, action: Action) -> usize {
        self.resources.iter().filter(|r| r.action == action).count()
    }
}

impl Tick for TerraformMode {
    const TITLE: &'static str = "terraform apply";
    const SUCCESS_MESSAGE: &'static str = "apply complete";

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
            Phase::Init => self.tick_init(),
            Phase::Refresh => self.tick_refresh(),
            Phase::Plan => self.tick_plan(),
            Phase::Apply => self.tick_apply(),
            Phase::Done => {}
        }
    }
}

impl TerraformMode {
    fn tick_init(&mut self) {
        let sim = &mut self.sim;
        match sim.step {
            0 => {
                sim.log.push(Line::styled(
                    "  Initializing the backend...",
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                sim.wait_ticks = sim.rng.random_range(4..10);
            }
            1 => {
                sim.log.push(Line::styled(
                    "  Initializing provider plugins...",
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                sim.wait_ticks = sim.rng.random_range(3..8);
            }
            2 => {
                sim.log.push(Line::raw(format!(
                    "  - Installing hashicorp/aws {}...",
                    self.provider_version
                )));
                sim.wait_ticks = sim.rng.random_range(10..25);
                sim.progress = 0.05;
            }
            _ => {
                sim.log.push(Line::styled(
                    "  Terraform has been successfully initialized!",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ));
                sim.log.push(Line::raw(""));
                sim.wait_ticks = sim.rng.random_range(4..8);
                sim.progress = 0.1;
            }
        }
        if self.sim.next_step(4) {
            self.phase = Phase::Refresh;
        }
    }

    fn tick_refresh(&mut self) {
        let r = &self.resources[self.existing[self.sim.step as usize]];
        self.sim.log.push(Line::from(vec![
            Span::raw(format!("  {}: ", r.address)),
            Span::styled(
                format!("Refreshing state... [id={}]", r.id),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        self.sim.wait_ticks = self.sim.rng.random_range(2..8);
        let total = self.existing.len() as u32;
        self.sim.progress = 0.1 + (self.sim.step + 1) as f64 / total as f64 * 0.15;

        if self.sim.next_step(total) {
            self.sim.log.push(Line::raw(""));
            self.phase = Phase::Plan;
        }
    }

    fn tick_plan(&mut self) {
        let n = self.resources.len() as u32;
        let step = self.sim.step;
        if step == 0 {
            self.sim
                .log
                .push(Line::raw("  Terraform will perform the following actions:"));
            self.sim.log.push(Line::raw(""));
            self.sim.wait_ticks = self.sim.rng.random_range(8..16);
        } else if step <= n {
            let r = &self.resources[step as usize - 1];
            let (sign, color, note) = match r.action {
                Action::Create => ("+", Color::Green, " will be created"),
                Action::Update => ("~", Color::Yellow, " will be updated in-place"),
                Action::Destroy => ("-", Color::Red, " will be destroyed"),
            };
            self.sim.log.push(Line::from(vec![
                Span::styled(format!("    {sign} "), Style::default().fg(color)),
                Span::styled(r.address.clone(), Style::default().fg(color)),
                Span::styled(note, Style::default().fg(Color::DarkGray)),
            ]));
            self.sim.wait_ticks = self.sim.rng.random_range(1..4);
            self.sim.progress = 0.25 + step as f64 / n as f64 * 0.15;
        } else {
            let summary = format!(
                "  Plan: {} to add, {} to change, {} to destroy.",
                self.count(Action::Create),
                self.count(Action::Update),
                self.count(Action::Destroy),
            );
            self.sim.log.push(Line::raw(""));
            self.sim.log.push(Line::styled(
                summary,
                Style::default().add_modifier(Modifier::BOLD),
            ));
            self.sim.log.push(Line::raw(""));
            self.sim.wait_ticks = self.sim.rng.random_range(10..20);
        }
        if self.sim.next_step(n + 2) {
            self.phase = Phase::Apply;
        }
    }

    fn tick_apply(&mut self) {
        let n = self.resources.len() as u32;
        let step = self.sim.step;

        if step == 2 * n {
            let summary = format!(
                "  Apply complete! Resources: {} added, {} changed, {} destroyed.",
                self.count(Action::Create),
                self.count(Action::Update),
                self.count(Action::Destroy),
            );
            self.sim.log.push(Line::raw(""));
            self.sim.log.push(Line::styled(
                summary,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
            self.sim.progress = 1.0;
            self.sim.status = Status::Success;
            self.phase = Phase::Done;
            return;
        }

        let r = &self.resources[(step / 2) as usize];
        if step.is_multiple_of(2) {
            let started = match r.action {
                Action::Create => "Creating...".to_string(),
                Action::Update => format!("Modifying... [id={}]", r.id),
                Action::Destroy => format!("Destroying... [id={}]", r.id),
            };
            self.sim.log.push(Line::from(vec![
                Span::raw(format!("  {}: ", r.address)),
                Span::styled(started, Style::default().fg(Color::DarkGray)),
            ]));
            self.apply_wait = self.sim.rng.random_range(10..60);
            self.sim.wait_ticks = self.apply_wait;
        } else {
            // Report the time actually spent waiting (in simulated seconds).
            let secs = (super::TICK * (self.apply_wait + 1)).as_secs();
            let finished = match r.action {
                Action::Create => format!("Creation complete after {secs}s [id={}]", r.id),
                Action::Update => format!("Modifications complete after {secs}s [id={}]", r.id),
                Action::Destroy => format!("Destruction complete after {secs}s"),
            };
            self.sim.log.push(Line::from(vec![
                Span::raw(format!("  {}: ", r.address)),
                Span::styled(finished, Style::default().fg(Color::Green)),
            ]));
            self.sim.wait_ticks = self.sim.rng.random_range(1..4);
            self.sim.progress = 0.4 + (step + 1) as f64 / (2 * n) as f64 * 0.58;
        }
        self.sim.step += 1;
    }
}

fn random_hex(rng: &mut ChaCha8Rng, len: usize) -> String {
    (0..len)
        .map(|_| char::from_digit(rng.random_range(0..16), 16).expect("hex digit"))
        .collect()
}

/// Resource type and the prefix of its provider-assigned id.
static KINDS: &[(&str, &str)] = &[
    ("aws_instance", "i-"),
    ("aws_security_group", "sg-"),
    ("aws_s3_bucket", "bkt-"),
    ("aws_iam_role", "role-"),
    ("aws_lb", "lb-"),
    ("aws_route53_record", "rec-"),
    ("aws_ecs_service", "svc-"),
    ("aws_cloudwatch_log_group", "lg-"),
];
static NAMES: &[&str] = &[
    "web", "api", "worker", "logs", "assets", "ingress", "db", "cache",
];
static WORKSPACES: &[&str] = &["production", "staging", "dev"];
