# Sources: The day oil was worth less than nothing

Everything in this article comes from the U.S. Energy Information Administration (EIA). EIA data
are public domain: U.S. government works, free to use and redistribute
(<https://www.eia.gov/about/copyrights_reuse.php>). Credit: "Source: U.S. Energy Information
Administration".

## Files

| file | what | from |
|---|---|---|
| `data/DCOILWTICO.csv` | West Texas Intermediate spot price, Cushing, Okla., $/barrel, daily | EIA, via FRED: <https://fred.stlouisfed.org/series/DCOILWTICO> |
| `data/DCOILBRENTEU.csv` | Brent spot price, Europe, $/barrel, daily | EIA, via FRED: <https://fred.stlouisfed.org/series/DCOILBRENTEU> |
| `data/DGASNYH.csv` | Conventional gasoline, New York Harbor, $/gallon, daily | EIA, via FRED: <https://fred.stlouisfed.org/series/DGASNYH> |
| `data/DHOILNYH.csv` | No. 2 heating oil, New York Harbor, $/gallon, daily | EIA, via FRED: <https://fred.stlouisfed.org/series/DHOILNYH> |
| `data/DHHNGSP.csv` | Henry Hub natural gas spot price, $/million Btu, daily | EIA, via FRED: <https://fred.stlouisfed.org/series/DHHNGSP> |
| `data/cushing-stocks.csv` | Crude oil stocks at tank farms and pipelines, Cushing, Okla., thousand barrels, weekly (week ending Friday) | EIA series W_EPC0_SAX_YCUOK_MBBL: <https://www.eia.gov/dnav/pet/hist/LeafHandler.ashx?n=PET&s=W_EPC0_SAX_YCUOK_MBBL&f=W> |

All six cover Jan. 2, 2019 to June 30, 2021 (the stocks to the week ending July 2, 2021),
downloaded September 2026. Missing days (holidays) are left out, as FRED marks them.

## From the files to `data.json`

- `days`: one row per series and trading day (`series`, `date`, `close`), with `j` the day's place
  among every date that has a price in any series, so a range of days is a range of `j` whatever the
  series (Jan. 2, 2020 is 257, Dec. 31, 2020 is 514, June 30, 2021 is 640).
- `weeks`: West Texas Intermediate by week ending Friday: `open` the week's first daily close,
  `high` and `low` its highest and lowest, `close` its last; `cushing` the EIA stocks for that week
  in millions of barrels (thousand barrels / 1,000, one decimal); `i` the week's number from 0.

These are daily closing spot prices, not intraday highs and lows: the candles show the range of a
week's closes. The −$37.63 in the text is the NYMEX settlement for the May 2020 futures contract,
which is not in these files.
