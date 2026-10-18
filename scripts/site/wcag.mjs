// The accessibility page's WCAG 2.2 conformance table, from site/a11y/wcag.json (every criterion:
// who answers for it, how, which checks) and site/a11y/wcag-report.json (what the checks found:
// scripts/a11y-report.mjs). Checked when the site builds: a test the inventory names that the
// report doesn't have, or a wcag_ test the inventory doesn't name, is a build error.
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { escapeHtml, inline } from "./markdown.mjs";

const WHO = { runtime: "datars", author: "You (datars checks)", page: "Your page", na: "—" };
const STATUS = { met: "Met", partial: "Partly met", "not-met": "Not met", na: "Not applicable" };
const PRINCIPLES = { 1: "Perceivable", 2: "Operable", 3: "Understandable", 4: "Robust" };

export function wcag({ root, blob }) {
  const inventory = JSON.parse(readFileSync(join(root, "site/a11y/wcag.json"), "utf8"));
  const reportFile = join(root, "site/a11y/wcag-report.json");
  if (!existsSync(reportFile)) throw new Error("site/a11y/wcag-report.json is missing: run node scripts/a11y-report.mjs");
  const report = JSON.parse(readFileSync(reportFile, "utf8"));
  // Drift: what the inventory names must be in the report, and every wcag_ test in the sources
  // must be named by the inventory.
  const named = new Set(inventory.criteria.flatMap((c) => c.tests ?? []));
  const missing = [...named].filter((id) => !report.tests[id]);
  if (missing.length) throw new Error(`site/a11y/wcag-report.json has no ${missing.join(", ")}: run node scripts/a11y-report.mjs`);
  const defined = new Set();
  for (const f of ["crates/datars-test/tests/wcag.rs", "packages/web/test/wcag.browser.test.mjs"]) {
    for (const m of readFileSync(join(root, f), "utf8").matchAll(/wcag_[0-9]_[0-9]+_[0-9]+_[a-z0-9_]+|wcag_axe/g)) defined.add(m[0]);
  }
  const unnamed = [...defined].filter((id) => !named.has(id) && id !== "wcag_axe");
  if (unnamed.length) throw new Error(`site/a11y/wcag.json names no criterion for ${unnamed.join(", ")}`);

  const ev = (k) => {
    const e = inventory.evidence[k];
    const href = e.href.replace("{{src}}", blob("").replace(/\/$/, ""));
    return `<a href="${escapeHtml(href)}">${escapeHtml(e.label)}</a>`;
  };
  const result = (id) => {
    const t = report.tests[id];
    const src = t.file ? blob(`${t.file}#L${t.line}`) : null;
    const word = t.result === "pass" ? "passes" : t.result === "fail" ? `fails${t.found ? ` (${t.found} found)` : ""}` : "skipped";
    const name = `<code>${escapeHtml(id)}</code>`;
    const what = t.checked ? ` <span class="wcag-checked">— ${escapeHtml(t.checked)}${t.authored ? `; ${t.authored} in the site's own figures to look at` : ""}</span>` : "";
    return `<li class="wcag-${t.result}">${src ? `<a href="${escapeHtml(src)}">${name}</a>` : name} ${word}${what}</li>`;
  };
  const sorted = [...inventory.criteria].sort((a, b) => {
    const pa = a.id.split(".").map(Number), pb = b.id.split(".").map(Number);
    return pa[0] - pb[0] || pa[1] - pb[1] || pa[2] - pb[2];
  });
  let last = 0;
  const rows = sorted.map((c) => {
    const p = Number(c.id.split(".")[0]);
    const head = p !== last ? `<tr class="wcag-principle"><th colspan="3" scope="colgroup">${p}. ${PRINCIPLES[p]}</th></tr>` : "";
    last = p;
    const how = [inline(c.how), c.author ? `<br><b>You:</b> ${inline(c.author)}` : "", c.notes ? `<br><b>Not met:</b> ${inline(c.notes)}` : ""].join("");
    const checks = [...(c.tests ?? []).map(result), ...(c.evidence ?? []).map((k) => `<li>${ev(k)}</li>`)];
    // The criterion, who answers for it and the result together; then how; then what checks it.
    const crit = `<span class="wcag-name">${c.id} ${escapeHtml(c.name)} <span class="wcag-level">${c.level}</span></span><span class="wcag-who">${WHO[c.class]}</span><span class="wcag-status wcag-${c.status}">${STATUS[c.status]}</span>`;
    return `${head}<tr id="wcag-${c.id.replaceAll(".", "-")}"><th scope="row">${crit}</th><td data-label="How">${how}</td><td data-label="Checked by">${checks.length ? `<ul class="wcag-checks">${checks.join("")}</ul>` : "—"}</td></tr>`;
  }).join("");
  const table = `<div class="table-wrap"><table class="wcag"><caption>WCAG 2.2, every success criterion at A and AA and four at AAA: who answers for it, how, what checks it — checked ${escapeHtml(report.date)}${report.commit ? ` at <code>${escapeHtml(report.commit)}</code>` : ""}, over ${report.corpus} documents</caption><thead><tr><th scope="col">Criterion, who, result</th><th scope="col">How</th><th scope="col">Checked by</th></tr></thead><tbody>${rows}</tbody></table></div>`;

  const aa = inventory.criteria.filter((c) => c.level !== "AAA");
  const by = (k, v, list = aa) => list.filter((c) => c[k] === v).length;
  const tests = Object.values(report.tests);
  const passes = tests.filter((t) => t.result === "pass").length;
  const summary = `<p class="wcag-summary">Of WCAG 2.2's ${aa.length} success criteria at A and AA, <b>${by("class", "runtime")}</b> are datars' to meet (the engine, the standard library, the web element and the native hosts), <b>${by("class", "author")}</b> are the chart's author's (datars helps and checks), <b>${by("class", "page")}</b> are the page's or app's, and <b>${by("class", "na")}</b> don't apply to a chart. Of the ones that apply to datars or its authors, <b>${aa.filter((c) => c.status === "met").length}</b> are met and <b>${aa.filter((c) => c.status === "partial").length}</b> partly; four AAA criteria are listed too. ${passes} of ${tests.length} checks pass (${escapeHtml(report.date)}); the ones that fail are what isn't met, failing on purpose until it is.</p>`;
  return { table, summary };
}
