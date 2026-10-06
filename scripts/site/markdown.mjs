// A small Markdown renderer for the site's docs: the CommonMark subset the docs use — front matter,
// ATX headings (with ids), paragraphs, bullet and numbered lists (nested by indentation), fenced
// code, GFM tables, blockquotes (`> **Note** …` becomes a callout), rules, and raw HTML blocks passed
// through (chart slots, figures, `{{placeholders}}`). No dependency: the build stays `node` only.

/** `---\nkey: value\n---\n` at the top of a file → [fields, rest]. Values are plain strings. */
export function frontMatter(src) {
  const m = src.match(/^---\n([\s\S]*?)\n---\n?/);
  if (!m) return [{}, src];
  const fields = {};
  for (const line of m[1].split("\n")) {
    const i = line.indexOf(":");
    if (i > 0) fields[line.slice(0, i).trim()] = line.slice(i + 1).trim();
  }
  return [fields, src.slice(m[0].length)];
}

export const escapeHtml = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
export const escapeAttr = (s) => escapeHtml(s).replace(/"/g, "&quot;");

/** A heading's id: lowercase words joined by dashes (`Scales & marks` → `scales-marks`). */
export function slug(text) {
  return String(text)
    .replace(/<[^>]+>/g, "")
    .replace(/`/g, "")
    .toLowerCase()
    .replace(/&[a-z]+;/g, "")
    .replace(/[^\p{L}\p{N}]+/gu, "-")
    .replace(/^-+|-+$/g, "");
}

const INLINE_TAGS = /^<\/?(kbd|br|b|i|em|strong|code|a|span|sup|sub|small|abbr|mark|s)\b[^>]*>/;

/** Inline Markdown: code spans, images, links, bold, italic; allowed inline HTML kept. */
export function inline(src) {
  const codes = [];
  let s = src.replace(/(`+)([\s\S]*?[^`])\1(?!`)/g, (_, _t, code) => {
    codes.push(`<code>${escapeHtml(code.trim() === "" ? code : code.replace(/^ (.*) $/, "$1"))}</code>`);
    return `\u0000${codes.length - 1}\u0000`;
  });
  // Escape what isn't an allowed tag or an entity.
  let out = "";
  for (let i = 0; i < s.length; ) {
    const c = s[i];
    if (c === "<") {
      const m = s.slice(i).match(INLINE_TAGS);
      if (m) { out += m[0]; i += m[0].length; continue; }
      out += "&lt;"; i++; continue;
    }
    if (c === "&") {
      const m = s.slice(i).match(/^&(#\d+|#x[0-9a-f]+|[a-z]+);/i);
      if (m) { out += m[0]; i += m[0].length; continue; }
      out += "&amp;"; i++; continue;
    }
    if (c === ">") { out += "&gt;"; i++; continue; }
    out += c; i++;
  }
  s = out;
  s = s.replace(/!\[([^\]]*)\]\(([^)\s]+)(?:\s+"([^"]*)")?\)/g, (_, alt, url, title) => `<img src="${url}" alt="${alt}"${title ? ` title="${title}"` : ""} loading="lazy">`);
  s = s.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, text, url) => `<a href="${url}">${text}</a>`);
  s = s.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
  s = s.replace(/(^|[^*\w])\*([^*\s][^*]*?)\*(?!\w)/g, "$1<em>$2</em>");
  s = s.replace(/\u0000(\d+)\u0000/g, (_, i) => codes[Number(i)]);
  return s;
}

const BLOCK_HTML = /^\s*(<\/?(div|figure|section|details|summary|table|thead|tbody|tr|td|th|p|pre|video|aside|ul|ol|li|h[1-6]|dl|dt|dd|nav|article|header|footer|figcaption|img|picture|iframe|form|hr|br|blockquote|script|style|noscript)\b|<!--|\{\{)/i;

/**
 * Render a Markdown body. Returns `{ html, headings }` — `headings` are the h2/h3 with their ids,
 * for the page's "On this page" list.
 */
export function markdown(src) {
  const lines = src.replace(/\r\n?/g, "\n").replace(/\t/g, "  ").split("\n");
  const headings = [];
  const used = new Map();
  const uniqueId = (text) => {
    const base = slug(text) || "section";
    const n = used.get(base) ?? 0;
    used.set(base, n + 1);
    return n ? `${base}-${n}` : base;
  };
  const out = [];
  let i = 0;
  const isBlank = (l) => l === undefined || l.trim() === "";

  while (i < lines.length) {
    const line = lines[i];
    if (isBlank(line)) { i++; continue; }

    // Fenced code: ```lang
    const fence = line.match(/^(\s*)(```+|~~~+)\s*([\w+-]*)\s*(.*)$/);
    if (fence) {
      const [, indent, marker, lang, title] = fence;
      const body = [];
      i++;
      while (i < lines.length && !lines[i].trim().startsWith(marker)) {
        body.push(lines[i].startsWith(indent) ? lines[i].slice(indent.length) : lines[i]);
        i++;
      }
      i++;
      const t = title ? `<div class="code-title">${escapeHtml(title)}</div>` : "";
      out.push(`${t}<pre><code${lang ? ` data-lang="${lang}"` : ""}>${escapeHtml(body.join("\n"))}</code></pre>`);
      continue;
    }

    // Headings
    const h = line.match(/^(#{1,6})\s+(.*?)\s*(?:\{#([\w-]+)\})?\s*#*\s*$/);
    if (h) {
      const level = h[1].length;
      const html = inline(h[2]);
      const id = h[3] ?? uniqueId(h[2]);
      if (level === 2 || level === 3) headings.push({ level, id, text: html.replace(/<[^>]+>/g, "") });
      out.push(level === 1 ? `<h1>${html}</h1>` : `<h${level} id="${id}"><a class="anchor" href="#${id}" aria-hidden="true" tabindex="-1">#</a>${html}</h${level}>`);
      i++;
      continue;
    }

    // Rule
    if (/^\s*(-{3,}|\*{3,}|_{3,})\s*$/.test(line)) { out.push("<hr>"); i++; continue; }

    // Raw HTML block (until a blank line)
    if (BLOCK_HTML.test(line)) {
      const body = [];
      while (i < lines.length && !isBlank(lines[i])) body.push(lines[i++]);
      out.push(body.join("\n"));
      continue;
    }

    // Table
    if (line.trim().startsWith("|") && lines[i + 1] && /^\s*\|?\s*:?-{2,}/.test(lines[i + 1])) {
      const cells = (l) => l.trim().replace(/^\||\|$/g, "").split(/(?<!\\)\|/).map((c) => c.trim().replace(/\\\|/g, "|"));
      const head = cells(line);
      const aligns = cells(lines[i + 1]).map((c) => (c.startsWith(":") && c.endsWith(":") ? "center" : c.endsWith(":") ? "right" : ""));
      i += 2;
      const rows = [];
      while (i < lines.length && lines[i].trim().startsWith("|")) rows.push(cells(lines[i++]));
      const al = (j) => (aligns[j] ? ` style="text-align:${aligns[j]}"` : "");
      out.push(`<div class="table-wrap"><table><thead><tr>${head.map((c, j) => `<th${al(j)}>${inline(c)}</th>`).join("")}</tr></thead><tbody>${rows.map((r) => `<tr>${r.map((c, j) => `<td${al(j)}>${inline(c)}</td>`).join("")}</tr>`).join("")}</tbody></table></div>`);
      continue;
    }

    // Blockquote / callout
    if (/^\s*>/.test(line)) {
      const body = [];
      while (i < lines.length && /^\s*>/.test(lines[i])) body.push(lines[i++].replace(/^\s*> ?/, ""));
      const inner = markdown(body.join("\n")).html;
      const kind = body[0]?.match(/^\*\*(Note|Tip|Warning|Heads up|Status)\*\*/i)?.[1];
      out.push(kind ? `<aside class="callout ${kind.toLowerCase().replace(/\s+/g, "-")}">${inner}</aside>` : `<blockquote>${inner}</blockquote>`);
      continue;
    }

    // Lists
    const li = line.match(/^(\s*)([-*+]|\d+[.)])\s+(.*)$/);
    if (li) {
      const [html, next] = list(lines, i);
      out.push(html);
      i = next;
      continue;
    }

    // Paragraph: until a blank line or another block
    const para = [];
    while (i < lines.length && !isBlank(lines[i]) && !/^(#{1,6})\s/.test(lines[i]) && !/^\s*(```|~~~)/.test(lines[i]) && !/^\s*([-*+]|\d+[.)])\s+/.test(lines[i]) && !/^\s*>/.test(lines[i]) && !(lines[i].trim().startsWith("|") && para.length === 0)) {
      para.push(lines[i++].trim());
    }
    if (para.length) out.push(`<p>${inline(para.join("\n"))}</p>`);
    else i++;
  }
  return { html: out.join("\n"), headings };

  /** A list starting at line `start`; nested lists by deeper indentation. Returns [html, next line]. */
  function list(lines, start) {
    const first = lines[start].match(/^(\s*)([-*+]|\d+[.)])\s+/);
    const indent = first[1].length;
    const ordered = /\d/.test(first[2]);
    const items = [];
    let i = start;
    while (i < lines.length) {
      const m = lines[i].match(/^(\s*)([-*+]|\d+[.)])\s+(.*)$/);
      if (!m || m[1].length !== indent || /\d/.test(m[2]) !== ordered) break;
      const body = [m[3]];
      i++;
      // Continuation: indented lines (deeper than the marker), or blank lines followed by them.
      while (i < lines.length) {
        const l = lines[i];
        if (isBlank(l)) {
          const nxt = lines[i + 1];
          if (nxt !== undefined && !isBlank(nxt) && nxt.match(/^\s*/)[0].length > indent) { body.push(""); i++; continue; }
          break;
        }
        const lead = l.match(/^\s*/)[0].length;
        if (lead > indent) { body.push(l.slice(Math.min(lead, indent + 2))); i++; continue; }
        if (/^\s*([-*+]|\d+[.)])\s+/.test(l)) break;
        // Lazy continuation of the item's paragraph.
        body.push(l.trim());
        i++;
      }
      items.push(body);
    }
    const html = items.map((body) => {
      const text = body.join("\n");
      const nested = /\n\s*([-*+]|\d+[.)])\s+/.test(text) || /\n\s*```/.test(text) || text.includes("\n\n");
      if (!nested) return `<li>${inline(text)}</li>`;
      // First line as the item's text, the rest rendered as blocks.
      const [head, ...rest] = body;
      return `<li>${inline(head)}${markdown(rest.join("\n")).html}</li>`;
    }).join("");
    return [`<${ordered ? "ol" : "ul"}>${html}</${ordered ? "ol" : "ul"}>`, i];
  }
}
