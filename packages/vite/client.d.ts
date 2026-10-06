// Types for chart imports the plugin compiles. Add to tsconfig `compilerOptions.types`
// (`"types": ["vite/client", "@datars/vite/client"]`) or reference it once:
//   /// <reference types="@datars/vite/client" />
//
// `import chart from "./sales.chart.ts"` is typed by TypeScript from the file itself (the document
// `doc()` returns), which <DatarsView chart> takes as it is; `?datars` imports get this type.

interface DatarsChartModule {
  readonly kind: "datars-chart";
  /** The chart file, relative to the project (HMR matches updates by it). */
  readonly id: string;
  /** The document's authored size, [width, height]: the aspect a page reserves for it. */
  readonly size: [number, number];
  readonly title?: string;
  /** The program's steps, by name. */
  readonly states: string[];
  /** The document (JSON IR), played by the full engine — without publish mode. */
  readonly doc?: Record<string, unknown>;
  /** A published bundle's manifest URL, played by the core engine — in publish mode. */
  readonly src?: string;
}

declare module "*?datars" {
  const chart: DatarsChartModule;
  export default chart;
}
declare module "*?datars&publish" {
  const chart: DatarsChartModule;
  export default chart;
}
declare module "*?datars&raw" {
  const chart: DatarsChartModule;
  export default chart;
}
