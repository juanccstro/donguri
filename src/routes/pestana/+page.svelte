<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { app, loadConfig, t } from "$lib/app.svelte";
  import { renderAcorn } from "$lib/sprite";

  // Hidden mode: only the acorn's cap sticks out of the screen edge. Hover it
  // and the acorn pops out a little more; click it and the squirrel comes out
  // (she goes back into hiding once you move away and nothing of hers is open).
  let hover = $state(false);

  onMount(async () => {
    await loadConfig();
    // Whenever it shows or hides, start from the resting (cap only) pose:
    // a "pointer left" can be lost while the window is hidden.
    await listen("tab-presence", () => (hover = false));
    invoke("tab_ready");
  });

  const edge = $derived(app.config?.edge ?? "bottom");
  const acorn = $derived(renderAcorn(edge));

  function call() {
    invoke("peek", { pinned: false });
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") call();
  }
</script>

<button
  class="tab"
  class:hover
  data-edge={edge}
  onpointerenter={() => (hover = true)}
  onpointerleave={() => (hover = false)}
  onclick={call}
  onkeydown={onKey}
  oncontextmenu={(e) => e.preventDefault()}
  aria-label={t("call_squirrel")}
  title={t("call_squirrel")}
>
  <img src={acorn.url} alt="" draggable="false" />
</button>

<style>
  .tab {
    display: block;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
    position: relative;
    cursor: pointer;
    border: 0;
    padding: 0;
    background: transparent;
  }

  /* The window is 5 acorn-pixels deep; the art is 7. At rest it is pushed
     back so only the stem and cap (3 rows) show; on hover all 5 rows do. */
  img {
    position: absolute;
    image-rendering: pixelated;
    pointer-events: none;
    transition: transform 160ms steps(2);
  }
  [data-edge="bottom"] img { left: 0; top: 0; width: 100vw; transform: translateY(calc(200% / 7)); }
  [data-edge="top"] img { left: 0; bottom: 0; width: 100vw; transform: translateY(calc(-200% / 7)); }
  [data-edge="left"] img { right: 0; top: 0; height: 100vh; transform: translateX(calc(-200% / 7)); }
  [data-edge="right"] img { left: 0; top: 0; height: 100vh; transform: translateX(calc(200% / 7)); }

  .tab.hover img,
  .tab:focus-visible img {
    transform: none;
    animation: wiggle 420ms steps(3) 160ms;
  }

  @keyframes wiggle {
    33% { rotate: -6deg; }
    66% { rotate: 6deg; }
  }

  .tab:focus-visible {
    outline: none;
  }
</style>
