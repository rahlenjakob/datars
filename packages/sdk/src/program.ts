// Programs: small statecharts (docs/08-programs.md). Presets build them.

import { clean } from "./prop.js";

export interface StepOpts { set?: Record<string, unknown>; hold?: number; title?: string; text?: string; anchor?: string }

export interface Step { name: string; set?: Record<string, unknown>; hold?: number; narration?: { title?: string; text?: string; anchor?: string } }

export function step(name: string, o: StepOpts = {}): Step {
  const narration = o.title || o.text || o.anchor ? clean({ title: o.title, text: o.text, anchor: o.anchor }) : undefined;
  return clean({ name, set: o.set, hold: o.hold, narration });
}

/** How the program is meant to be moved. The engine acts on `"autoplay"` only: each state holds for
 * its `hold`, then `next` (hosts pause it off screen, on interaction and for reduced motion). The
 * rest are hints for hosts: `"steps"` (step controls), `"keys"` (arrow keys), `{ scroll }` (the
 * page's scroll scrubs, or triggers each step) and `{ timer: seconds }` — recorded in the document,
 * acted on by no host yet. */
export type Driver = "steps" | "keys" | "autoplay" | { scroll: "scrub" | "trigger" } | { timer: number };

export interface Chapter { param: string; program: Program }
export interface Program { preset?: string; states: Step[]; edges?: { from: string; on: string; to: string }[]; initial?: string; drivers?: Driver[]; chapters?: Record<string, Chapter> }

/** A linear story: next/prev move through the steps. */
export function story(o: { steps: Step[]; drivers?: Driver[]; chapters?: Record<string, Chapter>; loop?: boolean }): Program {
  const edges = o.loop && o.steps.length > 1 ? [{ from: o.steps[o.steps.length - 1].name, on: "next", to: o.steps[0].name }] : undefined;
  return clean({ preset: "story", states: o.steps, edges, drivers: o.drivers ?? ["steps", "keys"], chapters: o.chapters });
}

/** A scrollytelling story: the page's scroll position scrubs through the steps (the host maps
 * scroll progress to a program position; `<datars-view scrub>` does it for the web). */
export function scrolly(o: { steps: Step[]; chapters?: Record<string, Chapter> }): Program {
  return clean({ preset: "story", states: o.steps, drivers: [{ scroll: "scrub" }, "keys"], chapters: o.chapters });
}

/** Autoplaying story: each step holds for its `hold` seconds (default 2.5); hosts pause it when
 * off-screen or when the reader prefers reduced motion. */
export function autoplay(o: { steps: Step[]; loop?: boolean }): Program {
  return story({ steps: o.steps, loop: o.loop, drivers: ["autoplay", "steps", "keys"] });
}

export function chapter(param: string, steps: Step[]): Chapter {
  return { param, program: { states: steps } };
}

/** An explorable: one state, interaction through signals. */
export function interactive(): Program {
  return { preset: "interactive", states: [{ name: "main" }] };
}

/** A film: every step held, rendered frame-exactly. */
export function film(steps: Step[]): Program {
  return { preset: "film", states: steps, drivers: ["autoplay"] };
}

export function loop(steps: Step[]): Program {
  return story({ steps, loop: true, drivers: ["autoplay"] });
}

export function dashboard(): Program {
  return { preset: "dashboard", states: [{ name: "main" }] };
}
