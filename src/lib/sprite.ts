// Squirrel sprites: flat, outline-free 16×16 pixel art in the terracotta family
// of Claude's own pixel icons. All art lives in sprites.json (also read by
// scripts/preview-sprites.py); this module turns it into image URLs once.

import data from "./sprites.json";

export const ART = data.size;

export type Frame = keyof typeof data.frames;
export type Palette = keyof typeof data.palettes;
export type Accessory = keyof typeof data.accessories | "ninguno";
export type Edge = "bottom" | "top" | "left" | "right";

export const PALETTES = Object.keys(data.palettes) as Palette[];
export const ACCESSORIES = ["ninguno", ...Object.keys(data.accessories)] as Accessory[];

type Pixel = [number, number, string];
interface Layer {
  colors?: Record<string, string>;
  pixels: Pixel[];
}
interface FrameDef {
  rows?: Record<string, string>;
  pixels?: Pixel[];
  shift?: { from: number; to: number; dx: number };
}

export interface Look {
  palette: Palette;
  accessory: Accessory;
  masked: boolean;
}

function grid(rows: string[]): string[][] {
  return rows.map((r) => [...r]);
}

function paint(rows: string[][], colors: Record<string, string>): string {
  const canvas = document.createElement("canvas");
  canvas.width = rows[0].length;
  canvas.height = rows.length;
  const ctx = canvas.getContext("2d")!;
  rows.forEach((row, y) =>
    row.forEach((ch, x) => {
      const color = colors[ch];
      if (!color) return;
      ctx.fillStyle = color;
      ctx.fillRect(x, y, 1, 1);
    }),
  );
  return canvas.toDataURL("image/png");
}

function applyLayer(rows: string[][], colors: Record<string, string>, layer: Layer) {
  Object.assign(colors, layer.colors ?? {});
  for (const [x, y, ch] of layer.pixels) rows[y][x] = ch;
}

function build(name: Frame, look: Look): string {
  const def = data.frames[name] as unknown as FrameDef;
  const rows = data.base.map((r, y) => [...(def.rows?.[String(y)] ?? r)]);
  for (const [x, y, ch] of def.pixels ?? []) rows[y][x] = ch;

  const colors: Record<string, string> = { ...data.common, ...data.palettes[look.palette] };
  if (look.palette === "ezo") applyLayer(rows, colors, data.overlays.ezo as unknown as Layer);
  if (look.accessory !== "ninguno") applyLayer(rows, colors, data.accessories[look.accessory] as unknown as Layer);
  if (look.masked) applyLayer(rows, colors, data.overlays.mask as unknown as Layer);

  if (def.shift) {
    const { from, to, dx } = def.shift;
    for (let y = from; y <= to; y++) {
      const r = rows[y];
      rows[y] = dx > 0 ? [...Array(dx).fill("."), ...r.slice(0, ART - dx)] : [...r.slice(-dx), ...Array(-dx).fill(".")];
    }
  }
  return paint(rows, colors);
}

/** Every frame for one look; switching frames is just swapping an image URL. */
export function renderFrames(look: Look): Record<Frame, string> {
  return Object.fromEntries((Object.keys(data.frames) as Frame[]).map((f) => [f, build(f, look)])) as Record<
    Frame,
    string
  >;
}

/** One still frame, e.g. for previews in the settings window. */
export function renderStill(look: Look, frame: Frame = "base"): string {
  return build(frame, look);
}

/**
 * The acorn that sticks out of the screen edge in hidden mode, rotated so the
 * cap points away from the edge. Returns the image and its size in art pixels.
 */
export function renderAcorn(edge: Edge): { url: string; w: number; h: number } {
  let rows = grid(data.acorn);
  const rotateCw = (g: string[][]) => g[0].map((_, x) => g.map((r) => r[x]).reverse());
  // Art is drawn cap-up (for the bottom edge).
  if (edge === "top") rows = rotateCw(rotateCw(rows));
  if (edge === "left") rows = rotateCw(rows);
  if (edge === "right") rows = rotateCw(rotateCw(rotateCw(rows)));
  const colors = { ...data.common, ...data.palettes.rojiza };
  return { url: paint(rows, colors), w: rows[0].length, h: rows.length };
}

export const ACORN_SIZE = { long: data.acorn[0].length, short: data.acorn.length };
