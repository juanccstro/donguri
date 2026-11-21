<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";
  import { app, loadConfig, t, type Tail } from "$lib/app.svelte";
  import Icon, { type IconName } from "$lib/Icon.svelte";
  import type { Key } from "$lib/i18n";
  import { clock, leftNow, type PomoStatus } from "$lib/pomodoro";

  // Radial menu around the squirrel. The window is kept inside the screen
  // (popups.rs), the arc opens toward the most room, and every option shows
  // its label, clamped so it is always readable. It never takes focus: it
  // closes when the pointer leaves it for a moment, or after a while untouched.
  const SIZE = 460;
  const RADIUS = 88;
  const BUTTON = 44;
  const PAD = 4;

  const items: { id: string; icon: IconName; label: Key }[] = [
    { id: "ask", icon: "ask", label: "m_ask" },
    { id: "kura", icon: "nest", label: "m_kura" },
    { id: "context", icon: "paste", label: "m_context" },
    { id: "pomodoro", icon: "clock", label: "m_pomodoro" },
    { id: "settings", icon: "gear", label: "m_settings" },
  ];

  // While a Pomodoro runs, its option shows the phase and the time left.
  let pomo = $state<PomoStatus | null>(null);
  let now = $state(Date.now());
  let ticker: number | undefined;

  function labelOf(id: string, key: Key): string {
    if (id !== "pomodoro" || !pomo?.active) return t(key);
    void now;
    const phase = t(pomo.phase === "focus" ? "pomo_focus" : pomo.phase === "long" ? "pomo_long" : "pomo_break");
    return `${phase} ${clock(leftNow(pomo))}${pomo.paused ? ` · ${t("pomo_paused")}` : ""}`;
  }

  let tail = $state<Tail & { space?: number[] }>({ edge: "bottom", x: SIZE / 2, y: SIZE / 2 });
  let ready = $state(false);
  let widths = $state<number[]>(items.map(() => 90));
  let leaveTimer: number | undefined;
  let idleTimer: number | undefined;

  const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), Math.max(lo, hi));

  // Angles in degrees, 0 = right, 90 = down. The arc's centre points to the
  // side of the screen with the most room; it narrows in corners.
  type Spot = { x: number; y: number; lx: number; ly: number };
  type R = { x: number; y: number; w: number; h: number };
  const LABEL_H = 22;
  const hit = (a: R, b: R) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
  const inside = (r: R) => r.x >= PAD && r.y >= PAD && r.x + r.w <= SIZE - PAD && r.y + r.h <= SIZE - PAD;

  const layout = $derived.by<Spot[]>(() => {
    const [left, right, up, down] = tail.space ?? [999, 999, 999, 0];
    const sq = 16 * (app.config?.scale ?? 4);
    const room = RADIUS + BUTTON + 90;
    const cramped = [left, right, up, down].filter((d) => d < room).length >= 2;

    // Corner: a tidy column beside her, labels toward the screen centre.
    if (cramped) {
      const vy = up >= down ? -1 : 1;
      const labelsRight = right >= left;
      const x = tail.x + (labelsRight ? 1 : -1) * 4;
      const first = sq / 2 + 10 + BUTTON / 2;
      const ys = items.map((_, i) => tail.y + vy * (first + i * (BUTTON + 8)));
      ys.sort((p, q) => p - q); // first option on top
      return ys.map((y, i) => {
        const w = widths[i];
        const cy = clamp(y, BUTTON / 2 + PAD, SIZE - BUTTON / 2 - PAD);
        const cx = clamp(x, BUTTON / 2 + PAD, SIZE - BUTTON / 2 - PAD);
        const lx = labelsRight ? cx + BUTTON / 2 + 6 : cx - BUTTON / 2 - 6 - w;
        return { x: cx, y: cy, lx: clamp(lx, PAD, SIZE - w - PAD), ly: cy - LABEL_H / 2 };
      });
    }

    // Edge: a half circle toward the open side of the screen.
    const sat = (v: number) => Math.min(v, 220);
    const dx = sat(right) - sat(left);
    const dy = sat(down) - sat(up);
    const centre = (Math.atan2(dy, dx) * 180) / Math.PI;
    const spread = 150;
    const spots = items.map((_, i) => {
      const a = ((centre - spread / 2 + (spread * i) / (items.length - 1)) * Math.PI) / 180;
      return {
        x: clamp(tail.x + Math.cos(a) * RADIUS, BUTTON / 2 + PAD, SIZE - BUTTON / 2 - PAD),
        y: clamp(tail.y + Math.sin(a) * RADIUS, BUTTON / 2 + PAD, SIZE - BUTTON / 2 - PAD),
      };
    });
    const horizontal = Math.abs(dy) >= Math.abs(dx);
    spots.sort((p, q) => (horizontal ? p.x - q.x : p.y - q.y));

    // Each label takes the first free spot: outer side, other side, above,
    // below — inside the window and clear of icons, labels and the squirrel.
    const blocked: R[] = [
      { x: tail.x - sq / 2, y: tail.y - sq / 2, w: sq, h: sq },
      ...spots.map((p) => ({ x: p.x - BUTTON / 2, y: p.y - BUTTON / 2, w: BUTTON, h: BUTTON })),
    ];
    return spots.map((p, i) => {
      const w = widths[i];
      const gap = BUTTON / 2 + 5;
      const outerRight = Math.abs(p.x - tail.x) < 8 ? p.x < SIZE / 2 : p.x > tail.x;
      const side = (r: boolean): R => ({ x: r ? p.x + gap : p.x - gap - w, y: p.y - LABEL_H / 2, w, h: LABEL_H });
      const candidates: R[] = [
        side(outerRight),
        side(!outerRight),
        { x: p.x - w / 2, y: p.y - BUTTON / 2 - 4 - LABEL_H, w, h: LABEL_H },
        { x: p.x - w / 2, y: p.y + BUTTON / 2 + 4, w, h: LABEL_H },
      ];
      const pick = candidates.find((r) => inside(r) && !blocked.some((b) => hit(r, b))) ?? candidates[0];
      blocked.push(pick);
      return { ...p, lx: clamp(pick.x, PAD, SIZE - w - PAD), ly: clamp(pick.y, PAD, SIZE - LABEL_H - PAD) };
    });
  });

  function close() {
    invoke("popup_close");
  }

  function pick(id: string) {
    invoke("menu_action", { action: id });
  }

  function onEnter() {
    clearTimeout(leaveTimer);
    clearTimeout(idleTimer);
  }

  function onLeave() {
    leaveTimer = setTimeout(close, 1100);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }

  // The window is built once at startup and only hidden and shown again,
  // so it appears the moment the squirrel is clicked.
  async function show() {
    clearTimeout(leaveTimer);
    clearTimeout(idleTimer);
    ready = false;
    pomo = await invoke<PomoStatus>("pomodoro_status");
    clearInterval(ticker);
    if (pomo.active) ticker = setInterval(() => (now = Date.now()), 1000); // only while shown
    tail = await invoke("popup_tail");
    ready = true; // re-creates the buttons: their pop plays again
    await tick();
    // Two frames: the page paints before the window shows (no black flash).
    requestAnimationFrame(() => requestAnimationFrame(() => invoke("popup_ready")));
    idleTimer = setTimeout(close, 6000);
  }

  function hide() {
    clearInterval(ticker);
    clearTimeout(leaveTimer);
    clearTimeout(idleTimer);
    ready = false; // nothing stale is left to flash on the next opening
  }

  onMount(() => {
    const offs = [
      listen("menu-show", show),
      listen("menu-hide", hide),
      listen<PomoStatus>("pomodoro", (e) => (pomo = e.payload)),
    ];
    loadConfig().then(async () => {
      if (await invoke<boolean>("popup_wanted")) show();
    });
    return () => offs.forEach((o) => o.then((f) => f()));
  });
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="ring" style:width="{SIZE}px" style:height="{SIZE}px" onpointerenter={onEnter} onpointerleave={onLeave}>
  {#if ready}
    <!-- The menu window covers the squirrel: clicking her again closes it. -->
    {@const s = 16 * (app.config?.scale ?? 4)}
    <button
      class="centre"
      style:left="{tail.x - s / 2}px"
      style:top="{tail.y - s / 2}px"
      style:width="{s}px"
      style:height="{s}px"
      onclick={close}
      aria-label={t("close")}
      title={t("close")}
    ></button>
    {#each items as item, i}
      {@const p = layout[i]}
      <button
        class="item"
        style:left="{p.x - BUTTON / 2}px"
        style:top="{p.y - BUTTON / 2}px"
        style:animation-delay="{i * 12}ms"
        onclick={() => pick(item.id)}
        aria-label={t(item.label)}
      >
        <Icon name={item.icon} size={22} />
      </button>
      <button
        class="label px-box"
        class:live={item.id === "pomodoro" && pomo?.active}
        class:rest={item.id === "pomodoro" && pomo?.active && pomo.phase !== "focus"}
        style:left="{p.lx}px"
        style:top="{p.ly}px"
        style:animation-delay="{i * 12 + 20}ms"
        bind:offsetWidth={widths[i]}
        onclick={() => pick(item.id)}
        tabindex="-1"
      >
        {labelOf(item.id, item.label)}
      </button>
    {/each}
  {/if}
</div>

<style>
  .ring {
    position: relative;
  }

  .centre {
    position: absolute;
    border: 0;
    background: transparent;
    cursor: pointer;
  }

  .item {
    position: absolute;
    width: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    border: 0;
    padding: 0;
    cursor: pointer;
    color: var(--ink);
    background: var(--panel);
    /* stepped "pixel circle" */
    clip-path: polygon(
      30% 0, 70% 0, 70% 6%, 85% 6%, 85% 15%, 94% 15%, 94% 30%, 100% 30%, 100% 70%, 94% 70%, 94% 85%,
      85% 85%, 85% 94%, 70% 94%, 70% 100%, 30% 100%, 30% 94%, 15% 94%, 15% 85%, 6% 85%, 6% 70%, 0 70%,
      0 30%, 6% 30%, 6% 15%, 15% 15%, 15% 6%, 30% 6%
    );
    box-shadow: inset 0 0 0 3px var(--line), inset 0 -6px 0 0 var(--sunk);
    animation: pop 90ms steps(3) both;
  }

  .item:hover,
  .item:focus-visible {
    background: var(--accent);
    color: var(--accent-ink);
    box-shadow: inset 0 0 0 3px var(--line), inset 0 -6px 0 0 var(--terracotta-dark);
    outline: none;
  }

  .label {
    position: absolute;
    margin: 0;
    padding: 1px 7px;
    font-size: 12px;
    white-space: nowrap;
    border: 0;
    cursor: pointer;
    color: var(--ink);
    animation: pop 90ms steps(3) both;
  }

  .label.live {
    background: var(--accent);
    color: var(--accent-ink);
    font-variant-numeric: tabular-nums;
  }

  .label.live.rest {
    background: var(--ok);
  }

  .item:hover + .label,
  .label:hover {
    background: var(--accent);
    color: var(--accent-ink);
  }

  @keyframes pop {
    from {
      transform: scale(0.4);
      opacity: 0;
    }
    to {
      transform: none;
      opacity: 1;
    }
  }
</style>
