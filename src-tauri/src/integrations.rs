//! Connects `donguri-mcp` to Claude Desktop and Claude Code (and undoes it).
//! Every config file is backed up before it is touched.

use crate::saving;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

const SERVER: &str = "donguri";

fn mcp_exe() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let name = if cfg!(windows) { "donguri-mcp.exe" } else { "donguri-mcp" };
    let path = dir.join(name);
    path.exists().then_some(path)
}

fn server_entry(app: &AppHandle) -> Result<Value, String> {
    let exe = mcp_exe().ok_or("No encuentro donguri-mcp junto a la app")?;
    let db = saving::data(app).db_path.clone();
    Ok(json!({
        "command": exe.to_string_lossy(),
        "args": ["--db", db.to_string_lossy()],
    }))
}

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// Claude Desktop's config file. On Windows the Store (MSIX) build keeps it
/// inside its package folder when it has written one there.
fn desktop_config() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
            && let Ok(entries) = std::fs::read_dir(local.join("Packages"))
        {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("Claude_") {
                    let p = e.path().join("LocalCache/Roaming/Claude/claude_desktop_config.json");
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Claude/claude_desktop_config.json"))
    }
    #[cfg(target_os = "macos")]
    {
        home().map(|h| h.join("Library/Application Support/Claude/claude_desktop_config.json"))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        home().map(|h| h.join(".config/Claude/claude_desktop_config.json"))
    }
}

fn code_config() -> Option<PathBuf> {
    home().map(|h| h.join(".claude.json"))
}

fn read_json(path: &Path) -> Result<Value, String> {
    match std::fs::read_to_string(path) {
        Ok(s) if !s.trim().is_empty() => serde_json::from_str(&s).map_err(|e| format!("{}: {e}", path.display())),
        _ => Ok(json!({})),
    }
}

fn backup(path: &Path) {
    if path.exists() {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let mut b = path.as_os_str().to_owned();
        b.push(format!(".donguri-{stamp}.bak"));
        let _ = std::fs::copy(path, PathBuf::from(b));
    }
}

fn write_json(path: &Path, v: &Value) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    backup(path);
    let text = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

fn has_server(path: &Path) -> bool {
    read_json(path).is_ok_and(|v| v.pointer(&format!("/mcpServers/{SERVER}")).is_some())
}

fn set_server(path: &Path, entry: Option<Value>) -> Result<(), String> {
    let mut v = read_json(path)?;
    let obj = v.as_object_mut().ok_or("config no es un objeto JSON")?;
    let servers = obj.entry("mcpServers").or_insert_with(|| Value::Object(Map::new()));
    let servers = servers.as_object_mut().ok_or("mcpServers no es un objeto")?;
    match entry {
        Some(e) => {
            servers.insert(SERVER.into(), e);
        }
        None => {
            servers.remove(SERVER);
        }
    }
    write_json(path, &v)
}

/// The `claude` CLI if installed (Claude Code).
fn claude_cli() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["claude.exe", "claude.cmd"] } else { &["claude"] };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if let Some(h) = home() {
        dirs.push(h.join(".local/bin"));
        dirs.push(h.join(".claude/local"));
    }
    if let Some(a) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(a).join("npm"));
    }
    dirs.into_iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.exists())
}

fn run_cli(args: &[&str]) -> Result<(), String> {
    let cli = claude_cli().ok_or("no cli")?;
    let mut cmd = std::process::Command::new(cli);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[derive(Serialize)]
pub struct Status {
    desktop: bool,
    desktop_path: Option<String>,
    code: bool,
    code_cli: bool,
    mcp_found: bool,
}

#[tauri::command]
pub fn integration_status() -> Status {
    let desktop_path = desktop_config();
    Status {
        desktop: desktop_path.as_deref().is_some_and(has_server),
        desktop_path: desktop_path.map(|p| p.to_string_lossy().into()),
        code: code_config().as_deref().is_some_and(has_server),
        code_cli: claude_cli().is_some(),
        mcp_found: mcp_exe().is_some(),
    }
}

/// target: "desktop" | "code"; connect: true to add, false to undo.
#[tauri::command]
pub fn integration_set(app: AppHandle, target: String, connect: bool) -> Result<Status, String> {
    match target.as_str() {
        "desktop" => {
            let path = desktop_config().ok_or("No encuentro la configuración de Claude Desktop")?;
            set_server(&path, if connect { Some(server_entry(&app)?) } else { None })?;
        }
        "code" => {
            let cli_result = if connect {
                let entry = server_entry(&app)?;
                let cmd = entry["command"].as_str().unwrap_or_default().to_string();
                let db = entry["args"][1].as_str().unwrap_or_default().to_string();
                run_cli(&["mcp", "add", "--scope", "user", SERVER, "--", &cmd, "--db", &db])
            } else {
                run_cli(&["mcp", "remove", "--scope", "user", SERVER])
            };
            // Without the CLI, edit ~/.claude.json the same way it would.
            if cli_result.is_err() {
                let path = code_config().ok_or("No encuentro ~/.claude.json")?;
                let entry = if connect {
                    let mut e = server_entry(&app)?;
                    e["type"] = json!("stdio");
                    e["env"] = json!({});
                    Some(e)
                } else {
                    None
                };
                set_server(&path, entry)?;
            }
        }
        _ => return Err("destino desconocido".into()),
    }
    Ok(integration_status())
}
