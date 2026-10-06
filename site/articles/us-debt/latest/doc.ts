// What the United States owes today, to the penny: the Treasury's latest daily figure and the 90
// business days before it. Live: the reader's browser asks the Treasury's Fiscal Data API for them
// when the chart opens (the page opens on the snapshot taken when it was published) and every hour
// after, and the figure rolls to the new total. Data: U.S. Treasury, Bureau of the Fiscal Service,
// Debt to the Penny (public domain) — see ../SOURCES.md.
import { doc, data, e, group, motion, op, text } from "@datars/sdk";
import { line, plot } from "@datars/std";
import { fiscal } from "../theme";

const API = "https://api.fiscaldata.treasury.gov/services/api/fiscal_service/v2/accounting/od/debt_to_penny";
const RECENT = `${API}?fields=record_date,tot_pub_debt_out_amt&sort=-record_date&page%5Bsize%5D=90`;
// The figure's size: as large as the width allows (22 characters: 46 px on a desktop, less on a phone).
const FIGURE = "min(46, (width - 36) / 13.5)";

export default doc({
  id: "us-debt/latest",
  title: "U.S. public debt, to the penny",
  description: "The total public debt outstanding on the Treasury's latest business day, to the cent, and the 90 business days before it — fetched live from the Treasury's Fiscal Data API.",
  size: [960, 420],
  locale: "en-US",
  theme: fiscal,
  data: {
    latest: data.url(RECENT, {
      rows: "data",
      key: "record_date",
      types: { record_date: "date", tot_pub_debt_out_amt: "num" },
      live: { every: 3600 },
    }),
  },
  tables: {
    days: { from: "latest", ops: [op.derive("t", e("d.tot_pub_debt_out_amt / 1e12")), op.sort("record_date")] },
  },
  // A new day slides in from the right; the figure counts to the new total.
  motion: motion({ duration: 0.9, easing: "cubic-in-out" }),
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 10, padding: [18, 22, 12, 14] },
    children: [
      group({
        key: "head",
        size: { h: "auto" },
        children: [
          text("Total public debt outstanding", [0, 13], { key: "kicker", style: { size: 12, weight: 600, ink: "$muted" } }),
          text("", [0, e(`20 + ${FIGURE}`)], {
            key: "figure",
            number: { value: e("table.last('days', 'tot_pub_debt_out_amt')"), format: "$,.2f" },
            style: { size: e(FIGURE), weight: 700 },
            semantics: { role: "title", label: e("'Total public debt outstanding: ' + format(table.last('days', 'tot_pub_debt_out_amt'), '$,.2f')") },
          }),
          text(e("'at the close of business on ' + formatDate(table.last('days', 'record_date'), '%B %-d, %Y')"), [0, e(`48 + ${FIGURE}`)], { key: "date", style: { size: 14, ink: "$muted" } }),
        ],
      }),
      plot({
        data: "days",
        x: "record_date",
        y: "t",
        xType: "time",
        format: ",.1f",
        prefix: "$",
        suffix: "T",
        zero: false,
        subtitle: "The last 90 business days, trillions of dollars",
        children: [line({ curve: "linear" })],
      }, { key: "recent" }),
    ],
  }),
});
