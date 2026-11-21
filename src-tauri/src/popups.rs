//! Every window besides the squirrel: created on use, destroyed on close,
//! placed next to the squirrel on the side that faces the screen centre.

use crate::{platform, presence, settings::Edge};
use serde::Serialize;
use std::{collections::HashMap, sync::Mutex};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Place {
    /// Beside the squirrel, toward the screen centre (bubbles, pickers).
    Beside,
    /// Centred on the squirrel, may hang off-screen (radial menu).
    Around,
    /// Centred on the squirrel's monitor (Kura, settings, welcome).
    Center,
    /// Covering a whole monitor (area capture).
    Cover { x: i32, y: i32, w: u32, h: u32 },
    /// At a screen point (physical px), kept inside that monitor.
    At { x: i32, y: i32 },
}

pub struct Spec {
    pub label: &'static str,
    pub url: &'static str,
    /// Logical size.
    pub w: f64,
    pub h: f64,
    pub focusable: bool,
    pub place: Place,
}

/// Where the squirrel's centre is, relative to a popup (logical px).
#[derive(Serialize, Clone, Copy, Default, Debug)]
pub struct Tail {
    pub edge: Option<Edge>,
    pub x: f64,
    pub y: f64,
    /// Room from the squirrel's centre to the work area edges (logical px):
    /// left, right, up, down. The radial menu opens toward the most room.
    pub space: [f64; 4],
}

#[derive(Default)]
pub struct Popups {
    places: HashMap<String, Place>,
    focusable: HashMap<String, bool>,
    /// Logical size each popup was opened with.
    sizes: HashMap<String, (f64, f64)>,
    tails: HashMap<String, Tail>,
    /// Kept popups that were asked to open and have not been closed since.
    wanted: std::collections::HashSet<String>,
}

pub fn init(app: &AppHandle) {
    app.manage(Mutex::new(Popups::default()));
}

fn popups(app: &AppHandle) -> std::sync::MutexGuard<'_, Popups> {
    app.state::<Mutex<Popups>>().inner().lock().unwrap_or_else(|e| e.into_inner())
}

/// Windows the squirrel "owns": while any is open she stays out of hiding.
const OWNED: [&str; 10] =
    ["menu", "bocadillo", "selector", "contexto", "kura", "ajustes", "bienvenida", "captura", "salir", "pomodoro"];

/// Built once and kept alive, so they open at once (building a window with
/// its web view takes a noticeable moment). Closing only hides them.
const KEPT: [&str; 2] = ["menu", "seleccion"];

/// Puts a kept popup away until it is opened again.
fn park(win: &WebviewWindow) {
    platform::set_visible(win, false);
}

/// A kept popup that is put away, not open.
fn parked(app: &AppHandle, label: &str) -> bool {
    KEPT.contains(&label) && !popups(app).wanted.contains(label)
}

/// Whether a popup is open (kept ones: asked to open; others count while loading).
pub fn is_open(app: &AppHandle, label: &str) -> bool {
    app.get_webview_window(label).is_some() && !parked(app, label)
}

pub fn any_open(app: &AppHandle) -> bool {
    OWNED.iter().any(|l| is_open(app, l))
}

/// Builds a kept popup hidden, ahead of its first use.
pub fn prepare(app: &AppHandle, spec: Spec) {
    if app.get_webview_window(spec.label).is_some() {
        return;
    }
    remember(app, &spec);
    build(app, spec);
}

/// The page of a kept popup asks on load whether it should show itself
/// (it was opened before it existed) or wait for the next opening.
#[tauri::command]
pub fn popup_wanted(app: AppHandle, window: WebviewWindow) -> bool {
    popups(&app).wanted.contains(window.label())
}

pub fn get(app: &AppHandle, label: &str) -> Option<WebviewWindow> {
    app.get_webview_window(label)
}

/// Opens (or re-shows) a popup. New windows are built off the main thread
/// (Tauri defers window creation requested from it) and stay hidden until
/// their page has sized itself and calls `popup_fit` or `popup_ready`.
pub fn open(app: &AppHandle, spec: Spec) {
    remember(app, &spec);
    let kept = KEPT.contains(&spec.label);
    if kept {
        popups(app).wanted.insert(spec.label.into());
    }
    if let Some(win) = app.get_webview_window(spec.label) {
        place(app, &win, spec.w, spec.h, spec.place);
        if kept {
            // Its page lays itself out again and then asks to be shown.
            // One event name per window: JS `listen` hears every window's events.
            let _ = app.emit_to(spec.label, &format!("{}-show", spec.label), ());
        } else {
            reveal(app, &win);
        }
        return;
    }
    build(app, spec);
}

fn remember(app: &AppHandle, spec: &Spec) {
    let mut p = popups(app);
    p.places.insert(spec.label.into(), spec.place);
    p.focusable.insert(spec.label.into(), spec.focusable);
    p.sizes.insert(spec.label.into(), (spec.w, spec.h));
}

fn build(app: &AppHandle, spec: Spec) {
    let protected = presence::state(app).settings.exclude_from_capture;
    let app = app.clone();
    std::thread::spawn(move || {
        let built = WebviewWindowBuilder::new(&app, spec.label, WebviewUrl::App(spec.url.into()))
            .title("Donguri")
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .focusable(spec.focusable)
            .focused(false)
            .visible(false)
            .content_protected(protected)
            .maximizable(false)
            .minimizable(false)
            .min_inner_size(1.0, 1.0)
            .inner_size(spec.w, spec.h)
            .build();
        // A kept popup built ahead of time stays hidden and unplaced until it
        // is opened (its page may load and report in before this runs).
        if let Ok(win) = built
            && !parked(&app, win.label())
        {
            ensure_placed(&app, &win);
        }
    });
}

/// Opens a popup from scratch. If one with that label is still around it is
/// reused and its page reloaded: closing it and building a new one at once
/// does not work, because the old window is only gone a moment later.
pub fn reopen(app: &AppHandle, spec: Spec) {
    if let Some(win) = app.get_webview_window(spec.label) {
        popups(app).tails.remove(spec.label);
        let _ = win.eval("location.reload()");
    }
    open(app, spec);
}

fn reveal(app: &AppHandle, win: &WebviewWindow) {
    platform::set_visible(win, true);
    if popups(app).focusable.get(win.label()).copied().unwrap_or(false) {
        platform::focus_window(win);
    }
}

pub fn close(app: &AppHandle, label: &str) {
    if KEPT.contains(&label) {
        popups(app).wanted.remove(label);
        if let Some(win) = app.get_webview_window(label) {
            let _ = app.emit_to(label, &format!("{label}-hide"), ());
            park(&win);
        }
        popups(app).tails.remove(label);
        return;
    }
    let had_focus = app
        .get_webview_window(label)
        .is_some_and(|w| w.is_focused().unwrap_or(false));
    if let Some(win) = app.get_webview_window(label) {
        let _ = win.destroy();
    }
    // Quick popups hand the keyboard back to the app the user was in.
    if had_focus && matches!(label, "selector" | "contexto" | "bocadillo" | "captura") {
        platform::focus_last_app();
    }
    let mut p = popups(app);
    p.places.remove(label);
    p.focusable.remove(label);
    p.sizes.remove(label);
    p.tails.remove(label);
}

/// Places a popup unless that already happened. Its page may ask where it is
/// before the thread that built it got to place it: both paths end up here.
fn ensure_placed(app: &AppHandle, win: &WebviewWindow) {
    let (placed, how, size) = {
        let p = popups(app);
        let label = win.label();
        (
            p.tails.contains_key(label),
            p.places.get(label).copied().unwrap_or(Place::Beside),
            p.sizes.get(label).copied(),
        )
    };
    if let (false, Some((w, h))) = (placed, size) {
        place(app, win, w, h, how);
    }
}

fn place(app: &AppHandle, win: &WebviewWindow, w: f64, h: f64, how: Place) {
    if let Place::At { x, y } = how {
        let Some(m) = app.monitor_from_point(x as f64, y as f64).ok().flatten() else { return };
        let sf = m.scale_factor();
        let (pw, ph) = ((w * sf).round() as i32, (h * sf).round() as i32);
        let area = m.work_area();
        let (ax, ay) = (area.position.x, area.position.y);
        let (aw, ah) = (area.size.width as i32, area.size.height as i32);
        let pos = PhysicalPosition::new(x.clamp(ax, ax + aw - pw), y.clamp(ay, ay + ah - ph));
        let _ = win.set_position(pos);
        let _ = win.set_size(PhysicalSize::new(pw as u32, ph as u32));
        let _ = win.set_position(pos);
        popups(app).tails.insert(win.label().into(), Tail::default());
        return;
    }
    let Some(a) = presence::anchor(app).or_else(|| fallback_anchor(app)) else { return };
    let sf = a.scale_factor;
    let (pw, ph) = ((w * sf).round() as i32, (h * sf).round() as i32);
    let (ax, ay, aw, ah) = a.area;
    let gap = (6.0 * sf) as i32;
    let (cx, cy) = (a.x + a.size / 2, a.y + a.size / 2);
    let clamp = |x: i32, y: i32| (x.clamp(ax, (ax + aw - pw).max(ax)), y.clamp(ay, (ay + ah - ph).max(ay)));

    let (x, y, tail_edge) = match how {
        // Centred on her but kept fully on screen, so no option is cut off.
        Place::Around => {
            let (x, y) = clamp(cx - pw / 2, cy - ph / 2);
            (x, y, None)
        }
        Place::Center => {
            let (x, y) = (ax + (aw - pw) / 2, ay + (ah - ph) / 2);
            (x, y, None)
        }
        Place::Cover { x, y, .. } => (x, y, None),
        Place::At { x, y } => (x, y, None),
        Place::Beside => {
            let (x, y) = match a.edge {
                Edge::Bottom => clamp(cx - pw / 2, a.y - ph - gap),
                Edge::Top => clamp(cx - pw / 2, a.y + a.size + gap),
                Edge::Left => clamp(a.x + a.size + gap, cy - ph / 2),
                Edge::Right => clamp(a.x - pw - gap, cy - ph / 2),
            };
            (x, y, Some(a.edge))
        }
    };
    let size = match how {
        Place::Cover { w, h, .. } => PhysicalSize::new(w, h),
        _ => PhysicalSize::new(pw as u32, ph as u32),
    };
    let pos = PhysicalPosition::new(x, y);
    let _ = win.set_position(pos);
    let _ = win.set_size(size);
    let _ = win.set_position(pos);

    let space = [
        (cx - ax) as f64 / sf,
        (ax + aw - cx) as f64 / sf,
        (cy - ay) as f64 / sf,
        (ay + ah - cy) as f64 / sf,
    ];
    let tail = Tail { edge: tail_edge, x: (cx - x) as f64 / sf, y: (cy - y) as f64 / sf, space };
    popups(app).tails.insert(win.label().into(), tail);
}

/// Without a known squirrel spot (she is still starting), pretend she sits
/// at the bottom centre of the primary monitor.
fn fallback_anchor(app: &AppHandle) -> Option<presence::Anchor> {
    let m = app.primary_monitor().ok().flatten()?;
    let area = m.work_area();
    let (ax, ay, aw, ah) = (area.position.x, area.position.y, area.size.width as i32, area.size.height as i32);
    let size = (64.0 * m.scale_factor()) as i32;
    Some(presence::Anchor {
        x: ax + (aw - size) / 2,
        y: ay + ah - size,
        size,
        edge: Edge::Bottom,
        area: (ax, ay, aw, ah),
        scale_factor: m.scale_factor(),
    })
}

/// The popup measured its content: resize it and keep it anchored.
#[tauri::command]
pub fn popup_fit(app: AppHandle, window: WebviewWindow, w: f64, h: f64) -> Tail {
    if parked(&app, window.label()) {
        return Tail::default(); // stays off-screen until it is opened
    }
    let how = popups(&app).places.get(window.label()).copied().unwrap_or(Place::Beside);
    place(&app, &window, w, h, how);
    if !window.is_visible().unwrap_or(false) {
        reveal(&app, &window);
    }
    popup_tail(app, window)
}

/// Pages that do not size themselves show up once loaded.
#[tauri::command]
pub fn popup_ready(app: AppHandle, window: WebviewWindow) {
    if parked(&app, window.label()) {
        park(&window); // built ahead of time: it shows when it is opened
        return;
    }
    ensure_placed(&app, &window);
    reveal(&app, &window);
}

#[tauri::command]
pub fn popup_tail(app: AppHandle, window: WebviewWindow) -> Tail {
    if parked(&app, window.label()) {
        return Tail::default();
    }
    ensure_placed(&app, &window);
    popups(&app).tails.get(window.label()).copied().unwrap_or_default()
}

#[tauri::command]
pub fn popup_close(app: AppHandle, window: WebviewWindow) {
    let label = window.label().to_string();
    close(&app, &label);
    if OWNED.contains(&label.as_str()) {
        presence::release_peek(&app);
    }
}
