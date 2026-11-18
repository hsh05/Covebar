import { describe, expect, it } from "vitest";
import { rate } from "./currency";
import type { Rates } from "../../lib/types";

const r = { rates: { USD: 1, AED: 3.6725, GBP: 0.79 } } as unknown as Rates;

describe("rate", () => {
  it("converts through the base currency", () => {
    expect(rate(r, "USD", "AED")).toBeCloseTo(3.6725);
    expect(rate(r, "GBP", "AED")).toBeCloseTo(3.6725 / 0.79);
  });
  it("is null for unknown currencies or no rates", () => {
    expect(rate(r, "USD", "XYZ")).toBeNull();
    expect(rate(null, "USD", "AED")).toBeNull();
  });
});
