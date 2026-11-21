<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { app, closeSelf, loadConfig, t } from "$lib/app.svelte";
  import Window from "$lib/Window.svelte";
  import Icon from "$lib/Icon.svelte";
  import { ACCESSORIES, PALETTES, renderStill, type Accessory, type Frame, type Palette } from "$lib/sprite";
  import type { Key } from "$lib/i18n";

  // First-run setup: one decision per screen, the recommended choice already
  // picked, every change applied at once (the real squirrel changes too).
  // Everything can be changed later in Ajustes; it can be repeated from Ayuda.
  const STEPS = ["lang", "squirrel", "quick", "privacy", "ai", "startup", "end"] as const;
  type Step = (typeof STEPS)[number];
  const FRAMES: Record<Step, Frame> = {
    lang: "base", squirrel: "alert", quick: "chew1", privacy: "blink", ai: "think2", startup: "leap", end: "success",
  };

  interface Settings {
    mode: "quieta" | "viva" | "escondida";
    scale: number;
    palette: Palette;
    accessory: Accessory;
    selection_hint: boolean;
    exclude_from_capture: boolean;
    hide_fullscreen: boolean;
    handoff: "claude" | "chatgpt";
    ai_suggest: boolean;
  }

  const PROVIDERS = [
    { id: "gemini", name: "Gemini", url: "https://aistudio.google.com/apikey", free: true },
    { id: "claude", name: "Claude", url: "https://platform.claude.com/settings/keys", free: false },
    { id: "openai", name: "OpenAI", url: "https://platform.openai.com/api-keys", free: false },
  ] as const;

  let i = $state(0);
  const step = $derived<Step>(STEPS[i]);
  let s = $state<Settings | null>(null);
  let autostart = $state(true); // recommended: she is meant to always be there
  let aiChoice = $state<"none" | "key">("none");
  let provider = $state<(typeof PROVIDERS)[number]["id"]>("gemini");
  let keyText = $state("");
  let keyState = $state<{ busy?: boolean; ok?: string; error?: string }>({});
  let hasKey = $state(false);

  const look = $derived({
    palette: (s?.palette ?? app.config?.palette ?? "rojiza") as Palette,
    accessory: (s?.accessory ?? app.config?.accessory ?? "ninguno") as Accessory,
    masked: false,
  });

  async function set(key: string, value: unknown) {
    if (s) (s as unknown as Record<string, unknown>)[key === "fullscreen" ? "hide_fullscreen" : key === "capture" ? "exclude_from_capture" : key] = value;
    await invoke("set_setting", { key, value });
  }

  async function testKey() {
    const key = keyText.trim();
    if (!key) return;
    keyState = { busy: true };
    try {
      await invoke("test_key", { provider, key });
      await invoke("set_key", { provider, key });
      keyText = "";
      hasKey = true;
      const list = await invoke<{ status: string }[]>("probe_models", { provider });
      keyState = { ok: t("models_checked", { n: list.filter((m) => m.status === "ok").length }) };
    } catch (e) {
      keyState = { error: String(e) };
    }
  }

  async function next() {
    // Only touch the Windows startup list when the choice changes something.
    if (step === "startup" && autostart !== (await invoke<boolean>("autostart_get"))) {
      await invoke("autostart_set", { on: autostart }).catch(() => {});
    }
    if (i < STEPS.length - 1) i++;
  }

  async function finish() {
    await invoke("set_setting", { key: "welcome_done", value: true });
    closeSelf();
  }

  function integrations() {
    invoke("set_setting", { key: "welcome_done", value: true });
    invoke("open_window", { name: "ajustes:integraciones" });
    closeSelf();
  }

  onMount(async () => {
    await loadConfig();
    const all = await invoke<Settings & { welcome_done: boolean }>("get_settings");
    s = all;
    // First run: suggest yes. Repeating the setup: keep what the user has.
    autostart = all.welcome_done ? await invoke<boolean>("autostart_get") : true;
    const status = await invoke<{ configured: boolean }[]>("ai_status");
    hasKey = status.some((p) => p.configured);
    if (hasKey) aiChoice = "key";
  });
</script>

<Window title={t("setup_title")} icon="acorn">
  <div class="page">
    <div class="head">
      <div class="art px-sunk">
        {#key step + look.palette + look.accessory}
          <img src={renderStill(look, FRAMES[step])} alt="" />
        {/key}
      </div>
      <div>
        <p class="kicker">{t("setup_step", { n: i + 1, of: STEPS.length })}</p>
        <h2 class="title">{t(`setup_${step}_t` as Key)}</h2>
        <p class="muted">{t(`setup_${step}_d` as Key)}</p>
      </div>
    </div>

    <div class="body">
      {#if !s}
        <p class="muted">…</p>
      {:else if step === "lang"}
        <div class="choices two">
          <button class="choice px-box" class:on={app.config?.lang !== "en"} onclick={() => set("lang", "es")}>
            <strong>Español</strong>
          </button>
          <button class="choice px-box" class:on={app.config?.lang === "en"} onclick={() => set("lang", "en")}>
            <strong>English</strong>
          </button>
        </div>
      {:else if step === "squirrel"}
        <p class="label">{t("setup_mode")}</p>
        <div class="choices three">
          {#each ["viva", "quieta", "escondida"] as const as m}
            <button class="choice px-box" class:on={s.mode === m} onclick={() => set("mode", m)}>
              <strong>{t(`mode_${m}` as Key)}{m === "viva" ? ` · ${t("recommended")}` : ""}</strong>
              <span class="muted small">{t(`mode_${m}_d` as Key)}</span>
            </button>
          {/each}
        </div>
        <p class="label">{t("size")}</p>
        <div class="row">
          {#each [[4, "small"], [6, "medium"], [8, "large"]] as const as [v, k]}
            <button class="btn" class:primary={s.scale === v} onclick={() => set("scale", v)}>{t(k)}</button>
          {/each}
        </div>
        <p class="label">{t("color")} · {t("accessory")}</p>
        <div class="swatches">
          {#each PALETTES as p}
            <button class="sw" class:on={s.palette === p} title={t(`pal_${p}` as Key)} onclick={() => set("palette", p)}>
              <img src={renderStill({ palette: p, accessory: "ninguno", masked: false })} alt={t(`pal_${p}` as Key)} />
            </button>
          {/each}
          <span class="sep"></span>
          {#each ACCESSORIES as a}
            <button class="sw" class:on={s.accessory === a} title={t(`acc_${a}` as Key)} onclick={() => set("accessory", a)}>
              <img src={renderStill({ palette: s.palette, accessory: a, masked: false })} alt={t(`acc_${a}` as Key)} />
            </button>
          {/each}
        </div>
      {:else if step === "quick"}
        <div class="demo px-sunk" aria-hidden="true">
          <span>{t("setup_demo_text_a")}</span><mark>{t("setup_demo_text_b")}</mark>
          <span class="demo-bar px-box">
            <span><Icon name="ask" size={12} />{t("m_ask")}</span>
            <span><Icon name="paste" size={12} />{t("summarize")}</span>
            <span><Icon name="globe" size={12} />{t("translate")}</span>
            <span><Icon name="star" size={12} />{t("explain")}</span>
            <span><Icon name="acorn" size={12} />{t("sel_save")}</span>
          </span>
        </div>
        <div class="choices two">
          <button class="choice px-box" class:on={s.selection_hint} onclick={() => set("selection_hint", true)}>
            <strong>{t("setup_quick_yes")}</strong>
            <span class="muted small">{t("setup_quick_yes_d")}</span>
          </button>
          <button class="choice px-box" class:on={!s.selection_hint} onclick={() => set("selection_hint", false)}>
            <strong>{t("setup_quick_no")}</strong>
            <span class="muted small">{t("setup_quick_no_d")}</span>
          </button>
        </div>
      {:else if step === "privacy"}
        <label class="check px-box">
          <input type="checkbox" checked={s.exclude_from_capture} onchange={(e) => set("capture", e.currentTarget.checked)} />
          <span><strong>{t("setup_capture")}</strong><span class="muted small">{t("setup_capture_d")}</span></span>
        </label>
        <label class="check px-box">
          <input type="checkbox" checked={s.hide_fullscreen} onchange={(e) => set("fullscreen", e.currentTarget.checked)} />
          <span><strong>{t("setup_fullscreen")}</strong><span class="muted small">{t("setup_fullscreen_d")}</span></span>
        </label>
        <p class="muted small">{t("setup_privacy_note")}</p>
      {:else if step === "ai"}
        <div class="choices two">
          <button class="choice px-box" class:on={aiChoice === "none"} onclick={() => (aiChoice = "none")}>
            <strong>{t("setup_ai_none")}</strong>
            <span class="muted small">{t("setup_ai_none_d")}</span>
          </button>
          <button class="choice px-box" class:on={aiChoice === "key"} onclick={() => (aiChoice = "key")}>
            <strong>{t("setup_ai_key")}</strong>
            <span class="muted small">{t("setup_ai_key_d")}</span>
          </button>
        </div>
        {#if aiChoice === "none"}
          <p class="label">{t("setup_ai_open_in")}</p>
          <div class="row">
            <button class="btn" class:primary={s.handoff === "claude"} onclick={() => set("handoff", "claude")}>Claude</button>
            <button class="btn" class:primary={s.handoff === "chatgpt"} onclick={() => set("handoff", "chatgpt")}>ChatGPT</button>
          </div>
          {#if hasKey}<p class="muted small">{t("setup_ai_key_kept")}</p>{/if}
        {:else}
          {#if hasKey && !keyState.error}
            <p class="ok small"><Icon name="check" size={12} /> {keyState.ok ?? t("setup_ai_ready")}</p>
          {/if}
          <div class="row">
            {#each PROVIDERS as p}
              <button class="btn" class:primary={provider === p.id} onclick={() => ((provider = p.id), (keyState = {}))}>
                {p.name}{p.free ? ` · ${t("free_tier")}` : ""}
              </button>
            {/each}
          </div>
          <div class="keyrow">
            <input class="field" type="password" aria-label={t("paste_key")} placeholder={t("paste_key")} bind:value={keyText} onkeydown={(e) => e.key === "Enter" && testKey()} />
            <button class="btn primary" disabled={keyState.busy || !keyText.trim()} onclick={testKey}>{keyState.busy ? t("testing") : t("test_key")}</button>
          </div>
          <button class="link" onclick={() => invoke("open_url", { url: PROVIDERS.find((p) => p.id === provider)!.url })}>{t("setup_get_key")}</button>
          {#if keyState.error}<p class="warn small">{keyState.error}</p>{/if}
          {#if hasKey}
            <label class="check">
              <input type="checkbox" checked={s.ai_suggest} onchange={(e) => set("ai_suggest", e.currentTarget.checked)} />
              <span class="small">{t("ai_suggest_d")}</span>
            </label>
          {/if}
        {/if}
      {:else if step === "startup"}
        <div class="choices two">
          <button class="choice px-box" class:on={autostart} onclick={() => (autostart = true)}>
            <strong>{t("setup_start_yes")}</strong>
            <span class="muted small">{t("setup_start_yes_d")}</span>
          </button>
          <button class="choice px-box" class:on={!autostart} onclick={() => (autostart = false)}>
            <strong>{t("setup_start_no")}</strong>
            <span class="muted small">{t("setup_start_no_d")}</span>
          </button>
        </div>
      {:else}
        <ul class="summary">
          <li><Icon name="globe" size={12} /> {app.config?.lang === "en" ? "English" : "Español"}</li>
          <li><Icon name="squirrel" size={12} /> {t(`mode_${s.mode}` as Key)} · {t(s.scale === 4 ? "small" : s.scale === 6 ? "medium" : "large")}</li>
          <li><Icon name="acorn" size={12} /> {t("sel_title")}: {s.selection_hint ? t("on") : t("off")}</li>
          <li><Icon name="lock" size={12} /> {t("setup_capture")}: {s.exclude_from_capture ? t("on") : t("off")}</li>
          <li><Icon name="key" size={12} /> {hasKey ? t("setup_ai_ready") : t("setup_ai_summary_none", { app: s.handoff === "chatgpt" ? "ChatGPT" : "Claude" })}</li>
          <li><Icon name="clock" size={12} /> {t("autostart")}: {autostart ? t("on") : t("off")}</li>
        </ul>
        <p class="label">{t("setup_tips")}</p>
        <ul class="tips">
          <li>{t("setup_tip_click")}</li>
          <li>{t("setup_tip_right")}</li>
          <li>{t("setup_tip_drag")}</li>
        </ul>
        <button class="btn ghost small-btn" onclick={integrations}><Icon name="plug" size={12} />{t("open_integrations")}</button>
      {/if}
    </div>

    <div class="nav">
      <div class="dots">
        {#each STEPS as _, n}<span class:on={n === i} class:done={n < i}></span>{/each}
      </div>
      <span class="grow"></span>
      {#if i > 0}<button class="btn ghost" onclick={() => i--}>{t("back")}</button>{/if}
      {#if step !== "end"}
        <button class="btn ghost" onclick={finish} title={t("setup_skip_d")}>{t("skip")}</button>
        <button class="btn primary" onclick={next}>{t("next")}</button>
      {:else}
        <button class="btn primary" onclick={finish}>{t("done")}</button>
      {/if}
    </div>
  </div>
</Window>

<style>
  .page {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 18px 12px;
  }

  .head {
    display: flex;
    gap: 14px;
    align-items: center;
  }

  .art {
    padding: 8px 12px;
    flex: none;
  }

  .art img {
    width: 80px;
    height: 80px;
    image-rendering: pixelated;
    display: block;
    animation: pop 200ms steps(4) both;
  }

  .kicker {
    margin: 0;
    font-size: 11px;
    color: var(--accent);
    font-family: var(--title);
  }

  h2 {
    font-size: 16px;
    margin: 2px 0 4px;
  }

  .head .muted {
    margin: 0;
    font-size: 13px;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .label {
    margin: 4px 0 0;
    font-family: var(--title);
    font-size: 11px;
    color: var(--muted, inherit);
  }

  .choices {
    display: grid;
    gap: 8px;
  }

  .choices.two {
    grid-template-columns: 1fr 1fr;
  }

  .choices.three {
    grid-template-columns: 1fr 1fr 1fr;
  }

  .choice {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 3px;
    padding: 9px 10px;
    border: 0;
    text-align: left;
    cursor: pointer;
    color: var(--ink);
  }

  .choice.on {
    box-shadow: inset 0 0 0 3px var(--accent);
  }

  .choice:hover {
    background: var(--sunk);
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: center;
  }

  .sw {
    padding: 2px;
    border: 0;
    background: var(--sunk);
    cursor: pointer;
  }

  .sw.on {
    box-shadow: inset 0 0 0 2px var(--accent);
  }

  .sw img {
    width: 32px;
    height: 32px;
    image-rendering: pixelated;
    display: block;
  }

  .sep {
    width: 8px;
  }

  .demo {
    position: relative;
    padding: 12px 12px 44px;
    font-size: 13px;
  }

  .demo mark {
    background: #3b6fd1;
    color: #fff;
  }

  .demo-bar {
    position: absolute;
    right: 10px;
    bottom: 8px;
    display: flex;
    gap: 8px;
    padding: 3px 7px;
    font-size: 11px;
    white-space: nowrap;
  }

  .demo-bar span {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }

  .check {
    display: flex;
    gap: 9px;
    align-items: flex-start;
    padding: 8px 10px;
  }

  .check > span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .keyrow {
    display: flex;
    gap: 6px;
  }

  .keyrow .field {
    flex: 1;
  }

  .link {
    align-self: flex-start;
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
    text-decoration: underline;
    font-size: 12px;
  }

  .summary,
  .tips {
    margin: 0;
    padding-left: 4px;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
  }

  .summary li {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .tips li::before {
    content: "· ";
    color: var(--accent);
  }

  .small {
    font-size: 12px;
  }

  .small-btn {
    align-self: flex-start;
  }

  .ok {
    color: var(--ok);
    margin: 0;
    display: flex;
    gap: 5px;
    align-items: center;
  }

  .nav {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .grow {
    flex: 1;
  }

  .dots {
    display: flex;
    gap: 5px;
  }

  .dots span {
    width: 8px;
    height: 8px;
    background: var(--sunk);
    box-shadow: 0 0 0 2px var(--line);
  }

  .dots span.done {
    background: var(--line);
  }

  .dots span.on {
    background: var(--accent);
  }

  @keyframes pop {
    from {
      transform: translateY(6px);
      opacity: 0;
    }
  }
</style>
