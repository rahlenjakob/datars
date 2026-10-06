import chart from "./sales.chart.ts";
import published from "./sales.chart.ts?datars&publish";

const el = document.createElement("datars-view");
import("@datars/web").then(() => {
  el.setDocument(chart.doc);
  document.body.append(el, published.src);
});
