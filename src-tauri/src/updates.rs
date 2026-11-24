//! New version notice. Once at startup (after a short while) and then once a
//! day, asks GitHub for the latest release — one anonymous request that sends
//! nothing about the user. If it is newer than this build, the squirrel says
//! so (a visual notice, never a sound) every day until the new one is
//! installed. Ajustes → Privacidad can turn it off.

use crate::{i18n::t, notices, presence};
use serde::{Deserialize, Serialize};
use std::{sync::Mutex, time::Duration};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

const LATEST: &str = "https://api.github.com/repos/juanccstro/donguri/releases/latest";
/// Where people get it (always the newest release page).
pub const RELEASES: &str = "https://github.com/juanccstro/donguri/releases/latest";
const FIRST_CHECK: Duration = Duration::from_secs(60);
const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Default)]
pub struct Updates {
    /// A newer version than this one, once found.
    available: Option<String>,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

pub fn init(app: &AppHandle) {
    app.manage(Mutex::new(Updates::default()));
    let app = app.clone();
    // One sleeping task: no work at all between checks.
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            if presence::state(&app).settings.update_check {
                check(&app, false).await;
            }
            tokio::time::sleep(EVERY).await;
        }
    });
}

/// "1.2.3" (or "v1.2.3") as numbers; anything after a "-" is ignored.
fn parse(v: &str) -> Option<Vec<u64>> {
    let core = v.trim().trim_start_matches(['v', 'V']).split(['-', '+']).next()?;
    core.split('.').map(|p| p.parse().ok()).collect()
}

/// Whether `latest` is a higher version than `current`.
fn newer(latest: &str, current: &str) -> bool {
    match (parse(latest), parse(current)) {
        (Some(mut a), Some(mut b)) => {
            let n = a.len().max(b.len());
            a.resize(n, 0);
            b.resize(n, 0);
            a > b
        }
        _ => false,
    }
}

async fn latest() -> Result<Option<String>, String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("Donguri/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(LATEST)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("GitHub {}", resp.status().as_u16()));
    }
    let r: Release = resp.json().await.map_err(|e| e.to_string())?;
    Ok((!r.draft && !r.prerelease).then(|| r.tag_name.trim_start_matches(['v', 'V']).to_string()))
}

/// Asks GitHub and tells the user if there is a newer version. `asked`: the
/// user pressed "Buscar actualizaciones", so "you're up to date" (or "could
/// not check") is said too. A failed check keeps what an earlier one found.
async fn check(app: &AppHandle, asked: bool) -> Check {
    let current = env!("CARGO_PKG_VERSION");
    let result = latest().await.map(|v| v.filter(|v| newer(v, current)));
    let found = match &result {
        Ok(found) => {
            if let Some(state) = app.try_state::<Mutex<Updates>>()
                && let Ok(mut u) = state.lock()
            {
                u.available = found.clone();
            }
            crate::menus::refresh(app);
            found.clone()
        }
        Err(_) => available(app),
    };
    let (en, private) = {
        let st = presence::state(app);
        (st.settings.en(), st.settings.private_mode)
    };
    match (&result, &found) {
        (Err(_), _) if asked => notices::show(app, "warn", t(en, "update_failed")),
        (Ok(_), Some(v)) if asked || !private => {
            notices::show(app, "update", &t(en, "update_available").replace("{v}", v));
        }
        (Ok(None), None) if asked => notices::show(app, "ok", t(en, "update_none")),
        _ => {}
    }
    Check { available: found, failed: result.is_err() }
}

struct Check {
    available: Option<String>,
    failed: bool,
}

/// The newer version found, if any (for the menus).
pub fn available(app: &AppHandle) -> Option<String> {
    // The tray menu is built before this module starts.
    app.try_state::<Mutex<Updates>>()?.lock().ok()?.available.clone()
}

#[derive(Serialize)]
pub struct UpdateInfo {
    current: &'static str,
    available: Option<String>,
    /// The last check could not reach GitHub (no connection…).
    failed: bool,
}

#[tauri::command]
pub fn update_info(app: AppHandle) -> UpdateInfo {
    UpdateInfo { current: env!("CARGO_PKG_VERSION"), available: available(&app), failed: false }
}

/// "Buscar actualizaciones" in Ajustes.
#[tauri::command]
pub async fn update_check_now(app: AppHandle) -> UpdateInfo {
    let c = check(&app, true).await;
    UpdateInfo { current: env!("CARGO_PKG_VERSION"), available: c.available, failed: c.failed }
}

/// Opens the release page to download the new version.
#[tauri::command]
pub fn open_update(app: AppHandle) {
    let _ = app.opener().open_url(RELEASES, None::<&str>);
}

#[cfg(test)]
mod tests {
    use super::newer;

    #[test]
    fn compares_versions() {
        assert!(newer("v0.1.3", "0.1.2"));
        assert!(newer("0.2.0", "0.1.9"));
        assert!(newer("1.0", "0.9.9"));
        assert!(!newer("v0.1.2", "0.1.2"));
        assert!(!newer("0.1.1", "0.1.2"));
        assert!(!newer("latest", "0.1.2"));
        assert!(!newer("0.1.2-beta", "0.1.2"));
    }
}
