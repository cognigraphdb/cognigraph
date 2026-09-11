// Parse the server's Prometheus exposition text (GET /metrics) into the
// handful of numbers the Operations screen shows. Shapes mirror
// crates/cognigraph-server/src/hardening.rs::Metrics::render — note the
// "2xx" response class actually counts every 2xx AND 3xx (record_metrics
// buckets 200..=399 together), so we label it "success", not "2xx".

export interface ServerMetrics {
  requests: number;
  success: number;
  clientErrors: number;
  serverErrors: number;
  /// Mean request duration in milliseconds (sum / count), or undefined
  /// when no requests have been served.
  avgLatencyMs?: number;
  uptimeSeconds: number;
}

/// Extract a bare `metric_name value` sample. Ignores `# HELP`/`# TYPE`.
function sample(text: string, name: string): number | undefined {
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    // `name value` — the name may carry {labels}; match the exact name
    // up to a space or brace.
    const match = trimmed.match(/^([^\s{]+)(?:\{[^}]*\})?\s+(-?[\d.eE+]+)$/);
    if (match && match[1] === name) {
      const value = Number(match[2]);
      if (Number.isFinite(value)) return value;
    }
  }
  return undefined;
}

/// Extract a labelled sample, e.g. cognigraph_responses_total{class="4xx"}.
function labelledSample(text: string, name: string, label: string, value: string): number {
  const needle = `${label}="${value}"`;
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed.startsWith(name) || !trimmed.includes(needle)) continue;
    const match = trimmed.match(/\s+(-?[\d.eE+]+)$/);
    if (match) {
      const parsed = Number(match[1]);
      if (Number.isFinite(parsed)) return parsed;
    }
  }
  return 0;
}

export function parseMetrics(text: string): ServerMetrics {
  const requests = sample(text, "cognigraph_requests_total") ?? 0;
  const durationSum = sample(text, "cognigraph_request_duration_seconds_sum") ?? 0;
  const durationCount = sample(text, "cognigraph_request_duration_seconds_count") ?? 0;
  return {
    requests,
    success: labelledSample(text, "cognigraph_responses_total", "class", "2xx"),
    clientErrors: labelledSample(text, "cognigraph_responses_total", "class", "4xx"),
    serverErrors: labelledSample(text, "cognigraph_responses_total", "class", "5xx"),
    avgLatencyMs: durationCount > 0 ? (durationSum / durationCount) * 1000 : undefined,
    uptimeSeconds: sample(text, "cognigraph_uptime_seconds") ?? 0,
  };
}

/// Human-readable uptime, e.g. "2d 3h 41m" or "18m".
export function formatUptime(seconds: number): string {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const parts: string[] = [];
  if (days) parts.push(`${days}d`);
  if (hours || days) parts.push(`${hours}h`);
  parts.push(`${minutes}m`);
  return parts.join(" ");
}
