<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";
  import { loadConfig, t } from "$lib/app.svelte";
  import Icon, { type IconName } from "$lib/Icon.svelte";
  import Window from "$lib/Window.svelte";
  import ClipHistory from "$lib/ClipHistory.svelte";
  import type { Key } from "$lib/i18n";

  // Kura (蔵): the storehouse. Nests on the left, acorns on the right,
  // search across everything on top. The clipboard history lives here too
  // (last entry on the left), so it needs no window of its own.

  interface Nest {
    id: number;
    name: string;
    color: string;
    count: number;
    chars: number;
    system: boolean;
  }
  interface Summary {
    id: number;
    nest_id: number;
    kind: string;
    title: string;
    preview: string;
    created_at: number;
  }
  interface Hit {
    acorn: Summary;
    nest: string;
    snippet: string;
  }
  interface AcornView extends Summary {
    content: string;
    source: string | null;
    image: string | null;
  }

  const COLORS = ["#D97757", "#A97A35", "#6AA84F", "#4F7CAC", "#8E6BB0", "#C9483B", "#7A5A4E", "#3A2420"];
  const KIND_ICON: Record<string, IconName> = { texto: "paste", imagen: "crop", pdf: "copy", archivo: "copy" };

  let nests = $state<Nest[]>([]);
  let selectedId = $state<number | null>(null);
  let acorns = $state<Summary[]>([]);
  let active = $state<number | null>(null);
  let query = $state("");
  let hits = $state<Hit[] | null>(null);
  let open = $state<AcornView | null>(null);
  let renaming = $state<number | null>(null);
  let renameText = $state("");
  let confirm = $state<{ text: string; run: () => void } | null>(null);
  let toast = $state<string | null>(null);
  let shortcut = $state("Alt+Shift+S");
  let moving = $state(false);
  let renameInput = $state<HTMLInputElement>();
  // ?vista=portapapeles (or the shortcut while Kura is open) shows the clipboard.
  let clipView = $state(new URLSearchParams(location.search).get("vista") === "portapapeles");

  const selected = $derived(nests.find((n) => n.id === selectedId) ?? null);
  const nestName = (n: Nest) => (n.system ? t("unsorted") : n.name);

  async function loadNests() {
    nests = await invoke<Nest[]>("nests");
    const s = await invoke<{ active_nest: number | null; shortcuts: { save: string } }>("get_settings");
    active = s.active_nest;
    shortcut = s.shortcuts.save;
    if (selectedId === null || !nests.some((n) => n.id === selectedId)) {
      selectedId = nests.find((n) => !n.system && n.count)?.id ?? nests[0]?.id ?? null;
    }
    await loadAcorns();
  }

  async function loadAcorns() {
    if (selectedId === null) return;
    acorns = await invoke<Summary[]>("acorns", { nestId: selectedId });
  }

  let searchTimer: number | undefined;
  function onSearch() {
    clearTimeout(searchTimer);
    if (clipView) return; // the clipboard list filters itself
    searchTimer = setTimeout(async () => {
      hits = query.trim() ? await invoke<Hit[]>("search", { query, nestId: null }) : null;
    }, 180);
  }

  function showClipboard() {
    clipView = true;
    open = null;
    hits = null;
    query = "";
  }

  async function selectNest(id: number) {
    clipView = false;
    selectedId = id;
    open = null;
    hits = null;
    query = "";
    await loadAcorns();
  }

  async function openAcorn(id: number) {
    open = await invoke<AcornView | null>("acorn", { id });
    moving = false;
  }

  function flash(text: string) {
    toast = text;
    setTimeout(() => (toast = null), 2200);
  }

  async function startRename(n: Nest) {
    renaming = n.id;
    renameText = n.name;
    await tick();
    renameInput?.select();
  }

  async function finishRename() {
    if (renaming !== null && renameText.trim()) {
      await invoke("nest_rename", { id: renaming, name: renameText.trim() });
    }
    renaming = null;
  }

  async function newNest() {
    const id = await invoke<number>("nest_create", { name: `${t("new_nest")} ${nests.length}` });
    await loadNests();
    await selectNest(id);
    const n = nests.find((x) => x.id === id);
    if (n) startRename(n);
  }

  async function toggleActive(n: Nest) {
    await invoke("set_setting", { key: "active_nest", value: active === n.id ? null : n.id });
    active = active === n.id ? null : n.id;
  }

  function askDeleteNest(n: Nest) {
    confirm = {
      text: t("delete_nest_q", { name: n.name, n: n.count }),
      run: async () => {
        await invoke("nest_delete", { id: n.id });
        selectedId = null;
        await loadNests();
      },
    };
  }

  function askDeleteAcorn(a: AcornView) {
    confirm = {
      text: t("delete_acorn_q"),
      run: async () => {
        await invoke("acorn_delete", { id: a.id });
        open = null;
        await loadNests();
      },
    };
  }

  async function exportNest(n: Nest) {
    const path = await invoke<string>("nest_export", { id: n.id });
    flash(t("exported", { path }));
  }

  async function moveAcorn(a: AcornView, nestId: number) {
    await invoke("acorn_move", { id: a.id, nestId });
    open = null;
    moving = false;
    await loadNests();
  }

  const date = (ts: number) =>
    new Date(ts * 1000).toLocaleString(document.documentElement.lang || "es", { dateStyle: "medium", timeStyle: "short" });

  onMount(() => {
    loadConfig();
    loadNests();
    const off = listen("data-changed", loadNests);
    const offClip = listen("kura-clipboard", showClipboard);
    // Clicking outside Kura closes it (unless a confirmation is pending).
    const onBlur = () =>
      setTimeout(() => {
        if (!document.hasFocus() && !confirm) invoke("popup_close");
      }, 200);
    window.addEventListener("blur", onBlur);
    return () => {
      off.then((f) => f());
      offClip.then((f) => f());
      window.removeEventListener("blur", onBlur);
    };
  });
</script>

<Window title={t("kura_title")} icon="nest">
  <div class="search">
    <Icon name="search" size={16} />
    <input class="field" type="search" aria-label={t("search_ph")} bind:value={query} oninput={onSearch} placeholder={t("search_ph")} />
  </div>

  <div class="cols">
    <nav class="nests scroll">
      {#each nests as n (n.id)}
        <div class="nest" class:sel={n.id === selectedId && !hits && !clipView}>
          {#if renaming === n.id}
            <input
              class="field rename"
              aria-label={t("rename")}
              bind:this={renameInput}
              bind:value={renameText}
              onkeydown={(e) => e.key === "Enter" ? finishRename() : e.key === "Escape" && (renaming = null)}
              onblur={finishRename}
            />
          {:else}
            <button class="pick" onclick={() => selectNest(n.id)} ondblclick={() => !n.system && startRename(n)}>
              <span class="dot" style:background={n.color}></span>
              <span class="name">{nestName(n)}</span>
              {#if active === n.id}<Icon name="pin" size={12} title={t("active")} />{/if}
              <span class="count">{n.count}</span>
            </button>
          {/if}
        </div>
      {/each}
      <button class="btn ghost add" onclick={newNest}><Icon name="plus" size={13} />{t("new_nest")}</button>
      <div class="nest clip" class:sel={clipView}>
        <button class="pick" onclick={showClipboard}>
          <Icon name="clipboard" size={13} />
          <span class="name">{t("clip_title")}</span>
        </button>
      </div>
    </nav>

    <section class="acorns">
      {#if clipView}
        <ClipHistory {query} />
      {:else if hits}
        <div class="bar muted">{hits.length === 1 ? t("results_one") : hits.length ? t("results_n", { n: hits.length }) : t("no_results")}</div>
        <div class="list scroll">
          {#each hits as h (h.acorn.id)}
            <button class="acorn" onclick={() => openAcorn(h.acorn.id)}>
              <Icon name={KIND_ICON[h.acorn.kind] ?? "paste"} size={14} />
              <div class="text">
                <strong>{h.acorn.title}</strong>
                <span class="muted small">{h.nest}</span>
                <span class="snippet">{h.snippet}</span>
              </div>
            </button>
          {/each}
        </div>
      {:else if open}
        <div class="bar">
          <button class="btn ghost" onclick={() => (open = null)}>‹ {t("back")}</button>
        </div>
        <div class="detail scroll">
          <h3>{open.title}</h3>
          <div class="muted small">{t("created", { date: date(open.created_at) })}{open.source ? ` · ${t("from_source", { source: open.source })}` : ""}</div>
          {#if open.image}
            <img src={open.image} alt={open.title} />
          {/if}
          {#if open.content}
            <pre class="content">{open.content}</pre>
          {/if}
        </div>
        <div class="tools">
          {#if open.image}
            <button class="btn primary" onclick={() => open && invoke("ask_acorn", { id: open.id })}><Icon name="ask" size={13} />{t("ask_image")}</button>
          {/if}
          {#if open.content}
            <button class="btn" onclick={() => invoke("copy_text", { text: open?.content ?? "" }).then(() => flash(t("copied")))}><Icon name="copy" size={13} />{t("copy")}</button>
          {/if}
          <div class="move">
            <button class="btn" onclick={() => (moving = !moving)}><Icon name="nest" size={13} />{t("move_to")}</button>
            {#if moving}
              <div class="menu px-box">
                {#each nests.filter((n) => n.id !== open?.nest_id) as n}
                  <button class="pick" onclick={() => open && moveAcorn(open, n.id)}>
                    <span class="dot" style:background={n.color}></span>{nestName(n)}
                  </button>
                {/each}
              </div>
            {/if}
          </div>
          <button class="btn" onclick={() => open && askDeleteAcorn(open)}><Icon name="trash" size={13} />{t("delete")}</button>
        </div>
      {:else if selected}
        <div class="bar">
          <span class="title nest-title">{nestName(selected)}</span>
          <span class="grow"></span>
          {#if !selected.system}
            <button class="btn ghost" title={active === selected.id ? t("unset_active") : t("set_active")} onclick={() => toggleActive(selected!)}>
              <Icon name="pin" size={14} />
            </button>
            <button class="btn ghost" title={t("rename")} onclick={() => startRename(selected!)}><Icon name="pencil" size={14} /></button>
          {/if}
          <button class="btn ghost" title={t("export_md")} onclick={() => exportNest(selected!)}><Icon name="down" size={14} /></button>
          {#if !selected.system}
            <button class="btn ghost" title={t("delete")} onclick={() => askDeleteNest(selected!)}><Icon name="trash" size={14} /></button>
          {/if}
        </div>
        {#if !selected.system}
          <div class="colors">
            {#each COLORS as c}
              <button
                class="swatch"
                class:on={selected.color.toLowerCase() === c.toLowerCase()}
                style:background={c}
                aria-label={t("color")}
                onclick={() => invoke("nest_color", { id: selected!.id, color: c })}
              ></button>
            {/each}
          </div>
        {/if}
        <div class="list scroll">
          {#each acorns as a (a.id)}
            <button class="acorn" onclick={() => openAcorn(a.id)}>
              <Icon name={KIND_ICON[a.kind] ?? "paste"} size={14} />
              <div class="text">
                <strong>{a.title || t(`kind_${a.kind}` as Key)}</strong>
                {#if a.preview}<span class="snippet">{a.preview}</span>{/if}
                <span class="muted small">{date(a.created_at)}</span>
              </div>
            </button>
          {:else}
            <p class="muted empty">{t("empty_kura", { key: shortcut })}</p>
          {/each}
        </div>
      {/if}
    </section>
  </div>

  {#if confirm}
    <div class="modal">
      <div class="px-box dialog">
        <p>{confirm.text}</p>
        <div class="row">
          <button class="btn" onclick={() => (confirm = null)}>{t("cancel")}</button>
          <button class="btn danger" onclick={() => { confirm?.run(); confirm = null; }}>{t("yes_delete")}</button>
        </div>
      </div>
    </div>
  {/if}
  {#if toast}<div class="toast px-box">{toast}</div>{/if}
</Window>

<style>
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px 6px;
  }

  .cols {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 150px 1fr;
    gap: 10px;
    padding: 6px 10px 10px;
  }

  .nests {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .pick {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 6px;
    border: 0;
    background: transparent;
    cursor: pointer;
    text-align: left;
  }

  .nest.sel .pick {
    background: var(--accent);
    color: var(--accent-ink);
  }

  .pick:hover {
    background: var(--sunk);
  }

  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    font-size: 11px;
    opacity: 0.75;
  }

  .dot {
    width: 9px;
    height: 9px;
    flex: none;
    box-shadow: 0 0 0 2px var(--line);
  }

  .rename {
    padding: 3px 5px;
  }

  .add {
    margin-top: 6px;
    justify-content: flex-start;
  }
  .clip {
    margin-top: auto;
    padding-top: 8px;
  }

  .acorns {
    min-height: 0;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    box-shadow:
      0 -3px 0 0 var(--line),
      0 3px 0 0 var(--line),
      -3px 0 0 0 var(--line),
      3px 0 0 0 var(--line);
    margin: 3px;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 6px 8px;
    border-bottom: 2px solid var(--sunk);
    font-size: 12px;
  }

  .nest-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .grow {
    flex: 1;
  }

  .colors {
    display: flex;
    gap: 6px;
    padding: 6px 8px;
  }

  .swatch {
    width: 14px;
    height: 14px;
    border: 0;
    cursor: pointer;
    box-shadow: 0 0 0 2px var(--line);
  }

  .swatch.on {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .list {
    flex: 1;
    min-height: 0;
    padding: 4px;
  }

  .acorn {
    width: 100%;
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 6px;
    border: 0;
    background: transparent;
    text-align: left;
    cursor: pointer;
    border-bottom: 2px dashed var(--sunk);
  }

  .acorn:hover {
    background: var(--sunk);
  }

  .acorn :global(svg) {
    margin-top: 2px;
    color: var(--accent);
  }

  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .text strong {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .snippet {
    font-size: 12px;
    color: var(--ink-soft);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .small {
    font-size: 11px;
  }

  .empty {
    padding: 20px 12px;
    text-align: center;
  }

  .detail {
    flex: 1;
    min-height: 0;
    padding: 8px 10px;
    user-select: text;
  }

  .detail h3 {
    margin: 0 0 2px;
    font-size: 15px;
  }

  .detail img {
    max-width: 100%;
    margin-top: 8px;
    box-shadow: 0 0 0 2px var(--line);
  }

  .content {
    white-space: pre-wrap;
    word-break: break-word;
    font-family: var(--font);
    font-size: 13px;
    margin: 8px 0 0;
  }

  .tools {
    display: flex;
    gap: 4px;
    padding: 6px;
    border-top: 2px solid var(--sunk);
    flex-wrap: wrap;
  }

  .move {
    position: relative;
  }

  .menu {
    position: absolute;
    bottom: 36px;
    left: 0;
    min-width: 160px;
    max-height: 200px;
    overflow-y: auto;
    z-index: 2;
  }

  .modal {
    position: fixed;
    inset: 0;
    background: rgba(36, 25, 21, 0.4);
    display: grid;
    place-items: center;
  }

  .dialog {
    padding: 14px;
    max-width: 320px;
  }

  .dialog p {
    margin: 0 0 12px;
  }

  .row {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }

  .toast {
    position: fixed;
    left: 50%;
    bottom: 20px;
    transform: translateX(-50%);
    padding: 4px 10px;
    font-size: 12px;
    max-width: 90%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
