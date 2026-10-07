// The extensibility page's figures share one small table: a hundred visitors by channel, and each
// channel's share in whole percentage points (the units the isotype and the waffle draw).
import { data, e, op } from "@datars/sdk";

export const visits = data.values({ channel: ["Search", "Social", "Direct", "Newsletter", "Referral", "Ads"], visitors: [38, 21, 17, 12, 8, 4] }, { key: "channel" });
export const points = { from: "visits", ops: [op.window("share_of_total", "visitors", "share"), op.derive("points", e("round(d.share * 100)"))] };
/** One colour per channel, in every state. */
export const colour = { color: { type: "categorical" as const, domain: { data: "visits", field: "channel" }, range: "$categorical" } };
