import { describe, expect, it } from "vitest";
import { timerFraction, timerLeft, timerPhase } from "./timer";
import type { TimerState } from "../../lib/types";

const idle: TimerState = { durationMs: 0, endsAt: null, remainingMs: null, ringing: false };

describe("timer", () => {
  it("knows its phase", () => {
    expect(timerPhase(idle)).toBe("idle");
    expect(timerPhase({ ...idle, durationMs: 60_000, endsAt: 1 })).toBe("running");
    expect(timerPhase({ ...idle, durationMs: 60_000, remainingMs: 5_000 })).toBe("paused");
    expect(timerPhase({ ...idle, durationMs: 60_000, remainingMs: 0, ringing: true })).toBe("ringing");
  });
  it("counts down and never below zero", () => {
    const t = { ...idle, durationMs: 60_000, endsAt: 100_000 };
    expect(timerLeft(t, 70_000)).toBe(30_000);
    expect(timerLeft(t, 200_000)).toBe(0);
    expect(timerFraction(t, 70_000)).toBe(0.5);
    expect(timerFraction(idle, 0)).toBe(0);
  });
});
