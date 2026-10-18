// The icon library (/docs/icons/library/): the built-in set, searchable and browsable. The page
// arrives with its first cells drawn into it; after first paint this fetches the set's index (names,
// tags, categories — scripts/site/icons.mjs writes it) and draws further cells from sprite chunks of
// CHUNK icons each, fetched as a search shows them or the grid scrolls to them — never the whole set.
// The details open in a <dialog>: the snippets, the icon at four sizes on light and dark paper, and a
// live <datars-view> drawing it, whose runtime loads only then.

const SITE = new URL(".", import.meta.url);
const LIB = new URL("docs/icons/library/", SITE);
/** Cells added per page of results (the grid's next page loads as its end scrolls into view). */
const PAGE = 120;

const $ = (id) => document.getElementById(id);
const grid = $("il-grid"), q = $("il-q"), count = $("il-count"), empty = $("il-empty"), more = $("il-more");
const cats = [...document.querySelectorAll(".il-cats button")];
const fmt = new Intl.NumberFormat("en");

let index = null; // { icons: [[name, tags, cats, chunk, feather?]], categories, tokens, chunk, … }
const byName = new Map();
const sprites = new Map(); // chunk → Promise<{ name: markup }>
let results = null; // the rows the grid shows now, in order (null: the set, before the index came)
let shown = grid.children.length;
let cat = "";

const sprite = (k) => {
  if (!sprites.has(k)) sprites.set(k, fetch(new URL(`sprite-${k}.json`, LIB)).then((r) => r.json()));
  return sprites.get(k);
};
const markupOf = async (row) => (await sprite(row[3]))[row[0]];

// ---- search ------------------------------------------------------------------------------------

const words = (s) => s.toLowerCase().replace(/[^a-z0-9]+/g, " ").trim().split(" ").filter(Boolean);

/** Rows matching the query and category, best first: the name itself, a name that starts with a
 * word, one that has it, then tags and categories. Every word has to match somewhere. */
function search(text) {
  const ws = words(text);
  const c = cat === "" ? -1 : Number(cat);
  const catWords = index.categories.map(([id, title]) => `${id} ${title}`.toLowerCase());
  const out = [];
  for (const row of index.icons) {
    if (c >= 0 && !row[2].includes(c)) continue;
    if (!ws.length) { out.push([0, row]); continue; }
    const name = row[0], parts = name.split("-"), tags = row[1].split(" ");
    let score = 0, ok = true;
    for (const w of ws) {
      let s;
      if (name === w) s = 0;
      else if (parts.includes(w)) s = 1;
      else if (parts.some((p) => p.startsWith(w))) s = 2;
      else if (name.includes(w)) s = 3;
      else if (tags.includes(w)) s = 4;
      else if (tags.some((t) => t.startsWith(w))) s = 5;
      else if (row[2].some((i) => catWords[i].includes(w))) s = 6;
      else { ok = false; break; }
      score += s;
    }
    if (ok) out.push([score + (name === ws.join("-") ? -10 : 0), row]);
  }
  out.sort((a, b) => a[0] - b[0] || a[1][0].length - b[1][0].length || (a[1][0] < b[1][0] ? -1 : 1));
  return out.map((x) => x[1]);
}

// ---- the grid ----------------------------------------------------------------------------------

function cell(name, markup) {
  const li = document.createElement("li");
  li.role = "option";
  li.id = `il-${name}`;
  li.dataset.name = name;
  li.setAttribute("aria-selected", "false");
  li.tabIndex = -1;
  li.innerHTML = `<svg viewBox="0 0 24 24" aria-hidden="true">${markup}</svg><span></span>`;
  li.lastChild.textContent = name;
  return li;
}

/** Draw the next page of results: their chunks first, so a cell never shows without its icon. */
let drawing = null;
async function drawMore() {
  if (!results || shown >= results.length || drawing) return drawing;
  const gen = generation;
  const rows = results.slice(shown, shown + PAGE);
  drawing = (async () => {
    const marks = await Promise.all(rows.map(markupOf));
    if (gen !== generation) return;
    const frag = document.createDocumentFragment();
    rows.forEach((r, i) => frag.append(cell(r[0], marks[i])));
    grid.append(frag);
    shown += rows.length;
  })().finally(() => { drawing = null; });
  await drawing;
  if (gen === generation && more.getBoundingClientRect().top < innerHeight + 600) drawMore();
}

let generation = 0;
let announce = 0;
async function show() {
  const gen = ++generation;
  const text = q.value.trim();
  const found = search(text);
  // The first page's icons before the grid changes: the old cells stay until the new ones can draw.
  await Promise.all(found.slice(0, PAGE).map(markupOf));
  if (gen !== generation) return;
  results = found;
  const frag = document.createDocumentFragment();
  const first = found.slice(0, PAGE);
  const marks = await Promise.all(first.map(markupOf));
  first.forEach((r, i) => frag.append(cell(r[0], marks[i])));
  grid.replaceChildren(frag);
  shown = first.length;
  active = null;
  roving(grid.firstElementChild);
  empty.hidden = found.length > 0;
  const where = cat === "" ? "" : ` in ${index.categories[Number(cat)][1]}`;
  const said = text ? `${fmt.format(found.length)} ${found.length === 1 ? "icon matches" : "icons match"} “${text}”${where}` : `${fmt.format(found.length)} icons${where}`;
  // Polite: the count settles before it's read, not at every keystroke.
  clearTimeout(announce);
  announce = setTimeout(() => { count.textContent = said; }, text ? 450 : 0);
  drawMore();
}

let typing = 0;
q.addEventListener("input", () => { clearTimeout(typing); typing = setTimeout(() => ready.then(show), 60); });
q.addEventListener("keydown", (e) => {
  if (e.key === "ArrowDown" && grid.firstElementChild) { e.preventDefault(); focusCell(active ?? grid.firstElementChild); }
  if (e.key === "Enter" && grid.firstElementChild) { e.preventDefault(); open(grid.firstElementChild.dataset.name); }
});
for (const b of cats) {
  b.addEventListener("click", () => {
    cat = b.dataset.cat;
    for (const x of cats) x.setAttribute("aria-pressed", String(x === b));
    ready.then(show);
  });
}

new IntersectionObserver((es) => { if (es.some((e) => e.isIntersecting)) drawMore(); }, { rootMargin: "600px 0px" }).observe(more);

// ---- keys: one cell in the tab order, arrows move it --------------------------------------------

let active = null;
function roving(li) {
  if (active && active !== li) { active.tabIndex = -1; active.setAttribute("aria-selected", "false"); }
  active = li;
  if (li) { li.tabIndex = 0; li.setAttribute("aria-selected", "true"); }
}
function focusCell(li) {
  if (!li) return;
  roving(li);
  li.focus();
  li.scrollIntoView({ block: "nearest" });
}
roving(grid.firstElementChild);

/** Cells per row now (the grid reflows with the window and the text size). */
function columns() {
  const kids = grid.children;
  const top = kids[0]?.offsetTop;
  let n = 0;
  while (n < kids.length && kids[n].offsetTop === top) n++;
  return Math.max(1, n);
}

grid.addEventListener("keydown", (e) => {
  const li = e.target.closest?.("li[role=option]");
  if (!li) return;
  const kids = [...grid.children];
  const i = kids.indexOf(li), cols = columns();
  const rows = Math.max(1, Math.floor(innerHeight / (li.offsetHeight || 1)) - 1);
  const to = {
    ArrowRight: i + 1, ArrowLeft: i - 1, ArrowDown: i + cols, ArrowUp: i - cols,
    Home: e.ctrlKey ? 0 : i - (i % cols), End: e.ctrlKey ? kids.length - 1 : Math.min(kids.length - 1, i - (i % cols) + cols - 1),
    PageDown: i + cols * rows, PageUp: i - cols * rows,
  }[e.key];
  if (e.key === "Enter" || e.key === " ") { e.preventDefault(); open(li.dataset.name); return; }
  if (to === undefined) return;
  e.preventDefault();
  if (e.key === "ArrowUp" && i < cols) { q.focus(); return; }
  focusCell(kids[Math.max(0, Math.min(kids.length - 1, to))]);
  if (to >= kids.length - cols * 2) drawMore();
});
grid.addEventListener("click", (e) => {
  const li = e.target.closest("li[role=option]");
  if (li) { roving(li); open(li.dataset.name); }
});

// ---- the details -------------------------------------------------------------------------------

const dlg = $("il-detail");
const esc = (s) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

function snippet(label, code, note = "") {
  const li = document.createElement("li");
  li.innerHTML = `<span class="il-s-label"></span><div class="il-s-row"><code></code><button type="button" class="il-copy">Copy</button></div>${note ? '<span class="il-s-note"></span>' : ""}`;
  li.querySelector(".il-s-label").textContent = label;
  if (note) li.querySelector(".il-s-note").textContent = note;
  li.querySelector("code").textContent = code;
  const b = li.querySelector("button");
  b.setAttribute("aria-label", `Copy ${label.toLowerCase()}`);
  b.addEventListener("click", async () => {
    try { await navigator.clipboard.writeText(code); b.textContent = "Copied"; } catch { b.textContent = "Select it"; }
    setTimeout(() => { b.textContent = "Copy"; }, 1400);
  });
  return li;
}

/** The live figure: a raw document for the full runtime (the recipe sandbox expands the pictogram;
 * the icon comes as its compact file from beside the runtime). */
function sampleDoc(name) {
  return {
    datars: 1,
    id: "icon-sample",
    title: `${name}, as a pictogram's units`,
    description: `The ${name} icon drawn as a pictogram: 9 in North, 6 in Centre and 4 in South.`,
    size: { width: 420, height: 150 },
    locale: "en",
    data: { rows: { values: { area: ["North", "Centre", "South"], n: [9, 6, 4] }, key: ["area"] } },
    scene: {
      kind: "group", key: "root", layout: { type: "stack", padding: [10, 12, 8, 8] },
      children: [{ kind: "use", recipe: "@datars/std/pictogram", key: "chart", params: { data: "rows", value: "n", category: "area", icons: { North: name, Centre: name, South: name }, note: false } }],
    },
  };
}

let runtime = null;
let view = null;
async function live(name) {
  runtime ??= import(new URL("runtime/datars.js", SITE).href);
  await runtime;
  const box = $("il-d-view");
  if (!view) {
    view = document.createElement("datars-view");
    view.setAttribute("no-controls", "");
    view.setAttribute("height", String(Math.round(box.clientHeight)));
    view.setAttribute("mode", document.documentElement.dataset.theme === "dark" ? "dark" : "light");
    view.setAttribute("aria-label", `A pictogram drawn with the ${name} icon`);
    box.append(view);
  }
  view.setAttribute("aria-label", `A pictogram drawn with the ${name} icon`);
  await view.setDocument(sampleDoc(name));
}
new MutationObserver(() => view?.setAttribute("mode", document.documentElement.dataset.theme === "dark" ? "dark" : "light"))
  .observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

let opener = null;
async function open(name) {
  await ready;
  const row = byName.get(name);
  if (!row) return;
  const markup = await markupOf(row);
  opener = document.activeElement;
  $("il-d-name").textContent = name;
  $("il-d-svg").innerHTML = markup;
  for (const s of dlg.querySelectorAll(".il-sw span")) s.innerHTML = `<svg viewBox="0 0 24 24" aria-hidden="true">${markup}</svg>`;
  const catNames = row[2].map((i) => index.categories[i][1]);
  const meta = $("il-d-meta");
  meta.innerHTML = "";
  const lic = row[4]
    ? `Licence: MIT — drawn from <a href="https://feathericons.com">Feather</a> (© Cole Bemis), in Lucide`
    : `Licence: ISC — <a href="https://lucide.dev/license">Lucide</a> {{v}}`.replace("{{v}}", esc(index.version));
  meta.innerHTML = `${catNames.length ? `<span>${catNames.map(esc).join(" · ")}</span>` : ""}${row[1] ? `<span class="il-tags">${row[1].split(" ").map(esc).join(", ")}</span>` : ""}<span>${lic}</span>`;
  const token = index.tokens[name];
  const role = name.split("-")[0];
  $("il-snips").replaceChildren(
    snippet("The icon", `icon("${name}", [x, y], { size: 24 })`),
    token
      ? snippet("Its theme token", `icon("${token}", [x, y])`, `The built-in theme's ${token} — a brand's theme can point it at another icon.`)
      : snippet("As a theme token", `icon("$icon.${role}", [x, y])`, `With "icon.${role}": "lucide:${name}" in your theme's tokens — swap it there, every chart follows.`),
    snippet("As a marker", `geom.symbol({ symbol: "lucide:${name}", x, y, size: 8 })`),
  );
  if (!dlg.open) dlg.showModal();
  live(name).catch((e) => console.warn("icon library: live figure", e));
}
dlg.addEventListener("close", () => { if (opener?.isConnected) opener.focus(); });
dlg.addEventListener("click", (e) => { if (e.target === dlg) dlg.close(); }); // the backdrop
$("il-close").addEventListener("click", () => dlg.close());

// ---- the index, after first paint --------------------------------------------------------------

const ready = new Promise((resolve) => {
  const go = () => fetch(new URL("index.json", LIB)).then((r) => r.json()).then((ix) => {
    index = ix;
    for (const row of ix.icons) byName.set(row[0], row);
    results = ix.icons;
    resolve();
  });
  const idle = () => (window.requestIdleCallback ? requestIdleCallback(go, { timeout: 800 }) : setTimeout(go, 0));
  if (document.readyState === "complete") idle();
  else addEventListener("load", idle, { once: true });
});
ready.then(() => {
  drawMore();
  // A link to one icon (#bike) opens it.
  const h = decodeURIComponent(location.hash.slice(1));
  if (h && byName.has(h)) open(h);
  // Typed before the index came: search now.
  if (q.value.trim()) show();
});
