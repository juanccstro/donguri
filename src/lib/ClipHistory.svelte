<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { t } from "./app.svelte";
  import Icon from "./Icon.svelte";

  // Clipboard history inside Kura (opt-in, memory only). Click an entry to
  // paste it in the app you were in; or ask about it, save it as an acorn,
  // or forget it. Kura's search box filters it.
  interface Entry {
    id: number;
    kind: "text" | "image";
    preview: string;
    chars: number;
    at: number;
    app: string | null;
  }

  let { query = "" }: { query?: string } = $props();

  let on = $state<boolean | null>(null);
  let items = $state<Entry[]>([]);

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return q ? items.filter((i) => i.kind === "text" && i.preview.toLowerCase().includes(q)) : items;
  });

  async function load() {
    const s = await invoke<{ clip_history: boolean }>("get_settings");
    on = s.clip_history;
    items = on ? await invoke<Entry[]>("clip_history") : [];
  }

  async function setOn(value: boolean) {
    await invoke("clip_history_enable", { on: value });
    await load();
  }

  const use = (e: Entry, action: "paste" | "ask" | "save") => invoke("clip_history_use", { id: e.id, action });

  async function forget(e: Entry) {
    await invoke("clip_history_delete", { id: e.id });
    await load();
  }

  const time = (at: number) => new Date(at * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

  onMount(() => {
    load();
    const off = listen("clip-history-changed", load);
    return () => {
      off.then((f) => f());
    };
  });
</script>

<div class="bar">
  <span class="title">{t("clip_title")}</span>
  <span class="grow"></span>
  {#if on && items.length}
    <button class="btn ghost" title={t("clip_clear")} onclick={() => invoke("clip_history_clear")}><Icon name="trash" size={14} /></button>
  {/if}
</div>

{#if on === false}
  <div class="off">
    <p>{t("clip_off")}</p>
    <p class="muted small">{t("clip_private")}</p>
    <button class="btn primary" onclick={() => setOn(true)}><Icon name="clipboard" size={14} />{t("clip_enable")}</button>
  </div>
{:else if on}
  <div class="list scroll">
    {#each shown as e (e.id)}
      <div class="entry">
        <button class="item" title={t("clip_paste")} onclick={() => use(e, "paste")}>
          {#if e.kind === "image"}
            <img src={e.preview} alt={t("using_image")} />
          {:else}
            <span class="text">{e.preview}</span>
          {/if}
          <span class="muted small">{time(e.at)}{e.app ? ` · ${e.app.replace(/\.exe$/i, "")}` : ""}</span>
        </button>
        <div class="tools">
          <button class="btn ghost" title={t("ask_image")} onclick={() => use(e, "ask")}><Icon name="ask" size={13} /></button>
          <button class="btn ghost" title={t("save_acorn")} onclick={() => use(e, "save")}><Icon name="acorn" size={13} /></button>
          <button class="btn ghost" title={t("delete")} onclick={() => forget(e)}><Icon name="trash" size={13} /></button>
        </div>
      </div>
    {:else}
      <p class="muted empty">{items.length ? t("clip_none") : t("clip_empty")}</p>
    {/each}
  </div>
  <p class="muted note">{t("clip_private")}</p>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 8px 10px 6px;
  }
  .title {
    font-family: "Silkscreen", monospace;
    font-size: 12px;
    letter-spacing: 1px;
    text-transform: uppercase;
  }
  .grow {
    flex: 1;
  }
  .off {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 4px 10px 10px;
  }
  .off .btn {
    align-self: flex-start;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 8px;
  }
  .entry {
    position: relative;
    padding: 4px 4px 2px;
    margin-bottom: 4px;
    background: var(--sunk);
  }
  .item {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 2px;
    text-align: left;
    background: none;
    border: 0;
    padding: 2px 4px;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .item:hover .text {
    color: var(--accent);
  }
  .text {
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    white-space: pre-wrap;
    word-break: break-word;
  }
  img {
    max-width: 100%;
    max-height: 80px;
    object-fit: contain;
    align-self: flex-start;
  }
  /* On the time line, at the right. */
  .tools {
    position: absolute;
    right: 2px;
    bottom: 0;
    display: flex;
  }
  .tools .btn {
    padding: 3px;
  }
  .empty {
    text-align: center;
    padding: 24px 0;
  }
  .note {
    margin: 0;
    padding: 6px 10px 8px;
    font-size: 11px;
    line-height: 1.25;
  }
</style>
