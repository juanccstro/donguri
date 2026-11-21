<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onDestroy, onMount } from "svelte";
  import { app, loadConfig, t } from "$lib/app.svelte";
  import { renderFrames, type Edge, type Frame } from "$lib/sprite";
  import { clock, leftNow, type PomoStatus } from "$lib/pomodoro";

  /** She naps once the user has left the computer this long. */
  const NAP_AFTER = 5 * 60_000;
  const FADE_AFTER = 5_000;
  const UNPEEK_AFTER = 2_000;
  const DRAG_THRESHOLD = 4;

  // Frames are re-rendered only when the look changes (colour, accessory, mask).
  const lookKey = $derived(
    app.config ? `${app.config.palette}|${app.config.accessory}|${app.config.masked}` : "",
  );
  const frames = $derived(
    app.config
      ? renderFrames({ palette: app.config.palette, accessory: app.config.accessory, masked: app.config.masked })
      : null,
  );

  let frame = $state<Frame>("base");
  let edge = $state<Edge>("bottom");
  let visible = $state(false);
  let peeked = $state(false);
  let faded = $state(false);
  let dragging = $state(false);
  let flipped = $state(false);
  let hop = $state(false);
  let peekIn = $state(false);
  let dropping = $state(false);
  let asleep = $state(false);
  let hovering = false;
  let lastInteraction = Date.now();

  const mode = $derived(app.config?.mode ?? "viva");

  // ---- Pomodoro: while focusing her acorn fills with green (5 steps) and the
  // next step ticks; during breaks she naps. One timer per step, no polling.
  let pomo = $state<PomoStatus | null>(null);
  let pomoLevel = $state(1);
  let levelTimer: number | undefined;
  const focusing = $derived(!!pomo?.active && pomo.phase === "focus");
  const resting = $derived(!!pomo?.active && pomo.phase !== "focus");
  const ticking = $derived(focusing && !pomo?.paused && pomoLevel < 5 && visible && !dragging);

  function idleFrame(): Frame {
    if (focusing) return `pomo${pomoLevel}` as Frame;
    if (resting) return "rest1";
    return asleep ? "sleep" : "base";
  }

  function scheduleLevel() {
    clearTimeout(levelTimer);
    if (!pomo?.active || pomo.phase !== "focus" || !pomo.total) return;
    const step = pomo.total / 5;
    const elapsed = pomo.total - leftNow(pomo);
    pomoLevel = Math.min(5, 1 + Math.floor(elapsed / step));
    if (!loop && !busy && !dragging) frame = idleFrame();
    if (!pomo.paused && pomoLevel < 5) levelTimer = setTimeout(scheduleLevel, pomoLevel * step - elapsed + 50);
  }

  function onPomodoro(s: PomoStatus) {
    const was = pomo?.active;
    pomo = s;
    if (s.active) {
      asleep = false;
      stopAmbient(); // no idle tricks while focusing or resting
      scheduleLevel();
    } else {
      clearTimeout(levelTimer);
      if (was) scheduleAmbient();
    }
    if (!loop && !busy && !dragging) frame = idleFrame();
  }

  let tip = $state("");
  function updateTip() {
    if (!pomo?.active) return (tip = t("squirrel_tip"));
    const phase = t(pomo.phase === "focus" ? "pomo_focus" : pomo.phase === "long" ? "pomo_long" : "pomo_break");
    tip = `Pomodoro · ${phase} · ${clock(leftNow(pomo))}${pomo.paused ? ` · ${t("pomo_paused")}` : ""}`;
  }
  // Lively mode, at rest: a slow, subtle breath (chest and tail tip), drawn
  // by a CSS step animation — no timers, paused whenever anything else plays.
  const breathing = $derived(mode === "viva" && visible && !asleep && !dragging && !hop && frame === "base");
  const fadeEnabled = $derived(app.config?.fade ?? true);

  // One-shot timers only: at rest the page schedules a single blink.
  let ambientTimer: number | undefined;
  let fadeTimer: number | undefined;
  let unpeekTimer: number | undefined;
  /** Bumped to cancel whatever sequence is playing. */
  let generation = 0;
  /** A looping state driven by Rust (thinking, talking) blocks ambience. */
  let loop: "think" | "talk" | null = null;
  let busy = false;

  const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
  const rand = (min: number, max: number) => min + Math.random() * (max - min);

  const POSTURE: Record<Edge, string> = {
    bottom: "",
    top: "scaleY(-1)",
    left: "rotate(90deg)",
    // Mirrored so she climbs head-up on both walls.
    right: "rotate(-90deg) scaleX(-1)",
  };
  // Join only non-empty parts: a whitespace-only transform is invalid CSS and
  // the browser would silently keep the previous rotation.
  const transform = $derived(
    [dragging ? "" : POSTURE[edge], hop ? "translateY(-6.25%)" : "", flipped ? "scaleX(-1)" : ""]
      .filter(Boolean)
      .join(" ") || "none",
  );

  async function play(seq: [Frame, number][]) {
    const gen = ++generation;
    for (const [f, ms] of seq) {
      if (gen !== generation || !visible) return false;
      frame = f;
      await wait(ms);
    }
    if (gen === generation) frame = idleFrame();
    return gen === generation;
  }

  async function runLoop(kind: "think" | "talk") {
    loop = kind;
    const gen = ++generation;
    const seq: Frame[] = kind === "think" ? ["think1", "think2", "think3", "think2"] : ["talk", "base"];
    for (let i = 0; gen === generation && loop === kind; i++) {
      frame = seq[i % seq.length];
      await wait(kind === "think" ? 260 : 150);
    }
  }

  function stopLoop() {
    loop = null;
    generation++;
    frame = idleFrame();
  }

  /** Animations requested by Rust. */
  async function animate(anim: string) {
    wake();
    switch (anim) {
      case "think":
      case "talk":
        runLoop(anim);
        return;
      case "idle":
        stopLoop();
        return;
      case "chew":
        loop = null;
        await play([
          ["chew1", 120], ["chew2", 120], ["chew1", 120], ["chew2", 120], ["chew1", 120], ["chew2", 120],
          ["success", 650],
        ]);
        return;
      case "success":
        loop = null;
        await play([["success", 700]]);
        return;
      case "error":
        loop = null;
        await play([["errorLeft", 110], ["errorRight", 110], ["errorLeft", 110], ["errorRight", 110], ["base", 80]]);
        return;
      case "alert":
        if (!loop) await play([["alert", 650]]);
        return;
      case "pomo-break":
        // Focus is over: a happy hop, then she lies down to rest.
        loop = null;
        await play([["success", 450], ["crouch", 110], ["leap", 160], ["base", 100], ["success", 450], ["blink", 300]]);
        return;
      case "pomo-focus":
        // Back to work: wakes up, stretches, ears up.
        loop = null;
        await play([["alert", 350], ["leap", 160], ["crouch", 120], ["alert", 300]]);
        return;
      case "pomo-done":
        loop = null;
        await play([["success", 400], ["crouch", 110], ["leap", 160], ["success", 400], ["crouch", 110], ["leap", 160], ["success", 500]]);
        return;
    }
  }

  function stopAmbient() {
    clearTimeout(ambientTimer);
    ambientTimer = undefined;
  }

  function scheduleAmbient() {
    stopAmbient();
    if (!visible) return;
    // Asleep she only checks now and then whether the user is back.
    const ms = asleep ? 15_000 : mode === "viva" ? rand(3_500, 7_000) : rand(6_000, 10_000);
    ambientTimer = setTimeout(ambient, ms);
  }

  async function ambient() {
    if (pomo?.active) return; // the Pomodoro animation is on instead
    if (busy || dragging || loop) return scheduleAmbient();
    const idle = await invoke<number>("user_idle_ms").catch(() => 0);
    if (asleep) {
      if (idle < NAP_AFTER) {
        // The user is back: wake up with a stretch.
        asleep = false;
        await play([["leap", 180], ["crouch", 120], ["base", 100], ["blink", 120]]);
      }
      return scheduleAmbient();
    }
    if (idle > NAP_AFTER && Date.now() - lastInteraction > NAP_AFTER) {
      generation++;
      asleep = true;
      frame = "sleep";
      return scheduleAmbient();
    }
    busy = true;
    if (mode === "quieta") await play([["blink", 140]]);
    else await lively();
    busy = false;
    scheduleAmbient();
  }

  type Act = "blink" | "blink2" | "tail" | "look" | "ear" | "nibble" | "drop" | "walk";
  // Relative weights of what she does on her own in lively mode.
  const ACTS: [Act, number][] = [
    ["blink", 20], ["blink2", 8], ["tail", 18], ["look", 12], ["ear", 8], ["nibble", 8], ["drop", 11], ["walk", 15],
  ];
  let lastAct: Act | null = null;

  function pickAct(): Act {
    const options = ACTS.filter(([a]) => a !== lastAct);
    let roll = Math.random() * options.reduce((n, [, w]) => n + w, 0);
    for (const [a, w] of options) if ((roll -= w) < 0) return a;
    return "blink";
  }

  async function lively() {
    const act = pickAct();
    lastAct = act;
    switch (act) {
      case "blink":
        await play([["blink", 140]]);
        break;
      case "blink2":
        await play([["blink", 110], ["base", 130], ["blink", 110]]);
        break;
      case "tail":
        await play([
          ["tailLeft", 140], ["base", 120], ["tailRight", 140], ["base", 120],
          ["tailLeft", 140], ["base", 120], ["blink", 120],
        ]);
        break;
      case "look":
        // Turns to look the other way for a moment.
        flipped = true;
        await play([["base", 900], ["blink", 120], ["base", 700]]);
        flipped = false;
        break;
      case "ear":
        await play([["alert", 110], ["base", 110], ["alert", 110], ["base", 260], ["blink", 120]]);
        break;
      case "nibble":
        await play([["chew1", 130], ["chew2", 130], ["chew1", 130], ["chew2", 130], ["chew1", 130], ["base", 120]]);
        break;
      case "drop":
        // Drops her acorn, looks at it, bends down and picks it up again.
        await play([
          ["dropFall", 90], ["dropLanded", 420], ["dropLook", 520], ["stoop", 380],
          ["dropLook", 120], ["success", 420], ["crouch", 120], ["base", 100], ["blink", 120],
        ]);
        break;
      case "walk":
        await walk();
        break;
    }
  }

  /** A short hop along the edge: crouch, leap (body lifts), land. */
  async function walk() {
    const dir = Math.random() < 0.5 ? -1 : 1;
    // Every posture leaves her facing toward decreasing x/y along the edge;
    // walking the other way mirrors her.
    flipped = dir > 0;
    const hops = Math.round(rand(3, 6));
    const gen = ++generation;
    outer: for (let i = 0; i < hops; i++) {
      for (const [f, lift, steps, ms] of [
        ["crouch", false, 0, 90],
        ["leap", true, 2, 90],
        ["base", false, 1, 90],
      ] as [Frame, boolean, number, number][]) {
        if (gen !== generation || !visible || dragging) break outer;
        frame = f;
        hop = lift;
        for (let s = 0; s < steps; s++) {
          if (!(await invoke<boolean>("walk_step", { dir }))) break outer;
        }
        await wait(ms);
      }
    }
    frame = idleFrame();
    hop = false;
    flipped = false;
    await invoke("walk_end");
  }

  function wake() {
    lastInteraction = Date.now();
    if (asleep) {
      asleep = false;
      frame = idleFrame();
      scheduleAmbient();
    }
  }

  function armFade() {
    clearTimeout(fadeTimer);
    faded = false;
    if (fadeEnabled && !hovering) fadeTimer = setTimeout(() => (faded = true), FADE_AFTER);
  }

  function armUnpeek() {
    clearTimeout(unpeekTimer);
    if (mode === "escondida" && peeked && !hovering && !dragging) {
      unpeekTimer = setTimeout(() => invoke("unpeek"), UNPEEK_AFTER);
    }
  }

  function onEnter() {
    updateTip();
    hovering = true;
    wake();
    clearTimeout(fadeTimer);
    clearTimeout(unpeekTimer);
    faded = false;
    invoke("hover_paste", { on: true });
  }

  function onLeave() {
    hovering = false;
    invoke("hover_paste", { on: false });
    armFade();
    armUnpeek();
  }

  // ---- drag: Rust tracks the real cursor so DPI and monitors stay consistent
  let down: { x: number; y: number } | null = null;
  let moveInFlight = false;
  let movePending = false;

  async function sendMove() {
    if (moveInFlight) {
      movePending = true;
      return;
    }
    moveInFlight = true;
    do {
      movePending = false;
      await invoke("drag_move");
    } while (movePending);
    moveInFlight = false;
  }

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    down = { x: e.screenX, y: e.screenY };
    // Record the grab point now, so even a fast flick moves the full distance.
    invoke("drag_start");
    wake();
  }

  function onPointerMove(e: PointerEvent) {
    if (!down) return;
    if (!dragging) {
      if (Math.hypot(e.screenX - down.x, e.screenY - down.y) < DRAG_THRESHOLD) return;
      dragging = true;
      generation++;
      loop = null;
      frame = "alert";
    }
    sendMove();
  }

  async function onPointerUp() {
    if (!down) return;
    down = null;
    if (dragging) {
      while (moveInFlight) await wait(10);
      edge = await invoke<Edge>("drag_end");
      dragging = false;
      frame = idleFrame();
      armUnpeek();
    } else {
      invoke("drag_cancel");
      invoke("hover_paste", { on: false });
      invoke("open_menu");
    }
  }

  function onContextMenu(e: MouseEvent) {
    e.preventDefault();
    hovering = false;
    wake();
    invoke("show_context_menu");
  }

  // ---- drops: text or files fed straight to her
  function onDragOver(e: DragEvent) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
    if (!dropping) {
      dropping = true;
      wake();
      frame = "alert";
    }
  }

  function onDragLeave() {
    dropping = false;
    frame = idleFrame();
  }

  async function onDrop(e: DragEvent) {
    e.preventDefault();
    dropping = false;
    const dt = e.dataTransfer;
    if (!dt) return;
    if (dt.files.length) {
      for (const file of Array.from(dt.files)) {
        const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
        await invoke("save_file", { name: file.name, bytes });
      }
      return;
    }
    const text = dt.getData("text/plain") || dt.getData("text/uri-list");
    if (text) invoke("save_text", { text, source: "arrastrar" });
  }

  const unlisteners: (() => void)[] = [];

  onMount(async () => {
    const cfg = await loadConfig();
    edge = cfg.edge;
    unlisteners.push(
      await listen<Edge>("posture", (e) => (edge = e.payload)),
      await listen<string>("anim", (e) => animate(e.payload)),
      await listen("peek-released", () => armUnpeek()),
      await listen<PomoStatus>("pomodoro", (e) => onPomodoro(e.payload)),
      await listen<{ visible: boolean; peeked: boolean }>("presence", (e) => {
        const wasPeeked = peeked;
        visible = e.payload.visible;
        peeked = e.payload.peeked;
        if (!visible) {
          stopAmbient();
          clearTimeout(fadeTimer);
          return;
        }
        if (peeked && !wasPeeked) {
          wake();
          peekIn = false;
          requestAnimationFrame(() => (peekIn = true));
        }
        armFade();
        armUnpeek();
        scheduleAmbient();
      }),
    );
    invoke("squirrel_ready");
    invoke<PomoStatus>("pomodoro_status").then(onPomodoro);
  });

  $effect(() => {
    // Re-arm the fade when the setting changes.
    void fadeEnabled;
    armFade();
  });

  onDestroy(() => {
    unlisteners.forEach((u) => u());
    stopAmbient();
    clearTimeout(fadeTimer);
    clearTimeout(unpeekTimer);
    clearTimeout(levelTimer);
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="stage"
  class:faded
  class:dropping
  class:peek-in={peekIn && mode === "escondida"}
  data-edge={edge}
  title={dragging ? undefined : tip || t("squirrel_tip")}
  onpointerenter={onEnter}
  onpointerleave={onLeave}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  oncontextmenu={onContextMenu}
  ondragover={onDragOver}
  ondragleave={onDragLeave}
  ondrop={onDrop}
  onanimationend={() => (peekIn = false)}
>
  {#if frames}
    <img src={frames[frame]} alt="Donguri" draggable="false" style:transform />
    {#if breathing}
      <img class="breath" src={frames.breathe} alt="" draggable="false" style:transform />
    {/if}
    {#if ticking && frame === idleFrame()}
      <img class="tick" src={frames[`pomo${pomoLevel + 1}` as Frame]} alt="" draggable="false" style:transform />
    {/if}
    {#if resting && visible && frame === "rest1"}
      <img class="snooze" src={frames.rest2} alt="" draggable="false" style:transform />
    {/if}
  {/if}
</div>

<style>
  .stage {
    width: 100vw;
    height: 100vh;
    cursor: grab;
    transition: opacity 400ms steps(4);
  }

  .stage:active {
    cursor: grabbing;
  }

  .stage.faded {
    opacity: 0.6;
  }

  .stage {
    position: relative;
  }

  img {
    display: block;
    width: 100%;
    height: 100%;
    image-rendering: pixelated;
    pointer-events: none;
  }

  .breath,
  .tick,
  .snooze {
    position: absolute;
    inset: 0;
    opacity: 0;
    animation: breathe 3.2s steps(1) infinite;
  }

  /* the next green step of the acorn blinks: one second on, one off */
  .tick {
    animation-duration: 2s;
  }

  /* the "z" drifts while she naps */
  .snooze {
    animation-duration: 2.4s;
  }

  @keyframes breathe {
    50% {
      opacity: 1;
    }
  }

  /* "asomarse": slide out from behind the edge in 5 pixel-art steps */
  .peek-in {
    animation: peek 420ms steps(5) both;
  }
  [data-edge="bottom"].peek-in { --from: translateY(100%); }
  [data-edge="top"].peek-in { --from: translateY(-100%); }
  [data-edge="left"].peek-in { --from: translateX(-100%); }
  [data-edge="right"].peek-in { --from: translateX(100%); }

  @keyframes peek {
    from {
      transform: var(--from);
    }
    to {
      transform: none;
    }
  }
</style>
