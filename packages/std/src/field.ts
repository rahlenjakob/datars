// A field of the row as expression source. Recipes build expressions from the field names a
// document gives them; a spreadsheet's or a pasted table's headers are rarely identifiers
// ("Avg. nightly rate", "Share (%)"), and `d.Avg. nightly rate` reads nothing.

/** A column of the current row as expression source: `d.sales` for an identifier — or a dotted
 * path of them (`d.address.city`), kept a path — else `d["Avg. nightly rate"]`. */
export function fld(name: string): string {
  return /^[A-Za-z_$][A-Za-z0-9_$]*(\.[A-Za-z_$][A-Za-z0-9_$]*)*$/.test(name) ? `d.${name}` : `d[${JSON.stringify(name)}]`;
}
