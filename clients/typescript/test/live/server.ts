/**
 * A disposable CogniGraph server for live client tests: fresh store, fresh
 * credentials, loopback only. The binary comes from CG_CLIENT_SERVER_BIN;
 * a missing binary fails the run instead of skipping it.
 */
import { mkdtempSync, rmSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";

export const PASSWORD = "synthetic-cg84-password";

async function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const probe = createServer();
    probe.once("error", reject);
    probe.listen(0, "127.0.0.1", () => {
      const address = probe.address();
      probe.close(() => resolve(typeof address === "object" && address ? address.port : 0));
    });
  });
}

export interface LiveServer {
  baseUrl: string;
  stop(): Promise<void>;
}

export async function startServer(): Promise<LiveServer> {
  const binary = process.env.CG_CLIENT_SERVER_BIN;
  if (!binary) throw new Error("CG_CLIENT_SERVER_BIN must name a cognigraph-server binary");
  const directory = mkdtempSync(join(tmpdir(), "cg84-live-"));
  const port = await freePort();
  const env: Record<string, string> = {
    PATH: process.env.PATH ?? "",
    HOME: directory,
    COGNIGRAPH_HOST: "127.0.0.1",
    COGNIGRAPH_PORT: String(port),
    COGNIGRAPH_AUTH_ENABLED: "true",
    COGNIGRAPH_ADMIN_PASSWORD: PASSWORD,
    COGNIGRAPH_HOST_ADMIN_PASSWORD: PASSWORD,
    COGNIGRAPH_JWT_SECRET: "synthetic-cg84-jwt-secret-with-enough-length",
    COGNIGRAPH_CGQL_MUTATIONS_ENABLED: "true",
    COGNIGRAPH_EMBEDDING_PROVIDER: "none",
    COGNIGRAPH_NATIVE_PATH: join(directory, "store.redb"),
  };
  const child = Bun.spawn([binary], { cwd: directory, env, stdout: "ignore", stderr: "pipe" });
  const baseUrl = `http://127.0.0.1:${port}`;
  for (let i = 0; ; i++) {
    if (child.exitCode !== null) {
      throw new Error(`server exited: ${await new Response(child.stderr).text()}`);
    }
    try {
      if ((await fetch(`${baseUrl}/health/database`)).ok) break;
    } catch {
      // not listening yet
    }
    if (i > 200) throw new Error("server did not become ready");
    await Bun.sleep(50);
  }
  return {
    baseUrl,
    async stop() {
      child.kill("SIGTERM");
      await child.exited;
      rmSync(directory, { recursive: true, force: true });
    },
  };
}
