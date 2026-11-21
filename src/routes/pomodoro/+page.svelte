<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { loadConfig, t } from "$lib/app.svelte";
  import Icon from "$lib/Icon.svelte";
  import type { Key } from "$lib/i18n";
  import { clock, leftNow, PRESETS, type Plan, type PomoStatus } from "$lib/pomodoro";
  import Window from "$lib/Window.svelte";

  // Pomodoro panel: pick a preset or your own times; while it runs, the
  // phase, the time left and the controls. The squirrel shows it too.
  let status = $state<PomoStatus | null>(null);
  let now = $state(Date.now());
  let choosing = $state(false);
  let custom = $state<Plan>({ focus: 30, short: 5, long: 0, rounds: 4 });

  const running = $derived(!!status?.active && !choosing);
  const left = $derived(status ? (void now, leftNow(status)) : 0);
  const progress = $derived(status && status.total ? 1 - left / status.total : 0);
  const phaseKey = $derived<Key>(status?.phase === "focus" ? "pomo_focus" : status?.phase === "long" ? "pomo_long" : "pomo_break");

  // The window fits what it shows: compact while running, taller to choose.
  $effect(() => {
    invoke("popup_fit", { w: 340, h: running ? 340 : 630 });
  });

  function start(plan: Plan) {
    choosing = false;
    invoke("pomodoro_start", { plan });
  }

  function customValid(p: Plan) {
    return p.focus >= 1 && p.focus <= 240 && p.short >= 1 && p.short <= 120 && p.rounds >= 1 && p.rounds <= 12 && p.long >= 0 && p.long <= 120;
  }

  const desc = (p: Plan) =>
    t("pomo_plan", { f: p.focus, b: p.short, r: p.rounds }) + (p.long ? t("pomo_plan_long", { l: p.long }) : "");

  onMount(() => {
    loadConfig();
    invoke<PomoStatus>("pomodoro_status").then((s) => (status = s));
    const off = listen<PomoStatus>("pomodoro", (e) => (status = e.payload));
    // Ticks only while this panel is open.
    const tick = setInterval(() => (now = Date.now()), 500);
    // Clicking outside closes it, like Kura.
    const onBlur = () =>
      setTimeout(() => {
        if (!document.hasFocus()) invoke("popup_close");
      }, 200);
    window.addEventListener("blur", onBlur);
    return () => {
      off.then((f) => f());
      clearInterval(tick);
      window.removeEventListener("blur", onBlur);
    };
  });
</script>

<Window title={t("pomo_title")} icon="clock">
  {#if running && status}
    <section class="run" class:rest={status.phase !== "focus"}>
      <p class="phase">{t(phaseKey)}{status.paused ? ` · ${t("pomo_paused")}` : ""}</p>
      <p class="time" aria-live="off">{clock(left)}</p>
      <div class="bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(progress * 100)}>
        <span style:width="{Math.round(progress * 100)}%"></span>
      </div>
      <p class="muted small">{t("pomo_round", { n: status.round, of: status.rounds })}</p>
      <div class="row">
        {#if status.paused}
          <button class="btn primary" onclick={() => invoke("pomodoro_resume")}><Icon name="play" size={14} />{t("pomo_resume")}</button>
        {:else}
          <button class="btn primary" onclick={() => invoke("pomodoro_pause")}><Icon name="pause" size={14} />{t("pomo_pause")}</button>
        {/if}
        <button class="btn" onclick={() => invoke("pomodoro_restart")} title={t("pomo_restart_d")}><Icon name="restart" size={14} />{t("pomo_restart")}</button>
      </div>
      <div class="row">
        <button class="btn ghost" onclick={() => invoke("pomodoro_skip")}>{status.phase === "focus" ? t("pomo_skip_focus") : t("pomo_skip_break")}</button>
        <button class="btn ghost" onclick={() => (choosing = true)}>{t("pomo_other")}</button>
        <button class="btn ghost" onclick={() => invoke("pomodoro_stop")}><Icon name="close" size={12} />{t("pomo_stop")}</button>
      </div>
    </section>
  {:else}
    <section class="pick">
      <p class="muted small">{t("pomo_intro")}</p>
      {#each PRESETS as p}
        <button class="preset px-box" onclick={() => start(p.plan)}>
          <strong>{t(`pomo_${p.id}` as Key)}</strong>
          <span>{desc(p.plan)}</span>
          <span class="muted small">{t(`pomo_${p.id}_d` as Key)}</span>
        </button>
      {/each}
      <div class="custom px-box">
        <strong>{t("pomo_custom")}</strong>
        <div class="fields">
          <label>{t("pomo_f_focus")}<input class="field" type="number" min="1" max="240" bind:value={custom.focus} /></label>
          <label>{t("pomo_f_break")}<input class="field" type="number" min="1" max="120" bind:value={custom.short} /></label>
          <label>{t("pomo_f_rounds")}<input class="field" type="number" min="1" max="12" bind:value={custom.rounds} /></label>
          <label>{t("pomo_f_long")}<input class="field" type="number" min="0" max="120" bind:value={custom.long} /></label>
        </div>
        <button class="btn primary" disabled={!customValid(custom)} onclick={() => start({ ...custom })}><Icon name="play" size={14} />{t("pomo_start")}</button>
      </div>
      {#if choosing}
        <button class="btn ghost" onclick={() => (choosing = false)}>{t("pomo_back")}</button>
      {/if}
    </section>
  {/if}
</Window>

<style>
  section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px 10px;
    min-height: 0;
    overflow-y: auto; /* never cut off, even with big fonts */
  }

  .run {
    align-items: center;
    text-align: center;
  }

  .phase {
    margin: 4px 0 0;
    font-family: var(--title);
    color: var(--accent);
  }

  .rest .phase {
    color: var(--ok);
  }

  .time {
    margin: 0;
    font-size: 44px;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
  }

  .bar {
    width: 100%;
    height: 10px;
    background: var(--sunk);
    box-shadow: inset 0 0 0 2px var(--line);
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
  }

  .rest .bar span {
    background: var(--ok);
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 6px;
  }

  .preset {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    padding: 6px 9px;
    border: 0;
    text-align: left;
    cursor: pointer;
    color: var(--ink);
  }

  .preset:hover,
  .preset:focus-visible {
    background: var(--accent);
    color: var(--accent-ink);
    outline: none;
  }

  .preset:hover .muted {
    color: inherit;
  }

  .custom {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 6px 9px 8px;
  }

  .fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }

  .fields label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12px;
  }

  .fields .field {
    width: 100%;
  }

  .small {
    font-size: 12px;
  }

  .muted {
    margin: 0;
  }
</style>
