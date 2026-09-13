import { describe, expect, test } from "bun:test";
import {
  diagnosticFromCgqlError,
  missingBindVariableDiagnostics,
  parseBindVariables,
  prepareCgqlValidation,
} from "./cgql-validation.ts";

describe("CGQL editor validation", () => {
  test("adds a non-executing EXPLAIN line while preserving source locations", () => {
    expect(prepareCgqlValidation("FOR d documents RETURN d")).toEqual({
      query: "EXPLAIN\nFOR d documents RETURN d",
      lineOffset: 1,
    });
  });

  test("downgrades EXPLAIN ANALYZE to non-executing EXPLAIN", () => {
    const prepared = prepareCgqlValidation(
      "// inspect\nEXPLAIN ANALYZE FOR d IN documents RETURN d",
    );
    expect(prepared.lineOffset).toBe(0);
    expect(prepared.query).toBe("// inspect\nEXPLAIN         FOR d IN documents RETURN d");
  });

  test("maps server parse coordinates back into the original query", () => {
    const query = "FOR d documents\nRETURN d";
    const diagnostic = diagnosticFromCgqlError(
      query,
      "Query execution error: parse error at line 2, column 7: expected kw_in",
      1,
    );
    expect(diagnostic.from).toBe(6);
    expect(diagnostic.message).toBe("expected kw_in");
  });

  test("marks unknown identifiers and missing bind variables", () => {
    const query = "FOR d IN documents FILTER nope == @missing RETURN d";
    expect(diagnosticFromCgqlError(query, "identifier `nope` is not in scope", 0).from).toBe(26);
    expect(diagnosticFromCgqlError(query, "missing bind variables: missing", 0).from).toBe(34);
  });

  test("accepts only JSON objects as bind variables", () => {
    expect(parseBindVariables('{"limit": 5}')).toEqual({ value: { limit: 5 } });
    expect(parseBindVariables("[]")).toEqual({ error: "Bind variables must be a JSON object" });
    expect(parseBindVariables("{")).toEqual({ error: "Bind variables must be valid JSON" });
  });

  test("reports missing bind variables outside strings and comments", () => {
    const query =
      'FOR d IN documents // ignore @comment\n  FILTER d.category == @category AND d.note == "@literal"\n  RETURN d';
    expect(missingBindVariableDiagnostics(query, {})).toEqual([
      expect.objectContaining({
        from: query.indexOf("@category"),
        to: query.indexOf("@category") + "@category".length,
        message: "Missing bind variable: category",
      }),
    ]);
    expect(missingBindVariableDiagnostics(query, { category: "strategy" })).toEqual([]);
  });
});
