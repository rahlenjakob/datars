// Themes (docs/18-themes.md): typed tokens, colour expressions, modes, locks, checks.

export interface ThemeDef {
  name: string;
  extends?: string;
  /** "dark": a dark-first brand — dark in light mode too, over its parents' dark values. */
  scheme?: "light" | "dark";
  /** Colours (`"#b3261e"`, `"$accent"`, `"mix($ink, $paper, 0.9)"`), palettes, numbers, fonts
   * ({@link FontToken}, e.g. `font.title`), text. */
  tokens: Record<string, unknown>;
  modes?: Partial<Record<"dark" | "high-contrast", Record<string, unknown>>>;
  locked?: string[];
  checks?: unknown[];
}

export function theme(def: ThemeDef): ThemeDef {
  return def;
}

/**
 * A font token (`font.body`, `font.strong`, `font.title`, `font.number`, or your own): the family
 * text is drawn with (then its fallbacks), its weight and style — and where that face comes from,
 * so the build step (`datars bundle`, `publish`, `render`, `dev`) can acquire it, subset it to the
 * chart's text and ship it inside the bundle. Text measures and renders the same everywhere
 * because fonts travel with charts; system fonts are never used.
 *
 * - `src`: a font file (TrueType/OpenType, or WOFF) — a path relative to the document, or an
 *   `https://` URL (downloaded at build time).
 * - `google`: a Google Fonts family — static instances downloaded at build time and cached on
 *   disk; no runtime ever calls Google.
 * - neither: the family must already have a face (another token's, or the default Inter);
 *   `datars check` reports families with no source.
 *
 * ```ts
 * { family: "Cambon", weight: 700, src: "fonts/Cambon-Bold.otf" }
 * { google: "Source Serif 4", weight: 600, italic: true }
 * ```
 */
export interface FontToken {
  /** The family, or a stack (first choice first). Optional with `google` (it names the family). */
  family?: string | string[];
  /** 100–900; default 400. */
  weight?: number;
  italic?: boolean;
  src?: string;
  google?: string;
}

/** Font token helpers. */
export const font = {
  /** A face from a file next to the document (or an `https://` URL). */
  file(family: string | string[], src: string, opts: { weight?: number; italic?: boolean } = {}): FontToken {
    return { family, weight: opts.weight ?? 400, ...(opts.italic ? { italic: true } : {}), src };
  },
  /** A Google Fonts family (downloaded by the build step, shipped in the bundle). `fallback`:
   * families to try after it for characters it lacks. */
  google(family: string, opts: { weight?: number; italic?: boolean; fallback?: string[] } = {}): FontToken {
    return { google: family, weight: opts.weight ?? 400, ...(opts.italic ? { italic: true } : {}), ...(opts.fallback?.length ? { family: opts.fallback } : {}) };
  },
};
