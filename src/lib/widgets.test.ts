import { describe, expect, it } from "vitest";
import { WIDGET_IDS, WIDGETS, widgetMismatch } from "./widgets";

describe("widget registry", () => {
  it("matches the id list the backend uses", () => {
    expect(widgetMismatch()).toEqual({ missing: [], unlisted: [], wrongId: [] });
    expect(new Set(WIDGET_IDS).size).toBe(WIDGET_IDS.length);
  });
  it("only gives cards to widgets that can open one", () => {
    for (const w of Object.values(WIDGETS)) if (w.cardSize || w.fit) expect(w.card, w.id).toBeTruthy();
  });
  it("spots mismatches", () => {
    expect(widgetMismatch(["a"], {})).toEqual({ missing: ["a"], unlisted: [], wrongId: [] });
  });
});
