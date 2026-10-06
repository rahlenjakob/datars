// Documents: the IR (crates/datars-ir). `doc()` fills defaults and normalizes lambdas.

import { clean } from "./prop.js";
import { Template } from "./nodes.js";
import { Program } from "./program.js";
import { ThemeDef } from "./theme.js";

export interface DocDef {
  id?: string;
  title?: string;
  description?: string;
  size?: [number, number] | { width: number; height: number };
  theme?: string | ThemeDef | { use?: string; themes?: ThemeDef[]; tokens?: Record<string, unknown> };
  locale?: string;
  data?: Record<string, unknown>;
  tables?: Record<string, { from: string; ops: unknown[] }>;
  signals?: Record<string, { type: string; default?: unknown; control?: unknown }>;
  keys?: Record<string, { name?: string; color?: string }>;
  scene: Template;
  motion?: unknown;
  program?: Program;
  /** User packages the sandbox loads: `source` inline, or `file` — a module next to the document
   * (relative path), embedded as its source when the document is built. */
  packages?: { name: string; version?: string; source?: string; file?: string }[];
}

export const FORMAT = 1;

export function doc(d: DocDef) {
  const size = Array.isArray(d.size) ? { width: d.size[0], height: d.size[1] } : d.size ?? { width: 800, height: 480 };
  let theme: unknown = undefined;
  if (typeof d.theme === "string") theme = { use: d.theme };
  else if (d.theme && "name" in d.theme) theme = { use: d.theme.name, themes: [d.theme] };
  else if (d.theme) theme = d.theme;
  return clean({
    datars: FORMAT,
    id: d.id,
    title: d.title,
    description: d.description,
    size,
    theme,
    locale: d.locale ?? "en",
    packages: d.packages,
    data: d.data,
    tables: d.tables,
    signals: d.signals,
    keys: d.keys,
    scene: d.scene,
    motion: d.motion,
    program: d.program,
  });
}

export const signal = {
  num: (def = 0, control?: unknown) => clean({ type: "num", default: def, control }),
  str: (def = "", control?: unknown) => clean({ type: "str", default: def, control }),
  bool: (def = false) => ({ type: "bool", default: def }),
  keyset: (def: string[] = []) => ({ type: "keyset", default: def }),
  key: (def: string | null = null) => ({ type: "key", default: def }),
  range: () => ({ type: "range", default: null }),
  /** A clock: counts up from `def` at `rate` units per second while a settled scene reads it (a
   * spinning globe: `projection center: [e("spin"), 15]`); paused during transitions, off screen
   * and for reduced motion. Renders and goldens see `def`. */
  clock: (rate: number, def = 0) => ({ type: "num", default: def, clock: rate }),
};

/** A hint recorded on a signal for hosts and editors (`signal.num(30000, control.slider(…))`). No
 * host draws it: to put a control on the chart, use the standard library's engine-drawn ones —
 * `slider`, `range`, `segmented`, `select`, `toggle`, `checklist`, `button`. */
export const control = {
  slider: (min: number, max: number, step = 1, label?: string) => clean({ type: "slider", min, max, step, label }),
  toggle: (label?: string) => clean({ type: "toggle", label }),
  select: (options: string[], label?: string) => clean({ type: "select", options, label }),
};
