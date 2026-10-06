// The SDK reference (/docs/sdk/…): its pages are Markdown (site/docs/sdk/), but every signature,
// field list and doc comment on them is read from packages/sdk/src with the TypeScript parser, so
// they can't go stale. Pages write `{{sdk:group}}` (an entry: heading, signature, the source's doc
// comment, a link to the line) or `{{sig:geom.rect}}` (a signature in a table cell), and
// `{{sdk:index}}` lists every export with a link to where it's documented. The build fails when a
// page names something the SDK doesn't export, and when the SDK exports something no page covers —
// so a new export can't ship undocumented.

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import ts from "typescript";
import { escapeAttr, escapeHtml, markdown } from "./markdown.mjs";
import { highlight } from "./highlight.mjs";

/** A signature formatted over several lines, back on one (for table cells). */
const oneLine = (s) => s.replace(/\(\n\s*/g, "(").replace(/,?\n\s*\)/g, ")").replace(/\{\n\s*/g, "{ ").replace(/;\n\s*\}/g, " }").replace(/([;,])\n\s*/g, "$1 ");
const collapse = (s) => s.replace(/\s+/g, " ").replace(/\(\s+/g, "(").replace(/\s+\)/g, ")").replace(/\[\s+/g, "[").replace(/\s+\]/g, "]").trim();
/** Long signatures break their object types one member per line. */
const WIDE = 84;

/** The doc comment right before `node`: a `/** … *\/` block (on the lines above, or on the same
 * line after the previous member), or consecutive `///` lines. */
function docOf(node, text) {
  const trivia = text.slice(node.getFullStart(), node.getStart());
  const block = [...trivia.matchAll(/\/\*\*([\s\S]*?)\*\//g)].pop();
  if (block && /^\s*$/.test(trivia.slice(block.index + block[0].length))) {
    return block[1].split("\n").map((l) => l.replace(/^\s*\* ?/, "")).join("\n").trim().replace(/\{@link\s+([^}]+)\}/g, "`$1`");
  }
  const lines = trivia.split("\n").map((l) => l.trim()).filter(Boolean);
  const doc = [];
  for (let i = lines.length - 1; i >= 0 && lines[i].startsWith("///"); i--) doc.unshift(lines[i].slice(3).trim());
  return doc.join(" ");
}

/** A type as written, collapsed; object types one member per line when `multi`. */
function typeText(node, multi, indent = "") {
  if (!node) return "";
  if (multi && ts.isTypeLiteralNode(node) && collapse(node.getText()).length > 40) {
    const inner = indent + "  ";
    return `{\n${node.members.map((m) => `${inner}${collapse(m.getText()).replace(/[;,]$/, "")};`).join("\n")}\n${indent}}`;
  }
  if (multi && ts.isIntersectionTypeNode(node)) return node.types.map((t) => typeText(t, multi, indent)).join(" & ");
  return collapse(node.getText());
}

function paramText(p, multi) {
  const init = p.initializer ? collapse(p.initializer.getText()) : null;
  const optional = !!p.questionToken || init === "{}";
  const type = p.type ? `: ${typeText(p.type, multi)}` : "";
  return `${p.dotDotDotToken ? "..." : ""}${p.name.getText()}${optional ? "?" : ""}${type}${init && init !== "{}" ? ` = ${init}` : ""}`;
}

/** `name<T>(a: A, b?: B): R` for a function-like node. */
function callSig(name, fn) {
  const tps = fn.typeParameters?.length ? `<${fn.typeParameters.map((t) => collapse(t.getText())).join(", ")}>` : "";
  const ret = fn.type ? `: ${collapse(fn.type.getText())}` : "";
  const one = `${name}${tps}(${fn.parameters.map((p) => paramText(p, false)).join(", ")})${ret}`;
  if (one.length <= WIDE) return one;
  const many = fn.parameters.map((p) => paramText(p, true));
  // One object parameter: keep it on the call's line; several: one per line.
  if (many.length === 1) return `${name}${tps}(${many[0]})${ret}`;
  return `${name}${tps}(\n${many.map((p) => `  ${p.replace(/\n/g, "\n  ")}`).join(",\n")}\n)${ret}`;
}

const fnOf = (init) => (init && (ts.isArrowFunction(init) || ts.isFunctionExpression(init)) ? init : null);

/** Read every module `index.ts` re-exports. Returns name → export. */
export function readApi(root) {
  const dir = join(root, "packages/sdk/src");
  const index = readFileSync(join(dir, "index.ts"), "utf8");
  const modules = [...index.matchAll(/export \* from "\.\/(\w+)\.js"/g)].map((m) => m[1]);
  const api = new Map();
  const add = (e) => {
    if (api.has(e.name)) throw new Error(`sdk reference: \`${e.name}\` is exported twice`);
    api.set(e.name, e);
  };
  for (const mod of modules) {
    const file = `packages/sdk/src/${mod}.ts`;
    const text = readFileSync(join(root, file), "utf8");
    const sf = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
    const line = (n) => sf.getLineAndCharacterOfPosition(n.getStart()).line + 1;
    const exported = (n) => n.modifiers?.some((m) => m.kind === ts.SyntaxKind.ExportKeyword);
    const base = { module: mod, file };
    for (const st of sf.statements) {
      if (!exported(st)) continue;
      if (ts.isFunctionDeclaration(st)) {
        add({ ...base, name: st.name.text, kind: "function", sig: callSig(st.name.text, st), doc: docOf(st, text), line: line(st) });
      } else if (ts.isInterfaceDeclaration(st)) {
        // Fields, methods (`measure(text, style?)`) and call signatures (a scale is callable), each a row.
        const params = (m) => m.parameters.map((p) => paramText(p, false)).join(", ");
        const fields = st.members.map((m) => {
          const doc = docOf(m, text);
          if (ts.isPropertySignature(m)) return { name: m.name.getText(), optional: !!m.questionToken, type: collapse(m.type?.getText() ?? ""), doc };
          if (ts.isMethodSignature(m)) return { name: `${m.name.getText()}(${params(m)})`, optional: false, type: collapse(m.type?.getText() ?? "void"), doc };
          if (ts.isCallSignatureDeclaration(m)) return { name: `(${params(m)})`, optional: false, type: collapse(m.type?.getText() ?? "void"), doc };
          return null;
        }).filter(Boolean);
        const ext = st.heritageClauses?.map((h) => collapse(h.getText())).join(" ") ?? "";
        const tps = st.typeParameters?.length ? `<${st.typeParameters.map((t) => collapse(t.getText())).join(", ")}>` : "";
        add({ ...base, name: st.name.text, kind: "interface", sig: `interface ${st.name.text}${tps}${ext ? ` ${ext}` : ""}`, fields, doc: docOf(st, text), line: line(st) });
      } else if (ts.isTypeAliasDeclaration(st)) {
        const tps = st.typeParameters?.length ? `<${st.typeParameters.map((t) => collapse(t.getText())).join(", ")}>` : "";
        let body = collapse(st.type.getText());
        // Unions of object types, one member per line; a union of literals wraps as text.
        if (`type ${st.name.text} = ${body}`.length > WIDE && ts.isUnionTypeNode(st.type) && st.type.types.some((t) => ts.isTypeLiteralNode(t))) body = `\n${st.type.types.map((t) => `  | ${collapse(t.getText())}`).join("\n")}`;
        add({ ...base, name: st.name.text, kind: "type", sig: `type ${st.name.text}${tps} =${body.startsWith("\n") ? "" : " "}${body}`, doc: docOf(st, text), line: line(st) });
      } else if (ts.isVariableStatement(st)) {
        for (const d of st.declarationList.declarations) {
          const name = d.name.getText();
          const init = d.initializer;
          const doc = docOf(st, text);
          if (init && ts.isObjectLiteralExpression(init)) {
            const members = [];
            const member = (prefix, p) => {
              const key = p.name.getText();
              const full = `${prefix}.${key}`;
              const mdoc = docOf(p, text);
              if (ts.isMethodDeclaration(p)) {
                members.push({ ...base, name: full, kind: "member", sig: callSig(full, p), doc: mdoc, line: line(p) });
              } else if (ts.isPropertyAssignment(p)) {
                const v = p.initializer;
                const fn = fnOf(v);
                if (fn) members.push({ ...base, name: full, kind: "member", sig: callSig(full, fn), doc: mdoc, line: line(p) });
                // `Object.assign(fn, { more })`: a callable with members of its own (`data.tiles.auto()`).
                else if (ts.isCallExpression(v) && v.expression.getText() === "Object.assign" && fnOf(v.arguments[0])) {
                  members.push({ ...base, name: full, kind: "member", sig: callSig(full, fnOf(v.arguments[0])), doc: mdoc, line: line(p) });
                  if (v.arguments[1] && ts.isObjectLiteralExpression(v.arguments[1])) for (const q of v.arguments[1].properties) member(full, q);
                } else members.push({ ...base, name: full, kind: "member", sig: `${full}: ${collapse(v.getText())}`, doc: mdoc, line: line(p) });
              }
            };
            for (const p of init.properties) member(name, p);
            add({ ...base, name, kind: "object", sig: `const ${name} = { ${init.properties.map((p) => p.name.getText()).join(", ")} }`, doc, line: line(st), members: members.map((m) => m.name) });
            for (const m of members) add(m);
          } else {
            add({ ...base, name, kind: "const", sig: `const ${name}${d.type ? `: ${collapse(d.type.getText())}` : ""}${init ? ` = ${collapse(init.getText())}` : ""}`, doc, line: line(st) });
          }
        }
      }
    }
  }
  return { api, modules };
}

/** Every `{{sdk:…}}` / `{{sig:…}}` in the SDK pages: name → [page slug, how]. */
function scanPages(docsDir) {
  const uses = new Map();
  const walk = (dir) => {
    for (const f of readdirSync(dir).sort()) {
      const p = join(dir, f);
      if (statSync(p).isDirectory()) walk(p);
      else if (f.endsWith(".md")) {
        const slug = `sdk/${relative(docsDir, p).replace(/\.md$/, "").replace(/(^|\/)index$/, "")}`.replace(/\/$/, "");
        const src = readFileSync(p, "utf8");
        for (const m of src.matchAll(/\{\{(sdk|sig):([^}]+)\}\}/g)) {
          if (m[2] === "index") continue;
          if (!uses.has(m[2]) || (m[1] === "sdk" && uses.get(m[2])[1] !== "sdk")) uses.set(m[2], [slug, m[1]]);
        }
      }
    }
  };
  if (existsSync(docsDir)) walk(docsDir);
  return uses;
}

const KIND = { function: "function", member: "function", object: "namespace", interface: "interface", type: "type", const: "constant" };

/**
 * The SDK's exports and the placeholders that document them. Throws at once — before any chart is
 * built — when a page names an unknown export or an export has no page.
 */
export function sdkApi({ root, blob }) {
  const { api, modules } = readApi(root);
  const docsDir = join(root, "site/docs/sdk");
  const uses = scanPages(docsDir);
  const unknown = [...uses.keys()].filter((n) => !api.has(n));
  if (unknown.length) throw new Error(`site/docs/sdk: not exported by @datars/sdk: ${unknown.join(", ")}`);
  if (existsSync(docsDir)) {
    const missing = [...api.keys()].filter((n) => !uses.has(n));
    if (missing.length) throw new Error(`site/docs/sdk: @datars/sdk exports ${missing.length} name(s) the SDK reference doesn't cover — add {{sdk:name}} or {{sig:name}} to a page in site/docs/sdk/: ${missing.join(", ")}`);
  }
  const get = (name, where) => {
    const e = api.get(name);
    if (!e) throw new Error(`${where}: @datars/sdk doesn't export \`${name}\``);
    return e;
  };
  const src = (e) => `<a href="${blob(e.file)}#L${e.line}"><code>${escapeHtml(e.file.replace("packages/sdk/src/", ""))}:${e.line}</code></a>`;
  const doc = (e) => (e.doc ? `<div class="api-doc">${markdown(e.doc).html}</div>` : "");
  // A Description column only when some field has a doc comment.
  const fieldsTable = (e) => {
    const docs = e.fields.some((f) => f.doc);
    return `<div class="table-wrap"><table class="api-fields"><thead><tr><th>Field</th><th>Type</th>${docs ? "<th>Description</th>" : ""}</tr></thead><tbody>${e.fields.map((f) => `<tr><td><code>${escapeHtml(f.name)}${f.optional ? "?" : ""}</code></td><td><code class="sig">${escapeHtml(f.type)}</code></td>${docs ? `<td>${f.doc ? markdown(f.doc).html.replace(/^<p>([\s\S]*)<\/p>$/, "$1") : ""}</td>` : ""}</tr>`).join("")}</tbody></table></div>`;
  };

  return {
    /** A full entry: heading, signature, the source's doc comment, fields (interfaces), source link. */
    entry(name, where) {
      if (name === "index") return this.index();
      const e = get(name, where);
      const sig = e.sig;
      return `<section class="api" id="${escapeAttr(name)}"><h3><a class="anchor" href="#${escapeAttr(name)}" aria-hidden="true" tabindex="-1">#</a><code>${escapeHtml(name)}</code> <small>${KIND[e.kind]}</small></h3>
<pre class="api-sig"><code data-lang="ts" data-hl>${highlight(sig, "ts")}</code></pre>
${doc(e)}${e.kind === "interface" && e.fields.length ? fieldsTable(e) : ""}<p class="src-note">Source: ${src(e)}</p></section>`;
    },
    /** A signature for a table cell, with an anchor. */
    sig(name, where) {
      const e = get(name, where);
      return `<code class="sig" id="${escapeAttr(name)}">${escapeHtml(oneLine(e.sig))}</code>`;
    },
    /** Every export, by module, linked to its entry. */
    index() {
      const rows = modules.map((mod) => {
        const names = [...api.values()].filter((e) => e.module === mod && e.kind !== "member");
        const link = (e) => {
          const u = uses.get(e.name);
          return u ? `<a class="chip" href="/docs/${u[0]}/#${escapeAttr(e.name)}" title="${escapeAttr(KIND[e.kind])}"><code>${escapeHtml(e.name)}</code></a>` : `<span class="chip"><code>${escapeHtml(e.name)}</code></span>`;
        };
        return `<tr><td><a href="${blob(`packages/sdk/src/${mod}.ts`)}"><code>${mod}.ts</code></a></td><td><div class="chips">${names.map(link).join("")}</div></td></tr>`;
      });
      return `<div class="table-wrap"><table class="api-index"><thead><tr><th>Module</th><th>Exports</th></tr></thead><tbody>${rows.join("")}</tbody></table></div>`;
    },
    count: () => api.size,
  };
}
