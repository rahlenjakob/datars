// Code highlighting at build time (crawlers and no-JS readers see the same page): one regex per
// language whose capture groups are token kinds, as the page script's tiny highlighter does.

const esc = (s) => s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]);

const kw = (words) => new RegExp(`\\b(${words.join("|")})\\b`).source;
const TS_KW = ["import", "from", "export", "default", "const", "let", "var", "return", "true", "false", "null", "undefined", "new", "function", "type", "interface", "as", "if", "else", "for", "of", "in", "await", "async", "extends"];
const SWIFT_KW = ["import", "let", "var", "func", "struct", "class", "some", "return", "true", "false", "nil", "if", "else", "for", "in", "self", "private", "public", "var", "@State", "@main", "try", "await", "async", "guard"];
const KOTLIN_KW = ["import", "val", "var", "fun", "class", "object", "override", "return", "true", "false", "null", "if", "else", "for", "in", "this", "private", "when", "package"];

export const LANGS = {
  ts: {
    re: new RegExp(`(\\/\\/[^\\n]*|\\/\\*[\\s\\S]*?\\*\\/)|("(?:[^"\\\\\\n]|\\\\.)*"|'(?:[^'\\\\\\n]|\\\\.)*'|\`(?:[^\`\\\\]|\\\\.)*\`)|${kw(TS_KW)}|\\b(\\d[\\d_]*(?:\\.\\d+)?)\\b|([A-Za-z_$][\\w$]*)(?=\\s*\\()|([A-Za-z_$][\\w$]*)(?=\\??:\\s)`, "g"),
    cls: ["c", "s", "k", "n", "f", "p"],
  },
  swift: {
    re: new RegExp(`(\\/\\/[^\\n]*|\\/\\*[\\s\\S]*?\\*\\/)|("(?:[^"\\\\\\n]|\\\\.)*")|(@\\w+|${kw(SWIFT_KW).slice(3, -3)})\\b|\\b(\\d[\\d_]*(?:\\.\\d+)?)\\b|([A-Za-z_][\\w]*)(?=\\s*\\()|(\\.[a-z]\\w*)`, "g"),
    cls: ["c", "s", "k", "n", "f", "p"],
  },
  kotlin: {
    re: new RegExp(`(\\/\\/[^\\n]*|\\/\\*[\\s\\S]*?\\*\\/)|("(?:[^"\\\\\\n]|\\\\.)*")|${kw(KOTLIN_KW)}|\\b(\\d[\\d_]*(?:\\.\\d+)?)\\b|([A-Za-z_][\\w]*)(?=\\s*\\()`, "g"),
    cls: ["c", "s", "k", "n", "f"],
  },
  sh: { re: /(#[^\n]*)|("(?:[^"\\\n]|\\.)*"|'[^'\n]*')|\b(datars|datars-mcp|pnpm|npm|npx|node|cargo|git|cd)\b|(--?[\w-]+)/g, cls: ["c", "s", "f", "p"] },
  html: { re: /(<!--[\s\S]*?-->)|("[^"]*")|(<\/?[\w-]+|\/?>)|([\w-]+)(?==)/g, cls: ["c", "s", "t", "p"] },
  json: { re: /("(?:[^"\\\n]|\\.)*")(?=\s*:)|("(?:[^"\\\n]|\\.)*")|\b(true|false|null)\b|(-?\d+(?:\.\d+)?(?:e[+-]?\d+)?)/g, cls: ["p", "s", "k", "n"] },
};
LANGS.js = LANGS.ts;
LANGS.typescript = LANGS.ts;
LANGS.javascript = LANGS.ts;
LANGS.jsonc = LANGS.json;
LANGS.bash = LANGS.sh;
LANGS.shell = LANGS.sh;
LANGS.kt = LANGS.kotlin;
LANGS.xml = LANGS.html;

/** Highlight source text (unescaped) → HTML. Unknown languages come back escaped. */
export function highlight(text, lang) {
  const l = LANGS[lang];
  if (!l) return esc(text);
  let out = "", last = 0;
  l.re.lastIndex = 0;
  for (let m; (m = l.re.exec(text)); ) {
    if (m[0] === "") { l.re.lastIndex++; continue; }
    const i = m.findIndex((g, j) => j > 0 && g !== undefined);
    out += esc(text.slice(last, m.index)) + `<span class="tk-${l.cls[i - 1]}">${esc(m[0])}</span>`;
    last = m.index + m[0].length;
  }
  return out + esc(text.slice(last));
}

const unescape = (s) => s.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, "&");

/** Highlight every `<pre><code data-lang="…">` block of a page (their text is HTML-escaped). */
export function highlightPage(html) {
  return html.replace(/<pre([^>]*)><code data-lang="([\w+-]+)"([^>]*)>([\s\S]*?)<\/code><\/pre>/g, (all, pre, lang, rest, body) => {
    if (/\bid=/.test(rest) || body.includes("<span class=\"tk-")) return all; // live (the page rewrites it) or done
    return `<pre${pre}><code data-lang="${lang}" data-hl${rest}>${highlight(unescape(body), lang)}</code></pre>`;
  });
}
