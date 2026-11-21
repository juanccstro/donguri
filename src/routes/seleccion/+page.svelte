<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";
  import { fitWindow, loadConfig, t } from "$lib/app.svelte";
  import Icon, { type IconName } from "$lib/Icon.svelte";
  import type { Key } from "$lib/i18n";

  // "Bellota rápida": the little acorn next to the cursor after selecting
  // text. Small and quiet: its actions open only when the pointer rests on it
  // (passing over it on the way elsewhere does nothing), and it leaves by
  // itself if ignored. The window is built once and reused, so it appears
  // the instant a selection ends.
  const IGNORED_MS = 8000;
  const LEAVE_MS = 1200;
  /** Resting this long on the acorn opens the actions. */
  const HOVER_MS = 140;

  const ACTIONS: { id: string; icon: IconName; label: Key }[] = [
    { id: "ask", icon: "ask", label: "m_ask" },
    { id: "summarize", icon: "paste", label: "summarize" },
    { id: "translate", icon: "globe", label: "translate" },
    { id: "explain", icon: "star", label: "explain" },
    { id: "save", icon: "acorn", label: "sel_save" },
  ];

  let open = $state(false);
  let shown = $state(false);
  let box: HTMLElement;
  let timer: number | undefined;
  let hoverTimer: number | undefined;

  function arm(ms: number) {
    clearTimeout(timer);
    timer = setTimeout(() => invoke("popup_close"), ms);
  }

  function onEnter() {
    clearTimeout(hoverTimer);
    hoverTimer = setTimeout(() => {
      clearTimeout(timer);
      open = true;
    }, HOVER_MS);
  }

  function onLeave() {
    clearTimeout(hoverTimer);
    // Only an opened bar leaves on its own; a pass-by keeps the acorn.
    if (open) arm(LEAVE_MS);
  }

  function openNow() {
    clearTimeout(hoverTimer);
    clearTimeout(timer);
    open = true;
  }

  function pick(id: string) {
    invoke("selection_action", { action: id });
  }

  async function show() {
    clearTimeout(timer);
    clearTimeout(hoverTimer);
    open = false;
    shown = false;
    await tick();
    shown = true; // re-creates the acorn: its pop plays again
    await tick();
    // Two frames: the page paints before the window shows (no black flash).
    requestAnimationFrame(() => requestAnimationFrame(() => invoke("popup_ready")));
    arm(IGNORED_MS);
  }

  function hide() {
    clearTimeout(timer);
    clearTimeout(hoverTimer);
    open = false;
    shown = false;
  }

  onMount(() => {
    loadConfig();
    const stop = fitWindow(box);
    const offs = [listen("seleccion-show", show), listen("seleccion-hide", hide)];
    invoke<boolean>("popup_wanted").then((w) => {
      if (w) show();
    });
    return () => {
      stop();
      offs.forEach((o) => o.then((f) => f()));
      clearTimeout(timer);
      clearTimeout(hoverTimer);
    };
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="wrap" bind:this={box} onpointerenter={onEnter} onpointerleave={onLeave}>
  {#if open}
    <div class="bar px-box">
      {#each ACTIONS as a}
        <button class="btn ghost act" onclick={() => pick(a.id)} title={t(a.label)}>
          <Icon name={a.icon} size={14} /><span>{t(a.label)}</span>
        </button>
      {/each}
    </div>
  {:else if shown}
    <button class="acorn" aria-label={t("sel_feature")} title={t("sel_feature")} onclick={openNow}>
      <Icon name="acorn" size={18} />
    </button>
  {:else}
    <div class="blank"></div>
  {/if}
</div>

<style>
  .wrap {
    display: inline-block;
    padding: 3px;
  }

  .blank {
    width: 24px;
    height: 24px;
  }

  .acorn {
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    border: 0;
    padding: 0;
    cursor: pointer;
    color: var(--acorn-dark);
    background: var(--panel);
    box-shadow:
      0 -2px 0 0 var(--line),
      0 2px 0 0 var(--line),
      -2px 0 0 0 var(--line),
      2px 0 0 0 var(--line);
    opacity: 0.85;
    animation: pop 100ms steps(2) both;
  }

  .acorn:hover {
    opacity: 1;
  }

  .bar {
    display: flex;
    gap: 0;
    padding: 2px;
    white-space: nowrap;
    animation: pop 90ms steps(2) both;
  }

  .act {
    font-size: 12px;
    gap: 4px;
    margin: 0;
  }

  @keyframes pop {
    from {
      transform: scale(0.6);
      opacity: 0;
    }
  }
</style>
