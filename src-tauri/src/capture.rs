//! Area capture: screenshot the monitor under the cursor, let the user drag a
//! rectangle over a dimmed copy of it, crop and offer it as an acorn.

use crate::{popups::{self, Place, Spec}, presence, saving};
use base64::Engine;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

pub const LABEL: &str = "captura";

#[derive(Default)]
pub struct Shot {
    image: Option<image::RgbaImage>,
}

pub fn init(app: &AppHandle) {
    app.manage(Mutex::new(Shot::default()));
}

pub fn start(app: &AppHandle) {
    let (private, en) = {
        let st = presence::state(app);
        (st.settings.private_mode, st.settings.en())
    };
    if private {
        crate::notices::show(app, "warn", crate::i18n::t(en, "private_on"));
        return;
    }
    // Wait for menus that started this (they fade out) before grabbing pixels.
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let Ok(cursor) = app.cursor_position() else { return };
        let Some(target) = app.monitor_from_point(cursor.x, cursor.y).ok().flatten() else { return };
        let Ok(monitor) = xcap::Monitor::from_point(cursor.x as i32, cursor.y as i32) else { return };
        let Ok(img) = monitor.capture_image() else { return };
        // Cover the whole monitor (physical pixels, from Tauri's view of it).
        let (px, py) = (target.position().x, target.position().y);
        let (w, h) = (target.size().width, target.size().height);
        app.state::<Mutex<Shot>>().lock().unwrap_or_else(|e| e.into_inner()).image = Some(img);
        popups::open(
            &app,
            Spec { label: LABEL, url: "captura", w: 100.0, h: 100.0, focusable: true, place: Place::Cover { x: px, y: py, w, h } },
        );
    });
}

/// The screenshot as a PNG data URL for the overlay to show.
#[tauri::command]
pub fn capture_image(app: AppHandle) -> Option<String> {
    let shot = app.state::<Mutex<Shot>>();
    let shot = shot.lock().ok()?;
    let img = shot.image.as_ref()?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out.into_inner())))
}

/// Rectangle as fractions (0..1) of the screenshot; `None` cancels.
#[tauri::command]
pub fn capture_finish(app: AppHandle, rect: Option<[f64; 4]>) {
    let img = app.state::<Mutex<Shot>>().lock().ok().and_then(|mut s| s.image.take());
    popups::close(&app, LABEL);
    let (Some(img), Some([fx, fy, fw, fh])) = (img, rect) else { return };
    let (w, h) = (img.width() as f64, img.height() as f64);
    let x = (fx * w).round().clamp(0.0, w - 1.0) as u32;
    let y = (fy * h).round().clamp(0.0, h - 1.0) as u32;
    let cw = (fw * w).round().clamp(1.0, w - x as f64) as u32;
    let ch = (fh * h).round().clamp(1.0, h - y as f64) as u32;
    if cw < 4 || ch < 4 {
        return;
    }
    let crop = image::imageops::crop_imm(&img, x, y, cw, ch).to_image();
    let (cw, ch) = crop.dimensions();
    if let Some(png) = saving::rgba_to_png(cw, ch, crop.into_raw()) {
        let title = format!("Captura {}", chrono::Local::now().format("%d/%m %H:%M"));
        saving::begin_image(&app, &png, title, "captura");
    }
}
