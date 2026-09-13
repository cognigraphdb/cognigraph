import { describe, expect, test } from "bun:test";
import { formatUptime, parseMetrics } from "./metrics.ts";

const sample = `# TYPE cognigraph_requests_total counter
cognigraph_requests_total 280
# TYPE cognigraph_responses_total counter
cognigraph_responses_total{class="2xx"} 267
cognigraph_responses_total{class="4xx"} 12
cognigraph_responses_total{class="5xx"} 1
# TYPE cognigraph_request_duration_seconds summary
cognigraph_request_duration_seconds_sum 7.0
cognigraph_request_duration_seconds_count 280
# TYPE cognigraph_uptime_seconds gauge
cognigraph_uptime_seconds 1632
`;

describe("parseMetrics", () => {
  test("reads counters, class labels, and derives mean latency", () => {
    const metrics = parseMetrics(sample);
    expect(metrics.requests).toBe(280);
    expect(metrics.success).toBe(267);
    expect(metrics.clientErrors).toBe(12);
    expect(metrics.serverErrors).toBe(1);
    expect(metrics.uptimeSeconds).toBe(1632);
    // 7.0s / 280 = 0.025s = 25ms
    expect(metrics.avgLatencyMs).toBeCloseTo(25, 5);
  });

  test("avg latency is undefined with no requests, other fields zero", () => {
    const empty = parseMetrics(
      "cognigraph_requests_total 0\ncognigraph_request_duration_seconds_count 0\n",
    );
    expect(empty.avgLatencyMs).toBeUndefined();
    expect(empty.success).toBe(0);
    expect(empty.requests).toBe(0);
  });

  test("a name is not matched by a longer name with the same prefix", () => {
    // _count must not be picked up as the base summary name.
    const text =
      "cognigraph_request_duration_seconds_sum 4\ncognigraph_request_duration_seconds_count 2\n";
    expect(parseMetrics(text).avgLatencyMs).toBeCloseTo(2000, 5);
  });
});

describe("formatUptime", () => {
  test("formats across scales", () => {
    expect(formatUptime(1080)).toBe("18m");
    expect(formatUptime(3660)).toBe("1h 1m");
    expect(formatUptime(183_660)).toBe("2d 3h 1m");
  });
});
