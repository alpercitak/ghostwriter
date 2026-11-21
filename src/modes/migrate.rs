use rand::prelude::*;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::{Sim, Status, TICK, Tick};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Connect,  // connect, lock, read current version
    Pending,  // list migrations to run
    Apply,    // run each migration
    Finalize, // schema dump, unlock, summary
    Done,
}

struct Migration {
    /// `20260314_093012_add_index_to_orders`
    name: String,
    /// Table rewritten by a slow migration, with its row count in millions.
    rewrite: Option<(&'static str, f32)>,
}

pub struct MigrateMode {
    sim: Sim,
    phase: Phase,

    database_url: String,
    pg_version: u8,
    current_version: String,
    migrations: Vec<Migration>,
    /// Ticks waited on the migration currently being applied.
    apply_wait: u32,
}

impl MigrateMode {
    pub fn new(seed: &str) -> Self {
        let mut sim = Sim::new(seed);
        let rng = &mut sim.rng;

        let db = seed.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
        let database_url = format!("postgres://{db}@db-primary:5432/{db}");
        let pg_version = rng.random_range(14..18u8);

        // Migration versions are timestamps, increasing through the year.
        let mut day_of_year = rng.random_range(1..200u32);
        let mut version = |rng: &mut rand_chacha::ChaCha8Rng| {
            day_of_year += rng.random_range(1..12u32);
            format!(
                "2026{:02}{:02}_{:02}{:02}{:02}",
                (day_of_year / 28).min(11) + 1,
                day_of_year % 28 + 1,
                rng.random_range(8..20u8),
                rng.random_range(0..60u8),
                rng.random_range(0..60u8),
            )
        };
        let current_version = version(rng);
        let count = rng.random_range(3..8usize);
        let migrations = (0..count)
            .map(|_| {
                let v = version(rng);
                let verb = VERBS[rng.random_range(0..VERBS.len())];
                let object = OBJECTS[rng.random_range(0..OBJECTS.len())];
                let rewrite = rng.random_bool(0.25).then(|| {
                    (
                        TABLES[rng.random_range(0..TABLES.len())],
                        rng.random_range(5..120u32) as f32 / 10.0,
                    )
                });
                Migration {
                    name: format!("{v}_{verb}_{object}"),
                    rewrite,
                }
            })
            .collect();

        let mut s = Self {
            sim,
            phase: Phase::Connect,
            database_url,
            pg_version,
            current_version,
            migrations,
            apply_wait: 0,
        };
        s.sim.log.push(Line::from(vec![
            Span::styled("  $ ", Style::default().fg(Color::DarkGray)),
            Span::styled("db migrate", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(format!("  {seed}"), Style::default().fg(Color::DarkGray)),
        ]));
        s.sim.log.push(Line::raw(""));
        s
    }
}

impl Tick for MigrateMode {
    const TITLE: &'static str = "db migrate";
    const SUCCESS_MESSAGE: &'static str = "migrations applied";

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
            Phase::Connect => self.tick_connect(),
            Phase::Pending => self.tick_pending(),
            Phase::Apply => self.tick_apply(),
            Phase::Finalize => self.tick_finalize(),
            Phase::Done => {}
        }
    }
}

impl MigrateMode {
    fn tick_connect(&mut self) {
        let sim = &mut self.sim;
        match sim.step {
            0 => {
                sim.log.push(Line::from(vec![
                    Span::styled("  connecting  ", Style::default().fg(Color::DarkGray)),
                    Span::raw(self.database_url.clone()),
                ]));
                sim.wait_ticks = sim.rng.random_range(6..14);
            }
            1 => {
                sim.log.push(Line::from(vec![
                    Span::styled("  ✔ connected ", Style::default().fg(Color::Green)),
                    Span::styled(
                        format!(
                            " PostgreSQL {}.{}",
                            self.pg_version,
                            sim.rng.random_range(1..9u8)
                        ),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                sim.wait_ticks = sim.rng.random_range(2..6);
            }
            2 => {
                sim.log.push(Line::from(vec![
                    Span::styled("  lock        ", Style::default().fg(Color::DarkGray)),
                    Span::raw("advisory lock acquired"),
                ]));
                sim.wait_ticks = sim.rng.random_range(2..6);
                sim.progress = 0.05;
            }
            _ => {
                sim.log.push(Line::from(vec![
                    Span::styled("  version     ", Style::default().fg(Color::DarkGray)),
                    Span::raw(self.current_version.clone()),
                ]));
                sim.log.push(Line::raw(""));
                sim.wait_ticks = sim.rng.random_range(4..10);
                sim.progress = 0.1;
            }
        }
        if self.sim.next_step(4) {
            self.phase = Phase::Pending;
        }
    }

    fn tick_pending(&mut self) {
        let n = self.migrations.len() as u32;
        match self.sim.step {
            0 => {
                self.sim.log.push(Line::styled(
                    format!("  {n} pending migrations"),
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                self.sim.wait_ticks = self.sim.rng.random_range(4..8);
            }
            step => {
                let m = &self.migrations[step as usize - 1];
                self.sim.log.push(Line::styled(
                    format!("    {}", m.name),
                    Style::default().fg(Color::DarkGray),
                ));
                self.sim.wait_ticks = self.sim.rng.random_range(1..3);
            }
        }
        if self.sim.next_step(n + 1) {
            self.sim.log.push(Line::raw(""));
            self.sim.progress = 0.2;
            self.phase = Phase::Apply;
        }
    }

    fn tick_apply(&mut self) {
        let n = self.migrations.len() as u32;
        let step = self.sim.step;
        let m = &self.migrations[(step / 2) as usize];

        if step.is_multiple_of(2) {
            self.sim.log.push(Line::from(vec![
                Span::styled("  ▶ ", Style::default().fg(Color::Magenta)),
                Span::raw(m.name.clone()),
            ]));
            self.apply_wait = match m.rewrite {
                Some((table, rows)) => {
                    self.sim.log.push(Line::styled(
                        format!("    rewriting table {table} ({rows:.1}M rows)…"),
                        Style::default().fg(Color::Yellow),
                    ));
                    self.sim.rng.random_range(30..70)
                }
                None => self.sim.rng.random_range(2..12),
            };
            self.sim.wait_ticks = self.apply_wait;
        } else {
            // Report the time actually spent waiting (in simulated time).
            let ms = (TICK * (self.apply_wait + 1)).as_millis()
                + self.sim.rng.random_range(0..100) as u128;
            self.sim.log.push(Line::from(vec![
                Span::styled("  ✔ ", Style::default().fg(Color::Green)),
                Span::raw(m.name.clone()),
                Span::styled(format!("  ({ms}ms)"), Style::default().fg(Color::DarkGray)),
            ]));
            self.sim.wait_ticks = self.sim.rng.random_range(1..4);
            self.sim.progress = 0.2 + (step + 1) as f64 / (2 * n) as f64 * 0.7;
        }
        if self.sim.next_step(2 * n) {
            self.sim.log.push(Line::raw(""));
            self.phase = Phase::Finalize;
        }
    }

    fn tick_finalize(&mut self) {
        let sim = &mut self.sim;
        match sim.step {
            0 => {
                sim.log.push(Line::from(vec![
                    Span::styled("  schema      ", Style::default().fg(Color::DarkGray)),
                    Span::raw("dumped to db/schema.sql"),
                ]));
                sim.wait_ticks = sim.rng.random_range(6..12);
                sim.progress = 0.95;
            }
            1 => {
                sim.log.push(Line::from(vec![
                    Span::styled("  lock        ", Style::default().fg(Color::DarkGray)),
                    Span::raw("advisory lock released"),
                ]));
                sim.wait_ticks = sim.rng.random_range(2..6);
            }
            _ => {
                let latest = &self.migrations[self.migrations.len() - 1].name[..15];
                sim.log.push(Line::raw(""));
                sim.log.push(Line::styled(
                    format!(
                        "  {} migrations applied · schema version {latest}",
                        self.migrations.len()
                    ),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ));
                sim.progress = 1.0;
                sim.status = Status::Success;
            }
        }
        if self.sim.next_step(3) {
            self.phase = Phase::Done;
        }
    }
}

static VERBS: &[&str] = &["add", "create", "drop", "rename", "backfill", "alter"];
static OBJECTS: &[&str] = &[
    "index_to_orders",
    "users_email_verified",
    "payments_status",
    "sessions_expires_at",
    "audit_log_table",
    "invoices_currency",
    "events_partition",
    "feature_flags",
];
static TABLES: &[&str] = &["orders", "users", "events", "payments", "audit_log"];
