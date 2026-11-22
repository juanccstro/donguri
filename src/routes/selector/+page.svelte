<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";
  import { closeSelf, fitWindow, loadConfig, t, type Tail } from "$lib/app.svelte";
  import Beak from "$lib/Beak.svelte";
  import Icon from "$lib/Icon.svelte";
  import type { Key } from "$lib/i18n";

  interface Nest {
    id: number;
    name: string;
    color: string;
    count: number;
    system: boolean;
    last_used: number;
  }
  interface PendingInfo {
    kind: string;
    title: string;
    preview: string;
    words: number;
    suggest: boolean;
  }
  type Option =
    | { type: "nest"; nest: Nest; tag?: Key }
    | { type: "suggest"; name: string; existing?: Nest }
    | { type: "new" };

  // ?modo=contexto picks a nest to paste as context instead of saving.
  const contextMode = new URLSearchParams(location.search).get("modo") === "contexto";

  let nests = $state<Nest[]>([]);
  let active = $state<number | null>(null);
  let pending = $state<PendingInfo | null>(null);
  let suggestion = $state<string | null>(null);
  let suggesting = $state(false);
  let selected = $state(0);
  let creating = $state(false);
  let newName = $state("");
  let tail = $state<Tail>({ edge: null, x: 0, y: 0, space: [0, 0, 0, 0] } as Tail);
  let box: HTMLElement;
  let input = $state<HTMLInputElement>();
  let done = false;
  // Pegar nido: as saved, or condensed by the AI first.
  let summary = $state(false);
  let hasKey = $state(false);

  const options = $derived.by<Option[]>(() => {
    const list: Option[] = [];
    const used = new Set<number>();
    if (contextMode) {
      return nests.filter((n) => n.count > 0).map((nest) => ({ type: "nest" as const, nest }));
    }
    if (suggestion) {
      const existing = nests.find((n) => n.name.toLowerCase() === suggestion!.toLowerCase());
      list.push({ type: "suggest", name: suggestion, existing });
      if (existing) used.add(existing.id);
    }
    const act = nests.find((n) => n.id === active);
    if (act && !used.has(act.id)) {
      list.push({ type: "nest", nest: act, tag: "active" });
      used.add(act.id);
    }
    for (const n of [...nests].filter((n) => !n.system && !used.has(n.id)).sort((a, b) => b.last_used - a.last_used).slice(0, 5)) {
      list.push({ type: "nest", nest: n });
      used.add(n.id);
    }
    list.push({ type: "new" });
    return list;
  });

  async function load() {
    nests = await invoke<Nest[]>("nests");
    const settings = await invoke<{ active_nest: number | null }>("get_settings");
    active = settings.active_nest;
  }

  async function choose(opt: Option) {
    if (done) return;
    if (opt.type === "new") {
      creating = true;
      await tick();
      input?.focus();
      return;
    }
    done = true;
    if (contextMode) {
      if (opt.type === "nest") await invoke("paste_context", { nestId: opt.nest.id, summary });
      return;
    }
    if (opt.type === "suggest") {
      const target = opt.existing ? { to: "nest", id: opt.existing.id } : { to: "new", name: opt.name };
      await invoke("save_pending", { target });
    } else {
      await invoke("save_pending", { target: { to: "nest", id: opt.nest.id } });
    }
  }

  /** An image: ask about it instead of saving it. */
  async function askIt() {
    if (done) return;
    done = true;
    await invoke("ask_pending");
  }

  async function createNest() {
    if (!newName.trim() || done) return;
    done = true;
    await invoke("save_pending", { target: { to: "new", name: newName.trim() } });
  }

  /** Esc (or clicking elsewhere): never lose it, it goes to Sin clasificar. */
  async function dismiss() {
    if (done) return;
    done = true;
    if (contextMode) await closeSelf();
    else await invoke("save_pending", { target: { to: "unsorted" } });
  }

  function onKey(e: KeyboardEvent) {
    if (creating) {
      if (e.key === "Enter") createNest();
      if (e.key === "Escape") {
        creating = false;
        e.preventDefault();
      }
      return;
    }
    if (e.key === "ArrowDown") selected = Math.min(options.length - 1, selected + 1);
    else if (e.key === "ArrowUp") selected = Math.max(0, selected - 1);
    else if (e.key === "Enter" && options[selected]) choose(options[selected]);
    else if (e.key === "Escape") dismiss();
    else return;
    e.preventDefault();
  }

  onMount(() => {
    loadConfig();
    load();
    if (contextMode) {
      invoke<{ configured: boolean }[]>("ai_status").then((s) => (hasKey = s.some((p) => p.configured)));
    }
    if (!contextMode) {
      // The AI suggestion is opt-in and arrives later; it is preselected.
      invoke<PendingInfo | null>("pending_info").then((p) => {
        pending = p;
        if (!p?.suggest) return;
        suggesting = true;
        invoke<string | null>("suggest_nest")
          .then((name) => {
            if (name && !done) {
              suggestion = name;
              selected = 0;
            }
          })
          .finally(() => (suggesting = false));
      });
    }
    const offData = listen("data-changed", load);
    const stop = fitWindow(box, (t) => (tail = t));
    const onBlur = () => setTimeout(() => !document.hasFocus() && dismiss(), 150);
    window.addEventListener("blur", onBlur);
    window.focus();
    return () => {
      offData.then((f) => f());
      stop();
      window.removeEventListener("blur", onBlur);
    };
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="wrap" bind:this={box} data-tail={tail.edge}>
  <div class="px-box panel">
    {#if tail.edge}
      <Beak
        edge={tail.edge}
        offset={tail.edge === "bottom" || tail.edge === "top" ? Math.min(Math.max(tail.x - 7, 16), 236) : Math.min(Math.max(tail.y - 7, 16), 260)}
      />
    {/if}
    <div class="head">
      <Icon name={contextMode ? "paste" : "acorn"} size={18} />
      <span class="title">{t(contextMode ? "which_context" : "where")}</span>
    </div>

    {#if contextMode}
      <div class="modes" role="radiogroup" aria-label={t("which_context")}>
        <button class="btn" class:primary={!summary} role="radio" aria-checked={!summary} onclick={() => (summary = false)}>{t("paste_raw")}</button>
        <button class="btn" class:primary={summary} role="radio" aria-checked={summary} disabled={!hasKey} title={hasKey ? "" : t("needs_key")} onclick={() => (summary = true)}>{t("paste_summary")}</button>
      </div>
      <p class="muted small mode-note">{summary ? t("paste_summary_d") : t("paste_raw_d")}</p>
    {/if}

    {#if pending}
      <div class="pending px-sunk">
        <strong>{pending.title || t(`kind_${pending.kind}` as Key)}</strong>
        <span class="muted small">
          {t(`kind_${pending.kind}` as Key)}{pending.words ? ` · ${t(pending.words === 1 ? "words_1" : "words_n", { n: pending.words })}` : ""}
        </span>
      </div>
    {/if}
    {#if pending?.kind === "imagen"}
      <button class="btn primary ask-it" onclick={askIt}><Icon name="ask" size={14} />{t("ask_about_it")}</button>
    {/if}

    <ul class="list" role="listbox">
      {#each options as opt, i}
        <li>
          <button
            class="opt"
            class:sel={i === selected}
            role="option"
            aria-selected={i === selected}
            onpointerenter={() => (selected = i)}
            onclick={() => choose(opt)}
          >
            {#if opt.type === "suggest"}
              <Icon name="star" size={14} />
              <span class="name">{opt.name}</span>
              <span class="tag">{t("suggested")}{opt.existing ? "" : ` · ${t("new_nest")}`}</span>
            {:else if opt.type === "nest"}
              <span class="dot" style:background={opt.nest.color}></span>
              <span class="name">{opt.nest.system ? t("unsorted") : opt.nest.name}</span>
              <span class="tag">{opt.tag ? t(opt.tag) : opt.nest.count ? t("acorns_n", { n: opt.nest.count }) : t("empty_nest")}</span>
            {:else if creating}
              <Icon name="plus" size={14} />
              <input class="field inline" aria-label={t("new_nest")} bind:this={input} bind:value={newName} placeholder={t("new_nest_ph")} />
            {:else}
              <Icon name="plus" size={14} />
              <span class="name">{t("new_nest")}</span>
            {/if}
          </button>
        </li>
      {/each}
      {#if contextMode && !options.length}
        <li class="muted small empty">{t("empty_kura", { key: "Alt+Shift+S" })}</li>
      {/if}
    </ul>

    {#if suggesting}
      <div class="muted small thinking">{t("thinking_nest")}</div>
    {/if}
    {#if !contextMode}
      <div class="foot muted small">{t("enter_save")} · {t("esc_unsorted")}</div>
    {/if}
  </div>
</div>

<style>
  .ask-it {
    width: 100%;
    justify-content: center;
    margin: 6px 0 2px;
  }
  .wrap {
    display: inline-block;
    width: 270px;
    padding: 6px;
  }
  .wrap[data-tail="bottom"] { padding-bottom: 16px; }
  .wrap[data-tail="top"] { padding-top: 16px; }
  .wrap[data-tail="left"] { padding-left: 16px; }
  .wrap[data-tail="right"] { padding-right: 16px; }

  .panel {
    padding: 8px;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-bottom: 7px;
  }

  .modes {
    display: flex;
    gap: 2px;
    margin-bottom: 4px;
  }

  .modes .btn {
    font-size: 12px;
    flex: 1;
    justify-content: center;
  }

  .mode-note {
    margin: 2px 0 8px;
  }

  .pending {
    display: flex;
    flex-direction: column;
    padding: 5px 7px;
    margin-bottom: 8px;
  }

  .pending strong {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 12px;
  }

  .list {
    list-style: none;
    padding: 0;
    margin: 0;
    max-height: 220px;
    overflow-y: auto;
  }

  .opt {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 5px 6px;
    border: 0;
    background: transparent;
    cursor: pointer;
    text-align: left;
  }

  .opt.sel {
    background: var(--accent);
    color: var(--accent-ink);
  }

  .opt.sel .tag {
    color: inherit;
    opacity: 0.85;
  }

  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tag {
    font-size: 11px;
    color: var(--ink-soft);
  }

  .dot {
    width: 9px;
    height: 9px;
    flex: none;
    box-shadow: 0 0 0 2px var(--line);
  }

  .inline {
    flex: 1;
    padding: 2px 5px;
    color: var(--ink);
  }

  .foot,
  .thinking,
  .empty {
    margin-top: 7px;
    text-align: center;
  }
</style>
