import { describe, expect, test } from "bun:test";
import type { NeuronDoc } from "./neurons.ts";
import { factOf, formatUnixSeconds, triggersOf, verdictsFor } from "./neurons.ts";

const base: NeuronDoc = {
  _key: "n1",
  id: "n1",
  type: "relation_hint",
  status: "proposed",
  space_type: "pharma",
  confidence: 0.9,
  rationale: "",
  evidence: [],
  entity: "",
  aliases: [],
  source: "Meridian",
  relation: "SUPPLIES",
  target: "Compound X",
  triggers: ["meridian supplies compound x"],
  boost: 0,
};

describe("factOf", () => {
  test("relation hint renders a directed triple", () => {
    expect(factOf(base)).toBe("Meridian —SUPPLIES→ Compound X");
  });

  test("blocker renders the vetoed triple", () => {
    expect(factOf({ ...base, type: "relation_blocker" })).toBe("Meridian —SUPPLIES→ Compound X");
  });

  test("alias renders entity and aliases", () => {
    expect(
      factOf({ ...base, type: "alias", entity: "Meridian", aliases: ["MRD", "Meridian AG"] }),
    ).toBe("Meridian ≡ MRD, Meridian AG");
  });

  test("rank hint renders signed boost", () => {
    expect(factOf({ ...base, type: "relation_rank_hint", boost: 0.5 })).toBe("SUPPLIES rank +0.5");
    expect(factOf({ ...base, type: "relation_rank_hint", boost: -0.25 })).toBe(
      "SUPPLIES rank -0.25",
    );
  });

  test("missing fields render placeholders, not undefined", () => {
    expect(factOf({ ...base, source: "", relation: "", target: "" })).toBe("? —?→ ?");
  });
});

describe("triggersOf", () => {
  test("hints and blockers expose triggers", () => {
    expect(triggersOf(base)).toEqual(["meridian supplies compound x"]);
    expect(triggersOf({ ...base, type: "relation_blocker" })).toEqual([
      "meridian supplies compound x",
    ]);
  });

  test("aliases and rank hints have none even if the field is set", () => {
    expect(triggersOf({ ...base, type: "alias" })).toEqual([]);
    expect(triggersOf({ ...base, type: "relation_rank_hint" })).toEqual([]);
  });
});

describe("verdictsFor", () => {
  test("proposed can be accepted or rejected", () => {
    expect(verdictsFor("proposed")).toEqual(["accept", "reject"]);
  });

  test("accepted can only be retired", () => {
    expect(verdictsFor("accepted")).toEqual(["retire"]);
  });

  test("rejected and retired are terminal", () => {
    expect(verdictsFor("rejected")).toEqual([]);
    expect(verdictsFor("retired")).toEqual([]);
  });
});

describe("formatUnixSeconds", () => {
  test("zero and absent mean never", () => {
    expect(formatUnixSeconds(0)).toBe("—");
    expect(formatUnixSeconds(undefined)).toBe("—");
    expect(formatUnixSeconds(null)).toBe("—");
  });

  test("renders a real timestamp", () => {
    // Concrete instant: 2026-07-15T00:00:00Z — just assert it produced a
    // formatted string, not the em-dash (exact text is locale-dependent).
    expect(formatUnixSeconds(1784073600)).not.toBe("—");
    expect(formatUnixSeconds(1784073600)).toContain("2026");
  });
});
