// The platforms page's evidence: the GPU renderer against the bit-exact CPU reference, for every
// example state — `datars gpu`'s own output, pasted as it printed it (Apple M3 Pro, Metal, a
// release build of this repository, 7 October 2026; 56 states, 0 over the limits). Each dot is a
// state: its mean colour difference (ΔE: distance in OKLab × 100, where 1–2 is about the smallest
// difference a person notices; crates/datars-test/src/gpu.rs) and the share of its pixels that
// differ visibly (ΔE over 10). The limits the command enforces are the dashed lines.
import { doc, data, e, group, motion, signal, step, story, text } from "@datars/sdk";
import { plot, point, rule } from "@datars/std";

const RUN = `
ok   budget      main       mean ΔE 0.062  p99 ΔE  2.44  visible 0.072%  solid   0
ok   business    bridge     mean ΔE 0.053  p99 ΔE  1.30  visible 0.047%  solid   0
ok   business    funnel     mean ΔE 0.038  p99 ΔE  0.00  visible 0.049%  solid   0
ok   business    stacked    mean ΔE 0.046  p99 ΔE  1.19  visible 0.074%  solid   0
ok   dashboard   main       mean ΔE 0.062  p99 ΔE  2.16  visible 0.048%  solid   0
ok   descent     world      mean ΔE 0.087  p99 ΔE  2.77  visible 0.102%  solid   0
ok   descent     sweden     mean ΔE 0.101  p99 ΔE  3.58  visible 0.138%  solid   0
ok   descent     stockholm  mean ΔE 0.153  p99 ΔE  1.86  visible 0.079%  solid   0
ok   election    main       mean ΔE 0.075  p99 ΔE  3.27  visible 0.096%  solid   0
ok   flows       main       mean ΔE 0.032  p99 ΔE  0.81  visible 0.027%  solid   0
ok   galaxy      galaxy     mean ΔE 0.163  p99 ΔE  2.74  visible 0.041%  solid   0
ok   galaxy      arm        mean ΔE 0.395  p99 ΔE  3.91  visible 0.040%  solid   0
ok   galaxy      bar        mean ΔE 0.410  p99 ΔE  3.68  visible 0.041%  solid   0
ok   galaxy      cluster    mean ΔE 0.031  p99 ΔE  0.50  visible 0.040%  solid   0
ok   galaxy      local      mean ΔE 0.051  p99 ΔE  0.73  visible 0.040%  solid   0
ok   hebrew      main       mean ΔE 0.066  p99 ΔE  3.28  visible 0.054%  solid   0
ok   inflation   main       mean ΔE 0.068  p99 ΔE  2.50  visible 0.080%  solid   0
ok   prices      year       mean ΔE 0.134  p99 ΔE  3.50  visible 0.107%  solid   0
ok   prices      winter     mean ΔE 0.141  p99 ΔE  3.50  visible 0.118%  solid   0
ok   prices      summer     mean ΔE 0.133  p99 ΔE  3.50  visible 0.119%  solid   0
ok   renewables  world      mean ΔE 0.091  p99 ΔE  3.14  visible 0.060%  solid   0
ok   renewables  europe     mean ΔE 0.075  p99 ΔE  2.54  visible 0.020%  solid   0
ok   renewables  nordics    mean ΔE 0.055  p99 ΔE  2.05  visible 0.015%  solid   0
ok   renewables  ranked     mean ΔE 0.056  p99 ΔE  2.08  visible 0.050%  solid   0
ok   riksdag     seats      mean ΔE 0.093  p99 ΔE  3.35  visible 0.006%  solid   0
ok   riksdag     bars       mean ΔE 0.048  p99 ΔE  1.26  visible 0.038%  solid   0
ok   rio         world      mean ΔE 0.088  p99 ΔE  2.81  visible 0.104%  solid   0
ok   rio         brazil     mean ΔE 0.121  p99 ΔE  4.68  visible 0.177%  solid   0
ok   rio         rio        mean ΔE 0.088  p99 ΔE  1.65  visible 0.083%  solid   0
ok   rio         copacabana mean ΔE 0.109  p99 ΔE  1.86  visible 0.121%  solid   0
ok   scatter     all        mean ΔE 0.131  p99 ΔE  2.56  visible 0.019%  solid   0
ok   scatter     focus      mean ΔE 0.117  p99 ΔE  1.74  visible 0.021%  solid   0
ok   serif       km         mean ΔE 0.101  p99 ΔE  4.35  visible 0.099%  solid   0
ok   serif       miles      mean ΔE 0.107  p99 ΔE  4.92  visible 0.101%  solid   0
ok   shapes      bars       mean ΔE 0.049  p99 ΔE  0.93  visible 0.041%  solid   0
ok   shapes      treemap    mean ΔE 0.036  p99 ΔE  0.00  visible 0.098%  solid   0
ok   shapes      waffle     mean ΔE 0.002  p99 ΔE  0.00  visible 0.001%  solid   0
ok   shapes      dots       mean ΔE 0.023  p99 ΔE  0.30  visible 0.018%  solid   0
ok   spending    months     mean ΔE 0.106  p99 ΔE  4.15  visible 0.109%  solid   0
ok   spending    categories mean ΔE 0.088  p99 ΔE  3.67  visible 0.105%  solid   0
ok   stocks      candles    mean ΔE 0.111  p99 ΔE  3.33  visible 0.068%  solid   0
ok   stocks      month      mean ΔE 0.082  p99 ΔE  2.83  visible 0.064%  solid   0
ok   stocks      indicators mean ΔE 0.157  p99 ΔE  3.77  visible 0.093%  solid   0
ok   stocks      indexed    mean ΔE 0.098  p99 ΔE  3.50  visible 0.107%  solid   0
ok   stocks      peers      mean ΔE 0.108  p99 ΔE  3.69  visible 0.113%  solid   0
ok   stocks      drawdown   mean ΔE 0.120  p99 ΔE  4.01  visible 0.105%  solid   0
ok   votes       bars       mean ΔE 0.085  p99 ΔE  3.56  visible 0.103%  solid   0
ok   votes       ranked     mean ΔE 0.067  p99 ΔE  2.73  visible 0.067%  solid   0
ok   votes       pie        mean ΔE 0.050  p99 ΔE  1.11  visible 0.077%  solid   0
ok   votes       donut      mean ΔE 0.048  p99 ΔE  0.81  visible 0.073%  solid   0
ok   warming     line       mean ΔE 0.102  p99 ΔE  3.36  visible 0.062%  solid   0
ok   warming     stripes    mean ΔE 0.015  p99 ΔE  0.34  visible 0.000%  solid   0
ok   worlds      globe      mean ΔE 0.140  p99 ΔE  5.22  visible 0.430%  solid   1
ok   worlds      orbits     mean ΔE 0.073  p99 ΔE  2.28  visible 0.129%  solid   0
ok   worlds      spiral     mean ΔE 0.061  p99 ΔE  0.74  visible 0.153%  solid   0
ok   worlds      blocks     mean ΔE 0.121  p99 ΔE  5.43  visible 0.077%  solid   0
`;
const rows = RUN.trim().split("\n").map((l) => {
  const m = l.match(/^\w+\s+(\S+)\s+(\S+)\s+mean ΔE\s+([\d.]+)\s+p99 ΔE\s+([\d.]+)\s+visible\s+([\d.]+)%\s+solid\s+(\d+)/)!;
  return { id: `${m[1]} · ${m[2]}`, example: m[1], state: m[2], de: Number(m[3]), visible: Number(m[5]) };
});
const col = <K extends keyof (typeof rows)[number]>(k: K) => rows.map((r) => r[k]);
const when = (s: string) => e(`metric == ${JSON.stringify(s)}`);

export default doc({
  id: "platforms-gpu",
  title: "GPU frames against the CPU reference",
  description: `Every example state (${rows.length}) drawn by the GPU renderer and by the CPU reference on an Apple M3 Pro: the mean colour difference stays under 0.42 ΔE (limit 1) and visibly different pixels under 0.5 %.`,
  size: [760, 470],
  data: { run: data.values({ id: col("id"), example: col("example"), state: col("state"), de: col("de"), visible: col("visible") }, { key: "id" }) },
  signals: { metric: signal.str("de") },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [8, 12, 8, 4] },
    children: [
      plot({ data: "run", x: "de", y: "example", xType: "linear", yType: "band", xDomain: [0, 1.1], title: "Mean colour difference per state (ΔE)", padding: 0.1,
        children: [point({ r: 4.5, fill: "$accent", label: e("d.id + ': mean ΔE ' + format(d.de, '.3f')") }), rule({ axis: "x", value: 1, label: "limit 1.0", ink: "$muted" })] },
        { key: "chart", when: when("de") }),
      plot({ data: "run", x: "visible", y: "example", xType: "linear", yType: "band", xDomain: [0, 0.55], title: "Pixels visibly different per state (%)", padding: 0.1,
        children: [point({ r: 4.5, fill: "$accent", label: e("d.id + ': ' + format(d.visible, '.3f') + ' % of pixels'") }), rule({ axis: "x", value: 0.5, label: "limit 0.5 %", ink: "$muted" })] },
        { key: "chart", when: when("visible") }),
    ],
  }),
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 0.9 }),
  program: story({ steps: [step("ΔE", { set: { metric: "de" } }), step("pixels", { set: { metric: "visible" } })] }),
});
