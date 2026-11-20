# Donguri

Pixel-art squirrel that lives on the screen edge and stores context ("bellotas" in "nidos") for any AI. Full spec: [docs/SPEC.md](docs/SPEC.md) — read the relevant section before each phase.

## Stack
- Tauri v2 (Rust, `src-tauri/`) + SvelteKit in SPA mode (`adapter-static`, `ssr = false`) + Svelte 5 runes + TypeScript + Vite.
- One route per window (`src/routes/<window>/+page.svelte`). Window labels follow SPEC §11: `ardilla`, `pestana-borde`, `menu`, `bocadillo`, `selector-nido`, `captura`, `kura`, `ajustes`.

## Commands
- `npm run tauri dev` — run the app
- `npm run check` — Svelte/TS type check
- `cd src-tauri && cargo clippy` — Rust lint
- `npm run tauri build` — release build

## Rules
- Never steal focus; windows are transparent, borderless, always on top, sized exactly to content.
- ~0% CPU at rest: event-driven only, no polling loops, animations 8–12 fps and paused when idle/hidden.
- Local-only data, zero telemetry, API keys only in the OS keychain (`keyring`).
- Everything pixel-art: integer scaling only, `image-rendering: pixelated` / `shape-rendering: crispEdges`. Palette in SPEC §3. No emoji in UI chrome — pixel-art SVG icons.
- UI copy in Spanish first (ES/EN later); code and identifiers in English except domain words (bellota, nido, kura, ardilla).
- Verify crate/plugin versions and external URLs before adding them; don't trust remembered ones.

## Build phases
SPEC §13. Status: phase 0 (base) in place.
