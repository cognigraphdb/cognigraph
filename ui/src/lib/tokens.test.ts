import { describe, expect, test } from "bun:test";
import { describeExpiry, formatUnixSeconds, TTL_PRESETS, ttlSecondsFor } from "./tokens.ts";

const NOW = 1_784_073_600; // 2026-07-15T00:00:00Z

describe("ttlSecondsFor", () => {
  test("server default omits the field", () => {
    expect(ttlSecondsFor("default")).toBeUndefined();
  });

  test("never maps to the explicit 0 sentinel", () => {
    expect(ttlSecondsFor("never")).toBe(0);
  });

  test("finite presets map to seconds", () => {
    expect(ttlSecondsFor("1h")).toBe(3_600);
    expect(ttlSecondsFor("1d")).toBe(86_400);
    expect(ttlSecondsFor("30d")).toBe(2_592_000);
  });

  test("every preset resolves through the lookup", () => {
    for (const preset of TTL_PRESETS) {
      expect(ttlSecondsFor(preset.value)).toBe(preset.seconds);
    }
  });
});

describe("describeExpiry", () => {
  test("null and zero mean never", () => {
    expect(describeExpiry(null, NOW)).toBe("Never expires");
    expect(describeExpiry(0, NOW)).toBe("Never expires");
  });

  test("past timestamps read as expired", () => {
    expect(describeExpiry(NOW - 60, NOW)).toBe("Expired");
  });

  test("future timestamps carry the formatted instant", () => {
    const text = describeExpiry(NOW + 3_600, NOW);
    expect(text.startsWith("Expires ")).toBe(true);
    expect(text).toContain("2026");
  });
});

describe("formatUnixSeconds", () => {
  test("absent means em dash", () => {
    expect(formatUnixSeconds(null)).toBe("—");
    expect(formatUnixSeconds(0)).toBe("—");
  });
});
