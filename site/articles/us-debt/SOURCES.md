# Sources: $40 trillion, to the penny

Everything in this article comes from Debt to the Penny, published every business day by the
Bureau of the Fiscal Service of the U.S. Treasury through its Fiscal Data API. Works of the U.S.
government are in the public domain. Credit: "Source: U.S. Treasury, Bureau of the Fiscal
Service, Debt to the Penny".

- Dataset: <https://fiscaldata.treasury.gov/datasets/debt-to-the-penny/>
- API endpoint (no key): `https://api.fiscaldata.treasury.gov/services/api/fiscal_service/v2/accounting/od/debt_to_penny`
- Fields used: `record_date`, `tot_pub_debt_out_amt` (total public debt outstanding),
  `debt_held_public_amt`, `intragov_hold_amt` (intragovernmental holdings). Amounts are dollars
  and cents, as strings; the split into public and intragovernmental holdings starts on
  Sept. 30, 1997 (earlier records have `null`).

## Live: `latest/doc.ts`

The hero chart is not built from a file. It declares the API as a live source:

```
…/debt_to_penny?fields=record_date,tot_pub_debt_out_amt&sort=-record_date&page[size]=90
```

with `rows: "data"` (the API's records are under `data`), `types` turning the strings into dates
and numbers, and `live: { every: 3600 }`. Publishing fetches it once, and the page opens on that
snapshot; the reader's browser then asks the API for it again at once (the API allows any origin)
and every hour while the page is open, and the chart moves to what it answers.

## Files

| file | what |
|---|---|
| `data/debt_to_penny.json` | The whole series, April 1, 1993 to Sept. 24, 2026 (8,400 business days): the API's answer to `?fields=record_date,tot_pub_debt_out_amt,debt_held_public_amt,intragov_hold_amt&sort=record_date&page[size]=10000`, downloaded Sept. 28, 2026 |
| `data/make.py` | From that file to `data.json` |
| `data.json` | What the history and holders charts draw |

## From the file to `data.json`

- `months`: the last business day on record in each month (and the last day in the file):
  `date`, `x` (the date as a decimal year), `total`, `public` and `intragov` in trillions of
  dollars, three decimals.
- `milestones`: the first business day the total reached each multiple of $5 trillion.
- `holders`: `months` from September 1997 in long form, one row per month and holder (`Held by
  the public`, `Held by government accounts`), for the stacked chart.

The figures in the text (the $2.6 trillion of the last 12 months, the year 2000's fall of
$89.5 billion, 2020's rise of $4.58 trillion, the shares held by the public) are computed from
the daily file, first and last business days of each period.
