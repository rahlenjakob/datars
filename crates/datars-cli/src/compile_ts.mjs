// Compile a TypeScript document in a user's project and print its JSON IR. The CLI carries this
// script and runs it with Node when the document's project has @datars/sdk installed: imports
// (@datars/sdk, @datars/std, the project's own modules) resolve from the project's node_modules,
// with the project's esbuild — nothing from the datars repo. Local recipes (`@local/<file>/…`,
// `recipes/<file>.ts` next to the document) and `{ name, file }` packages are embedded, as
// scripts/local-recipes.mjs does inside the repo.
//
//   node compile_ts.mjs path/to/doc.ts
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const src = resolve(process.argv[2]);
const docDir = dirname(src);
const require = createRequire(join(docDir, "__datars__.js"));
let esbuildPath;
try {
  esbuildPath = require.resolve("esbuild");
} catch {
  process.stderr.write(`${src}: compiling a TypeScript document needs esbuild in its project (npm install --save-dev esbuild)\n`);
  process.exit(2);
}
const { build } = await import(pathToFileURL(esbuildPath).href);

const dir = mkdtempSync(join(tmpdir(), "datars-doc-"));
try {
  const out = join(dir, "doc.mjs");
  // A syntax or import error: esbuild has printed it, with the line and a caret. Nothing to add.
  await build({ entryPoints: [src], bundle: true, format: "esm", platform: "node", outfile: out, logLevel: "error", absWorkingDir: docDir }).catch(() => process.exit(1));
  const mod = await import(pathToFileURL(out).href);
  process.stdout.write(JSON.stringify(await embedLocalRecipes(mod.default, docDir)));
} catch (e) {
  // An error the document threw while building its graph: the message, not the bundler's stack.
  process.stderr.write(`${src}: ${e instanceof Error ? e.message : String(e)}\n`);
  process.exitCode = 1;
} finally {
  rmSync(dir, { recursive: true, force: true });
}

function localModules(v, out = new Set()) {
  if (Array.isArray(v)) v.forEach((x) => localModules(x, out));
  else if (v && typeof v === "object") {
    if (v.kind === "use" && typeof v.recipe === "string" && v.recipe.startsWith("@local/")) out.add(v.recipe.split("/").slice(0, 2).join("/"));
    Object.values(v).forEach((x) => localModules(x, out));
  }
  return out;
}

async function embedLocalRecipes(input, docDir) {
  let doc = input;
  if ((doc.packages ?? []).some((p) => p.file)) {
    doc = { ...doc, packages: doc.packages.map((p) => {
      if (!p.file) return p;
      const { file, ...rest } = p;
      const path = resolve(docDir, file);
      if (!existsSync(path)) throw new Error(`package ${p.name}: expected ${path}`);
      return { ...rest, source: readFileSync(path, "utf8") };
    }) };
  }
  const mods = [...localModules(doc.scene)];
  if (!mods.length) return doc;
  const packages = (doc.packages ?? []).filter((p) => !mods.includes(p.name));
  for (const name of mods) {
    const file = join(docDir, "recipes", `${name.slice("@local/".length)}.ts`);
    if (!existsSync(file)) throw new Error(`${name}: expected ${file}`);
    const r = await build({ entryPoints: [file], absWorkingDir: docDir, bundle: true, write: false, format: "esm", platform: "neutral", target: "es2020", logLevel: "error", external: ["@datars/sdk", "@datars/std"] });
    packages.push({ name, source: r.outputFiles[0].text });
  }
  return { ...doc, packages };
}
