<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { fitWindow, loadConfig, t, type Tail } from "$lib/app.svelte";
  import Beak from "$lib/Beak.svelte";
  import Icon, { type IconName } from "$lib/Icon.svelte";

  interface Notice {
    kind: "ok" | "warn" | "answer" | "info" | "pomo" | "update" | "saved";
    text: string;
    id: number;
    acorn: number | null;
  }

  const ICON: Record<Notice["kind"], IconName> = { ok: "check", warn: "warn", answer: "answer", info: "acorn", pomo: "clock", update: "down", saved: "check" };
  // Pomodoro phase changes stay a while: they may happen while you look elsewhere.
  const LIFETIME: Record<Notice["kind"], number> = { ok: 1500, warn: 2600, info: 2600, answer: 20000, pomo: 9000, update: 15000, saved: 4500 };

  let notice = $state<Notice | null>(null);
  let tail = $state<Tail>({ edge: null, x: 0, y: 0 });
  let box: HTMLElement;
  let timer: number | undefined;

  function show(n: Notice) {
    notice = n;
    clearTimeout(timer);
    timer = setTimeout(() => invoke("popup_close"), LIFETIME[n.kind]);
  }

  // Saved: Deshacer removes the acorn again, Mover opens it in Kura.
  async function undo(e: MouseEvent) {
    e.stopPropagation();
    if (notice?.acorn == null) return;
    const id = notice.acorn;
    notice = null;
    await invoke("acorn_delete", { id });
    invoke("popup_close");
  }

  function move(e: MouseEvent) {
    e.stopPropagation();
    if (notice?.acorn == null) return;
    invoke("open_acorn", { id: notice.acorn });
    invoke("popup_close");
  }

  function click() {
    if (notice?.kind === "answer") invoke("bubble_show");
    else if (notice?.kind === "update") invoke("open_update").then(() => invoke("popup_close"));
    else invoke("popup_close");
  }

  onMount(() => {
    loadConfig();
    invoke<Notice>("current_notice").then(show);
    const off = listen<Notice>("notice", (e) => show(e.payload));
    const stop = fitWindow(box, (t) => (tail = t));
    return () => {
      off.then((f) => f());
      stop();
    };
  });
</script>

<div class="wrap" bind:this={box} data-tail={tail.edge}>
  {#if notice}
    <button class="px-box bubble kind-{notice.kind}" onclick={click}>
      <Icon name={ICON[notice.kind]} size={16} />
      <span>{notice.text}</span>
      {#if notice.kind === "saved" && notice.acorn != null}
        <span class="acts">
          <span class="act" role="button" tabindex="-1" onclick={undo} onkeydown={() => {}}>{t("undo")}</span>
          <span class="act" role="button" tabindex="-1" onclick={move} onkeydown={() => {}}>{t("move")}</span>
        </span>
      {/if}
      {#if tail.edge}
        <Beak edge={tail.edge} offset={tail.edge === "bottom" || tail.edge === "top" ? Math.max(14, tail.x - 6) : Math.max(10, tail.y - 6)} />
      {/if}
    </button>
  {/if}
</div>

<style>
  .wrap {
    display: inline-block;
    padding: 6px;
  }
  .wrap[data-tail="bottom"] { padding-bottom: 18px; }
  .wrap[data-tail="top"] { padding-top: 18px; }
  .wrap[data-tail="left"] { padding-left: 18px; }
  .wrap[data-tail="right"] { padding-right: 18px; }

  .bubble {
    position: relative;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 4px 9px;
    border: 0;
    cursor: pointer;
    white-space: nowrap;
    font-size: 13px;
    animation: pop 140ms steps(3) both;
  }

  .kind-ok :global(svg),
  .kind-saved :global(svg) { color: var(--ok); }
  .acts {
    display: flex;
    gap: 2px;
    margin-left: 4px;
  }
  .act {
    padding: 1px 6px;
    background: var(--sunk);
    font-size: 12px;
  }
  .act:hover {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .kind-warn :global(svg) { color: var(--warn); }
  .kind-pomo :global(svg) { color: var(--accent); }
  .kind-pomo { box-shadow: inset 0 0 0 2px var(--accent); }
  .kind-update :global(svg) { color: var(--ok); }
  .kind-update { box-shadow: inset 0 0 0 2px var(--ok); }

  @keyframes pop {
    from { transform: translateY(4px); opacity: 0; }
    to { transform: none; opacity: 1; }
  }
</style>
