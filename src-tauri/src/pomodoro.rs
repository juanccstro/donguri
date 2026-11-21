//! Pomodoro: focus and break phases. One sleeping thread waits for the end of
//! the current phase (a newer one cancels it), so nothing polls. Every change
//! is broadcast as the `pomodoro` event: the squirrel shows the phase, the
//! menu and the panel show the time, and a notice marks each switch.
//!
//! Order: focus, break, focus, break… and after the last focus the long
//! break (if the plan has one); then everything goes back to normal.

use crate::{i18n::t, notices, presence};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    #[default]
    Focus,
    Break,
    Long,
}

/// Minutes; `long` 0 means no long break at the end.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub struct Plan {
    pub focus: u32,
    pub short: u32,
    pub long: u32,
    pub rounds: u32,
}

impl Plan {
    fn sane(self) -> Self {
        Self {
            focus: self.focus.clamp(1, 240),
            short: self.short.clamp(1, 120),
            long: self.long.min(120),
            rounds: self.rounds.clamp(1, 12),
        }
    }

    fn minutes(&self, phase: Phase) -> u32 {
        match phase {
            Phase::Focus => self.focus,
            Phase::Break => self.short,
            Phase::Long => self.long,
        }
    }
}

#[derive(Default)]
struct Timer {
    plan: Option<Plan>,
    phase: Phase,
    /// 1-based focus round.
    round: u32,
    /// Unix ms when the phase ends (while running).
    ends_at: i64,
    /// Time left while paused.
    paused_left: Option<i64>,
    generation: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct Status {
    active: bool,
    phase: Phase,
    round: u32,
    rounds: u32,
    plan: Option<Plan>,
    /// Unix ms when the phase ends (0 while paused or idle).
    ends_at: i64,
    /// Milliseconds left in the phase.
    left: i64,
    /// Full length of the phase in milliseconds.
    total: i64,
    paused: bool,
}

pub fn init(app: &AppHandle) {
    app.manage(Mutex::new(Timer::default()));
}

fn timer(app: &AppHandle) -> MutexGuard<'_, Timer> {
    app.state::<Mutex<Timer>>().inner().lock().unwrap_or_else(|e| e.into_inner())
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn status_of(tm: &Timer) -> Status {
    let left = tm.paused_left.unwrap_or_else(|| (tm.ends_at - now_ms()).max(0));
    Status {
        active: tm.plan.is_some(),
        phase: tm.phase,
        round: tm.round,
        rounds: tm.plan.map_or(0, |p| p.rounds),
        plan: tm.plan,
        ends_at: if tm.paused_left.is_some() { 0 } else { tm.ends_at },
        left: if tm.plan.is_some() { left } else { 0 },
        total: tm.plan.map_or(0, |p| p.minutes(tm.phase) as i64 * 60_000),
        paused: tm.paused_left.is_some(),
    }
}

pub fn status(app: &AppHandle) -> Status {
    status_of(&timer(app))
}

fn broadcast(app: &AppHandle) {
    let _ = app.emit("pomodoro", status(app));
}

/// Starts `phase` from its full length and waits for its end.
fn begin(app: &AppHandle, phase: Phase, round: u32) {
    let ms = {
        let mut tm = timer(app);
        let Some(plan) = tm.plan else { return };
        tm.phase = phase;
        tm.round = round;
        tm.paused_left = None;
        let ms = plan.minutes(phase) as i64 * 60_000;
        tm.ends_at = now_ms() + ms;
        ms
    };
    wait(app, ms);
    broadcast(app);
}

/// One thread sleeps until the phase ends; any change makes it stale.
fn wait(app: &AppHandle, ms: i64) {
    let generation = {
        let mut tm = timer(app);
        tm.generation += 1;
        tm.generation
    };
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(ms.max(0) as u64));
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if timer(&handle).generation == generation {
                next(&handle);
            }
        });
    });
}

/// The current phase is over: on to the next one, or done.
fn next(app: &AppHandle) {
    let (plan, phase, round) = {
        let tm = timer(app);
        let Some(plan) = tm.plan else { return };
        (plan, tm.phase, tm.round)
    };
    let en = presence::state(app).settings.en();
    match phase {
        Phase::Focus if round < plan.rounds => {
            begin(app, Phase::Break, round);
            presence::animate(app, "pomo-break");
            let text = if en {
                format!("Break time! {} min · round {round} of {} done", plan.short, plan.rounds)
            } else {
                format!("¡Descanso! {} min · ronda {round} de {} hecha", plan.short, plan.rounds)
            };
            notices::show(app, "pomo", &text);
        }
        Phase::Focus if plan.long > 0 => {
            begin(app, Phase::Long, round);
            presence::animate(app, "pomo-break");
            let text = if en {
                format!("All rounds done! Long break: {} min", plan.long)
            } else {
                format!("¡Rondas completadas! Descanso largo: {} min", plan.long)
            };
            notices::show(app, "pomo", &text);
        }
        Phase::Break => {
            begin(app, Phase::Focus, round + 1);
            presence::animate(app, "pomo-focus");
            let text = if en {
                format!("Focus! {} min · round {} of {}", plan.focus, round + 1, plan.rounds)
            } else {
                format!("¡A concentrarse! {} min · ronda {} de {}", plan.focus, round + 1, plan.rounds)
            };
            notices::show(app, "pomo", &text);
        }
        _ => finish(app, true),
    }
}

fn finish(app: &AppHandle, completed: bool) {
    {
        let mut tm = timer(app);
        tm.generation += 1;
        tm.plan = None;
        tm.paused_left = None;
    }
    broadcast(app);
    if completed {
        presence::animate(app, "pomo-done");
        let en = presence::state(app).settings.en();
        notices::show(app, "pomo", t(en, "pomo_done"));
    } else {
        presence::animate(app, "idle");
    }
}

// ------------------------------------------------------------------ commands

#[tauri::command]
pub fn pomodoro_status(app: AppHandle) -> Status {
    status(&app)
}

#[tauri::command]
pub fn pomodoro_start(app: AppHandle, plan: Plan) {
    timer(&app).plan = Some(plan.sane());
    begin(&app, Phase::Focus, 1);
    presence::animate(&app, "pomo-focus");
    let en = presence::state(&app).settings.en();
    let p = plan.sane();
    let text = if en {
        format!("Pomodoro on: {} min focus · {} rounds", p.focus, p.rounds)
    } else {
        format!("Pomodoro en marcha: {} min de concentración · {} rondas", p.focus, p.rounds)
    };
    notices::show(&app, "pomo", &text);
}

#[tauri::command]
pub fn pomodoro_pause(app: AppHandle) {
    {
        let mut tm = timer(&app);
        if tm.plan.is_none() || tm.paused_left.is_some() {
            return;
        }
        tm.paused_left = Some((tm.ends_at - now_ms()).max(0));
        tm.generation += 1; // the waiting thread is now stale
    }
    broadcast(&app);
}

#[tauri::command]
pub fn pomodoro_resume(app: AppHandle) {
    let left = {
        let mut tm = timer(&app);
        let Some(left) = tm.paused_left.take() else { return };
        tm.ends_at = now_ms() + left;
        left
    };
    wait(&app, left);
    broadcast(&app);
}

/// Back to the start: round 1, full focus time.
#[tauri::command]
pub fn pomodoro_restart(app: AppHandle) {
    if timer(&app).plan.is_none() {
        return;
    }
    begin(&app, Phase::Focus, 1);
    presence::animate(&app, "pomo-focus");
}

/// Skips to the next phase now.
#[tauri::command]
pub fn pomodoro_skip(app: AppHandle) {
    if timer(&app).plan.is_some() {
        next(&app);
    }
}

#[tauri::command]
pub fn pomodoro_stop(app: AppHandle) {
    if timer(&app).plan.is_some() {
        finish(&app, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_are_kept_in_range() {
        let p = Plan { focus: 0, short: 500, long: 999, rounds: 0 }.sane();
        assert_eq!((p.focus, p.short, p.long, p.rounds), (1, 120, 120, 1));
        assert_eq!(p.minutes(Phase::Break), 120);
    }
}
