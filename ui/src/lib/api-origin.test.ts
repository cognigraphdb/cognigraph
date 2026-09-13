import { expect, test } from "bun:test";
import { apiOrigin } from "./api-origin.ts";

test("production preserves non-default ports, HTTPS, default ports and IPv6 origins", () => {
  for (const origin of [
    "http://127.0.0.1:38471",
    "https://console.example",
    "http://localhost",
    "http://[::1]:38471",
  ]) {
    expect(apiOrigin(origin)).toBe(origin);
    expect(apiOrigin(origin, null)).toBe(origin);
  }
});

test("the explicit Bun dev port changes only the port, keeping host and protocol", () => {
  expect(apiOrigin("http://127.0.0.1:3000", "3001")).toBe("http://127.0.0.1:3001");
  expect(apiOrigin("http://192.168.0.10:3000", "3001")).toBe("http://192.168.0.10:3001");
  expect(apiOrigin("https://[::1]:3000", "38471")).toBe("https://[::1]:38471");
});
