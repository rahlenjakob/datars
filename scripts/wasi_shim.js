// The few WASI calls QuickJS (compiled into the web runtime for T3 bundles) makes: a clock and
// writes to stderr. The engine never reads the clock itself (P10); QuickJS's libc links it anyway.
let mem = null;
const dec = new TextDecoder();
export function __setMemory(m) { mem = m; }
const view = () => new DataView(mem.buffer);
export function clock_time_get(_id, _precision, out) { view().setBigUint64(out, 0n, true); return 0; }
export function fd_write(_fd, iovs, n, written) {
  const v = view();
  let total = 0, text = "";
  for (let i = 0; i < n; i++) {
    const ptr = v.getUint32(iovs + i * 8, true), len = v.getUint32(iovs + i * 8 + 4, true);
    text += dec.decode(new Uint8Array(mem.buffer, ptr, len));
    total += len;
  }
  if (text.trim()) console.warn("[datars sandbox]", text.trim());
  v.setUint32(written, total, true);
  return 0;
}
export function fd_close() { return 0; }
export function fd_seek() { return 70; }
export function fd_fdstat_get() { return 8; }
export function proc_exit() { throw new Error("proc_exit"); }
export function environ_get() { return 0; }
export function environ_sizes_get(c, s) { view().setUint32(c, 0, true); view().setUint32(s, 0, true); return 0; }
