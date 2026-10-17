// This article's chart theme: the house editorial style (../editorial.ts), and — because a page you
// keep open to watch prices is as likely read at night — its dark mode: the page follows the
// reader's setting, and so does the chart. Gains and losses are the theme's `up` and `down`.
import { theme } from "@datars/sdk";
import { editorial } from "../editorial";

export const markets = theme({
  name: "articles/markets",
  extends: "datars/neutral",
  tokens: { ...editorial.tokens, up: "#1d7a4c", down: "#c0392b" },
  modes: {
    dark: {
      paper: "#121417",
      ink: "#eceef0",
      "ink-2": "#c9ccd1",
      muted: "#9aa0a8",
      grid: "#272a30",
      rule: "#5d636b",
      accent: "#f0794f",
      highlight: "#f0794f",
      up: "#4cc489",
      down: "#ff7a6b",
      card: "#1b1e22f2",
      "card-ink": "#eceef0",
      "card-ink-2": "#b3b8bf",
      "card-line": "#33373d",
    },
  },
});
