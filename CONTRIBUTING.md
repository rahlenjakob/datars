# Contributing to datars

Thanks for your interest in datars. datars is proprietary for now ([LICENSE](LICENSE)), so
**pull requests aren't accepted** — but bug reports, examples that break and ideas are very
welcome as issues and discussions. This page also covers how to build the repository, run the
tests, and the rules every change in it follows.

- **Questions and ideas:** [GitHub Discussions](https://github.com/rahlenjakob/datars/discussions)
- **Bugs and feature requests:** [GitHub Issues](https://github.com/rahlenjakob/datars/issues)
- **Security problems:** please don't open an issue. See [SECURITY.md](SECURITY.md).
- **Conduct:** everyone taking part agrees to the [Code of Conduct](CODE_OF_CONDUCT.md).

A small document that reproduces a bug helps most: it can be checked, fixed and kept as a test.

## Contents

1. [Prerequisites](#prerequisites)
2. [Build and run](#build-and-run)
3. [Tests](#tests)
4. [Goldens: when and how to accept them](#goldens-when-and-how-to-accept-them)
5. [Rules that are not optional](#rules-that-are-not-optional)
6. [Code style](#code-style)
7. [Generated files](#generated-files)
8. [Commits and pull requests](#commits-and-pull-requests)
9. [Where things are](#where-things-are)

## Prerequisites

You need these for the Rust engine, the CLI and the TypeScript packages:

| Tool | Version | Notes |
|---|---|---|
| Rust | latest stable | via [rustup](https://rustup.rs). CI uses stable too |
| Node.js | 22 LTS (20.19 or later also works) | runs the TypeScript SDK/std builds and the JS tests |
| pnpm | 8.15.1 | pinned in `package.json` (`packageManager`); `corepack enable` picks it up |

For the web runtime (`packages/web`, `<datars-view>`), which `datars dev` and the browser tests
need:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128   # must match the workspace's pinned wasm-bindgen
brew install llvm wasi-libc                        # QuickJS is C: a clang with a wasm backend + a libc
```

Not on macOS? Install LLVM/clang and wasi-libc from your package manager. Then set `LLVM` to the
LLVM prefix (the directory containing `bin/clang`) and `WASI_SYSROOT` to the wasi-sysroot
directory. See the top of `scripts/build-wasm.sh`.

Optional, depending on what you work on:

| For | You need |
|---|---|
| iOS/macOS (`apple/DatarsKit`) | Xcode, plus `rustup target add aarch64-apple-darwin x86_64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim` |
| Android (`android/`) | the Android SDK and NDK (`ANDROID_HOME`, optionally `ANDROID_NDK_HOME`), plus `rustup target add aarch64-linux-android x86_64-linux-android` |
| `datars video` | `ffmpeg` on your `PATH` |
| The Python package (`packages/python`) | Python 3.9 or later; its README uses [uv](https://docs.astral.sh/uv/) |
| Browser tests and `scripts/perf-browser.mjs` | Google Chrome |
| `datars gpu` | a machine with a GPU (Metal, Vulkan or DX12) |

## Build and run

```sh
cargo build --release -p datars-cli           # the `datars` binary → target/release/datars
pnpm install
bash scripts/build-js.sh                      # @datars/sdk + @datars/std, copied into the engine
node scripts/build-examples.mjs               # examples/*/doc.ts → doc.json
```

Render a state, or look at a transition:

```sh
target/release/datars render examples/votes/doc.json --state 2 --out out/votes.png
target/release/datars film   examples/votes/doc.json --from 0 --to 1
```

### The dev server

`datars dev` serves a live page that rebuilds on save and morphs the new version in, with the
program's states, preview widths and light/dark mode, diagnostics and lint findings, and an
explanation of any mark you click (recipes, data row, expression values), and a Profile tab (live
frame times, each transition scored, `datars profile` re-run on every save). Working on the engine
or a recipe, it's the quickest way to see what a change does — and what it costs. It needs the web runtime to be built once:

```sh
bash scripts/build-wasm.sh && pnpm -C packages/web build
target/release/datars dev examples/votes/doc.ts          # → http://127.0.0.1:8788/
target/release/datars dev examples/votes/doc.ts --port 9000
```

It accepts `doc.ts` or `doc.json`. `target/release/datars help` lists every command (`check`,
`lint`, `inspect`, `explain`, `profile`, `publish`, `serve`, …).

### Other targets

```sh
bash scripts/build-apple.sh      # DatarsFFI.xcframework for apple/DatarsKit (macOS, needs Xcode)
bash scripts/build-android.sh    # libdatars_ffi.so for android/datars
node scripts/build-site.mjs      # the showcase site → out/pages
```

## Tests

Run the suites that cover what you changed. CI runs all of these on every pull request, with
clippy's determinism lints (`disallowed_methods`, `disallowed_types`) as errors.

```sh
cargo test --workspace                          # all Rust unit + integration tests
cargo test -p datars-scene                      # one crate
cargo clippy --workspace                        # includes the determinism lints (clippy.toml)
target/release/datars test                      # visual + motion goldens over examples/
node --test packages/sdk/test/*.test.js packages/std/test/*.test.js   # SDK + std (after build-js.sh)
```

Also available, when your change touches these areas:

```sh
target/release/datars test votes                # only examples whose name contains "votes"
target/release/datars gpu                       # wgpu vs the CPU reference (ΔE); needs a GPU
target/release/datars budgets                   # bundle + runtime sizes against budgets.json
node --test packages/web/test/*.test.mjs        # wasm runtime: goldens + delivery (after build-wasm.sh
                                                #   and `pnpm -C packages/web build`; some need Chrome)
(cd apple/DatarsKit && swift test)              # Swift package (after scripts/build-apple.sh)
scripts/test-android.sh                         # C ABI goldens on an Android emulator
```

`datars test` compares exact hashes: every example state, a 64-sample sweep of every transition,
and motion invariants (no flashes, pops, NaNs, exact endpoints). When something differs it prints
what changed and writes artifacts (current snapshots, filmstrips, `report.json`) to `out/test/`.
The design is in [docs/13-testing.md](docs/13-testing.md).

## Goldens: when and how to accept them

The goldens in `tests/golden/` are the cross-target oracle. The wasm runtime, iOS, Android and the
bundle-tier tests all compare against them, so a bad golden hides a regression everywhere at once.
Accept new goldens only when:

1. **every other test is green**, `cargo test --workspace` first;
2. you have **looked at the renders** (the snapshots and strips in `out/test/`, or
   `datars render` / `datars film` of the affected states) and the change is the one you intended.

Then update them, only for the examples you meant to change:

```sh
target/release/datars test <filter> --update
```

Say in the pull request which goldens changed and why.

## Rules that are not optional

These come from [docs/02-principles.md](docs/02-principles.md). Reviews check them, and some are
enforced by tests and lints.

- **Determinism (P1).** Engine crates never call `f64::sin/cos/tan/atan2/exp/ln/powf/cbrt/hypot/…`.
  Use `datars_math::m::*` instead. `sqrt`, `floor`, `ceil`, `round` and `abs` are fine. No
  `std::collections::HashMap`/`HashSet` in engine crates: use `BTreeMap`/`BTreeSet`. No time,
  and no randomness except `datars_math::Rng` with an explicit seed. `clippy.toml` bans the usual
  suspects.
- **Sans-IO (P10).** Engine crates do no IO: bytes in, values out.
- **No chart vocabulary in engine crates (P4).** Words like bar, pie, axis, legend and story
  belong in the standard library (`packages/std`). `datars-algo` algorithms are fine, because
  they're algorithms.
- **Colours are inks.** Scene colours are `datars_theme::Ink` (`"#hex"`, `"$token"`,
  `"$palette[i]"`), resolved at flatten time. Never bake theme colours into scenes.
- **Every feature has a test.** Unit tests go in the crate, integration tests in its `tests/`. A
  new example also becomes part of the golden suite.
- **Dependencies (P15).** Declare them once in `[workspace.dependencies]` in the root
  `Cargo.toml`. They should be pure Rust (or vetted C), deterministic, wasm-friendly and
  permissively licensed. Ask in an issue before adding a heavy one.

## Code style

**Match the surrounding code.** Keep modules small, and write doc comments that say *why* and not
just *what*. The code is not formatted with `rustfmt` and CI doesn't check formatting, so please
don't reformat files you aren't otherwise changing: whole-file reformatting buries the real
change in the diff.

For TypeScript in `packages/*`, follow the style of the package you're editing. Std recipes are
written against the public SDK, the same API any user has (P5).

## Generated files

Some committed files are generated. Regenerate them in the same change as their sources:

| File(s) | Regenerate with | When |
|---|---|---|
| `crates/datars-engine/js/sdk.js`, `std.js` | `bash scripts/build-js.sh` | after changing `packages/sdk` or `packages/std` |
| `llms.txt`, `docs/reference/std.md`, `docs/reference/ir.schema.json` | `target/release/datars docs` | after changing recipes, their params or docs, the IR, or the CLI help. A test fails when they're stale |
| `examples/*/doc.json` | `node scripts/build-examples.mjs [filter]` | after changing an example's `doc.ts` |
| `tests/golden/**` | `target/release/datars test <filter> --update` | see [the goldens policy](#goldens-when-and-how-to-accept-them) |
| `budgets.json` | `target/release/datars budgets --update` | only when a size change is intended and explained |

## Commits and pull requests

- Write the subject as a sentence that says what changed, in the present tense, optionally
  prefixed with the area, like `Map tiles: styling budgeted by features` or
  `datars serve: gzip whole responses`. Use the body for the *why*, measurements, and anything a
  reviewer should look at.
- Keep one logical change per pull request. Put mechanical changes (renames, regenerated files)
  in separate commits from behavioural ones where you can.
- Fill in the pull request checklist: tests, goldens looked at, generated files refreshed,
  determinism rules.
- The code is proprietary ([LICENSE](LICENSE)): changes come from the maintainer, and outside
  pull requests are closed unmerged. Please open an issue instead.

## Where things are

- **Design docs:** [docs/README.md](docs/README.md) is the index and reading order. Start with
  [docs/02-principles.md](docs/02-principles.md).
- **What's built and how it's verified:** [docs/19-status.md](docs/19-status.md).
- **Crate API contracts:** [docs/dev/contracts.md](docs/dev/contracts.md).
- **Guides:** [docs/guides/](docs/guides/) (getting started, React, Python).
- **For coding agents:** [llms.txt](llms.txt) and [docs/reference/std.md](docs/reference/std.md).
  The same tools are available over MCP (`datars-mcp`).

| Path | What |
|---|---|
| `crates/datars-{math,color,theme,scene,render,text,data,expr,algo,layout,graph,motion,geo,ir}` | the engine's layers (deterministic, sans-IO) |
| `crates/datars-render-{cpu,svg,pdf,wgpu}` | backends |
| `crates/datars-engine`, `datars-sandbox`, `datars-bundle`, `datars-build`, `datars-runtime` | the engine, the QuickJS sandbox, bundles, the publish compiler, the installed runtime |
| `crates/datars-host-{web,native}`, `datars-ffi` | hosts and the C ABI |
| `crates/datars-{headless,test,devtools,mcp,cli,geo-build}` | tools |
| `packages/*` | npm packages (`sdk`, `std`, `web`, `react`, `vite`, `next`, `compile`) and the Python package |
| `apple/`, `android/` | Swift package and Android library |
| `examples/` | example documents, which are also the golden test corpus |
| `site/` | the showcase site and articles |
