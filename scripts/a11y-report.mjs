// The WCAG report: runs every check site/a11y/wcag.json names — the engine's over the corpus
// (crates/datars-test/tests/wcag.rs, with the site's charts and figures compiled by
// scripts/wcag-corpus.mjs) and the web element's in Chrome (packages/web/test/wcag.browser.test.mjs,
// with DATARS_PLAYWRIGHT; axe-core's rules too with DATARS_AXE) — and writes what each found to
// site/a11y/wcag-report.json: pass, fail or skipped, what it checked, where it is, and the date.
// The site build turns the two files into the accessibility page's conformance table, and fails
// if a test the inventory names isn't in the report (run this again) or a wcag_ test exists that
// the inventory doesn't name.
//   node scripts/a11y-report.mjs [--no-web]
import { execFileSync, spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(new URL("..", import.meta.url).pathname);
const args = process.argv.slice(2);
const inventory = JSON.parse(readFileSync(join(root, "site/a11y/wcag.json"), "utf8"));
const sources = { rust: "crates/datars-test/tests/wcag.rs", web: "packages/web/test/wcag.browser.test.mjs" };

/** Where each wcag_ test is defined: `{ id: { file, line } }`. */
export function testSources(root) {
  const out = {};
  for (const file of Object.values(sources)) {
    readFileSync(join(root, file), "utf8").split("\n").forEach((l, i) => {
      for (const m of l.matchAll(/(?:fn |test\(")(wcag_[a-z0-9_]+)/g)) out[m[1]] ??= { file, line: i + 1 };
      // A browser test can answer several: "wcag_2_1_1_keyboard and wcag_2_1_2_no_keyboard_trap: …".
      const t = l.match(/test\("([^"]+?):/);
      if (t) for (const id of t[1].match(/wcag_[a-z0-9_]+/g) ?? []) out[id] ??= { file, line: i + 1 };
    });
  }
  return out;
}

const tests = {};
const note = (id, k, v) => ((tests[id] ??= {})[k] = v);

// The engine's checks over the corpus (every example, the site's charts and figures).
const corpus = execFileSync("node", [join(root, "scripts/wcag-corpus.mjs")], { cwd: root }).toString().trim();
console.log(corpus);
const rust = spawnSync("cargo", ["test", "--release", "-p", "datars-test", "--test", "wcag", "--", "--include-ignored", "--nocapture", "--test-threads", "4"], {
  cwd: root, env: { ...process.env, DATARS_WCAG_CORPUS: "out/wcag-corpus" }, maxBuffer: 1 << 28, encoding: "utf8",
});
const rustOut = `${rust.stdout}\n${rust.stderr}`;
for (const m of rustOut.matchAll(/^test (wcag_[a-z0-9_]+) \.\.\. (ok|FAILED|ignored)/gm)) note(m[1], "result", m[2] === "ok" ? "pass" : m[2] === "FAILED" ? "fail" : "skipped");
for (const m of rustOut.matchAll(/^WCAG-CHECKED (wcag_[a-z0-9_]+) (\d+ )?(.+)$/gm)) note(m[1], "checked", !m[2] ? m[3].trim() : m[3].startsWith("documents") ? `${m[2].trim()} ${m[3].trim()}` : `${m[2].trim()} documents: ${m[3].trim()}`);
const authored = {};
for (const m of rustOut.matchAll(/^WCAG-AUTHOR (wcag_[a-z0-9_]+) /gm)) authored[m[1]] = (authored[m[1]] ?? 0) + 1;
for (const [id, n] of Object.entries(authored)) note(id, "authored", n);
// A failing check's first findings, for the page to say what isn't met.
for (const m of rustOut.matchAll(/thread '(wcag_[a-z0-9_]+)' [^\n]*panicked at [^\n]*\n(\d+) found:\n((?:[^\n]+\n){1,3})/g)) {
  note(m[1], "found", +m[2]);
  note(m[1], "examples", m[3].split("\n").filter((l) => l && !l.startsWith("WCAG-") && !l.startsWith("note:")));
}

// The web element's, in Chrome.
if (!args.includes("--no-web") && process.env.DATARS_PLAYWRIGHT) {
  const web = spawnSync("node", ["--test", "--test-reporter=tap", join(root, sources.web)], { cwd: root, env: process.env, maxBuffer: 1 << 26, encoding: "utf8" });
  for (const m of `${web.stdout}`.matchAll(/^(ok|not ok) \d+ - ((?:wcag_[a-z0-9_]+(?:,? and |, )?)+)[^\n]*?( # SKIP.*)?$/gm)) {
    for (const id of m[2].match(/wcag_[a-z0-9_]+/g)) {
      note(id, "result", m[3] ? "skipped" : m[1] === "ok" ? "pass" : "fail");
      note(id, "checked", "in Chrome, through Playwright");
    }
  }
}

const where = testSources(root);
for (const [id, src] of Object.entries(where)) if (tests[id]) Object.assign(tests[id], src);
const commit = (() => { try { return execFileSync("git", ["rev-parse", "--short", "HEAD"], { cwd: root }).toString().trim(); } catch { return null; } })();
const report = {
  about: "What scripts/a11y-report.mjs found: each wcag_ test's result, what it checked and where it is. Written by the script; read by the site build.",
  date: new Date().toLocaleDateString("sv-SE"), // (the local date, as YYYY-MM-DD)
  commit,
  corpus: Number(corpus.match(/: (\d+) documents/)?.[1] ?? 0),
  tests: Object.fromEntries(Object.entries(tests).sort(([a], [b]) => a.localeCompare(b))),
};
writeFileSync(join(root, "site/a11y/wcag-report.json"), `${JSON.stringify(report, null, 1)}\n`);

// What the inventory names that nothing ran, and what ran that the inventory doesn't name.
const named = new Set(inventory.criteria.flatMap((c) => c.tests ?? []));
const missing = [...named].filter((id) => !tests[id]?.result);
const unnamed = Object.keys(where).filter((id) => !named.has(id));
const counts = Object.values(tests).reduce((n, t) => ((n[t.result] = (n[t.result] ?? 0) + 1), n), {});
console.log(`site/a11y/wcag-report.json: ${Object.entries(counts).map(([k, v]) => `${v} ${k}`).join(", ")}`);
if (missing.length) console.log(`not run: ${missing.join(", ")}`);
if (unnamed.length) console.log(`not in site/a11y/wcag.json: ${unnamed.join(", ")}`);
const failed = Object.entries(tests).filter(([, t]) => t.result === "fail").map(([id]) => id);
// (Checks the inventory says are not met fail on purpose: they're the report's "not met".)
const expected = new Set(inventory.criteria.filter((c) => c.status !== "met").flatMap((c) => c.tests ?? []));
const unexpected = failed.filter((id) => !expected.has(id));
if (unexpected.length) console.log(`failing: ${unexpected.join(", ")}`);
process.exitCode = unexpected.length || missing.length || unnamed.length ? 1 : 0;
