// Shared Pomodoro types and helpers (panel, menu chip, squirrel).

export type Phase = "focus" | "break" | "long";

export interface Plan {
  focus: number;
  short: number;
  long: number;
  rounds: number;
}

export interface PomoStatus {
  active: boolean;
  phase: Phase;
  round: number;
  rounds: number;
  plan: Plan | null;
  /** Unix ms when the phase ends (0 while paused or idle). */
  ends_at: number;
  /** Milliseconds left when the status was sent. */
  left: number;
  /** Full phase length in ms. */
  total: number;
  paused: boolean;
}

/**
 * Presets. There is no single "best" interval in controlled studies; these
 * are the three most used, each with breaks of about a fifth of the work:
 * - 25/5 ×4 + 15: Cirillo's original Pomodoro.
 * - 50/10 ×3: the common longer block, close to DeskTime's 52/17 data.
 * - 90/20 ×2: one ~90-minute cycle of the body's rest-activity rhythm.
 */
export const PRESETS: { id: "classic" | "steady" | "deep"; plan: Plan }[] = [
  { id: "classic", plan: { focus: 25, short: 5, long: 15, rounds: 4 } },
  { id: "steady", plan: { focus: 50, short: 10, long: 0, rounds: 3 } },
  { id: "deep", plan: { focus: 90, short: 20, long: 0, rounds: 2 } },
];

/** Milliseconds left right now. */
export function leftNow(s: PomoStatus): number {
  return s.paused || !s.ends_at ? s.left : Math.max(0, s.ends_at - Date.now());
}

export function clock(ms: number): string {
  const total = Math.ceil(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = String(m).padStart(h ? 2 : 1, "0");
  return `${h ? `${h}:` : ""}${mm}:${String(s).padStart(2, "0")}`;
}
