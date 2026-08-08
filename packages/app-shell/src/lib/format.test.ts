import { describe, expect, it } from "vitest";
import { formatDuration } from "./format";

describe("formatDuration", () => {
  it("keeps sub-second durations in milliseconds", () => {
    expect(formatDuration(0)).toBe("0ms");
    expect(formatDuration(850)).toBe("850ms");
  });

  it("shows seconds under a minute", () => {
    expect(formatDuration(45_000)).toBe("45s");
    expect(formatDuration(1_000)).toBe("1s");
  });

  it("shows minutes and seconds from one minute up", () => {
    expect(formatDuration(60_000)).toBe("1m");
    expect(formatDuration(150_000)).toBe("2m 30s");
    expect(formatDuration(7_200_000)).toBe("120m");
  });
});
