//! Gemini, Claude and OpenAI with the user's own keys. Keys live in the OS
//! keychain; models come from each provider's model list (never hard-coded),
//! filtered to text chat, probed with a tiny request, sorted cheapest first.
//! Network traffic happens here only on request (plus the daily version
//! check in updates.rs).

use crate::{
    presence, saving,
    settings::Probe,
    store,
};
use futures_util::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

const KEYCHAIN_SERVICE: &str = "com.donguri.app";
pub const PROVIDERS: [&str; 3] = ["gemini", "claude", "openai"];
/// How much of a nest goes into a question (characters, ~30k tokens).
const NEST_BUDGET: usize = 120_000;
const BUBBLE: &str = "bocadillo";
/// A model that gives no text within this is saturated: the next one is tried.
/// Big requests get longer (see `first_text_wait`): reading a long context
/// takes a healthy model a while before its first word.
const FIRST_TEXT_TIMEOUT: Duration = Duration::from_secs(3);
/// Extra second per this many words sent (system prompt, nest, selection…).
const WORDS_PER_EXTRA_SECOND: usize = 10_000;
/// The wait never grows beyond this before trying the next model.
const FIRST_TEXT_MAX: Duration = Duration::from_secs(8);
/// The last model in line gets longer, so the answer still comes.
const LAST_TRY_TIMEOUT: Duration = Duration::from_secs(20);
/// A saturated model waits this long at the back of the line; then it is
/// first again, so the cheapest one comes back by itself once it recovers.
const PENALTY: Duration = Duration::from_secs(2 * 60);
const PROBE_TIMEOUT: Duration = Duration::from_secs(25);
/// Extra wait for the first words per attached image (up to two).
const IMAGE_EXTRA: Duration = Duration::from_secs(2);
/// Images are scaled down to this longest side before they are sent: enough
/// to read a screenshot, and every provider accepts it.
const IMAGE_MAX_SIDE: u32 = 1568;
/// At most this many images of a nest go with a question (newest first).
const NEST_IMAGES: usize = 3;

// ------------------------------------------------------------------ keys

fn entry(provider: &str) -> Option<keyring::Entry> {
    keyring::Entry::new(KEYCHAIN_SERVICE, provider).ok()
}

pub fn key(provider: &str) -> Option<String> {
    entry(provider)?.get_password().ok().filter(|k| !k.trim().is_empty())
}

pub fn any_configured() -> bool {
    PROVIDERS.iter().any(|p| key(p).is_some())
}

#[tauri::command]
pub fn set_key(app: AppHandle, provider: String, key: String) -> Result<(), String> {
    let e = entry(&provider).ok_or("keychain no disponible")?;
    if key.trim().is_empty() {
        let _ = e.delete_credential();
        presence::update(&app, |s| {
            s.ai.models.remove(&provider);
            s.ai.manual.remove(&provider);
            s.ai.probes.remove(&provider);
            if s.ai.default_provider.as_deref() == Some(provider.as_str()) {
                s.ai.default_provider = None;
            }
        });
        return Ok(());
    }
    e.set_password(key.trim()).map_err(|e| e.to_string())?;
    presence::update(&app, |s| {
        if s.ai.default_provider.is_none() {
            s.ai.default_provider = Some(provider.clone());
        }
    });
    Ok(())
}

#[derive(Serialize)]
pub struct ProviderStatus {
    id: &'static str,
    configured: bool,
    model: Option<String>,
    default: bool,
    manual: bool,
}

#[tauri::command]
pub fn ai_status(app: AppHandle) -> Vec<ProviderStatus> {
    let st = presence::state(&app);
    PROVIDERS
        .iter()
        .map(|p| ProviderStatus {
            id: p,
            configured: key(p).is_some(),
            model: st.settings.ai.models.get(*p).cloned(),
            default: st.settings.ai.default_provider.as_deref() == Some(*p),
            manual: st.settings.ai.manual.get(*p).copied().unwrap_or(false),
        })
        .collect()
}

/// The user picked a model by hand (stops automatic choice for that provider).
#[tauri::command]
pub fn set_model(app: AppHandle, provider: String, model: String) {
    presence::update(&app, |s| {
        s.ai.models.insert(provider.clone(), model);
        s.ai.manual.insert(provider, true);
    });
}

/// Back to "always use the cheapest model that works".
#[tauri::command]
pub fn set_model_auto(app: AppHandle, provider: String) -> Option<String> {
    presence::update(&app, |s| {
        s.ai.manual.remove(&provider);
    });
    let pick = auto_pick(&app, &provider);
    if let Some(m) = &pick {
        let (p, m) = (provider.clone(), m.clone());
        presence::update(&app, move |s| {
            s.ai.models.insert(p, m);
        });
    }
    pick
}

#[tauri::command]
pub fn set_default_provider(app: AppHandle, provider: String) {
    presence::update(&app, |s| s.ai.default_provider = Some(provider));
}

// --------------------------------------------------------------- catalog

/// Model families that are not text chat (images, video, audio, speech,
/// agents, robotics…): never offered.
const NOT_TEXT: [&str; 26] = [
    "image", "banana", "imagen", "veo", "tts", "audio", "speech", "lyria", "music", "transcribe",
    "whisper", "realtime", "embedding", "embed", "aqa", "robotics", "computer-use", "antigravity",
    "deep-research", "customtools", "omni", "dall-e", "moderation", "search", "-latest", "sora",
];

fn is_text_chat(provider: &str, id: &str) -> bool {
    let id = id.to_lowercase();
    if NOT_TEXT.iter().any(|s| id.contains(s)) {
        return false;
    }
    match provider {
        "openai" => {
            ["gpt-", "o1", "o3", "o4", "chatgpt-"].iter().any(|p| id.starts_with(p))
                && !id.contains("instruct")
                && !id.contains("codex")
        }
        "claude" => id.starts_with("claude-"),
        _ => id.starts_with("gemini-") || id.starts_with("gemma-"),
    }
}

/// Known list prices in USD per million tokens (input, output). Used only to
/// sort and label; unknown models fall back to their tier.
fn known_price(provider: &str, id: &str) -> Option<(f64, f64)> {
    if provider != "claude" {
        return None;
    }
    const TABLE: [(&str, f64, f64); 12] = [
        ("claude-haiku-5", 0.10, 0.50),
        ("claude-haiku-4", 1.0, 5.0),
        ("claude-sonnet-5", 2.0, 10.0),
        ("claude-sonnet-4", 3.0, 15.0),
        ("claude-opus-5-5", 4.0, 20.0),
        ("claude-opus-5", 5.0, 25.0),
        ("claude-opus-4", 5.0, 25.0),
        ("claude-fable-5-2", 8.0, 40.0),
        ("claude-mythos-5-2", 8.0, 40.0),
        ("claude-fable-5", 10.0, 50.0),
        ("claude-mythos-5", 10.0, 50.0),
        ("claude-3", 3.0, 15.0),
    ];
    TABLE.iter().find(|(p, _, _)| id.starts_with(p)).map(|(_, i, o)| (*i, *o))
}

/// Cost tier from the name when no price is known: 1 cheapest … 4 priciest.
fn tier(id: &str) -> u32 {
    // Whole name parts only: "gemini" must not count as "mini".
    let id = id.to_lowercase();
    let parts: Vec<&str> = id.split(['-', '_', '.', ':']).collect();
    let has = |words: &[&str]| words.iter().any(|w| parts.contains(w));
    if has(&["lite", "nano", "haiku", "small"]) || id.starts_with("gemma") {
        1
    } else if has(&["flash", "mini"]) {
        2
    } else if has(&["pro", "opus", "fable", "mythos", "ultra", "max"]) {
        4
    } else {
        3
    }
}

fn cost_rank(provider: &str, id: &str) -> f64 {
    match known_price(provider, id) {
        // Typical chat: ~1k tokens in, ~0.6k out.
        Some((i, o)) => i + o * 0.6,
        None => [0.0, 0.3, 1.5, 6.0, 25.0][tier(id) as usize],
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Model {
    id: String,
    name: String,
    /// 1 cheapest … 4 priciest.
    tier: u32,
    /// "$0.10 / $0.50" per million tokens, when known.
    price: Option<String>,
    /// "ok" | "busy" | "untested"
    status: String,
    ms: Option<u64>,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("Donguri/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}

async fn error_text(resp: reqwest::Response) -> (u16, String) {
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    let msg = serde_json::from_str::<Value>(&body).ok().and_then(|v| {
        v.pointer("/error/message")
            .or_else(|| v.pointer("/0/error/message"))
            .and_then(Value::as_str)
            .map(String::from)
    });
    let text = match status {
        401 | 403 => format!("Clave no válida o sin permiso ({status})"),
        _ => msg.unwrap_or_else(|| format!("Error {status}")),
    };
    (status, text)
}

/// Raw list from the provider, already filtered to text chat models.
async fn fetch_models(provider: &str, key: &str) -> Result<Vec<(String, String)>, String> {
    let c = client();
    let req = match provider {
        "claude" => c
            .get("https://api.anthropic.com/v1/models?limit=100")
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01"),
        "openai" => c.get("https://api.openai.com/v1/models").bearer_auth(key),
        "gemini" => c
            .get("https://generativelanguage.googleapis.com/v1beta/models?pageSize=1000")
            .header("x-goog-api-key", key),
        _ => return Err("proveedor desconocido".into()),
    };
    let resp = req.send().await.map_err(|e| format!("Sin conexión: {e}"))?;
    if !resp.status().is_success() {
        return Err(error_text(resp).await.1);
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    let list: Vec<(String, String)> = match provider {
        "claude" => v["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| {
                let id = m["id"].as_str()?.to_string();
                let name = m["display_name"].as_str().unwrap_or(&id).to_string();
                Some((id, name))
            })
            .collect(),
        "openai" => v["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| {
                let id = m["id"].as_str()?.to_string();
                Some((id.clone(), id))
            })
            .collect(),
        _ => v["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| {
                let methods = m["supportedGenerationMethods"].as_array()?;
                if !methods.iter().any(|x| x.as_str() == Some("generateContent")) {
                    return None;
                }
                let id = m["name"].as_str()?.trim_start_matches("models/").to_string();
                let name = m["displayName"].as_str().unwrap_or(&id).to_string();
                Some((id, name))
            })
            .collect(),
    };
    Ok(list.into_iter().filter(|(id, _)| is_text_chat(provider, id)).collect())
}

/// Text models, sorted cheapest first, with what the last probe found.
/// Models that failed the probe (not offered to this key) are left out.
fn catalog(provider: &str, raw: Vec<(String, String)>, probes: &[Probe]) -> Vec<Model> {
    let mut models: Vec<Model> = raw
        .into_iter()
        .filter_map(|(id, name)| {
            let probe = probes.iter().find(|p| p.id == id);
            if probe.is_some_and(|p| p.status == "unavailable") {
                return None;
            }
            Some(Model {
                tier: tier(&id),
                price: known_price(provider, &id).map(|(i, o)| format!("${i:.2} / ${o:.2}")),
                status: probe.map_or("untested".into(), |p| p.status.clone()),
                ms: probe.and_then(|p| p.ms),
                name,
                id,
            })
        })
        .collect();
    models.sort_by(|a, b| {
        cost_rank(provider, &a.id)
            .total_cmp(&cost_rank(provider, &b.id))
            .then(a.ms.unwrap_or(u64::MAX).cmp(&b.ms.unwrap_or(u64::MAX)))
    });
    models
}

fn saved_probes(app: &AppHandle, provider: &str) -> Vec<Probe> {
    presence::state(app).settings.ai.probes.get(provider).cloned().unwrap_or_default()
}

/// Cheapest model that answered the probe, else the cheapest not ruled out.
fn auto_pick(app: &AppHandle, provider: &str) -> Option<String> {
    let probes = saved_probes(app, provider);
    let mut ok: Vec<&Probe> = probes.iter().filter(|p| p.status == "ok").collect();
    ok.sort_by(|a, b| {
        cost_rank(provider, &a.id)
            .total_cmp(&cost_rank(provider, &b.id))
            .then(a.ms.unwrap_or(u64::MAX).cmp(&b.ms.unwrap_or(u64::MAX)))
    });
    ok.first().map(|p| p.id.clone())
}

// ----------------------------------------------------------------- probes

/// A minimal request ("ok") that costs next to nothing and tells us whether
/// the model works for this key and how fast it answers.
async fn probe_one(provider: &str, key: &str, id: &str) -> Probe {
    let msgs = [Msg { role: "user".into(), content: "Responde solo: ok".into() }];
    let start = Instant::now();
    let fut = async {
        let mut fast = true;
        loop {
            let resp = request_with(provider, key, id, "Responde con una palabra.", &msgs, &[], false, Some(64), fast)
                .send()
                .await
                .map_err(|_| (true, String::new()))?;
            if resp.status().is_success() {
                return Ok(());
            }
            let (code, msg) = error_text(resp).await;
            // Same as the chat: a model that rejects "think less" is retried
            // without it before being judged.
            let lower = msg.to_lowercase();
            if fast && code == 400 && ["thinking", "effort", "reasoning"].iter().any(|w| lower.contains(w)) {
                fast = false;
                continue;
            }
            return Err((retryable(code, &msg), msg));
        }
    };
    let status = match tokio::time::timeout(PROBE_TIMEOUT, fut).await {
        Ok(Ok(())) => "ok",
        Ok(Err((true, _))) | Err(_) => "busy",
        Ok(Err((false, _))) => "unavailable",
    };
    Probe {
        id: id.to_string(),
        status: status.into(),
        ms: (status == "ok").then(|| start.elapsed().as_millis() as u64),
    }
}

/// Overloaded, rate-limited for a moment or timed out: worth another model.
/// Quota exhausted / not offered / bad request: this model will not work.
/// The model is not offered to this key at all: retired, unknown, or zero
/// quota on its plan ("limit: 0"). A plain per-minute rate limit is temporary.
fn not_offered(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("no longer available") || m.contains("not found") || m.contains("limit: 0")
}

fn retryable(code: u16, msg: &str) -> bool {
    if not_offered(msg) {
        return false;
    }
    matches!(code, 0 | 408 | 429 | 500 | 502 | 503 | 504 | 529)
}

/// Lists, probes (2 at a time) and stores which text models work, then picks
/// the cheapest working one unless the user chose a model by hand.
#[tauri::command]
pub async fn probe_models(app: AppHandle, provider: String) -> Result<Vec<Model>, String> {
    let key = key(&provider).ok_or("Sin clave")?;
    let raw = fetch_models(&provider, &key).await?;
    let ids: Vec<String> = raw.iter().map(|(id, _)| id.clone()).collect();
    let mut futures = Vec::with_capacity(ids.len());
    for id in ids {
        let (p, k) = (provider.clone(), key.clone());
        futures.push(Box::pin(async move { probe_one(&p, &k, &id).await }));
    }
    // Two at a time: free tiers count requests per minute.
    let probes: Vec<Probe> = stream::iter(futures).buffer_unordered(2).collect().await;
    {
        let (p, list) = (provider.clone(), probes.clone());
        presence::update(&app, move |s| {
            s.ai.probes.insert(p, list);
        });
    }
    let manual = presence::state(&app).settings.ai.manual.get(&provider).copied().unwrap_or(false);
    if !manual && let Some(best) = auto_pick(&app, &provider) {
        let p = provider.clone();
        presence::update(&app, move |s| {
            s.ai.models.insert(p, best);
        });
    }
    Ok(catalog(&provider, raw, &probes))
}

/// "Probar clave": lists the models (free); probing happens right after.
#[tauri::command]
pub async fn test_key(provider: String, key: String) -> Result<usize, String> {
    let models = fetch_models(&provider, key.trim()).await?;
    if models.is_empty() {
        return Err("La clave funciona pero no hay modelos de texto disponibles".into());
    }
    Ok(models.len())
}

#[tauri::command]
pub async fn list_models(app: AppHandle, provider: String) -> Result<Vec<Model>, String> {
    let key = key(&provider).ok_or("Sin clave")?;
    let raw = fetch_models(&provider, &key).await?;
    Ok(catalog(&provider, raw, &saved_probes(&app, &provider)))
}

/// Provider, key and the models to try in order: the chosen one first, then
/// the other working ones, cheapest first (for automatic fallback).
fn plan(app: &AppHandle) -> Result<(String, String, Vec<String>), String> {
    let (default, models) = {
        let st = presence::state(app);
        (st.settings.ai.default_provider.clone(), st.settings.ai.models.clone())
    };
    let provider = default
        .into_iter()
        .chain(PROVIDERS.iter().map(|p| p.to_string()))
        .find(|p| key(p).is_some())
        .ok_or("NO_KEY")?;
    let key = key(&provider).ok_or("NO_KEY")?;
    let manual = presence::state(app).settings.ai.manual.get(&provider).copied().unwrap_or(false);
    let mut probes = saved_probes(app, &provider);
    let dead = |id: &str| probes.iter().any(|p| p.id == id && p.status == "unavailable");
    // A model picked by hand goes first; otherwise the cheapest working one.
    let first = if manual || probes.is_empty() { models.get(&provider).filter(|m| !dead(m)).cloned() } else { None };
    let mut order: Vec<String> = first.into_iter().collect();
    probes.sort_by(|a, b| {
        cost_rank(&provider, &a.id)
            .total_cmp(&cost_rank(&provider, &b.id))
            .then(a.ms.unwrap_or(u64::MAX).cmp(&b.ms.unwrap_or(u64::MAX)))
    });
    for p in probes.iter().filter(|p| p.status != "unavailable") {
        if !order.contains(&p.id) {
            order.push(p.id.clone());
        }
    }
    if order.is_empty() {
        return Err(format!("NO_MODEL:{provider}"));
    }
    recent::reorder(&mut order);
    Ok((provider, key, order))
}

/// Models that were saturated a moment ago (no text within a few seconds)
/// wait at the back of the line for a while; then they are tried first again.
mod recent {
    use super::PENALTY;
    use std::{collections::HashMap, sync::Mutex, time::Instant};

    static SLOW: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);

    pub fn failed(model: &str) {
        let mut slow = SLOW.lock().unwrap_or_else(|e| e.into_inner());
        slow.get_or_insert_with(HashMap::new).insert(model.to_string(), Instant::now() + PENALTY);
    }

    pub fn answered(model: &str) {
        if let Some(slow) = SLOW.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            slow.remove(model);
        }
    }

    /// Keeps the order (chosen or cheapest first) but moves saturated ones last.
    pub fn reorder(order: &mut [String]) {
        let now = Instant::now();
        let slow = SLOW.lock().unwrap_or_else(|e| e.into_inner());
        order.sort_by_key(|m| slow.as_ref().and_then(|s| s.get(m)).is_some_and(|until| *until > now));
    }
}

/// `plan`, probing the provider's models first if that was never done.
async fn planned(app: &AppHandle) -> Result<(String, String, Vec<String>), String> {
    match plan(app) {
        Err(e) if e.starts_with("NO_MODEL:") => {
            // Never checked (a key saved before, a fresh install…): answer now
            // with the cheapest text models of the list (any that does not
            // respond is skipped) and check them all in the background.
            let provider = e.trim_start_matches("NO_MODEL:").to_string();
            let key = key(&provider).ok_or("NO_KEY")?;
            let raw = fetch_models(&provider, &key).await?;
            let order: Vec<String> = catalog(&provider, raw, &[]).into_iter().map(|m| m.id).collect();
            if order.is_empty() {
                return Err("Ningún modelo de texto disponible con esta clave".into());
            }
            let (handle, p) = (app.clone(), provider.clone());
            tauri::async_runtime::spawn(async move {
                let _ = probe_models(handle, p).await;
            });
            Ok((provider, key, order))
        }
        other => other,
    }
}

// ------------------------------------------------------------- streaming

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Msg {
    /// "user" | "assistant"
    pub role: String,
    pub content: String,
}

/// An image sent with a question, already scaled down and base64-encoded.
#[derive(Clone, Debug)]
pub struct Img {
    pub mime: &'static str,
    pub data: String,
}

/// A PNG (or any image the `image` crate reads) ready to send: scaled to
/// `IMAGE_MAX_SIDE` at most and encoded as PNG.
pub fn prepare_image(bytes: &[u8]) -> Option<Img> {
    use base64::Engine;
    let img = image::load_from_memory(bytes).ok()?;
    let img = if img.width().max(img.height()) > IMAGE_MAX_SIDE {
        img.resize(IMAGE_MAX_SIDE, IMAGE_MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(Img { mime: "image/png", data: base64::engine::general_purpose::STANDARD.encode(out.into_inner()) })
}

/// The newest images saved in a nest, ready to send.
fn nest_images(app: &AppHandle, nest_id: i64) -> Vec<Img> {
    let files = saving::with_db(app, |c| store::nest_images(c, nest_id, NEST_IMAGES)).unwrap_or_default();
    let dir = saving::data(app).files.clone();
    files.iter().filter_map(|f| std::fs::read(dir.join(f)).ok()).filter_map(|b| prepare_image(&b)).collect()
}

/// Request body per provider. `fast` asks for the least thinking each model
/// accepts: chat answers don't need deep reasoning and it saves time and money.
#[allow(clippy::too_many_arguments)]
fn request_with(
    provider: &str,
    key: &str,
    model: &str,
    system: &str,
    messages: &[Msg],
    images: &[Img],
    stream: bool,
    max_tokens: Option<u32>,
    fast: bool,
) -> reqwest::RequestBuilder {
    let c = client();
    // Images go with the first question of the conversation.
    let first = if images.is_empty() { None } else { messages.iter().position(|m| m.role == "user") };
    match provider {
        "claude" => {
            let msgs: Vec<Value> = messages
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    if Some(i) != first {
                        return json!({"role": m.role, "content": m.content});
                    }
                    let mut parts: Vec<Value> = images
                        .iter()
                        .map(|im| json!({"type": "image", "source": {"type": "base64", "media_type": im.mime, "data": im.data}}))
                        .collect();
                    parts.push(json!({"type": "text", "text": m.content}));
                    json!({"role": m.role, "content": parts})
                })
                .collect();
            let mut body = json!({
                "model": model,
                "max_tokens": max_tokens.unwrap_or(4096),
                "system": system,
                "messages": msgs,
                "stream": stream,
            });
            if fast {
                body["output_config"] = json!({"effort": "low"});
            }
            c.post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .json(&body)
        }
        "openai" => {
            let mut msgs = vec![json!({"role": "system", "content": system})];
            msgs.extend(messages.iter().enumerate().map(|(i, m)| {
                if Some(i) != first {
                    return json!({"role": m.role, "content": m.content});
                }
                let mut parts = vec![json!({"type": "text", "text": m.content})];
                parts.extend(images.iter().map(|im| {
                    json!({"type": "image_url", "image_url": {"url": format!("data:{};base64,{}", im.mime, im.data)}})
                }));
                json!({"role": m.role, "content": parts})
            }));
            let mut body = json!({"model": model, "messages": msgs, "stream": stream});
            if let Some(n) = max_tokens {
                body["max_completion_tokens"] = json!(n.max(256));
            }
            let reasoning = model.starts_with('o') || model.starts_with("gpt-5");
            if fast && reasoning {
                body["reasoning_effort"] = json!("low");
            }
            c.post("https://api.openai.com/v1/chat/completions").bearer_auth(key).json(&body)
        }
        _ => {
            let contents: Vec<Value> = messages
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let mut parts: Vec<Value> = if Some(i) == first {
                        images.iter().map(|im| json!({"inlineData": {"mimeType": im.mime, "data": im.data}})).collect()
                    } else {
                        Vec::new()
                    };
                    parts.push(json!({"text": m.content}));
                    json!({
                        "role": if m.role == "assistant" { "model" } else { "user" },
                        "parts": parts,
                    })
                })
                .collect();
            let mut config = json!({});
            if let Some(n) = max_tokens {
                config["maxOutputTokens"] = json!(n);
            }
            if fast && !model.starts_with("gemma") {
                config["thinkingConfig"] = if model.starts_with("gemini-2") {
                    json!({"thinkingBudget": 0})
                } else {
                    json!({"thinkingLevel": "minimal"})
                };
            }
            let action = if stream { "streamGenerateContent?alt=sse" } else { "generateContent" };
            c.post(format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:{action}"))
                .header("x-goog-api-key", key)
                .json(&json!({
                    "systemInstruction": {"parts": [{"text": system}]},
                    "contents": contents,
                    "generationConfig": config,
                }))
        }
    }
}

/// Text carried by one SSE `data:` payload, or an error.
fn delta(provider: &str, data: &str) -> Result<Option<String>, String> {
    if data == "[DONE]" {
        return Ok(None);
    }
    let v: Value = serde_json::from_str(data).map_err(|_| "respuesta ilegible".to_string())?;
    if let Some(msg) = v.pointer("/error/message").and_then(Value::as_str) {
        return Err(msg.into());
    }
    Ok(match provider {
        "claude" => (v["type"] == "content_block_delta")
            .then(|| v.pointer("/delta/text").and_then(Value::as_str).map(String::from))
            .flatten(),
        "openai" => v.pointer("/choices/0/delta/content").and_then(Value::as_str).map(String::from),
        _ => {
            let parts = v.pointer("/candidates/0/content/parts").and_then(Value::as_array);
            parts.map(|ps| {
                ps.iter()
                    .filter(|p| p["thought"] != true)
                    .filter_map(|p| p["text"].as_str())
                    .collect::<String>()
            })
        }
    })
}

/// (retryable, message)
type StreamError = (bool, String);

/// One streamed answer from one model. Gives up (retryably) if no text has
/// arrived within `first_text`.
#[allow(clippy::too_many_arguments)]
async fn stream_once(
    provider: &str,
    key: &str,
    model: &str,
    system: &str,
    messages: &[Msg],
    images: &[Img],
    on_text: &mut (dyn FnMut(&str) + Send),
    first_text: Duration,
) -> Result<(), StreamError> {
    let deadline = Instant::now() + first_text;
    let mut fast = true;
    let resp = loop {
        let send = request_with(provider, key, model, system, messages, images, true, None, fast).send();
        let resp = tokio::time::timeout_at(deadline.into(), send)
            .await
            .map_err(|_| (true, "tarda demasiado".to_string()))?
            .map_err(|e| (true, format!("Sin conexión: {e}")))?;
        if resp.status().is_success() {
            break resp;
        }
        let (code, msg) = error_text(resp).await;
        // A model that rejects the "think less" option: ask again without it.
        let lower = msg.to_lowercase();
        if fast && code == 400 && ["thinking", "effort", "reasoning"].iter().any(|w| lower.contains(w)) {
            fast = false;
            continue;
        }
        return Err((retryable(code, &msg), msg));
    };
    let mut body = resp.bytes_stream();
    let mut buf = String::new();
    let mut started = false;
    loop {
        let next = if started {
            tokio::time::timeout(Duration::from_secs(90), body.next()).await
        } else {
            tokio::time::timeout_at(deadline.into(), body.next()).await
        };
        let chunk = match next {
            Err(_) => return Err((!started, "tarda demasiado".into())),
            Ok(None) => break,
            Ok(Some(c)) => c.map_err(|e| (!started, e.to_string()))?,
        };
        buf.push_str(&String::from_utf8_lossy(&chunk).replace('\r', ""));
        while let Some(end) = buf.find("\n\n") {
            let event: String = buf.drain(..end + 2).collect();
            let data: String = event
                .lines()
                .filter_map(|l| l.strip_prefix("data:"))
                .map(str::trim_start)
                .collect::<Vec<_>>()
                .join("\n");
            if data.is_empty() {
                continue;
            }
            match delta(provider, &data) {
                Ok(Some(text)) if !text.is_empty() => {
                    started = true;
                    on_text(&text);
                }
                Ok(_) => {}
                Err(e) => return Err((!started, e)),
            }
        }
    }
    if started {
        Ok(())
    } else {
        Err((true, "respuesta vacía".into()))
    }
}

/// Streams from the planned models in order, moving on when one is
/// overloaded, out of quota for now or too slow to start. `on_switch` is told
/// about each change. Returns the model that answered.
#[allow(clippy::too_many_arguments)]
async fn stream_planned(
    order: &[String],
    provider: &str,
    key: &str,
    system: &str,
    messages: &[Msg],
    images: &[Img],
    on_text: &mut (dyn FnMut(&str) + Send),
    on_switch: &mut (dyn FnMut(&str, &str) + Send),
) -> Result<String, String> {
    let mut last = String::from("sin modelos");
    let tries = order.len().min(6);
    let words = crate::store::word_count(system) + messages.iter().map(|m| crate::store::word_count(&m.content)).sum::<usize>();
    // Reading an image takes a model a moment more before its first word.
    let first_wait = first_text_wait(words) + IMAGE_EXTRA * images.len().min(2) as u32;
    for (i, model) in order.iter().take(tries).enumerate() {
        let wait = if i + 1 == tries { LAST_TRY_TIMEOUT } else { first_wait };
        match stream_once(provider, key, model, system, messages, images, on_text, wait).await {
            Ok(()) => {
                recent::answered(model);
                return Ok(model.clone());
            }
            // Saturated, or not offered to this key: try the next one.
            // With images, a text-only model refuses them: the next one may see.
            Err((retry, msg)) if (retry || not_offered(&msg) || !images.is_empty()) && i + 1 < tries => {
                recent::failed(model);
                on_switch(model, &order[i + 1]);
                last = msg;
            }
            Err((_, msg)) => return Err(msg),
        }
    }
    Err(last)
}

/// How long to wait for the first words before trying the next model:
/// 3 s, plus 1 s per ~10 000 words sent, at most 8 s.
fn first_text_wait(words: usize) -> Duration {
    (FIRST_TEXT_TIMEOUT + Duration::from_secs((words / WORDS_PER_EXTRA_SECOND) as u64)).min(FIRST_TEXT_MAX)
}

/// One-shot answer (nest suggestion, nest summary).
pub async fn complete(app: &AppHandle, prompt: &str) -> Result<String, String> {
    let (provider, key, order) = planned(app).await?;
    let mut out = String::new();
    let msgs = [Msg { role: "user".into(), content: prompt.into() }];
    stream_planned(
        &order,
        &provider,
        &key,
        "Responde de forma directa, sin preámbulos.",
        &msgs,
        &[],
        &mut |t| out.push_str(t),
        &mut |_, _| {},
    )
    .await?;
    Ok(out)
}

// ------------------------------------------------------------------ chat

#[derive(Default)]
pub struct Chats {
    running: HashMap<u64, tauri::async_runtime::JoinHandle<()>>,
}

pub fn init(app: &AppHandle) {
    app.manage(Mutex::new(Chats::default()));
}

#[derive(Deserialize)]
pub struct ChatRequest {
    id: u64,
    messages: Vec<Msg>,
    selection: Option<String>,
    nest_id: Option<i64>,
    /// Send the image the bubble was opened with (a capture, a saved image).
    #[serde(default)]
    image: bool,
}

#[derive(Serialize)]
pub struct ChatMeta {
    provider: String,
    model: String,
    /// The nest did not fit and was trimmed to the most relevant acorns.
    truncated: bool,
}

#[derive(Serialize, Clone)]
struct ChatEvent {
    id: u64,
    /// "delta" | "done" | "error" | "switch"
    kind: &'static str,
    text: String,
}

fn system_prompt(en: bool) -> String {
    if en {
        "You are Donguri, a pixel-art squirrel living on the edge of the user's screen. You help with what they read and save. Answer clearly and briefly, in the user's language, using simple Markdown (bold, lists, code). Go straight to the answer: no greetings, no sentences about yourself, no role-play.".into()
    } else {
        "Eres Donguri, una ardilla pixel-art que vive en el borde de la pantalla del usuario y le ayuda con lo que lee y guarda. Responde de forma clara y breve, en el idioma del usuario, con Markdown sencillo (negritas, listas, código). Ve directo a la respuesta: sin saludos, sin frases sobre ti y sin hacer el papel de ardilla.".into()
    }
}

/// Builds the context block (selection + nest) the question carries.
pub fn context(app: &AppHandle, selection: Option<&str>, nest_id: Option<i64>, query: Option<&str>) -> (String, bool) {
    let mut out = String::new();
    let mut truncated = false;
    if let Some(sel) = selection.filter(|s| !s.trim().is_empty()) {
        out.push_str(&format!("\n\n## Texto seleccionado por el usuario\n\n{}\n", sel.trim()));
    }
    if let Some(id) = nest_id
        && let Ok((nest, text)) = saving::with_db(app, |c| {
            Ok((store::nest(c, id)?, store::nest_text(c, id, NEST_BUDGET, query)?))
        })
        && !text.text.is_empty()
    {
        truncated = text.truncated;
        let name = nest.map(|n| n.name).unwrap_or_default();
        out.push_str(&format!("\n\n## Nido «{name}» (notas guardadas por el usuario)\n\n{}\n", text.text));
    }
    (out, truncated)
}

#[tauri::command]
pub async fn chat_start(app: AppHandle, req: ChatRequest) -> Result<ChatMeta, String> {
    let (provider, key, order) = planned(&app).await?;
    let en = presence::state(&app).settings.en();
    let last_question = req.messages.iter().rev().find(|m| m.role == "user").map(|m| m.content.clone());
    let (ctx, truncated) = context(&app, req.selection.as_deref(), req.nest_id, last_question.as_deref());
    let system = format!("{}{}", system_prompt(en), ctx);
    let mut images: Vec<Img> = Vec::new();
    if req.image
        && let Some(img) = crate::actions::ask_image(&app).and_then(|png| prepare_image(&png))
    {
        images.push(img);
    }
    if let Some(id) = req.nest_id {
        // The image asked about may be in the nest too: send it once.
        for img in nest_images(&app, id) {
            if !images.iter().any(|i| i.data == img.data) {
                images.push(img);
            }
        }
    }

    presence::animate(&app, "think");
    let id = req.id;
    let handle = app.clone();
    let p = provider.clone();
    let first = order[0].clone();
    let task = tauri::async_runtime::spawn(async move {
        let emit = |kind: &'static str, text: String| {
            let _ = handle.emit_to(BUBBLE, "chat", ChatEvent { id, kind, text });
        };
        let mut started = false;
        let result = stream_planned(
            &order,
            &p,
            &key,
            &system,
            &req.messages,
            &images,
            &mut |t| {
                if !started {
                    started = true;
                    presence::animate(&handle, "talk");
                }
                emit("delta", t.to_string());
            },
            &mut |from, to| emit("switch", format!("{from} → {to}")),
        )
        .await;
        match result {
            Ok(model) => {
                presence::animate(&handle, "idle");
                emit("done", model);
                crate::actions::answer_finished(&handle);
            }
            Err(e) => {
                presence::animate(&handle, "error");
                emit("error", e);
            }
        }
        if let Ok(mut chats) = handle.state::<Mutex<Chats>>().lock() {
            chats.running.remove(&id);
        }
    });
    if let Ok(mut chats) = app.state::<Mutex<Chats>>().lock() {
        chats.running.insert(id, task);
    }
    Ok(ChatMeta { provider, model: first, truncated })
}

#[tauri::command]
pub fn chat_stop(app: AppHandle, id: u64) {
    if let Some(task) = app.state::<Mutex<Chats>>().lock().ok().and_then(|mut c| c.running.remove(&id)) {
        task.abort();
        presence::animate(&app, "idle");
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn first_text_wait_grows_with_the_request() {
        use std::time::Duration;
        assert_eq!(super::first_text_wait(200), Duration::from_secs(3));
        assert_eq!(super::first_text_wait(25_000), Duration::from_secs(5));
        assert_eq!(super::first_text_wait(500_000), Duration::from_secs(8));
    }

    use super::{cost_rank, delta, is_text_chat, retryable};

    #[test]
    fn claude_text_delta() {
        let d = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hola"}}"#;
        assert_eq!(delta("claude", d).unwrap().as_deref(), Some("Hola"));
        let start = r#"{"type":"message_start","message":{}}"#;
        assert_eq!(delta("claude", start).unwrap(), None);
    }

    #[test]
    fn openai_delta_and_done() {
        let d = r#"{"choices":[{"index":0,"delta":{"content":"Hi"}}]}"#;
        assert_eq!(delta("openai", d).unwrap().as_deref(), Some("Hi"));
        assert_eq!(delta("openai", "[DONE]").unwrap(), None);
    }

    #[test]
    fn gemini_parts_are_joined_without_thoughts() {
        let d = r#"{"candidates":[{"content":{"parts":[{"text":"pensando","thought":true},{"text":"a"},{"text":"b"}],"role":"model"}}]}"#;
        assert_eq!(delta("gemini", d).unwrap().as_deref(), Some("ab"));
    }

    #[test]
    fn provider_errors_surface() {
        let d = r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;
        assert_eq!(delta("claude", d).unwrap_err(), "Overloaded");
    }

    #[test]
    fn only_text_chat_models_are_offered() {
        for id in ["gemini-2.5-flash-image", "gemini-3.8-flash-tts", "lyria-3.5", "gemini-3.5-transcribe", "gemini-omni-1.2-flash", "nano-banana-pro-preview", "gemini-flash-latest"] {
            assert!(!is_text_chat("gemini", id), "{id}");
        }
        for id in ["gemini-3.5-flash-lite", "gemini-3.6-flash", "gemma-4-31b-it"] {
            assert!(is_text_chat("gemini", id), "{id}");
        }
        assert!(!is_text_chat("openai", "gpt-4o-mini-tts"));
        assert!(!is_text_chat("openai", "dall-e-3"));
        assert!(is_text_chat("openai", "gpt-4.1-mini"));
    }

    #[test]
    fn cheaper_models_sort_first() {
        assert!(cost_rank("gemini", "gemini-3.5-flash-lite") < cost_rank("gemini", "gemini-3.6-flash"));
        assert!(cost_rank("gemini", "gemini-3.6-flash") < cost_rank("gemini", "gemini-3.1-pro-preview"));
        assert!(cost_rank("claude", "claude-haiku-5-5") < cost_rank("claude", "claude-sonnet-5-5"));
        assert!(cost_rank("claude", "claude-sonnet-5-5") < cost_rank("claude", "claude-opus-5-5"));
        assert!(cost_rank("openai", "gpt-5-nano") < cost_rank("openai", "gpt-5-mini"));
    }

    #[test]
    fn quota_and_retired_models_are_not_retried() {
        assert!(!retryable(429, "You exceeded your current quota ... limit: 0, model: gemini-4-argon"));
        assert!(retryable(429, "You exceeded your current quota, please check your plan"));
        assert!(!retryable(404, "This model is no longer available to new users"));
        assert!(retryable(503, "This model is currently experiencing high demand"));
        assert!(retryable(429, "Rate limit, slow down"));
    }
}
