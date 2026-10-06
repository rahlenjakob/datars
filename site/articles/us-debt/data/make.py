# From the Treasury's Debt to the Penny download (debt_to_penny.json, every business day since
# April 1, 1993) to ../data.json: month-end totals, the $5-trillion milestones, and who holds it.
# Amounts in trillions of dollars; x is the date as a decimal year.
import datetime as dt, json, os

here = os.path.dirname(__file__)
rows = json.load(open(os.path.join(here, "debt_to_penny.json")))["data"]
num = lambda s: None if s in (None, "", "null") else float(s)
days = [(dt.date.fromisoformat(r["record_date"]), num(r["tot_pub_debt_out_amt"]), num(r["debt_held_public_amt"]), num(r["intragov_hold_amt"])) for r in rows]
days.sort()

def year(d):
    start = dt.date(d.year, 1, 1)
    return round(d.year + (d - start).days / (dt.date(d.year + 1, 1, 1) - start).days, 4)

tn = lambda v: None if v is None else round(v / 1e12, 3)
last_of_month = {}
for d, t, p, g in days:
    last_of_month[(d.year, d.month)] = (d, t, p, g)
months = [{"x": year(d), "date": d.isoformat(), "total": tn(t), "public": tn(p), "intragov": tn(g)} for d, t, p, g in sorted(last_of_month.values())]
# The last business day on record, too, so the line ends where the data does.
d, t, p, g = days[-1]
if months[-1]["date"] != d.isoformat():
    months.append({"x": year(d), "date": d.isoformat(), "total": tn(t), "public": tn(p), "intragov": tn(g)})

milestones = []
for m in range(5, 45, 5):
    hit = next(((d, t) for d, t, _, _ in days if t >= m * 1e12), None)
    if hit:
        milestones.append({"t": m, "date": hit[0].isoformat(), "x": year(hit[0]), "y": m})

holders = []
for r in months:
    if r["public"] is None:
        continue
    holders.append({"x": r["x"], "series": "Held by the public", "y": r["public"]})
    holders.append({"x": r["x"], "series": "Held by government accounts", "y": r["intragov"]})

json.dump({"months": months, "milestones": milestones, "holders": holders, "through": days[-1][0].isoformat()}, open(os.path.join(here, "..", "data.json"), "w"), separators=(",", ":"))
print(len(months), "months,", len(milestones), "milestones,", len(holders), "holder rows, through", days[-1][0])
