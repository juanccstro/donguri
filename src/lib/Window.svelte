<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, type Snippet } from "svelte";
  import { closeSelf, t } from "./app.svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  let { title, icon, children }: { title: string; icon: IconName; children: Snippet } = $props();

  // Shown once the page has rendered (see popups.rs).
  onMount(() => {
    requestAnimationFrame(() => invoke("popup_ready"));
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && !(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) {
      closeSelf();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="frame px-box">
  <header data-tauri-drag-region>
    <span class="icon" data-tauri-drag-region><Icon name={icon} size={16} /></span>
    <span class="title" data-tauri-drag-region>{title}</span>
    <span class="grow" data-tauri-drag-region></span>
    <button class="btn ghost" title={t("close")} onclick={() => closeSelf()}><Icon name="close" size={14} /></button>
  </header>
  <div class="body">
    {@render children()}
  </div>
</div>

<style>
  .frame {
    position: fixed;
    inset: 4px 4px 8px 4px;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 6px 6px 10px;
    background: var(--accent);
    color: var(--accent-ink);
    cursor: grab;
  }

  header .btn {
    color: inherit;
  }

  header .btn:hover {
    background: var(--terracotta-dark);
  }

  .icon {
    display: flex;
  }

  .grow {
    flex: 1;
    align-self: stretch;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
</style>
