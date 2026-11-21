<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { app, closeSelf, loadConfig, t } from "$lib/app.svelte";
  import Icon from "$lib/Icon.svelte";
  import Window from "$lib/Window.svelte";
  import { renderStill } from "$lib/sprite";

  // "Salir" never closes silently: it explains how to bring Donguri back and
  // offers hiding instead, plus starting with Windows.
  let autostart = $state(false);
  let toggleKey = $state("Alt+Shift+B");

  onMount(async () => {
    await loadConfig();
    autostart = await invoke<boolean>("autostart_get");
    const s = await invoke<{ shortcuts: { toggle: string } }>("get_settings");
    toggleKey = s.shortcuts.toggle;
  });

  async function setAutostart(on: boolean) {
    autostart = await invoke<boolean>("autostart_set", { on }).catch(() => autostart);
  }

  async function hideInstead() {
    await invoke("menu_action", { action: "hide" });
    closeSelf();
  }

  const look = $derived({
    palette: app.config?.palette ?? "rojiza",
    accessory: app.config?.accessory ?? "ninguno",
    masked: false,
  });
</script>

<Window title={t("quit_title")} icon="close">
  <div class="page">
    <div class="row top">
      <img src={renderStill(look, "sleep")} alt="" />
      <p>{t("quit_body")}</p>
    </div>
    <div class="how px-sunk">
      <strong>{t("quit_how")}</strong>
      <ol>
        <li>{t("quit_how1")}</li>
        <li>{t("quit_how2")}</li>
      </ol>
      <label class="check">
        <input type="checkbox" checked={autostart} onchange={(e) => setAutostart(e.currentTarget.checked)} />
        {t("autostart")}
      </label>
    </div>
    <p class="muted small">{t("quit_hide_instead", { key: toggleKey })}</p>
    <div class="actions">
      <button class="btn primary" onclick={hideInstead}><Icon name="minus" size={13} />{t("quit_hide")}</button>
      <button class="btn danger" onclick={() => invoke("quit_app")}><Icon name="close" size={13} />{t("quit_confirm")}</button>
    </div>
  </div>
</Window>

<style>
  .page {
    padding: 12px 16px 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .row.top {
    display: flex;
    gap: 12px;
    align-items: center;
  }

  .row.top img {
    width: 64px;
    height: 64px;
    image-rendering: pixelated;
    flex: none;
  }

  p {
    margin: 0;
  }

  .how {
    padding: 8px 10px;
  }

  .how ol {
    margin: 4px 0 6px;
    padding-left: 18px;
  }

  .check {
    display: flex;
    gap: 8px;
    align-items: center;
    cursor: pointer;
  }

  input[type="checkbox"] {
    accent-color: var(--accent);
  }

  .small {
    font-size: 12px;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
</style>
