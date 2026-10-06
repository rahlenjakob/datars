// Compile every examples/<name>/doc.ts to examples/<name>/doc.json (the document IR).
//   node scripts/build-examples.mjs [filter]
import { build } from "esbuild";
import { readdirSync, existsSync, writeFileSync, mkdirSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { embedLocalRecipes } from "./local-recipes.mjs";

const root = resolve(new URL("..", import.meta.url).pathname);
const filter = process.argv[2] ?? "";
const tmp = join(root, "out", "examples-tmp");
mkdirSync(tmp, { recursive: true });
let n = 0;
for (const name of readdirSync(join(root, "examples")).sort()) {
  const src = join(root, "examples", name, "doc.ts");
  if (!existsSync(src) || !name.includes(filter)) continue;
  const out = join(tmp, `${name}.mjs`);
  await build({ entryPoints: [src], bundle: true, format: "esm", platform: "node", outfile: out, logLevel: "error",
    alias: { "@datars/sdk": join(root, "packages/sdk/src/index.ts"), "@datars/std": join(root, "packages/std/src/index.ts") } });
  const mod = await import(pathToFileURL(out).href + `?t=${Date.now()}`);
  const doc = await embedLocalRecipes(mod.default, join(root, "examples", name));
  writeFileSync(join(root, "examples", name, "doc.json"), JSON.stringify(doc, null, 1) + "\n");
  n++;
}
rmSync(tmp, { recursive: true, force: true });
console.log(`built ${n} example document(s)`);
