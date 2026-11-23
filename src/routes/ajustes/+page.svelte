<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { app, loadConfig, t } from "$lib/app.svelte";
  import Icon, { type IconName } from "$lib/Icon.svelte";
  import Window from "$lib/Window.svelte";
  import { ACCESSORIES, PALETTES, renderStill, type Accessory, type Palette } from "$lib/sprite";
  import type { Key } from "$lib/i18n";
  import { HELP } from "$lib/help";

  type Tab = "ardilla" | "comportamiento" | "atajos" | "ia" | "integraciones" | "privacidad" | "idioma" | "ayuda";
  const TABS: { id: Tab; icon: IconName; label: Key }[] = [
    { id: "ardilla", icon: "squirrel", label: "tab_squirrel" },
    { id: "comportamiento", icon: "gear", label: "tab_behaviour" },
    { id: "atajos", icon: "keyboard", label: "tab_shortcuts" },
    { id: "ia", icon: "key", label: "tab_ai" },
    { id: "integraciones", icon: "plug", label: "tab_integrations" },
    { id: "privacidad", icon: "lock", label: "tab_privacy" },
    { id: "idioma", icon: "globe", label: "tab_language" },
    { id: "ayuda", icon: "ask", label: "tab_help" },
  ];

  interface Settings {
    scale: number;
    mode: string;
    palette: Palette;
    accessory: Accessory;
    lang: string;
    hide_fullscreen: boolean;
    exclude_from_capture: boolean;
    fade_when_idle: boolean;
    selection_hint: boolean;
    ai_suggest: boolean;
    handoff: "claude" | "chatgpt";
    private_mode: boolean;
    blacklist: string[];
    shortcuts: Record<"toggle" | "save" | "capture" | "quick" | "capture_ask" | "clipboard", string>;
    clip_history: boolean;
    update_check: boolean;
  }
  interface Provider {
    id: "gemini" | "claude" | "openai";
    configured: boolean;
    model: string | null;
    default: boolean;
    manual: boolean;
  }
  interface Model {
    id: string;
    name: string;
    tier: number;
    price: string | null;
    status: "ok" | "busy" | "untested";
    ms: number | null;
  }

  const PROVIDERS = {
    gemini: { name: "Gemini", url: "https://aistudio.google.com/apikey", free: true },
    claude: { name: "Claude", url: "https://platform.claude.com/settings/keys", free: false },
    openai: { name: "OpenAI", url: "https://platform.openai.com/api-keys", free: false },
  } as const;

  let tab = $state<Tab>((new URLSearchParams(location.search).get("tab") as Tab) || "ardilla");
  let s = $state<Settings | null>(null);
  let providers = $state<Provider[]>([]);
  let keyInput = $state<Record<string, string>>({});
  let keyState = $state<Record<string, { busy?: boolean; probing?: boolean; ok?: string; error?: string }>>({});
  let models = $state<Record<string, Model[]>>({});
  let recording = $state<string | null>(null);
  let failed = $state<string[]>([]);
  let integ = $state<{ desktop: boolean; desktop_path: string | null; code: boolean; code_cli: boolean; mcp_found: boolean } | null>(null);
  let integError = $state<string | null>(null);
  let dataFolder = $state("");
  let wipe = $state(0);
  let toast = $state<string | null>(null);
  let autostart = $state(false);

  async function refresh() {
    s = await invoke<Settings>("get_settings");
  }

  let update = $state<{ current: string; available: string | null } | null>(null);
  let checking = $state(false);
  async function checkUpdates() {
    checking = true;
    update = await invoke("update_check_now");
    checking = false;
  }

  async function set(key: string, value: unknown) {
    const result = await invoke<string[]>("set_setting", { key, value });
    if (key.startsWith("shortcut.")) failed = result;
    await refresh();
  }

  function flash(text: string) {
    toast = text;
    setTimeout(() => (toast = null), 2000);
  }

  // ---- AI
  async function loadProviders() {
    providers = await invoke<Provider[]>("ai_status");
    for (const p of providers) {
      if (p.configured && !models[p.id]) {
        invoke<Model[]>("list_models", { provider: p.id })
          .then((m) => {
            models[p.id] = m;
            // Never checked with this key yet: check now (tiny requests).
            if (m.every((x) => x.status === "untested")) probe(p.id);
          })
          .catch((e) => (keyState[p.id] = { error: String(e) }));
      }
    }
  }

  /** Sends a minimal request to every text model, keeps the ones that work. */
  async function probe(id: string) {
    keyState[id] = { ...keyState[id], probing: true, error: undefined };
    try {
      const list = await invoke<Model[]>("probe_models", { provider: id });
      models[id] = list;
      const working = list.filter((m) => m.status === "ok").length;
      keyState[id] = { ok: t("models_checked", { n: working }) };
      await loadProviders();
    } catch (e) {
      keyState[id] = { error: String(e) };
    }
  }

  async function testKey(id: string) {
    const key = keyInput[id]?.trim();
    if (!key) return;
    keyState[id] = { busy: true };
    try {
      await invoke<number>("test_key", { provider: id, key });
      await invoke("set_key", { provider: id, key });
      keyInput[id] = "";
      await loadProviders();
      await probe(id);
    } catch (e) {
      keyState[id] = { error: String(e) };
    }
  }

  async function chooseModel(provider: string, value: string) {
    if (value === "__auto__") await invoke("set_model_auto", { provider });
    else await invoke("set_model", { provider, model: value });
    await loadProviders();
  }

  const COST = ["", "€", "€€", "€€€", "€€€€"];
  async function removeKey(id: string) {
    await invoke("set_key", { provider: id, key: "" });
    delete models[id];
    keyState[id] = {};
    await loadProviders();
  }

  // ---- shortcuts
  const CODE_NAMES: Record<string, string> = { Space: "Space", Enter: "Enter", Backquote: "`", Minus: "-", Equal: "=", Comma: ",", Period: "." };

  function onRecordKey(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      recording = null;
      return;
    }
    let key: string | null = null;
    if (e.code.startsWith("Key")) key = e.code.slice(3);
    else if (e.code.startsWith("Digit")) key = e.code.slice(5);
    else if (/^F\d+$/.test(e.code)) key = e.code;
    else if (e.code.startsWith("Arrow")) key = e.code.slice(5);
    else key = CODE_NAMES[e.code] ?? null;
    const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(Boolean);
    if (!key || !mods.length) return; // wait for a full combination
    const which = recording;
    recording = null;
    set(`shortcut.${which}`, [...mods, key].join("+"));
  }

  // ---- integrations
  async function loadIntegrations() {
    integ = await invoke("integration_status");
  }

  async function integration(target: "desktop" | "code", connect: boolean) {
    integError = null;
    try {
      integ = await invoke("integration_set", { target, connect });
    } catch (e) {
      integError = String(e);
    }
  }

  async function deleteAll() {
    if (wipe < 2) {
      wipe++;
      return;
    }
    await invoke("delete_all_data");
    wipe = 0;
    flash(t("deleted_all"));
  }

  $effect(() => {
    if (tab === "ia") loadProviders();
    if (tab === "integraciones") loadIntegrations();
    if (tab === "privacidad") invoke<string>("data_folder").then((d) => (dataFolder = d));
    if (tab === "ayuda") invoke<{ current: string; available: string | null }>("update_info").then((u) => (update = u));
    if (tab === "comportamiento") invoke<boolean>("autostart_get").then((v) => (autostart = v));
  });

  onMount(() => {
    loadConfig();
    refresh();
    const off = listen<Tab>("settings-tab", (e) => (tab = e.payload));
    const offCfg = listen("config", refresh);
    return () => {
      off.then((f) => f());
      offCfg.then((f) => f());
    };
  });

  const look = $derived(s ? { palette: s.palette, accessory: s.accessory, masked: s.private_mode } : null);
</script>

<svelte:window onkeydowncapture={onRecordKey} />

<Window title={t("settings_title")} icon="gear">
  <div class="layout">
    <nav class="tabs">
      {#each TABS as item}
        <button class="tab" class:on={tab === item.id} onclick={() => (tab = item.id)}>
          <Icon name={item.icon} size={15} />{t(item.label)}
        </button>
      {/each}
    </nav>

    <main class="scroll">
      {#if s}
        {#if tab === "ardilla"}
          <section class="hero px-sunk">
            {#if look}
              <img class="preview" src={renderStill(look)} alt="Donguri" style:width="{16 * s.scale}px" />
            {/if}
          </section>

          <h4 class="title">{t("colour")}</h4>
          <div class="grid">
            {#each PALETTES as p}
              <button class="tile" class:on={s.palette === p} onclick={() => set("palette", p)}>
                <img src={renderStill({ palette: p, accessory: "ninguno", masked: false })} alt="" />
                <span>{t(`pal_${p}` as Key)}</span>
              </button>
            {/each}
          </div>

          <h4 class="title">{t("accessory")}</h4>
          <div class="grid">
            {#each ACCESSORIES as a}
              <button class="tile" class:on={s.accessory === a} onclick={() => set("accessory", a)}>
                <img src={renderStill({ palette: s.palette, accessory: a, masked: false })} alt="" />
                <span>{t(`acc_${a}` as Key)}</span>
              </button>
            {/each}
          </div>

          <h4 class="title">{t("size")}</h4>
          <div class="seg">
            {#each [[4, "small"], [6, "medium"], [8, "large"]] as [v, label]}
              <button class="btn" class:primary={s.scale === v} onclick={() => set("scale", v)}>{t(label as Key)} · {16 * (v as number)} px</button>
            {/each}
          </div>

          <h4 class="title">{t("mode")}</h4>
          {#each ["quieta", "viva", "escondida"] as m}
            <label class="radio">
              <input type="radio" name="mode" checked={s.mode === m} onchange={() => set("mode", m)} />
              <span><strong>{t(`mode_${m}` as Key)}</strong> <span class="muted">{t(`mode_${m}_d` as Key)}</span></span>
            </label>
          {/each}
        {:else if tab === "comportamiento"}
          <h4 class="title">{t("auto_hide")}</h4>
          <label class="check"><input type="checkbox" checked={s.hide_fullscreen} onchange={(e) => set("fullscreen", e.currentTarget.checked)} />{t("opt_fullscreen")}</label>
          <label class="check"><input type="checkbox" checked={s.exclude_from_capture} onchange={(e) => set("capture", e.currentTarget.checked)} />{t("opt_capture")}</label>
          <label class="check"><input type="checkbox" checked={s.fade_when_idle} onchange={(e) => set("fade", e.currentTarget.checked)} />{t("opt_fade")}</label>

          <h4 class="title">{t("sel_title")}</h4>
          <label class="check">
            <input type="checkbox" checked={s.selection_hint} onchange={(e) => set("selection_hint", e.currentTarget.checked)} />
            <span>{t("sel_hint_d")}</span>
          </label>

          <h4 class="title">{t("autostart")}</h4>
          <label class="check">
            <input
              type="checkbox"
              checked={autostart}
              onchange={async (e) => (autostart = await invoke<boolean>("autostart_set", { on: e.currentTarget.checked }).catch(() => autostart))}
            />
            <span>{t("autostart_d")}</span>
          </label>

          <h4 class="title">{t("tray_hint")}</h4>
          <p class="muted small">{t("tray_hint_d")}</p>

          <h4 class="title">{t("blacklist")}</h4>
          {#if s.blacklist.length}
            <ul class="plain">
              {#each s.blacklist as exe}
                <li class="row">
                  <span>{exe.replace(/\.exe$/, "")}</span>
                  <button class="btn ghost" title={t("delete")} onclick={() => set("unblock", exe)}><Icon name="close" size={12} /></button>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="muted">{t("blacklist_empty")}</p>
          {/if}
        {:else if tab === "atajos"}
          {#each [["toggle", "sc_toggle"], ["save", "sc_save"], ["capture", "sc_capture"], ["capture_ask", "sc_capture_ask"], ["quick", "sc_quick"], ["clipboard", "sc_clipboard"]] as [id, label]}
            {@const key = id as keyof Settings["shortcuts"]}
            <div class="row sc">
              <span class="grow">{t(label as Key)}</span>
              <button class="btn kbd" class:primary={recording === id} onclick={() => (recording = id)}>
                {recording === id ? t("sc_press") : s.shortcuts[key]}
              </button>
              {#if failed.includes(id)}<span class="warn small"><Icon name="warn" size={12} />{t("sc_failed")}</span>{/if}
            </div>
          {/each}
          <p class="muted small">{t("sc_hover")}</p>
          <button class="btn" onclick={async () => { failed = await invoke("reset_shortcuts"); refresh(); }}>{t("sc_reset")}</button>
        {:else if tab === "ia"}
          <p>{t("ai_intro")}</p>
          <p class="muted small">{t("ai_what")}</p>
          <section class="card px-box">
            <strong>{t("handoff_title")}</strong>
            <p class="muted small">{t("handoff_d")}</p>
            <div class="seg">
              <button class="btn" class:primary={s.handoff === "claude"} onclick={() => set("handoff", "claude")}>Claude</button>
              <button class="btn" class:primary={s.handoff === "chatgpt"} onclick={() => set("handoff", "chatgpt")}>ChatGPT</button>
            </div>
          </section>
          {#each providers as p}
            {@const info = PROVIDERS[p.id]}
            {@const st = keyState[p.id] ?? {}}
            <section class="card px-box">
              <div class="row">
                <strong class="grow">{info.name}</strong>
                {#if info.free}<span class="chip">{t("free_option")}</span>{/if}
                {#if p.configured}
                  {#if p.default}<span class="chip ok">{t("is_default")}</span>{:else}<button class="btn" onclick={async () => { await invoke("set_default_provider", { provider: p.id }); loadProviders(); }}>{t("default_provider")}</button>{/if}
                {/if}
              </div>
              <p class="muted small">{info.free ? t("gemini_note") : t("paid_note")}</p>
              {#if p.configured}
                <div class="row">
                  <span class="ok small"><Icon name="check" size={12} />{t("key_saved")}</span>
                  <span class="grow"></span>
                  <button class="btn ghost small" onclick={() => removeKey(p.id)}>{t("remove_key")}</button>
                </div>
                {#if st.probing}
                  <p class="muted small">{t("models_checking")}</p>
                {:else if models[p.id]}
                  {@const current = models[p.id].find((m) => m.id === p.model)}
                  <div class="stack">
                    <span class="small">{t("model")}</span>
                    <div class="models scroll" role="radiogroup" aria-label={t("model")}>
                      <button class="mrow" class:sel={!p.manual} role="radio" aria-checked={!p.manual} onclick={() => chooseModel(p.id, "__auto__")}>
                        <span class="mname">{t("model_auto")}</span>
                        <span class="mmeta">{!p.manual && current ? current.name : ""}</span>
                      </button>
                      {#each models[p.id] as m}
                        <button class="mrow" class:sel={p.manual && p.model === m.id} role="radio" aria-checked={p.manual && p.model === m.id} onclick={() => chooseModel(p.id, m.id)}>
                          <span class="mname">{m.name}</span>
                          <span class="mmeta">
                            <span class="cost" title={t("cost_tier")}>{COST[m.tier]}</span>
                            {#if m.price}<span>{m.price}</span>{/if}
                            {#if m.status === "busy"}<span class="busy">{t("model_busy")}</span>{:else if m.ms != null}<span>{(m.ms / 1000).toFixed(1)} s</span>{/if}
                          </span>
                        </button>
                      {/each}
                    </div>
                  </div>
                  <p class="muted small">{t("model_order_note")}</p>
                  <div class="row">
                    <button class="btn ghost small" onclick={() => probe(p.id)}><Icon name="search" size={12} />{t("check_models")}</button>
                  </div>
                {/if}
              {:else}
                <ol class="steps small">
                  <li>{t("step1")}</li>
                  <li>{t("step2")}</li>
                  <li>{t("step3")}</li>
                </ol>
                <div class="row">
                  <button class="btn" onclick={() => invoke("open_url", { url: info.url })}><Icon name="out" size={13} />{t("get_key")}</button>
                </div>
                <div class="row">
                  <input class="field" type="password" aria-label="{info.name}: {t('paste_key')}" placeholder={t("paste_key")} bind:value={keyInput[p.id]} onkeydown={(e) => e.key === "Enter" && testKey(p.id)} />
                  <button class="btn primary" disabled={st.busy || !keyInput[p.id]} onclick={() => testKey(p.id)}>{st.busy ? t("testing") : t("test_key")}</button>
                </div>
              {/if}
              {#if st.ok}<p class="ok small">{st.ok}</p>{/if}
              {#if st.error}<p class="warn small">{st.error}</p>{/if}
            </section>
          {/each}
          <label class="check">
            <input type="checkbox" checked={s.ai_suggest} onchange={(e) => set("ai_suggest", e.currentTarget.checked)} />
            <span>{t("ai_suggest_d")}</span>
          </label>
        {:else if tab === "integraciones"}
          <p>{t("int_intro")}</p>
          {#if integ && !integ.mcp_found}<p class="warn small"><Icon name="warn" size={12} />{t("mcp_missing")}</p>{/if}
          {#each [["desktop", "int_desktop", "int_desktop_d"], ["code", "int_code", "int_code_d"]] as [id, name, desc]}
            {@const on = integ ? (id === "desktop" ? integ.desktop : integ.code) : false}
            <section class="card px-box">
              <div class="row">
                <strong class="grow">{t(name as Key)}</strong>
                <span class="chip" class:ok={on}>{on ? t("connected") : t("not_connected")}</span>
              </div>
              <p class="muted small">{t(desc as Key)}</p>
              {#if id === "desktop" && integ?.desktop_path}<p class="muted tiny">{integ.desktop_path}</p>{/if}
              <div class="row">
                {#if on}
                  <button class="btn" onclick={() => integration(id as "desktop" | "code", false)}>{t("undo")}</button>
                {:else}
                  <button class="btn primary" disabled={!integ?.mcp_found} onclick={() => integration(id as "desktop" | "code", true)}><Icon name="plug" size={13} />{t("connect")}</button>
                {/if}
              </div>
            </section>
          {/each}
          {#if integError}<p class="warn small">{integError}</p>{/if}
        {:else if tab === "privacidad"}
          <p>{t("privacy_points")}</p>
          <label class="check">
            <input type="checkbox" checked={s.private_mode} onchange={(e) => set("private", e.currentTarget.checked)} />
            <span><strong>{t("private_mode")}</strong> <span class="muted">{t("private_d")}</span></span>
          </label>
          <label class="check">
            <input type="checkbox" checked={s.clip_history} onchange={(e) => set("clip_history", e.currentTarget.checked)} />
            <span>{t("clip_history_d")}</span>
          </label>
          <label class="check">
            <input type="checkbox" checked={s.update_check} onchange={(e) => set("update_check", e.currentTarget.checked)} />
            <span><strong>{t("setup_updates")}</strong> <span class="muted">{t("update_check_d")}</span></span>
          </label>
          <h4 class="title">{t("data_folder")}</h4>
          <p class="muted tiny">{dataFolder}</p>
          <button class="btn" onclick={() => invoke("open_data_folder")}><Icon name="nest" size={13} />{t("open_folder")}</button>
          <h4 class="title">{t("delete_all")}</h4>
          {#if wipe === 1}<p class="warn small">{t("delete_all_q")}</p>{/if}
          {#if wipe === 2}<p class="warn small">{t("delete_all_q2")}</p>{/if}
          <div class="row">
            <button class="btn danger" onclick={deleteAll}><Icon name="trash" size={13} />{wipe ? t("yes_delete") : t("delete_all")}</button>
            {#if wipe}<button class="btn" onclick={() => (wipe = 0)}>{t("cancel")}</button>{/if}
          </div>
        {:else if tab === "ayuda"}
          {@const vars = { save: s.shortcuts.save, capture: s.shortcuts.capture, quick: s.shortcuts.quick, toggle: s.shortcuts.toggle, capture_ask: s.shortcuts.capture_ask, clipboard: s.shortcuts.clipboard }}
          {#each HELP[(app.config?.lang ?? "es") as "es" | "en"] as topic}
            <section class="help">
              <h4 class="title"><Icon name={topic.icon} size={15} />{topic.title}</h4>
              <ul>
                {#each topic.points as point}
                  <li>{point.replace(/\{(\w+)\}/g, (_, k) => vars[k as keyof typeof vars] ?? k)}</li>
                {/each}
              </ul>
            </section>
          {/each}
          <button class="btn" onclick={() => invoke("open_window", { name: "bienvenida" })}><Icon name="acorn" size={13} />{t("setup_again")}</button>
          <h4 class="title">{t("version_t")}</h4>
          <div class="row">
            <span class="grow">Donguri {update?.current ?? ""}{update?.available ? ` · ${t("update_new", { v: update.available })}` : ""}</span>
            {#if update?.available}
              <button class="btn primary" onclick={() => invoke("open_update")}><Icon name="down" size={13} />{t("update_get")}</button>
            {:else}
              <button class="btn" disabled={checking} onclick={checkUpdates}><Icon name="restart" size={13} />{t("update_check")}</button>
            {/if}
          </div>
        {:else if tab === "idioma"}
          <h4 class="title">{t("language")}</h4>
          {#each [["es", "Español"], ["en", "English"]] as [code, name]}
            <label class="radio">
              <input type="radio" name="lang" checked={(app.config?.lang ?? s.lang) === code} onchange={() => set("lang", code)} />
              <span>{name}</span>
            </label>
          {/each}
        {/if}
      {/if}
    </main>
  </div>
  {#if toast}<div class="toast px-box">{toast}</div>{/if}
</Window>

<style>
  .layout {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 172px 1fr;
  }

  .tabs {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 6px;
    background: var(--sunk);
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 8px;
    white-space: nowrap;
    border: 0;
    background: transparent;
    cursor: pointer;
    text-align: left;
  }

  .tab:hover {
    background: var(--panel);
  }

  .tab.on {
    background: var(--accent);
    color: var(--accent-ink);
  }

  main {
    padding: 12px 16px 20px;
    min-height: 0;
  }

  main p {
    margin: 0 0 8px;
  }

  .title {
    margin: 14px 0 6px;
  }

  .hero {
    display: grid;
    place-items: center;
    padding: 14px;
    min-height: 120px;
  }

  .preview {
    image-rendering: pixelated;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(78px, 1fr));
    gap: 6px;
  }

  .tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    padding: 6px 2px;
    border: 0;
    background: var(--panel);
    cursor: pointer;
    font-size: 11px;
    box-shadow: 0 0 0 2px var(--sunk);
  }

  .tile img {
    width: 48px;
    height: 48px;
    image-rendering: pixelated;
  }

  .tile.on {
    box-shadow: 0 0 0 3px var(--accent);
  }

  .seg {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .radio,
  .check {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    margin: 6px 0;
    cursor: pointer;
  }

  input[type="radio"],
  input[type="checkbox"] {
    accent-color: var(--accent);
    margin-top: 3px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 4px 0;
  }

  .grow {
    flex: 1;
  }

  .sc {
    padding: 4px 0;
    border-bottom: 2px dashed var(--sunk);
  }

  .kbd {
    min-width: 130px;
    justify-content: center;
    font-family: var(--title);
    font-size: 11px;
  }

  .card {
    padding: 10px 12px;
    margin: 10px 3px 12px;
  }

  .models {
    max-height: 190px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 2px;
    background: var(--sunk);
  }

  .mrow {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 6px;
    border: 0;
    background: transparent;
    text-align: left;
    cursor: pointer;
    font-size: 12px;
  }

  .mrow:hover {
    background: var(--panel);
  }

  .mrow.sel {
    background: var(--accent);
    color: var(--accent-ink);
  }

  .mname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mmeta {
    display: flex;
    gap: 6px;
    white-space: nowrap;
    opacity: 0.85;
  }

  .cost {
    font-family: var(--title);
    font-size: 10px;
  }

  .busy {
    color: var(--warn);
  }

  .mrow.sel .busy {
    color: inherit;
  }

  .stack {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 6px 0 4px;
  }

  .steps {
    margin: 4px 0 8px;
    padding-left: 18px;
  }

  .plain {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .small {
    font-size: 12px;
  }

  .tiny {
    font-size: 11px;
    word-break: break-all;
  }

  .ok {
    color: var(--ok);
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .chip.ok {
    background: var(--ok);
    color: #fff;
  }

  .warn {
    color: var(--warn);
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .help h4 {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .help ul {
    margin: 0 0 6px;
    padding-left: 18px;
    font-size: 13px;
  }

  .help li {
    margin: 3px 0;
  }

  .toast {
    position: fixed;
    left: 50%;
    bottom: 20px;
    transform: translateX(-50%);
    padding: 4px 10px;
    font-size: 12px;
  }
</style>
