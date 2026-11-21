<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { loadConfig, t } from "$lib/app.svelte";

  // Full-monitor overlay over a frozen screenshot: drag a rectangle, release
  // to capture it, Esc to cancel.
  let src = $state<string | null>(null);
  let start = $state<{ x: number; y: number } | null>(null);
  let end = $state<{ x: number; y: number } | null>(null);
  let finished = false;

  const rect = $derived.by(() => {
    if (!start || !end) return null;
    return {
      x: Math.min(start.x, end.x),
      y: Math.min(start.y, end.y),
      w: Math.abs(end.x - start.x),
      h: Math.abs(end.y - start.y),
    };
  });

  function finish(r: [number, number, number, number] | null) {
    if (finished) return;
    finished = true;
    invoke("capture_finish", { rect: r });
  }

  function onDown(e: PointerEvent) {
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    start = { x: e.clientX, y: e.clientY };
    end = start;
  }

  function onMove(e: PointerEvent) {
    if (start) end = { x: e.clientX, y: e.clientY };
  }

  function onUp() {
    const r = rect;
    if (!r || r.w < 4 || r.h < 4) {
      start = end = null;
      return;
    }
    const W = window.innerWidth;
    const H = window.innerHeight;
    finish([r.x / W, r.y / H, r.w / W, r.h / H]);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") finish(null);
  }

  onMount(async () => {
    loadConfig();
    src = await invoke<string | null>("capture_image");
    if (!src) return finish(null);
    await invoke("popup_ready");
    window.focus();
  });
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" onpointerdown={onDown} onpointermove={onMove} onpointerup={onUp}>
  {#if src}
    <img class="shot" {src} alt="" draggable="false" />
    <div class="dim"></div>
    {#if rect}
      <div
        class="sel"
        style:left="{rect.x}px"
        style:top="{rect.y}px"
        style:width="{rect.w}px"
        style:height="{rect.h}px"
        style:background-image="url({src})"
        style:background-position="-{rect.x}px -{rect.y}px"
      ></div>
      <div class="size px-box" style:left="{rect.x}px" style:top="{Math.max(0, rect.y - 30)}px">
        {Math.round(rect.w * devicePixelRatio)} × {Math.round(rect.h * devicePixelRatio)}
      </div>
    {/if}
    {#if !start}
      <div class="hint px-box">{t("capture_hint")}</div>
    {/if}
  {/if}
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    cursor: crosshair;
  }

  .shot {
    position: absolute;
    inset: 0;
    width: 100vw;
    height: 100vh;
    pointer-events: none;
  }

  .dim {
    position: absolute;
    inset: 0;
    background: rgba(36, 25, 21, 0.55);
    pointer-events: none;
  }

  .sel {
    position: absolute;
    background-size: 100vw 100vh;
    outline: 2px dashed var(--terracotta-light);
    pointer-events: none;
  }

  .size,
  .hint {
    position: absolute;
    padding: 2px 8px;
    font-size: 12px;
    pointer-events: none;
  }

  .hint {
    left: 50%;
    top: 40px;
    transform: translateX(-50%);
  }
</style>
