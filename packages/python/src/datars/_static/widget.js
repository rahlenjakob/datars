// The anywidget front end of `datars.widget.ChartWidget` (appended to loader.js by Python).
//
// The runtime comes over the widget's comm from the kernel, the first time a page needs it —
// never stored in widget state, so saved notebooks stay small — and every chart on the page
// shares it. Python → chart: `doc` (morphs to the new document), `state`, `signals`, `tokens`,
// `mode`. Chart → Python: `state` changes, and clicks (`pick` messages with the datum's row).

function runtimeFromKernel(model) {
  if (NB.runtime) return NB.runtime;
  NB.runtime = new Promise((resolve, reject) => {
    const onMsg = (msg, buffers) => {
      if (msg?.type !== "runtime") return;
      model.off("msg:custom", onMsg);
      if (msg.error) return reject(new Error(msg.error));
      const files = {};
      msg.paths.forEach((p, i) => { files[p] = buffers[i]; });
      runtimeFromFiles(files).then(resolve, reject);
    };
    model.on("msg:custom", onMsg);
    model.send({ type: "need-runtime" });
  });
  NB.runtime.catch(() => { NB.runtime = null; }); // a later chart may ask again
  return NB.runtime;
}

/** The document's JSON text (it arrives as bytes: a binary buffer on the comm). */
function docText(model) {
  const v = model.get("doc");
  return typeof v === "string" ? v : TEXT.decode(bytesOf(v));
}

function render({ model, el }) {
  el.classList.add("datars-widget");
  const host = document.createElement("div");
  host.className = "datars-output";
  host.style.position = "relative";
  host.style.maxWidth = `${model.get("width")}px`;
  const posterSrc = model.get("poster");
  if (posterSrc) {
    const img = document.createElement("img");
    img.className = "datars-poster";
    img.src = posterSrc;
    img.alt = model.get("alt") || "chart";
    img.style.cssText = "display:block;width:100%;height:auto";
    host.appendChild(img);
  }
  el.appendChild(host);
  let view = null;
  const applySignals = (prev = {}) => {
    const s = model.get("signals") || {};
    for (const [k, v] of Object.entries(s)) if (JSON.stringify(prev[k]) !== JSON.stringify(v)) view?.setSignal(k, v);
  };
  let current = null;
  let initial = model.get("state") || null;
  const goto = () => {
    const want = model.get("state");
    if (view && want && want !== current && (model.get("states") || []).includes(want)) view.send(`goto:${want}`);
  };
  mount(host, {
    doc: docText(model),
    width: model.get("width"),
    height: model.get("height"),
    mode: model.get("mode"),
    runtime: runtimeFromKernel(model),
    onState: (status) => {
      if (!status) return;
      current = status.state;
      if (status.states) model.set("states", status.states);
      // The state Python asked for before the chart ran: go there once it's drawn.
      const want = initial;
      initial = null;
      if (want && want !== status.state && (status.states || []).includes(want)) {
        model.save_changes();
        view?.send(`goto:${want}`);
        return;
      }
      if (status.state && status.state !== model.get("state")) model.set("state", status.state);
      model.save_changes();
    },
    onPick: (detail) => model.send({ type: "pick", ...detail }),
    onFail: (err) => model.send({ type: "error", message: String(err?.message ?? err) }),
  }).then((v) => {
    view = v;
    if (!v) return;
    applySignals();
    const tokens = model.get("tokens");
    if (tokens && Object.keys(tokens).length) v.setTokens(tokens);
  });
  let prevSignals = { ...(model.get("signals") || {}) };
  const handlers = {
    "change:doc": () => view?.setDocument(docText(model)),
    "change:state": goto,
    "change:signals": () => { applySignals(prevSignals); prevSignals = { ...(model.get("signals") || {}) }; },
    "change:tokens": () => view?.setTokens(model.get("tokens") || {}),
    "change:mode": () => { const m = model.get("mode"); if (view) m && m !== "auto" ? view.setAttribute("mode", m) : view.removeAttribute("mode"); },
  };
  for (const [ev, f] of Object.entries(handlers)) model.on(ev, f);
  const onMsg = (msg) => {
    if (msg?.type === "event" && view) view.send(msg.name);
  };
  model.on("msg:custom", onMsg);
  return () => {
    for (const [ev, f] of Object.entries(handlers)) model.off(ev, f);
    model.off("msg:custom", onMsg);
    view?.remove();
  };
}

export default { render };
