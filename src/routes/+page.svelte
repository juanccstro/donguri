<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  // Provisional 32×32 sprite (an acorn) until the real squirrel lands in phase 1.
  // Each char maps to a palette entry from docs/SPEC.md §3; "." is transparent.
  const PALETTE: Record<string, string> = {
    o: "#3B2417", // contorno
    c: "#6B4F2A", // caperuza
    b: "#C98B3C", // bellota
    l: "#E0A060", // pelaje claro (luz)
    w: "#FFFFFF", // brillo
  };

  const ROWS = [
    "...............oo...............",
    "...............oo...............",
    "..........oooooooooooo..........",
    "........oocccccccccccccoo.......",
    ".......occcccccccccccccccco.....",
    "......occccccccccccccccccco.....",
    "......occccccccccccccccccco.....",
    "......oooooooooooooooooooooo....",
    ".......obbbbbbbbbbbbbbbbbbo.....",
    ".......oblwwbbbbbbbbbbbbbbo.....",
    ".......oblwbbbbbbbbbbbbbbbo.....",
    ".......obllbbbbbbbbbbbbbbbo.....",
    ".......oblbbbbbbbbbbbbbbbbo.....",
    "........oblbbbbbbbbbbbbbbo......",
    "........obbbbbbbbbbbbbbbbo......",
    ".........obbbbbbbbbbbbbbo.......",
    "..........obbbbbbbbbbbbo........",
    "...........obbbbbbbbbbo.........",
    "............obbbbbbbbo..........",
    ".............oobbbboo...........",
    "...............oooo.............",
  ];

  const TOP = 6; // vertical offset to center the acorn on the 32×32 canvas
  const pixels = ROWS.flatMap((row, y) =>
    [...row].flatMap((ch, x) =>
      PALETTE[ch] ? [{ x, y: y + TOP, fill: PALETTE[ch] }] : [],
    ),
  );

  function onPointerDown(e: PointerEvent) {
    if (e.button === 0) getCurrentWindow().startDragging();
  }
</script>

<svg
  viewBox="0 0 32 32"
  shape-rendering="crispEdges"
  role="img"
  aria-label="Donguri"
  onpointerdown={onPointerDown}
>
  {#each pixels as p}
    <rect x={p.x} y={p.y} width="1" height="1" fill={p.fill} />
  {/each}
</svg>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    padding: 0;
    background: transparent;
    overflow: hidden;
    user-select: none;
  }

  svg {
    display: block;
    width: 100vw;
    height: 100vh;
    cursor: grab;
  }
</style>
