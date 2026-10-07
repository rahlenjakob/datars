// Shared by the features overview's small figures (/features/): one size, one padding, a key per
// state's chart so every state morphs into the next. Illustrative figures.
import { e } from "@datars/sdk";

export const SIZE: [number, number] = [420, 260];
export const PAD: [number, number, number, number] = [10, 12, 8, 8];
/** The chart a state shows: every state's has the key "chart", so the engine pairs them. */
export const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
