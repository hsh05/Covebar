import type { Rates } from "../../lib/types";

let names: Intl.DisplayNames | null = null;
try {
  names = new Intl.DisplayNames(["en"], { type: "currency" });
} catch {
  names = null;
}

/** "AED" -> "UAE Dirham". */
export function currencyName(code: string) {
  try {
    return names?.of(code) ?? code;
  } catch {
    return code;
  }
}

/** Units of `to` per 1 `from`, or null if either is unknown. */
export function rate(r: Rates | null | undefined, from: string, to: string) {
  const a = r?.rates[from];
  const b = r?.rates[to];
  return a && b ? b / a : null;
}

/** Enough digits to be useful at any size: 3.6725, 0.7891, 152.4, 12,345. */
export function money(v: number) {
  const abs = Math.abs(v);
  const digits = abs >= 1000 ? 0 : abs >= 100 ? 2 : abs >= 1 ? 4 : abs >= 0.01 ? 4 : 6;
  return v.toLocaleString(undefined, { maximumFractionDigits: digits, minimumFractionDigits: Math.min(2, digits) });
}
