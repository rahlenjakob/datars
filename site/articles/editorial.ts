// The chart theme every article shares: a newsroom graphics desk's house style over datars/neutral.
// Near-black ink on white, greys for everything that isn't the point and one warm accent for what
// is; hairline rules, square cards, quiet maps (grey land, white borders). Colours that carry
// meaning — a party's, a winner's — come from each story's keys, not from here.
import { theme } from "@datars/sdk";

export const editorial = theme({
  name: "articles/editorial",
  extends: "datars/neutral",
  tokens: {
    paper: "#ffffff",
    ink: "#121212",
    "ink-2": "#333333",
    muted: "#6f6f6f",
    grid: "#e6e6e6",
    rule: "#8f8f8f",
    accent: "#c4431d",
    highlight: "#c4431d",
    categorical: {
      kind: "categorical",
      colors: ["#c4431d", "#2f5f8a", "#8c8c8c", "#d8a13a", "#4e8062", "#7b5ea7", "#b5b5b5"],
    },
    card: "#fffffff2",
    "card-ink": "#121212",
    "card-ink-2": "#555555",
    "card-line": "#d6d6d6",
    "radius.card": 2,
    "map.land": "#efeeea",
    "map.water": "#dde6ec",
    "map.border": "#ffffff",
    "map.label": "#333333",
    "map.label-halo": "#ffffff",
    "map.marker": "#121212",
  },
});
