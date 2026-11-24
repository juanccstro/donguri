//! Persistent user settings (`settings.json` in the app config dir).
//! API keys are never stored here: they live in the OS keychain (see ai.rs).

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Only blinks.
    Quieta,
    /// Occasional idle animations and short walks.
    #[default]
    Viva,
    /// Tucked away: only an acorn sticks out of the edge.
    Escondida,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}

impl Edge {
    pub fn horizontal(self) -> bool {
        matches!(self, Edge::Bottom | Edge::Top)
    }
}

/// Where the squirrel sits on one monitor: an edge plus a 0..1 fraction along it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub edge: Edge,
    pub along: f64,
}

impl Default for Placement {
    fn default() -> Self {
        Self { edge: Edge::Bottom, along: 0.9 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Shortcuts {
    pub toggle: String,
    pub save: String,
    pub capture: String,
    pub quick: String,
    pub capture_ask: String,
    pub clipboard: String,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            toggle: "Alt+Shift+B".into(),
            save: "Alt+Shift+S".into(),
            capture: "Alt+Shift+X".into(),
            quick: "Alt+Shift+Space".into(),
            capture_ask: "Alt+Shift+A".into(),
            clipboard: "Alt+Shift+V".into(),
        }
    }
}

/// Result of the last minimal test request to one model.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Probe {
    pub id: String,
    /// "ok" | "busy" (overloaded/timeout) | "unavailable" (404/403/429…)
    pub status: String,
    pub ms: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct AiSettings {
    /// "gemini" | "claude" | "openai"
    pub default_provider: Option<String>,
    /// Chosen model id per provider.
    pub models: HashMap<String, String>,
    /// Providers whose model the user picked by hand (not auto-chosen).
    pub manual: HashMap<String, bool>,
    /// Last probe results per provider.
    pub probes: HashMap<String, Vec<Probe>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// Integer sprite scale (4, 6 or 8) in logical pixels per 16×16 art pixel.
    pub scale: u32,
    pub mode: Mode,
    pub palette: String,
    pub accessory: String,
    /// "es" | "en"
    pub lang: String,
    pub hide_fullscreen: bool,
    pub exclude_from_capture: bool,
    pub fade_when_idle: bool,
    pub private_mode: bool,
    pub welcome_done: bool,
    /// Show the little acorn next to the cursor after selecting text.
    pub selection_hint: bool,
    /// Let the AI suggest a nest when saving (sends what is saved to the AI).
    pub ai_suggest: bool,
    /// Keep a clipboard history in memory (opt-in).
    pub clip_history: bool,
    /// The clipboard history also keeps copied images (screenshots…).
    pub clip_images: bool,
    /// Ask GitHub once a day whether there is a newer version.
    pub update_check: bool,
    /// Without an API key, questions open here: "claude" | "chatgpt".
    pub handoff: String,
    /// Lower-cased executable names where the squirrel always hides.
    pub blacklist: Vec<String>,
    /// Unix timestamp until which the squirrel stays hidden.
    pub hidden_until: Option<i64>,
    pub shortcuts: Shortcuts,
    pub active_nest: Option<i64>,
    pub ai: AiSettings,
    /// Monitor the user last dropped the squirrel on.
    pub preferred_monitor: Option<String>,
    pub placements: HashMap<String, Placement>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            scale: 4,
            mode: Mode::default(),
            palette: "rojiza".into(),
            accessory: "ninguno".into(),
            lang: "es".into(),
            hide_fullscreen: true,
            exclude_from_capture: true,
            fade_when_idle: true,
            private_mode: false,
            welcome_done: false,
            selection_hint: true,
            ai_suggest: false,
            clip_history: false,
            clip_images: true,
            update_check: true,
            handoff: "claude".into(),
            blacklist: Vec::new(),
            hidden_until: None,
            shortcuts: Shortcuts::default(),
            active_nest: None,
            ai: AiSettings::default(),
            preferred_monitor: None,
            placements: HashMap::new(),
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        let mut settings: Self = fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if !matches!(settings.scale, 4 | 6 | 8) {
            settings.scale = Self::default().scale;
        }
        if !matches!(settings.lang.as_str(), "es" | "en") {
            settings.lang = "es".into();
        }
        settings
    }

    pub fn save(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }

    pub fn en(&self) -> bool {
        self.lang == "en"
    }
}
