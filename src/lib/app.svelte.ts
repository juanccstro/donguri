// Shared, reactive app state for every window: the config Rust broadcasts,
// the UI language, and helpers for popups that size themselves to content.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { Accessory, Edge, Palette } from "./sprite";
import { dict, type Key } from "./i18n";

export type Mode = "quieta" | "viva" | "escondida";

export interface Config {
  mode: Mode;
  fade: boolean;
  edge: Edge;
  palette: Palette;
  accessory: Accessory;
  masked: boolean;
  lang: "es" | "en";
  scale: number;
}

export const app = $state<{ config: Config | null }>({ config: null });

let started = false;

/** Loads the config once and keeps it fresh. */
export async function loadConfig(): Promise<Config> {
  if (!started) {
    started = true;
    await listen<Config>("config", (e) => {
      app.config = e.payload;
    });
  }
  app.config = await invoke<Config>("get_config");
  document.documentElement.lang = app.config.lang;
  return app.config;
}

export function t(key: Key, vars?: Record<string, string | number>): string {
  const lang = app.config?.lang ?? "es";
  let s: string = dict[lang][key] ?? dict.es[key] ?? key;
  // "singular|plural": picked by {n}.
  if (s.includes("|") && vars && "n" in vars) s = s.split("|")[Number(vars.n) === 1 ? 0 : 1];
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}

export interface Tail {
  edge: Edge | null;
  x: number;
  y: number;
}

/**
 * Keeps a popup window exactly the size of `el` (transparent pixels still
 * catch clicks, so the window must not be bigger than what it shows).
 */
export function fitWindow(el: HTMLElement, onTail?: (t: Tail) => void): () => void {
  let last = "";
  const fit = async () => {
    const r = el.getBoundingClientRect();
    const w = Math.ceil(r.width);
    const h = Math.ceil(r.height);
    const key = `${w}x${h}`;
    if (key === last || w === 0 || h === 0) return;
    last = key;
    const tail = await invoke<Tail>("popup_fit", { w, h });
    onTail?.(tail);
  };
  const ro = new ResizeObserver(() => fit());
  ro.observe(el);
  fit();
  return () => ro.disconnect();
}

export function closeSelf() {
  return invoke("popup_close");
}

export const self = () => getCurrentWebviewWindow();
