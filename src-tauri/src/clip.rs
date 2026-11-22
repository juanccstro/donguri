//! Clipboard and keyboard simulation: grab the current selection from any app
//! and paste text back into it, restoring the user's clipboard afterwards.

use crate::platform;
use arboard::{Clipboard, ImageData};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::{borrow::Cow, thread::sleep, time::Duration};

/// Until when (ms since the epoch) clipboard changes are Donguri's own: the
/// copy that grabs a selection, the paste and its restore. The clipboard
/// history skips them.
static OURS_UNTIL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Marks the clipboard changes of the next `ms` as ours.
fn ours_for(ms: u64) {
    OURS_UNTIL.fetch_max(now_ms() + ms, std::sync::atomic::Ordering::Relaxed);
}

/// Whether a clipboard change happening now was made by Donguri.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn is_ours() -> bool {
    now_ms() <= OURS_UNTIL.load(std::sync::atomic::Ordering::Relaxed)
}

pub enum Content {
    Text(String),
    Image { width: usize, height: usize, rgba: Vec<u8> },
}

/// Apps where Ctrl+C means "interrupt", never "copy".
const TERMINALS: &[&str] = &[
    "windowsterminal.exe", "cmd.exe", "powershell.exe", "pwsh.exe", "conhost.exe", "wt.exe",
    "alacritty.exe", "wezterm-gui.exe", "mintty.exe", "terminal", "iterm2", "warp",
];

pub fn is_terminal(exe: Option<&str>) -> bool {
    exe.is_some_and(|e| TERMINALS.contains(&e))
}

#[cfg(target_os = "macos")]
const MODIFIER: Key = Key::Meta;
#[cfg(not(target_os = "macos"))]
const MODIFIER: Key = Key::Control;

#[cfg(target_os = "windows")]
fn letter(c: char) -> Key {
    match c {
        'c' => Key::C,
        _ => Key::V,
    }
}
#[cfg(not(target_os = "windows"))]
fn letter(c: char) -> Key {
    Key::Unicode(c)
}

fn chord(c: char) -> Option<()> {
    let mut e = Enigo::new(&Settings::default()).ok()?;
    // The global shortcut's own modifiers may still be down: lift them first
    // or the app would receive Ctrl+Alt+Shift+C.
    for k in [Key::Alt, Key::Shift, Key::Control, Key::Meta] {
        let _ = e.key(k, Direction::Release);
    }
    sleep(Duration::from_millis(30));
    e.key(MODIFIER, Direction::Press).ok()?;
    let _ = e.key(letter(c), Direction::Click);
    e.key(MODIFIER, Direction::Release).ok()
}

enum Saved {
    Text(String),
    Image(ImageData<'static>),
    Nothing,
}

fn save(cb: &mut Clipboard) -> Saved {
    if let Ok(t) = cb.get_text() {
        return Saved::Text(t);
    }
    match cb.get_image() {
        Ok(i) => Saved::Image(i),
        Err(_) => Saved::Nothing,
    }
}

fn restore(cb: &mut Clipboard, saved: Saved) {
    let _ = match saved {
        Saved::Text(t) => cb.set_text(t),
        Saved::Image(i) => cb.set_image(i),
        Saved::Nothing => cb.clear(),
    };
}

/// Changes whenever anything is put on the clipboard (Windows); elsewhere
/// the text itself is compared.
#[cfg(windows)]
fn clipboard_version() -> u32 {
    unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() }
}

/// Copies whatever is selected in the foreground app. The clipboard is never
/// emptied, and is restored only if the copy really put something there.
/// Blocks up to ~0.6 s, so call it off the main thread.
pub fn copy_selection(foreground_exe: Option<&str>) -> Option<String> {
    if is_terminal(foreground_exe) {
        return None;
    }
    // Let a just-restored foreground window settle before sending Ctrl+C.
    sleep(Duration::from_millis(90));
    ours_for(1500);
    let mut cb = Clipboard::new().ok()?;
    let saved = save(&mut cb);
    #[cfg(windows)]
    let before = clipboard_version();
    #[cfg(not(windows))]
    let before_text = match &saved {
        Saved::Text(t) => Some(t.clone()),
        _ => None,
    };
    chord('c')?;
    let mut copied = None;
    for _ in 0..24 {
        sleep(Duration::from_millis(25));
        #[cfg(windows)]
        let changed = clipboard_version() != before;
        #[cfg(not(windows))]
        let changed = cb.get_text().ok() != before_text;
        if changed {
            // The app may announce the copy a moment before the text is there.
            sleep(Duration::from_millis(20));
            copied = Some(cb.get_text().ok().filter(|t| !t.trim().is_empty()));
            break;
        }
    }
    let copied = copied?;
    ours_for(500);
    restore(&mut cb, saved);
    copied
}

pub fn read() -> Option<Content> {
    let mut cb = Clipboard::new().ok()?;
    if let Ok(t) = cb.get_text()
        && !t.trim().is_empty()
    {
        return Some(Content::Text(t));
    }
    let img = cb.get_image().ok()?;
    Some(Content::Image { width: img.width, height: img.height, rgba: img.bytes.into_owned() })
}

pub fn set_text(text: &str) -> bool {
    ours_for(500);
    Clipboard::new().and_then(|mut cb| cb.set_text(text)).is_ok()
}

fn png_to_image(png: &[u8]) -> Option<ImageData<'static>> {
    let img = image::load_from_memory(png).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(ImageData { width: w as usize, height: h as usize, bytes: Cow::Owned(img.into_raw()) })
}

/// Puts an image (PNG bytes) on the clipboard.
pub fn set_png(png: &[u8]) -> bool {
    ours_for(500);
    let Some(img) = png_to_image(png) else { return false };
    Clipboard::new().and_then(|mut cb| cb.set_image(img)).is_ok()
}

/// Re-sends Ctrl/Cmd+V to whatever has focus (used when our temporary
/// paste shortcut catches a paste that was meant for another app).
pub fn forward_paste() {
    std::thread::spawn(|| {
        sleep(Duration::from_millis(30));
        let _ = chord('v');
    });
}

/// Puts `text` into the app the user was last in. The clipboard is restored
/// a moment later. Call off the main thread.
pub fn paste_into_last_app(text: &str) -> bool {
    paste_with(|cb| cb.set_text(Cow::Borrowed(text)).is_ok())
}

/// Same, with an image (PNG bytes).
pub fn paste_png_into_last_app(png: &[u8]) -> bool {
    let Some(img) = png_to_image(png) else { return false };
    paste_with(move |cb| cb.set_image(img).is_ok())
}

fn paste_with(put: impl FnOnce(&mut Clipboard) -> bool) -> bool {
    ours_for(1500);
    let Ok(mut cb) = Clipboard::new() else { return false };
    let saved = save(&mut cb);
    if !put(&mut cb) {
        return false;
    }
    if !platform::focus_last_app() {
        // Leave the text on the clipboard so the user can paste by hand.
        return false;
    }
    sleep(Duration::from_millis(150));
    let ok = chord('v').is_some();
    sleep(Duration::from_millis(400));
    ours_for(500);
    restore(&mut cb, saved);
    ok
}
