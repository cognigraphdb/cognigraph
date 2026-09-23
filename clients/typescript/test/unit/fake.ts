/** A scripted `fetch` that records every request (CG-84 unit tests). */

export interface Recorded {
  method: string;
  url: string;
  headers: Record<string, string>;
  body: unknown;
}

export type Reply =
  | { status: number; json?: unknown; text?: string; headers?: Record<string, string> }
  | Error
  | "hang";

export function fakeFetch(...replies: Reply[]) {
  const calls: Recorded[] = [];
  const fetch = (input: string | URL | Request, init?: RequestInit): Promise<Response> => {
    const headers: Record<string, string> = {};
    new Headers(init?.headers).forEach((value, key) => {
      headers[key] = value;
    });
    const raw = init?.body;
    calls.push({
      method: init?.method ?? "GET",
      url: String(input),
      headers,
      body: typeof raw === "string" ? JSON.parse(raw) : undefined,
    });
    const reply = replies.length > 1 ? replies.shift() : replies[0];
    if (reply === undefined) throw new Error("no scripted reply");
    if (reply === "hang") {
      return new Promise((_, reject) => {
        init?.signal?.addEventListener("abort", () => reject(init.signal?.reason));
      });
    }
    if (reply instanceof Error) return Promise.reject(reply);
    const body = reply.text ?? (reply.json === undefined ? "" : JSON.stringify(reply.json));
    const type = reply.text === undefined ? "application/json" : "text/plain";
    return Promise.resolve(
      new Response(body, {
        status: reply.status,
        headers: { "content-type": type, ...reply.headers },
      }),
    );
  };
  return { fetch: fetch as typeof globalThis.fetch, calls };
}
