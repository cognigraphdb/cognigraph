import { Button } from "antd";
import { BrandMark } from "./BrandMark.tsx";
import { ErrorAlert } from "./ErrorAlert.tsx";

interface Props {
  server: string;
  checking: boolean;
  error: string;
  onRetry: () => void;
  onReset?: () => void;
}

export function ConnectionGate({ server, checking, error, onRetry, onReset }: Props) {
  return (
    <main className="auth-screen">
      <section className="auth-card connection-card" aria-busy={checking}>
        <div className="auth-brand">
          <BrandMark />
          <h1>{checking ? "Connecting to server" : "Unable to verify server"}</h1>
        </div>
        <p className="connection-server">{server}</p>
        <p role="status">
          {checking
            ? "Checking whether this server requires sign-in…"
            : "Authentication has not been verified. Check that the server is available, then retry."}
        </p>
        {!checking ? (
          <>
            <ErrorAlert title={error} />
            <div className="actions-row-base connection-actions">
              <Button type="primary" onClick={onRetry}>
                Retry connection
              </Button>
              {onReset ? <Button onClick={onReset}>Use default server</Button> : null}
            </div>
          </>
        ) : null}
      </section>
    </main>
  );
}
