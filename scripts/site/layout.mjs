// The site's shared chrome: the head (title, description, canonical, Open Graph), the nav, the
// footer, and the docs' sidebar and "On this page" list. Pages are written with root-absolute
// links (`/docs/`); `relativize` turns them into page-relative ones so the site works under any
// subpath (GitHub Pages) and from a plain folder.

import { escapeAttr, escapeHtml } from "./markdown.mjs";

/** The strengths, in the order the home page and the menus list them. */
export const FEATURES = [
  { slug: "charts", name: "Batteries included", blurb: "Ready-made recipes: bars to sankeys, maps to cards", icon: '<rect x="3" y="11" width="4" height="9" rx="1"/><rect x="10" y="6" width="4" height="14" rx="1"/><rect x="17" y="3" width="4" height="17" rx="1"/>' },
  { slug: "data", name: "Data from anywhere", blurb: "In the bundle, from your API, live, from the app, generated", icon: '<ellipse cx="12" cy="5.5" rx="7" ry="2.5"/><path d="M5 5.5v6c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5v-6"/><path d="M5 11.5v6c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5v-6"/>' },
  { slug: "theming", name: "Your brand", blurb: "Tokens, fonts, light, dark and high contrast — at runtime", icon: '<circle cx="8" cy="9" r="3.2"/><circle cx="16" cy="9" r="3.2"/><circle cx="12" cy="15.5" r="3.2"/>' },
  { slug: "big-data", name: "Big data, 60 fps", blurb: "Four million points, streamed by the tile, drawn by the GPU", icon: '<circle cx="5" cy="17" r="1.3"/><circle cx="9" cy="12" r="1.3"/><circle cx="13" cy="15" r="1.3"/><circle cx="16" cy="8" r="1.3"/><circle cx="19" cy="5" r="1.3"/><circle cx="7" cy="7" r="1.3"/><circle cx="18" cy="16" r="1.3"/>' },
  { slug: "platforms", name: "Every platform", blurb: "Web, iOS, Android, desktop, video, PNG, SVG and PDF", icon: '<rect x="2.5" y="4" width="13" height="10" rx="1.6"/><rect x="15.5" y="8" width="6" height="12" rx="1.4"/><path d="M6 18h6"/>' },
  { slug: "maps", name: "Maps, no tile server", blurb: "Automatic basemaps, world to street corner, no API key", icon: '<path d="M9 4 3 6.5v13L9 17l6 2.5 6-2.5v-13L15 6.5 9 4z"/><path d="M9 4v13M15 6.5v13"/>' },
  { slug: "animation", name: "Animation included", blurb: "Every change morphs, by key — stories steer themselves", icon: '<rect x="3" y="11" width="4" height="9" rx="1"/><circle cx="17" cy="9" r="4"/><path d="M8.5 8.5c2-3 4.5-4 6-4"/>' },
  { slug: "interaction", name: "Interactive", blurb: "Hover, click, brush, drag and zoom — linked views without glue code", icon: '<path d="M9 11V5.5a1.5 1.5 0 0 1 3 0V11"/><path d="M12 10.5V9a1.5 1.5 0 0 1 3 0v2"/><path d="M15 11a1.5 1.5 0 0 1 3 0v3.5a6 6 0 0 1-6 6h-.6a6 6 0 0 1-4.9-2.5L4 14.4a1.5 1.5 0 0 1 2.4-1.8L9 15V11"/>' },
  { slug: "accessibility", name: "Accessible", blurb: "Screen readers, keyboard, reduced motion, text alternatives", icon: '<circle cx="12" cy="5.5" r="2.2"/><path d="M5 9.5l7 1.5 7-1.5M12 11v4l-3 5M12 15l3 5"/>' },
  { slug: "extensibility", name: "Extendable", blurb: "Recipes are TypeScript: eject one, or draw a scene of your own", icon: '<path d="M8 6 3 12l5 6M16 6l5 6-5 6M13.5 4l-3 16"/>' },
  { slug: "delivery", name: "Charts are content", blurb: "Publish to any static host; republish, every embed updates", icon: '<path d="M12 3l8 4.5v9L12 21l-8-4.5v-9z"/><path d="M4 7.5l8 4.5 8-4.5M12 12v9"/>' },
  { slug: "developers", name: "For developers and agents", blurb: "Live page, lint, explain, film, profile, MCP, llms.txt", icon: '<path d="M4 6h16v12H4z"/><path d="M7 10l3 2-3 2M12 15h5"/>' },
];

/** The docs, in reading order: [section, [[slug, title], …]]. `slug` is the path under /docs/. A
 * third field names a parent (`"sdk"`): the entry is listed under it only while one of its pages is
 * open, and joins the reading order (previous / next) either way. */
export const DOCS_NAV = [
  ["Start here", [["", "Overview"], ["getting-started", "Getting started"], ["react", "React, Vite and Next.js"], ["python", "Python and Jupyter"], ["concepts", "Core concepts"]]],
  ["Write charts", [["documents", "Documents"], ["data", "Data and tables"], ["charts", "Charts, scales and marks"], ["expressions", "Expressions"], ["interaction", "Signals and interaction"], ["stories", "States and stories"], ["motion", "Motion"]]],
  ["Style", [["theming", "Themes and brands"]]],
  ["Maps and big data", [["maps", "Maps"], ["big-data", "Big data"]]],
  ["Extend", [["custom-recipes", "Custom recipes and scenes"]]],
  ["Ship", [["publishing", "Publishing and hosting"], ["embed/web", "Web: <datars-view>"], ["embed/ios", "iOS and macOS"], ["embed/android", "Android"], ["export", "Video, images and PDF"]]],
  ["Quality", [["accessibility", "Accessibility"], ["tools", "Preview, check and debug"], ["agents", "Coding agents and MCP"]]],
  ["Reference", [
    ["std/", "Chart reference"],
    ["sdk", "SDK reference"], ["sdk/documents", "Documents", "sdk"], ["sdk/data", "Data", "sdk"], ["sdk/nodes", "Nodes", "sdk"], ["sdk/options", "Node options and scales", "sdk"],
    ["sdk/props", "Props and expressions", "sdk"], ["sdk/motion", "Motion", "sdk"], ["sdk/programs", "Programs", "sdk"], ["sdk/themes", "Themes", "sdk"], ["sdk/recipes", "Recipes", "sdk"],
    ["cli", "CLI"], ["ir", "Document format"],
  ]],
];

const MARK = '<svg class="mark" viewBox="0 0 28 28" aria-hidden="true"><rect x="3" y="14" width="5.5" height="11" rx="1.6" fill="#4f7cff"/><rect x="11.25" y="8" width="5.5" height="17" rx="1.6" fill="#f5b53d"/><rect x="19.5" y="3" width="5.5" height="22" rx="1.6" fill="#ff6b5b"/></svg>';
const FAVICON = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 28 28'%3E%3Crect x='3' y='14' width='5.5' height='11' rx='1.6' fill='%234f7cff'/%3E%3Crect x='11.25' y='8' width='5.5' height='17' rx='1.6' fill='%23f5b53d'/%3E%3Crect x='19.5' y='3' width='5.5' height='22' rx='1.6' fill='%23ff6b5b'/%3E%3C/svg%3E";

export const featureIcon = (f) => `<svg viewBox="0 0 24 24" aria-hidden="true">${f.icon}</svg>`;

/** `/docs/maps/` from `docs/maps/index.html`; `/` from `index.html`. */
export function urlOf(file) {
  return "/" + file.replace(/index\.html$/, "").replace(/\.html$/, ".html");
}

/** Root-absolute `href`/`src` (and `srcset`, `poster`, `data-src`) → relative to the page at `file`. */
export function relativize(html, file, base) {
  const depth = file.split("/").length - 1;
  // `base`: an absolute prefix instead (the 404 page is served at any depth).
  const up = base ?? (depth ? "../".repeat(depth) : "./");
  // Code samples keep their text as written (`<script src="/runtime/datars.js">` in an example).
  return html.split(/(<pre[\s\S]*?<\/pre>|<code[\s\S]*?<\/code>)/).map((part, i) => (i % 2 ? part : part.replace(/\b(href|src|poster|data-src|content)="\/(?!\/)([^"]*)"/g, (all, attr, rest) => {
    if (attr === "content") return all; // meta content stays absolute (canonical, og)
    return `${attr}="${up}${rest}"`;
  }))).join("");
}

const THEME_BOOT = `<script>try{var t=localStorage.getItem("datars-theme");if(t)document.documentElement.dataset.theme=t}catch(e){}</script>`;

function head({ title, description, url, site, ogImage, charts, jsonld, noindex, css = [] }) {
  const abs = site + url;
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(title)}</title>
<meta name="description" content="${escapeAttr(description)}">
<link rel="canonical" href="${escapeAttr(abs)}">
<meta property="og:type" content="website">
<meta property="og:site_name" content="datars">
<meta property="og:title" content="${escapeAttr(title)}">
<meta property="og:description" content="${escapeAttr(description)}">
<meta property="og:url" content="${escapeAttr(abs)}">
<meta property="og:image" content="${escapeAttr(`${site}/${ogImage}`)}">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta name="twitter:card" content="summary_large_image">
${noindex ? '<meta name="robots" content="noindex">\n' : ""}<meta name="color-scheme" content="light dark">
<link rel="icon" href="${FAVICON}">
${THEME_BOOT}
<link rel="preload" href="/fonts/Inter-Regular.ttf" as="font" type="font/ttf" crossorigin>
<link rel="preload" href="/fonts/Inter-SemiBold.ttf" as="font" type="font/ttf" crossorigin>
<link rel="preload" href="/fonts/Inter-Bold.ttf" as="font" type="font/ttf" crossorigin>
<link rel="stylesheet" href="/site.css">
${css.map((f) => `<link rel="stylesheet" href="/${escapeAttr(f)}">\n`).join("")}${charts ? '<link rel="modulepreload" href="/runtime/datars.js">\n<script type="module" src="/runtime/datars.js"></script>\n' : ""}<script type="module" src="/site.js"></script>
${jsonld ? `<script type="application/ld+json">${JSON.stringify(jsonld)}</script>\n` : ""}</head>`;
}

/** "Make it yours" in the header: the reader restyles every chart on the site (site.js keeps the
 * choice and applies it); more choices on the home page, a whole theme in the studio (/themes/). */
const LOOK_PRESETS = [["default", "Default", "#4269d0", "#efb118", "#ff725c"], ["newsprint", "Newsprint", "#b3261e", "#1d3f6e", "#f4eee2"], ["nordic", "Nordic", "#0f766e", "#6cb8d0", "#e8a33d"], ["neon", "Neon", "#ff2bd6", "#22e5ff", "#0b0620"], ["sunset", "Sunset", "#e8590c", "#c2185b", "#fff4ea"]];
const LOOK_ACCENTS = [["#4269d0", "Blue"], ["#0f766e", "Teal"], ["#2f8f5b", "Green"], ["#e8a33d", "Amber"], ["#e8590c", "Orange"], ["#c8423b", "Red"], ["#c2185b", "Magenta"], ["#6a3fb5", "Violet"]];
export const lookPresets = (cls = "") => LOOK_PRESETS.map(([id, name, a, b, c]) => `<button type="button" class="${cls}" data-look-preset="${id}" aria-pressed="${id === "default"}"><span class="sw" style="--a:${a};--b:${b};--c:${c}"></span>${name}</button>`).join("");
export const lookAccents = () => `${LOOK_ACCENTS.map(([c, name]) => `<button type="button" class="dot" data-look-accent="${c}" style="--dot:${c}" aria-label="${name}" title="${name}" aria-pressed="false"></button>`).join("")}<label class="dot custom" title="Any colour"><input type="color" data-look-custom value="#4269d0" aria-label="Any colour"></label><button type="button" class="dot none" data-look-accent="" aria-label="The preset's accent" title="The preset's accent" aria-pressed="true"></button>`;
const LOOK_MENU = `<details class="dd look" id="look-menu">
        <summary class="icon-btn" aria-label="Make it yours: restyle every chart" title="Make it yours"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3.2a8.8 8.8 0 1 0 0 17.6c1.2 0 1.8-.8 1.8-1.7 0-.5-.2-.9-.5-1.2-.3-.4-.5-.8-.5-1.3 0-1 .8-1.7 1.8-1.7h2.1a4.9 4.9 0 0 0 4.9-4.9c0-3.9-4-6.8-9.6-6.8z"/><circle class="la" cx="7.4" cy="11.6" r="1.7"/><circle class="lb" cx="10.4" cy="7.4" r="1.7"/><circle class="lc" cx="15.2" cy="7.6" r="1.7"/></svg></summary>
        <div class="dd-panel look-panel">
          <p class="look-h"><b>Make it yours</b><span data-look-status>The site's own look.</span></p>
          <p class="look-l">Preset</p>
          <div class="look-presets" role="group" aria-label="Preset">${lookPresets()}</div>
          <p class="look-l">Accent</p>
          <div class="dots" role="group" aria-label="Accent">${lookAccents()}</div>
          <div class="look-foot"><a href="/themes/">Build your own theme →</a><button type="button" class="btn btn-ghost small" data-look-reset disabled>Reset</button></div>
        </div>
      </details>`;

function nav(current, repo) {
  const is = (p) => (current === p || (p !== "/" && current.startsWith(p)) ? ' aria-current="page"' : "");
  const featureLinks = FEATURES.map((f) => `<a href="/features/${f.slug}/"${is(`/features/${f.slug}/`)}>${featureIcon(f)}<span><b>${escapeHtml(f.name)}</b><small>${escapeHtml(f.blurb)}</small></span></a>`).join("");
  const main = [["/gallery/", "Gallery"], ["/themes/", "Theme studio"], ["/docs/", "Docs"], ["/performance/", "Performance"], ["/why/", "Why datars"], ["/articles/", "Articles"]];
  return `<a class="skip" href="#main">Skip to content</a>
<header class="nav">
  <div class="wrap nav-inner">
    <a class="brand" href="/" aria-label="datars home">${MARK}<span>datars</span></a>
    <nav class="links" aria-label="Main">
      <details class="dd">
        <summary${current.startsWith("/features/") ? ' aria-current="page"' : ""}>Features</summary>
        <div class="dd-panel">${featureLinks}<a class="dd-all" href="/features/">All features →</a></div>
      </details>
      ${main.map(([h, t]) => `<a href="${h}"${is(h)}>${t}</a>`).join("\n      ")}
    </nav>
    <div class="nav-actions">
      ${LOOK_MENU}
      <button class="icon-btn" id="theme-toggle" type="button" aria-label="Switch light or dark mode" title="Light / dark">
        <svg viewBox="0 0 24 24" class="sun" aria-hidden="true"><circle cx="12" cy="12" r="4.2"/><path d="M12 2.5v2.2M12 19.3v2.2M4.6 4.6l1.6 1.6M17.8 17.8l1.6 1.6M2.5 12h2.2M19.3 12h2.2M4.6 19.4l1.6-1.6M17.8 6.2l1.6-1.6"/></svg>
        <svg viewBox="0 0 24 24" class="moon" aria-hidden="true"><path d="M20 14.6A8.5 8.5 0 0 1 9.4 4a8.5 8.5 0 1 0 10.6 10.6z"/></svg>
      </button>
      <a class="btn btn-ghost small gh" href="${repo}">GitHub</a>
      <details class="burger">
        <summary aria-label="Menu"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M4 12h16M4 17h16"/></svg></summary>
        <div class="sheet">
          <p class="sheet-h">Features</p>
          <div class="sheet-features">${FEATURES.map((f) => `<a href="/features/${f.slug}/">${featureIcon(f)}${escapeHtml(f.name)}</a>`).join("")}</div>
          <p class="sheet-h">Site</p>
          <div class="sheet-main">${main.map(([h, t]) => `<a href="${h}">${t}</a>`).join("")}<a href="${repo}">GitHub</a></div>
        </div>
      </details>
    </div>
  </div>
</header>`;
}

function footer(repo, runtime) {
  const docs = [["/docs/getting-started/", "Getting started"], ["/docs/std/", "Chart reference"], ["/docs/embed/web/", "Embed on the web"], ["/docs/embed/ios/", "iOS and macOS"], ["/docs/embed/android/", "Android"], ["/docs/cli/", "CLI reference"]];
  const project = [["/why/", "Why datars"], ["/performance/", "Performance"], ["/under-the-hood/", "Under the hood"], ["/gallery/", "Gallery"], ["/articles/", "Example articles"], ["/llms.txt", "llms.txt"], [repo, "Source on GitHub"]];
  const col = (h, links) => `<div><p class="f-h">${h}</p>${links.map(([u, t]) => `<a href="${u}">${escapeHtml(t)}</a>`).join("")}</div>`;
  return `<footer class="footer">
  <div class="wrap">
    <div class="f-cols">
      <div class="f-about"><a class="brand" href="/" aria-label="datars home">${MARK}<span>datars</span></a><p>An engine for charts, stories and maps. One document; the same pixels on the web, iOS, Android, desktop, in video and print. © 2026 Jakob Råhlén.</p></div>
      ${col("Features", FEATURES.map((f) => [`/features/${f.slug}/`, f.name]))}
      ${col("Docs", docs)}
      ${col("Project", project)}
    </div>
    <p class="fine">Charts on this site use approximate, illustrative figures unless noted; map data © OpenStreetMap contributors, Natural Earth. This site is static: HTML, a ${runtime} runtime (gzipped, loaded only on pages with charts) and published bundles, built by <code>scripts/build-site.mjs</code>.</p>
  </div>
</footer>`;
}

/** A whole page. `body` is the page's own content (already filled in); `file` its output path. */
export function page({ file, title, description, body, site, repo, runtime, ogImage = "card.png", charts = false, jsonld, noindex, css = [] }) {
  const url = urlOf(file);
  const html = `${head({ title, description, url, site, ogImage, charts, jsonld, noindex, css })}
<body>
${nav(url, repo)}
<main id="main">
${body}
</main>
${footer(repo, runtime)}
</body>
</html>
`;
  // The 404 page is served for any missing URL, at any depth: its links are absolute.
  return relativize(html, file, file === "404.html" ? `${new URL(site).pathname.replace(/\/$/, "")}/` : undefined);
}

/** The docs' sidebar (desktop) and its phone twin (a disclosure above the article). */
export function docsSidebar(currentSlug) {
  const link = ([slug, t, parent]) => {
    if (parent && currentSlug !== parent && !currentSlug.startsWith(`${parent}/`)) return "";
    const href = `/docs/${slug ? (slug.endsWith("/") ? slug : slug + "/") : ""}`;
    const cur = slug === currentSlug || (slug === "std/" && currentSlug.startsWith("std/"));
    return `<a href="${href}"${parent ? ' class="sub"' : ""}${cur ? ' aria-current="page"' : ""}>${escapeHtml(t)}</a>`;
  };
  const sections = DOCS_NAV.map(([h, items]) => `<div class="ds-sec"><p>${escapeHtml(h)}</p>${items.map(link).join("")}</div>`).join("");
  return { desktop: `<nav class="docs-side" aria-label="Documentation">${sections}</nav>`, phone: `<details class="docs-phone-nav"><summary>Documentation menu</summary><nav aria-label="Documentation">${sections}</nav></details>` };
}

/** Previous and next pages in reading order. */
export function docsPrevNext(currentSlug) {
  const flat = DOCS_NAV.flatMap(([, items]) => items);
  const i = flat.findIndex(([s]) => s === currentSlug || (s === "std/" && currentSlug === "std"));
  if (i < 0) return "";
  const href = (s) => `/docs/${s ? (s.endsWith("/") ? s : s + "/") : ""}`;
  const prev = flat[i - 1], next = flat[i + 1];
  return `<nav class="prevnext" aria-label="Previous and next">${prev ? `<a class="pn-prev" href="${href(prev[0])}"><small>Previous</small>${escapeHtml(prev[1])}</a>` : "<span></span>"}${next ? `<a class="pn-next" href="${href(next[0])}"><small>Next</small>${escapeHtml(next[1])}</a>` : "<span></span>"}</nav>`;
}

export function docsToc(headings) {
  const h2 = headings.filter((h) => h.level === 2);
  if (h2.length < 2) return "";
  return `<nav class="docs-toc" aria-label="On this page"><p>On this page</p>${h2.map((h) => `<a href="#${h.id}">${h.text}</a>`).join("")}</nav>`;
}
