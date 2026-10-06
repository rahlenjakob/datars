#!/usr/bin/env node
// Native frame rates: the sample apps (apple/DatarsSample.swiftpm, android/sample) step published
// charts through every state by themselves (DATARS_BENCH / the `bench` extra) and log each
// transition's frame profile — frames, fps, worst gap, dropped 60 Hz frames, CPU per frame — which
// this collects into site/perf/native.json for the performance page.
//
//   node scripts/bench-native.mjs [--ios] [--android] [--charts votes,descent,galaxy,scatter]
//
// Needs the iOS simulator (Xcode) and/or the Android emulator (ANDROID_HOME, an AVD), the CLI, and
// the native libraries built (scripts/build-apple.sh, scripts/build-android.sh). The simulator
// draws with the Mac's GPU and the headless emulator in software: neither is a phone, and the page
// says so.
import { execFileSync, execSync, spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const opt = (name, def) => { const i = args.indexOf(`--${name}`); return i >= 0 ? args[i + 1] : def; };
const charts = (opt("charts") ?? "votes,descent,galaxy,scatter").split(",");
const want = (p) => !args.some((a) => a === "--ios" || a === "--android") || args.includes(`--${p}`);
const cli = join(root, "target/release/datars");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const sh = (cmd, o = {}) => execSync(cmd, { cwd: root, stdio: ["ignore", "pipe", "pipe"], maxBuffer: 1 << 26, ...o }).toString();
const PORT = 8793;

// Publish the charts and serve them.
const site = join(root, "out/bench-site");
rmSync(site, { recursive: true, force: true });
for (const c of charts) {
  const doc = [join(root, "examples", c, "doc.json"), join(root, "examples", c, "doc.ts")].find(existsSync);
  execFileSync(cli, ["publish", doc, "--alias", c, "--to", site], { stdio: "ignore" });
}
const server = spawn(cli, ["serve", site, "--port", String(PORT)], { stdio: "ignore" });
process.on("exit", () => server.kill());
await sleep(800);

/** Wait for `done(text)` to hold on what `read()` returns, polling; the last text either way. */
async function until(read, done, ms) {
  const end = Date.now() + ms;
  let text = "";
  while (Date.now() < end) {
    text = read();
    if (done(text)) return text;
    await sleep(1500);
  }
  return text;
}

/** The DATARS-BENCH lines of a log, and the renderer the DONE line names. */
function parse(text, chart) {
  const rows = [...text.matchAll(/DATARS-BENCH (\{.*\})/g)].map((m) => JSON.parse(m[1])).filter((r) => r.chart === chart);
  const done = text.match(new RegExp(`DATARS-BENCH-DONE ${chart} (.+)`));
  return { rows, renderer: done?.[1]?.trim() ?? null };
}

const runs = [];

if (want("ios")) {
  const sim = process.env.SIM ?? sh("xcrun simctl list devices available").match(/iPhone[^(]*\(([0-9A-F-]{36})\)/)?.[1];
  // The device's own runtime: the `-- iOS 18.5 --` section it's listed under.
  let os = "", name = "";
  for (const l of sh("xcrun simctl list devices").split("\n")) {
    const head = l.match(/^-- (iOS [\d.]+) --/);
    if (head) os = head[1];
    if (l.includes(sim)) { name = l.trim().replace(/\s*\(.*$/, ""); break; }
  }
  sh(`xcrun simctl boot ${sim} || true`);
  sh(`cd apple/DatarsSample.swiftpm && xcodebuild -scheme DatarsSample -destination "platform=iOS Simulator,id=${sim}" -derivedDataPath ../../out/ios-sample build -quiet`);
  const app = sh("find out/ios-sample -name DatarsSample.app -type d").trim().split("\n")[0];
  sh(`xcrun simctl install ${sim} "${app}"`);
  for (const chart of charts) {
    sh(`xcrun simctl terminate ${sim} dev.datars.sample || true`);
    const logFile = join(root, "out/bench-ios.log");
    const stream = spawn("xcrun", ["simctl", "spawn", sim, "log", "stream", "--style", "compact", "--predicate", 'subsystem == "dev.datars" AND category == "bench"'], { stdio: ["ignore", "pipe", "ignore"] });
    let log = "";
    stream.stdout.on("data", (d) => (log += d));
    await sleep(1500);
    execSync(`xcrun simctl launch ${sim} dev.datars.sample`, { env: { ...process.env, SIMCTL_CHILD_DATARS_URL: `http://127.0.0.1:${PORT}/c/${chart}`, SIMCTL_CHILD_DATARS_BENCH: "1" }, stdio: "ignore" });
    await until(() => log, (t) => t.includes(`DATARS-BENCH-DONE ${chart}`), 180000);
    stream.kill();
    writeFileSync(logFile, log);
    const { rows, renderer } = parse(log, chart);
    console.log(`iOS ${chart}: ${rows.length} transitions (${renderer ?? "no result"})`);
    runs.push({ platform: "iOS", device: `${name} simulator, ${os}`, renderer, chart, rows });
  }
  sh(`xcrun simctl terminate ${sim} dev.datars.sample || true`);
}

if (want("android")) {
  const adb = join(process.env.ANDROID_HOME, "platform-tools/adb");
  const emulator = join(process.env.ANDROID_HOME, "emulator/emulator");
  if (!sh(`"${adb}" devices`).split("\n").slice(1).some((l) => /\tdevice$/.test(l))) {
    const avd = sh(`"${emulator}" -list-avds`).trim().split("\n")[0];
    spawn(emulator, ["-avd", avd, "-no-window", "-no-audio", "-no-boot-anim", "-no-snapshot-save", "-gpu", process.env.EMU_GPU ?? "auto"], { stdio: "ignore", detached: true }).unref();
    sh(`"${adb}" wait-for-device`);
    await until(() => sh(`"${adb}" shell getprop sys.boot_completed || true`), (t) => t.trim() === "1", 180000);
    // A fresh boot keeps optimising apps for a minute or two: frames measured then are the
    // emulator's housekeeping. Wait until the load average settles.
    await until(() => sh(`"${adb}" shell cat /proc/loadavg || true`), (t) => parseFloat(t) < 1.5, 240000);
  }
  const model = sh(`"${adb}" shell getprop ro.product.model`).trim();
  const release = sh(`"${adb}" shell getprop ro.build.version.release`).trim();
  sh("bash scripts/build-android-sample.sh");
  sh(`"${adb}" install -r out/android-sample/datars-sample.apk`);
  sh(`"${adb}" reverse tcp:${PORT} tcp:${PORT}`);
  for (const chart of charts) {
    sh(`"${adb}" shell am force-stop dev.datars.sample`);
    sh(`"${adb}" logcat -c`);
    sh(`"${adb}" shell am start -n dev.datars.sample/.MainActivity --es url http://127.0.0.1:${PORT}/c/${chart} --ez bench true`);
    const log = await until(() => sh(`"${adb}" logcat -d -s datars:I`), (t) => t.includes(`DATARS-BENCH-DONE ${chart}`), 240000);
    const { rows, renderer } = parse(log, chart);
    console.log(`Android ${chart}: ${rows.length} transitions (${renderer ?? "no result"})`);
    runs.push({ platform: "Android", device: `${model} emulator, Android ${release}`, renderer, chart, rows });
  }
  sh(`"${adb}" shell am force-stop dev.datars.sample`);
}

const out = resolve(root, opt("json") ?? "site/perf/native.json");
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, JSON.stringify({ machine: sh("sysctl -n machdep.cpu.brand_string").trim(), date: new Date().toISOString().slice(0, 10), runs }, null, 2) + "\n");
console.log(`wrote ${out}`);
process.exit(0);
