<script lang="ts">
  import type { Edge } from "./sprite";

  // The comic bubble's "beak": a stepped pixel triangle with an outline that
  // points at the squirrel. `edge` is the screen edge she sits on, so the beak
  // sticks out of the bubble's side that faces her.
  let { edge, offset }: { edge: Edge; offset: number } = $props();

  const U = 3; // one UI pixel
  // Drawn pointing right: L = outline, P = panel.
  const RIGHT = ["L...", "PL..", "PPL.", "PPPL", "PPL.", "PL..", "L..."];

  function rotate(grid: string[], turns: number): string[] {
    let g = grid.map((r) => [...r]);
    for (let i = 0; i < turns; i++) g = g[0].map((_, x) => g.map((r) => r[x]).reverse());
    return g.map((r) => r.join(""));
  }

  // right → 0 turns, bottom (points down) → 1, left → 2, top → 3
  const grid = $derived(rotate(RIGHT, { right: 0, bottom: 1, left: 2, top: 3 }[edge]));
  const w = $derived(grid[0].length * U);
  const h = $derived(grid.length * U);
  const style = $derived(
    {
      right: `left:100%;top:${offset - h / 2}px;margin-left:-${U}px`,
      left: `right:100%;top:${offset - h / 2}px;margin-right:-${U}px`,
      bottom: `top:100%;left:${offset - w / 2}px;margin-top:-${U}px`,
      top: `bottom:100%;left:${offset - w / 2}px;margin-bottom:-${U}px`,
    }[edge],
  );
</script>

<svg class="beak" width={w} height={h} viewBox="0 0 {w} {h}" shape-rendering="crispEdges" {style} aria-hidden="true">
  {#each grid as row, y}
    {#each row as ch, x}
      {#if ch !== "."}
        <rect x={x * U} y={y * U} width={U} height={U} fill={ch === "L" ? "var(--line)" : "var(--panel)"} />
      {/if}
    {/each}
  {/each}
</svg>

<style>
  .beak {
    position: absolute;
    pointer-events: none;
    display: block;
  }
</style>
