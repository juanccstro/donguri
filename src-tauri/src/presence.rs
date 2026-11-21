//! Where the squirrel is and whether it is shown: edge snapping, per-monitor
//! placement, modes, the hidden-mode tab and every reason to hide.
//!
//! All coordinates are physical pixels. Nothing here polls: state changes come
//! from commands, menu events, OS hooks or one-shot timers.

use crate::{
    menus, platform,
    settings::{Edge, Mode, Placement, Settings},
};
use chrono::{Duration, Local, NaiveTime, TimeZone};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalRect, PhysicalSize,
    WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

pub const SQUIRREL: &str = "ardilla";
pub const TAB: &str = "pestana-borde";
/// Sprite canvas size in art pixels.
const ART: u32 = 16;
/// Hidden-mode acorn window, in acorn art pixels: length along the edge and
/// depth. At rest only the cap shows; on hover the acorn pops out further.
const ACORN_LONG: u32 = 8;
const ACORN_OUT: u32 = 5;

#[derive(Clone)]
struct Drag {
    cursor: PhysicalPosition<f64>,
    window: PhysicalPosition<i32>,
}

/// Where the squirrel actually is right now (may differ from the preferred
/// monitor while that one is disconnected).
#[derive(Clone)]
struct Current {
    monitor: String,
    origin: (i32, i32),
    placement: Placement,
    unit: u32,
    scale_factor: f64,
}

pub struct State {
    pub settings: Settings,
    path: PathBuf,
    manual_hidden: bool,
    /// User asked to see it despite an automatic reason; cleared when the
    /// foreground window changes.
    force_show: bool,
    fullscreen_origin: Option<(i32, i32)>,
    presentation: bool,
    pub foreground_exe: Option<String>,
    peeked: bool,
    /// Peek opened by clicking the acorn: stays out until an action finishes.
    pinned: bool,
    /// The acorn window is being built (don't start another one).
    tab_pending: bool,
    drag: Option<Drag>,
    current: Option<Current>,
    hide_generation: u64,
}

impl State {
    fn save(&self) {
        self.settings.save(&self.path);
    }

    fn timed_hidden(&self) -> bool {
        self.settings.hidden_until.is_some_and(|t| t > now())
    }

    fn auto_hidden(&self) -> bool {
        let fullscreen_here = self.settings.hide_fullscreen
            && (self.presentation
                || match (&self.current, self.fullscreen_origin) {
                    (Some(c), Some(o)) => c.origin == o,
                    _ => false,
                });
        let blacklisted = self
            .foreground_exe
            .as_ref()
            .is_some_and(|exe| self.settings.blacklist.contains(exe));
        fullscreen_here || blacklisted
    }

    pub fn hidden(&self) -> bool {
        self.manual_hidden || self.timed_hidden() || (!self.force_show && self.auto_hidden())
    }
}

pub fn state(app: &AppHandle) -> MutexGuard<'_, State> {
    app.state::<Mutex<State>>().inner().lock().unwrap_or_else(|e| e.into_inner())
}

fn squirrel(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(SQUIRREL)
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn monitor_name(m: &Monitor) -> String {
    m.name().cloned().unwrap_or_else(|| format!("{:?}", m.position()))
}

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let path = app.path().app_config_dir()?.join("settings.json");
    let settings = Settings::load(&path);
    let protected = settings.exclude_from_capture;
    app.manage(Mutex::new(State {
        settings,
        path,
        manual_hidden: false,
        force_show: false,
        fullscreen_origin: None,
        presentation: false,
        foreground_exe: None,
        peeked: false,
        pinned: false,
        tab_pending: false,
        drag: None,
        current: None,
        hide_generation: 0,
    }));

    if let Some(win) = squirrel(app) {
        let _ = win.set_content_protected(protected);
        platform::install(app, &win);
    }
    restore(app);
    schedule_unhide(app);
    recompute(app);
    Ok(())
}

// ---------------------------------------------------------------- placement

/// Puts the squirrel on its preferred monitor, or the primary one if that
/// monitor is gone. Called at startup and whenever displays change.
pub fn restore(app: &AppHandle) {
    let monitors = app.available_monitors().unwrap_or_default();
    let preferred = state(app).settings.preferred_monitor.clone();
    let monitor = preferred
        .and_then(|name| monitors.iter().find(|m| monitor_name(m) == name).cloned())
        .or_else(|| app.primary_monitor().ok().flatten())
        .or_else(|| monitors.first().cloned());
    let Some(monitor) = monitor else { return };
    let placement = state(app)
        .settings
        .placements
        .get(&monitor_name(&monitor))
        .copied()
        .unwrap_or_default();
    apply(app, &monitor, placement);
}

/// Physical pixels per art pixel: always an integer so the sprite stays crisp.
fn unit_for(scale: u32, scale_factor: f64) -> u32 {
    ((scale as f64 * scale_factor).round() as u32).max(1)
}

fn work_area(m: &Monitor) -> (i32, i32, i32, i32) {
    let PhysicalRect { position, size } = *m.work_area();
    (position.x, position.y, size.width as i32, size.height as i32)
}

fn edge_position(m: &Monitor, p: Placement, size: u32) -> PhysicalPosition<i32> {
    let (x0, y0, w, h) = work_area(m);
    let s = size as i32;
    let along = p.along.clamp(0.0, 1.0);
    let slide = |len: i32| ((len - s).max(0) as f64 * along).round() as i32;
    let (x, y) = match p.edge {
        Edge::Bottom => (x0 + slide(w), y0 + h - s),
        Edge::Top => (x0 + slide(w), y0),
        Edge::Left => (x0, y0 + slide(h)),
        Edge::Right => (x0 + w - s, y0 + slide(h)),
    };
    PhysicalPosition::new(x, y)
}

fn apply(app: &AppHandle, monitor: &Monitor, placement: Placement) {
    let Some(win) = squirrel(app) else { return };
    let scale_factor = monitor.scale_factor();
    let unit = unit_for(state(app).settings.scale, scale_factor);
    let size = ART * unit;
    let pos = edge_position(monitor, placement, size);
    // Move first so a DPI change on the new monitor happens before we fix the size.
    let _ = win.set_position(pos);
    let _ = win.set_size(PhysicalSize::new(size, size));
    let _ = win.set_position(pos);
    let origin = (monitor.position().x, monitor.position().y);
    state(app).current = Some(Current {
        monitor: monitor_name(monitor),
        origin,
        placement,
        unit,
        scale_factor,
    });
    let _ = app.emit_to(SQUIRREL, "posture", placement.edge);
    place_tab(app);
}

fn monitor_by_name(app: &AppHandle, name: &str) -> Option<Monitor> {
    app.available_monitors()
        .ok()?
        .into_iter()
        .find(|m| monitor_name(m) == name)
}

/// Snaps to the nearest edge of the monitor under the squirrel's center and
/// remembers that spot for that monitor.
fn snap(app: &AppHandle) -> Option<Edge> {
    let win = squirrel(app)?;
    let (pos, size) = (win.outer_position().ok()?, win.outer_size().ok()?);
    let (cx, cy) = (pos.x + size.width as i32 / 2, pos.y + size.height as i32 / 2);
    let monitor = app
        .monitor_from_point(cx as f64, cy as f64)
        .ok()
        .flatten()
        .or_else(|| win.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten())?;

    let (x0, y0, w, h) = work_area(&monitor);
    let s = (ART * unit_for(state(app).settings.scale, monitor.scale_factor())) as i32;
    let distances = [
        (Edge::Bottom, y0 + h - cy),
        (Edge::Top, cy - y0),
        (Edge::Left, cx - x0),
        (Edge::Right, x0 + w - cx),
    ];
    let edge = distances.iter().min_by_key(|(_, d)| d.abs()).map_or(Edge::Bottom, |(e, _)| *e);
    let along = if edge.horizontal() {
        (cx - s / 2 - x0) as f64 / (w - s).max(1) as f64
    } else {
        (cy - s / 2 - y0) as f64 / (h - s).max(1) as f64
    }
    .clamp(0.0, 1.0);

    let placement = Placement { edge, along };
    let name = monitor_name(&monitor);
    {
        let mut st = state(app);
        st.settings.placements.insert(name.clone(), placement);
        st.settings.preferred_monitor = Some(name);
        st.save();
    }
    apply(app, &monitor, placement);
    Some(edge)
}

// --------------------------------------------------------------- visibility

pub fn recompute(app: &AppHandle) {
    let (show_squirrel, show_tab, hidden_mode, peeked, protected) = {
        let st = state(app);
        let hidden = st.hidden();
        let hidden_mode = st.settings.mode == Mode::Escondida;
        #[cfg(debug_assertions)]
        eprintln!(
            "[presence] hidden={hidden} manual={} timed={} auto={} force={} fg={:?} fs={:?} pres={} cur={:?}",
            st.manual_hidden,
            st.timed_hidden(),
            st.auto_hidden(),
            st.force_show,
            st.foreground_exe,
            st.fullscreen_origin,
            st.presentation,
            st.current.as_ref().map(|c| (&c.monitor, c.origin, c.placement)),
        );
        (
            !hidden && (!hidden_mode || st.peeked),
            !hidden && hidden_mode && !st.peeked,
            hidden_mode,
            st.peeked,
            st.settings.exclude_from_capture,
        )
    };

    if let Some(win) = squirrel(app) {
        platform::set_visible(&win, show_squirrel);
    }
    match (hidden_mode, app.get_webview_window(TAB)) {
        // Not shown before its page has placed it (it would show at a default size).
        (true, Some(tab)) => {
            let pending = state(app).tab_pending;
            platform::set_visible(&tab, show_tab && !pending);
        }
        // Built off the main thread (Tauri defers window creation requested
        // from it); the acorn's page calls `tab_ready` once it can be placed.
        (true, None) => {
            let start = !std::mem::replace(&mut state(app).tab_pending, true);
            if start {
                let app = app.clone();
                std::thread::spawn(move || {
                    if create_tab(&app, protected).is_none() {
                        state(&app).tab_pending = false;
                    }
                });
            }
        }
        (false, Some(tab)) => {
            let _ = tab.destroy();
        }
        (false, None) => {}
    }

    #[derive(Serialize, Clone)]
    struct Presence {
        visible: bool,
        peeked: bool,
    }
    let _ = app.emit_to(SQUIRREL, "presence", Presence { visible: show_squirrel, peeked });
    // Its own event name: JS `listen` hears events sent to any window, and
    // the squirrel must not take the acorn's visibility for hers.
    let _ = app.emit_to(TAB, "tab-presence", Presence { visible: show_tab, peeked });
}

fn create_tab(app: &AppHandle, protected: bool) -> Option<WebviewWindow> {
    WebviewWindowBuilder::new(app, TAB, WebviewUrl::App("pestana".into()))
        .title("Donguri")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focusable(false)
        .focused(false)
        .visible(false)
        .content_protected(protected)
        .maximizable(false)
        .minimizable(false)
        // Without an explicit minimum, Windows clamps tiny windows to its
        // default minimum (~136×39 px) and the acorn would look huge.
        .min_inner_size(1.0, 1.0)
        .inner_size(8.0, 5.0)
        .build()
        .ok()
}

/// Lays the acorn flush against the edge, centred on the squirrel's spot.
fn place_tab(app: &AppHandle) {
    let Some(tab) = app.get_webview_window(TAB) else { return };
    let Some(cur) = state(app).current.clone() else { return };
    let Some(monitor) = monitor_by_name(app, &cur.monitor) else { return };
    let size = ART * cur.unit;
    // The acorn is drawn 1.5× the squirrel's pixel size: a bit smaller than
    // her overall, but clearly visible.
    let au = ((cur.unit as f64) * 1.5).round().max(1.0) as u32;
    let long = ACORN_LONG * au;
    let out = ACORN_OUT * au;
    let squirrel_pos = edge_position(&monitor, cur.placement, size);
    let (x0, y0, w, h) = work_area(&monitor);
    let offset = ((size - long) / 2) as i32;
    let (pos, dims) = match cur.placement.edge {
        Edge::Bottom => ((squirrel_pos.x + offset, y0 + h - out as i32), (long, out)),
        Edge::Top => ((squirrel_pos.x + offset, y0), (long, out)),
        Edge::Left => ((x0, squirrel_pos.y + offset), (out, long)),
        Edge::Right => ((x0 + w - out as i32, squirrel_pos.y + offset), (out, long)),
    };
    let pos = PhysicalPosition::new(pos.0, pos.1);
    let _ = tab.set_position(pos);
    let _ = tab.set_size(PhysicalSize::new(dims.0, dims.1));
    let _ = tab.set_position(pos);
}

/// Shortcut / tray click: hide if shown, show (overriding auto-hide) if hidden.
pub fn toggle(app: &AppHandle) {
    {
        let mut st = state(app);
        if st.hidden() {
            st.manual_hidden = false;
            st.settings.hidden_until = None;
            st.force_show = true;
            st.save();
        } else {
            st.manual_hidden = true;
            st.force_show = false;
        }
    }
    let (hidden, en, key) = {
        let st = state(app);
        (st.manual_hidden, st.settings.en(), st.settings.shortcuts.toggle.clone())
    };
    if hidden {
        // Tell the user how to get her back (the notice outlives her).
        let text = if en {
            format!("Back with {key} or the acorn in the tray")
        } else {
            format!("Vuelvo con {key} o la bellota de la bandeja")
        };
        crate::notices::show(app, "info", &text);
    }
    recompute(app);
    menus::refresh(app);
}

pub fn hide_until(app: &AppHandle, until: i64) {
    {
        let mut st = state(app);
        st.settings.hidden_until = Some(until);
        st.force_show = false;
        st.save();
    }
    schedule_unhide(app);
    recompute(app);
}

pub fn hide_one_hour(app: &AppHandle) {
    hide_until(app, now() + 3600);
}

/// "Hasta mañana": the next 06:00 local time.
pub fn hide_until_tomorrow(app: &AppHandle) {
    let local = Local::now();
    let six = NaiveTime::from_hms_opt(6, 0, 0).unwrap_or_default();
    let mut day = local.date_naive();
    if local.time() >= six {
        day += Duration::days(1);
    }
    let target = Local
        .from_local_datetime(&day.and_time(six))
        .earliest()
        .map_or(now() + 12 * 3600, |t| t.timestamp());
    hide_until(app, target);
}

/// One sleeping thread per timed hide; a newer hide invalidates older ones.
fn schedule_unhide(app: &AppHandle) {
    let (until, generation) = {
        let mut st = state(app);
        st.hide_generation += 1;
        (st.settings.hidden_until, st.hide_generation)
    };
    let Some(until) = until else { return };
    let wait = until - now();
    if wait <= 0 {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(wait as u64 + 1));
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if state(&handle).hide_generation == generation {
                recompute(&handle);
                menus::refresh(&handle);
            }
        });
    });
}

/// Called by the platform hooks when the foreground window changes or moves.
pub fn on_foreground(
    app: &AppHandle,
    exe: Option<String>,
    fullscreen_origin: Option<(i32, i32)>,
    presentation: bool,
    new_window: bool,
) {
    {
        let mut st = state(app);
        let unchanged = st.foreground_exe == exe
            && st.fullscreen_origin == fullscreen_origin
            && st.presentation == presentation
            && !(new_window && st.force_show);
        if unchanged {
            return;
        }
        if new_window {
            st.force_show = false;
        }
        st.foreground_exe = exe;
        st.fullscreen_origin = fullscreen_origin;
        st.presentation = presentation;
    }
    recompute(app);
}

pub fn blacklist_current(app: &AppHandle) {
    {
        let mut st = state(app);
        let Some(exe) = st.foreground_exe.clone() else { return };
        if !st.settings.blacklist.contains(&exe) {
            st.settings.blacklist.push(exe);
            st.settings.blacklist.sort();
        }
        st.force_show = false;
        st.save();
    }
    recompute(app);
}

pub fn unblock(app: &AppHandle, exe: &str) {
    {
        let mut st = state(app);
        st.settings.blacklist.retain(|e| e != exe);
        st.save();
    }
    recompute(app);
}

// ------------------------------------------------------------------ settings

/// What the windows need to know to look right; sent on every change.
#[derive(Serialize, Clone)]
pub struct Config {
    mode: Mode,
    fade: bool,
    edge: Edge,
    palette: String,
    accessory: String,
    masked: bool,
    lang: String,
    scale: u32,
}

fn config(st: &State) -> Config {
    let s = &st.settings;
    Config {
        mode: s.mode,
        fade: s.fade_when_idle,
        edge: st.current.as_ref().map_or(Edge::Bottom, |c| c.placement.edge),
        palette: s.palette.clone(),
        accessory: s.accessory.clone(),
        masked: s.private_mode,
        lang: s.lang.clone(),
        scale: s.scale,
    }
}

pub fn emit_config(app: &AppHandle) {
    let cfg = config(&state(app));
    let _ = app.emit("config", cfg);
}

/// Applies a settings change, saves it and lets every window know.
pub fn update(app: &AppHandle, change: impl FnOnce(&mut Settings)) {
    {
        let mut st = state(app);
        change(&mut st.settings);
        st.save();
    }
    emit_config(app);
    menus::refresh(app);
}

pub fn set_mode(app: &AppHandle, mode: Mode) {
    {
        let mut st = state(app);
        st.settings.mode = mode;
        st.peeked = false;
        st.pinned = false;
        st.save();
    }
    recompute(app);
    emit_config(app);
}

pub fn set_scale(app: &AppHandle, scale: u32) {
    {
        let mut st = state(app);
        st.settings.scale = if matches!(scale, 4 | 6 | 8) { scale } else { 4 };
        st.save();
    }
    let current = state(app).current.clone();
    match current.and_then(|c| monitor_by_name(app, &c.monitor).map(|m| (m, c.placement))) {
        Some((monitor, placement)) => apply(app, &monitor, placement),
        None => restore(app),
    }
    emit_config(app);
}

/// Applies the "invisible in recordings" setting to every open window.
pub fn apply_protection(app: &AppHandle) {
    let protected = state(app).settings.exclude_from_capture;
    for win in app.webview_windows().values() {
        let _ = win.set_content_protected(protected);
    }
}

pub fn set_option(app: &AppHandle, option: &str, value: Option<bool>) {
    {
        let mut st = state(app);
        let s = &mut st.settings;
        let flag = match option {
            "fullscreen" => &mut s.hide_fullscreen,
            "capture" => &mut s.exclude_from_capture,
            "fade" => &mut s.fade_when_idle,
            "private" => &mut s.private_mode,
            "selection_hint" => &mut s.selection_hint,
            _ => return,
        };
        *flag = value.unwrap_or(!*flag);
        st.save();
    }
    apply_protection(app);
    recompute(app);
    emit_config(app);
}

// ------------------------------------------------------------------ commands

#[tauri::command]
pub fn get_config(app: AppHandle) -> Config {
    config(&state(&app))
}

#[tauri::command]
pub fn drag_start(app: AppHandle) {
    let Some(win) = squirrel(&app) else { return };
    if let (Ok(cursor), Ok(window)) = (app.cursor_position(), win.outer_position()) {
        state(&app).drag = Some(Drag { cursor, window });
    }
}

#[tauri::command]
pub fn drag_move(app: AppHandle) {
    let Some(drag) = state(&app).drag.clone() else { return };
    let (Some(win), Ok(cursor)) = (squirrel(&app), app.cursor_position()) else { return };
    let _ = win.set_position(PhysicalPosition::new(
        drag.window.x + (cursor.x - drag.cursor.x).round() as i32,
        drag.window.y + (cursor.y - drag.cursor.y).round() as i32,
    ));
}

#[tauri::command]
pub fn drag_end(app: AppHandle) -> Edge {
    state(&app).drag = None;
    snap(&app).unwrap_or_default()
}

/// Moves one art pixel along the current edge. Returns false at the end of it.
#[tauri::command]
pub fn walk_step(app: AppHandle, dir: i32) -> bool {
    let Some(cur) = state(&app).current.clone() else { return false };
    let (Some(win), Some(monitor)) = (squirrel(&app), monitor_by_name(&app, &cur.monitor)) else {
        return false;
    };
    let Ok(pos) = win.outer_position() else { return false };
    let (x0, y0, w, h) = work_area(&monitor);
    let s = (ART * cur.unit) as i32;
    let step = dir.signum() * cur.unit as i32;
    let horizontal = cur.placement.edge.horizontal();
    let (start, len, at) = if horizontal { (x0, w, pos.x) } else { (y0, h, pos.y) };
    let next = (at + step).clamp(start, start + len - s);
    if next == at {
        return false;
    }
    let new_pos = if horizontal {
        PhysicalPosition::new(next, pos.y)
    } else {
        PhysicalPosition::new(pos.x, next)
    };
    let _ = win.set_position(new_pos);
    let along = (next - start) as f64 / (len - s).max(1) as f64;
    let mut st = state(&app);
    if let Some(c) = st.current.as_mut() {
        c.placement.along = along;
    }
    st.settings
        .placements
        .insert(cur.monitor, Placement { edge: cur.placement.edge, along });
    true
}

#[tauri::command]
pub fn walk_end(app: AppHandle) {
    state(&app).save();
    place_tab(&app);
}

#[tauri::command]
pub fn peek(app: AppHandle, pinned: Option<bool>) {
    {
        let mut st = state(&app);
        st.peeked = true;
        st.pinned |= pinned.unwrap_or(false);
    }
    recompute(&app);
}

#[tauri::command]
pub fn unpeek(app: AppHandle) {
    {
        if crate::popups::any_open(&app) {
            return; // she stays out while one of her windows is open
        }
        let mut st = state(&app);
        if st.drag.is_some() || st.pinned {
            return;
        }
        st.peeked = false;
    }
    recompute(&app);
}

/// An action that started from a pinned peek has finished: let her hide again.
pub fn release_peek(app: &AppHandle) {
    state(app).pinned = false;
    let _ = app.emit_to(SQUIRREL, "peek-released", ());
}

// ------------------------------------------------------------- for popups

/// Where the squirrel is (physical pixels), for placing popups next to it.
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub x: i32,
    pub y: i32,
    pub size: i32,
    pub edge: Edge,
    /// Work area of its monitor: x, y, width, height.
    pub area: (i32, i32, i32, i32),
    pub scale_factor: f64,
}

pub fn anchor(app: &AppHandle) -> Option<Anchor> {
    let cur = state(app).current.clone()?;
    let monitor = monitor_by_name(app, &cur.monitor)?;
    let size = ART * cur.unit;
    let pos = edge_position(&monitor, cur.placement, size);
    Some(Anchor {
        x: pos.x,
        y: pos.y,
        size: size as i32,
        edge: cur.placement.edge,
        area: work_area(&monitor),
        scale_factor: cur.scale_factor,
    })
}

/// Plays a one-shot or looping animation: chew, success, error, alert,
/// think, talk, idle.
pub fn animate(app: &AppHandle, anim: &str) {
    let _ = app.emit_to(SQUIRREL, "anim", anim);
}

/// In hidden mode, brings her out (pinned) so the user sees what happens.
pub fn show_for_action(app: &AppHandle) {
    let needs_peek = {
        let mut st = state(app);
        let out = st.settings.mode == Mode::Escondida && !st.peeked;
        if out {
            st.peeked = true;
            st.pinned = true;
        }
        out
    };
    if needs_peek {
        recompute(app);
    }
}

/// The squirrel's page is loaded and listening: show her now. (Showing her
/// during setup does not stick: Tauri applies the config's `visible: false`
/// afterwards, and the page would miss the first presence event anyway.)
#[tauri::command]
pub fn squirrel_ready(app: AppHandle) {
    recompute(&app);
    emit_config(&app);
}

/// A press that turned out to be a click, not a drag.
#[tauri::command]
pub fn drag_cancel(app: AppHandle) {
    state(&app).drag = None;
}

/// The hidden-mode acorn's page has loaded: size, place and show it.
#[tauri::command]
pub fn tab_ready(app: AppHandle) {
    state(&app).tab_pending = false;
    place_tab(&app);
    recompute(&app);
}

/// How long the user has not touched keyboard or mouse (she naps meanwhile).
#[tauri::command]
pub fn user_idle_ms() -> u64 {
    platform::idle_ms()
}

/// Whether the mouse is over the (visible) squirrel right now.
pub fn cursor_over_squirrel(app: &AppHandle) -> bool {
    let Some(win) = squirrel(app) else { return false };
    if !win.is_visible().unwrap_or(false) {
        return false;
    }
    let (Ok(c), Ok(p), Ok(s)) = (app.cursor_position(), win.outer_position(), win.outer_size()) else {
        return false;
    };
    c.x >= p.x as f64 && c.y >= p.y as f64 && c.x < (p.x + s.width as i32) as f64 && c.y < (p.y + s.height as i32) as f64
}
