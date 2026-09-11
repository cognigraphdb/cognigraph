import { CheckCircle, Copy, Key } from "@phosphor-icons/react";
import { Button, Modal } from "antd";
import { useState } from "react";
import { describeExpiry, type TokenGrant } from "../lib/tokens.ts";

// Rendered conditionally by the parent (mount = open, unmount = closed).
interface TokenGrantDialogProps {
  grant: TokenGrant;
  username: string;
  onClose: () => void;
}

/// Shows a freshly issued/rotated token's plaintext — the only time it is
/// ever readable (the server stores just the hash). Closing is explicit and
/// the copy is one click, so the value never needs to sit on screen long.
export function TokenGrantDialog({ grant, username, onClose }: TokenGrantDialogProps) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    await navigator.clipboard.writeText(grant.token);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
  };

  return (
    <Modal
      footer={
        <Button onClick={onClose} type="primary">
          Done — I stored it
        </Button>
      }
      onCancel={onClose}
      open
      title={`API token for ${username}`}
    >
      <p className="dialog-hint">
        Copy this token now — it is shown <strong>only once</strong>. The server keeps a hash, not
        the value, so it cannot be recovered later (only revoked or rotated).
      </p>
      <div className="chip-base token-plaintext">
        <Key aria-hidden="true" size={15} />
        <code>{grant.token}</code>
        <Button
          icon={
            copied ? (
              <CheckCircle className="token-copied" size={16} weight="fill" />
            ) : (
              <Copy size={16} />
            )
          }
          onClick={() => void copy()}
          size="small"
          type="text"
        >
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
      <p className="dialog-hint token-expiry">
        {describeExpiry(grant.expires_at, Math.floor(Date.now() / 1000))}
      </p>
    </Modal>
  );
}
