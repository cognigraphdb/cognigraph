import { describe, expect, test } from "bun:test";
import { normalizeLogEvent, statusClass } from "./server-logs.ts";

describe("normalizeLogEvent", () => {
  test("maps the server event shape", () => {
    expect(
      normalizeLogEvent({
        seq: 7,
        at: 1_700_000_000,
        method: "GET",
        path: "/api/documents/labels/x",
        status: 404,
        latency_ms: 3,
        message: "Document not found: labels/x",
        tenant: "dailymed",
      }),
    ).toEqual({
      seq: 7,
      at: 1_700_000_000,
      method: "GET",
      path: "/api/documents/labels/x",
      status: 404,
      latencyMs: 3,
      message: "Document not found: labels/x",
      tenant: "dailymed",
    });
  });

  test("tolerates a missing tenant and defaults", () => {
    const event = normalizeLogEvent({ status: 401, message: "missing bearer token" });
    expect(event.tenant).toBeUndefined();
    expect(event.method).toBe("");
    expect(event.latencyMs).toBe(0);
  });
});

describe("statusClass", () => {
  test("bands by severity", () => {
    expect(statusClass(500)).toBe("server");
    expect(statusClass(404)).toBe("client");
    expect(statusClass(200)).toBe("other");
  });
});
