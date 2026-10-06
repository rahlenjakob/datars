// Embed a document's local recipes (ejected or written in the project) as packages: a `use` node
// with recipe id `@local/<file>/<export>` loads `recipes/<file>.ts` next to the document, bundled
// with the SDK and std left to the engine's sandbox (docs/07-extensibility.md). A package given as
// `{ name, file }` (a plain ES module against the public SDK, shared by several documents) is
// embedded as its source, byte for byte.
import { build } from "esbuild";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// Bundled from the public root whatever the working directory: the source carries the path.
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function localModules(v, out = new Set()) {
  if (Array.isArray(v)) v.forEach((x) => localModules(x, out));
  else if (v && typeof v === "object") {
    if (v.kind === "use" && typeof v.recipe === "string" && v.recipe.startsWith("@local/")) out.add(v.recipe.split("/").slice(0, 2).join("/"));
    Object.values(v).forEach((x) => localModules(x, out));
  }
  return out;
}

/** `{ name, file }` packages → `{ name, source }`, the file read relative to the document. */
function filePackages(doc, docDir) {
  if (!(doc.packages ?? []).some((p) => p.file)) return doc;
  const packages = doc.packages.map((p) => {
    if (!p.file) return p;
    const { file, ...rest } = p;
    const path = resolve(docDir, file);
    if (!existsSync(path)) throw new Error(`package ${p.name}: expected ${path}`);
    return { ...rest, source: readFileSync(path, "utf8") };
  });
  return { ...doc, packages };
}

export async function embedLocalRecipes(input, docDir) {
  const doc = filePackages(input, docDir);
  const mods = [...localModules(doc.scene)];
  if (!mods.length) return doc;
  const packages = (doc.packages ?? []).filter((p) => !mods.includes(p.name));
  for (const name of mods) {
    const file = join(docDir, "recipes", `${name.slice("@local/".length)}.ts`);
    if (!existsSync(file)) throw new Error(`${name}: expected ${file}`);
    const r = await build({ entryPoints: [file], absWorkingDir: root, bundle: true, write: false, format: "esm", platform: "neutral", target: "es2020", logLevel: "error", external: ["@datars/sdk", "@datars/std"] });
    packages.push({ name, source: r.outputFiles[0].text });
  }
  return { ...doc, packages };
}
