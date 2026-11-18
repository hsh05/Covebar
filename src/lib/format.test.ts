import { describe, expect, it } from "vitest";
import { ago, bytes, countdown, duration, presetLabel, span } from "./format";

describe("duration", () => {
  it("formats minutes and hours", () => {
    expect(duration(65_000)).toBe("1:05");
    expect(duration(3_725_000)).toBe("1:02:05");
    expect(duration(-5)).toBe("0:00");
  });
  it("adds tenths or hundredths", () => {
    expect(duration(1_234, { tenths: true })).toBe("0:01.2");
    expect(duration(1_234, { hundredths: true })).toBe("0:01.23");
  });
});

describe("countdown", () => {
  it("rounds up so the last second shows until the end", () => {
    expect(countdown(1)).toBe("0:01");
    expect(countdown(59_001)).toBe("1:00");
    expect(countdown(0)).toBe("0:00");
  });
});

describe("labels", () => {
  it("names timer presets", () => {
    expect(presetLabel(60)).toBe("1 min");
    expect(presetLabel(3600)).toBe("1 h");
    expect(presetLabel(90)).toBe("1:30");
  });
  it("sizes", () => {
    expect(bytes(512)).toBe("512 bytes");
    expect(bytes(1536)).toBe("1.5 KB");
    expect(bytes(50 * 1024 * 1024)).toBe("50 MB");
  });
  it("spans and ages", () => {
    expect(span(20_000)).toBe("less than a minute");
    expect(span(8_100_000)).toBe("2 h 15 min");
    expect(span(360_000_000)).toBe("4 d 4 h");
    expect(ago(1_000, 30_000)).toBe("just now");
    expect(ago(0, 8_100_000)).toBe("2 h ago");
  });
});
