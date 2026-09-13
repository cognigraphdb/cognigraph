import { Alert, Modal } from "antd";
import { useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { parseTenantDeletion, type TenantDeletionResponse } from "../lib/tenants.ts";
import { ErrorAlert } from "./ErrorAlert.tsx";

export interface TenantDeletionResult extends TenantDeletionResponse {
  name: string;
}

// Mount only while open, following the console's Modal compatibility convention.
export function DeleteTenantDialog({
  api,
  name,
  onClose,
  onDeleted,
}: {
  api: CogniGraphApi;
  name: string;
  onClose: () => void;
  onDeleted: (result: TenantDeletionResult) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  async function remove() {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      const response = parseTenantDeletion(
        await api.delete<unknown>(`/tenants/${encodeURIComponent(name)}`),
      );
      onDeleted({ name, ...response });
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Tenant deletion failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      cancelButtonProps={{ disabled: busy }}
      cancelText="Cancel"
      closable={!busy}
      confirmLoading={busy}
      keyboard={!busy}
      mask={{ closable: !busy }}
      okButtonProps={{ danger: true, disabled: !!error }}
      okText="Delete tenant"
      onCancel={() => !busy && onClose()}
      onOk={() => void remove()}
      open
      title="Delete tenant?"
      width={560}
    >
      <p className="tenant-deletion-name">
        Delete <strong>{name}</strong>:
      </p>
      <ul>
        <li>Remove its user accounts and API tokens; existing sessions lose access.</li>
        <li>Retire its queued work and promotions.</li>
        <li>Quarantine its data files for operator-managed recovery or purge.</li>
      </ul>
      <p>
        Recreating this name starts empty. It does not restore accounts, credentials or data. Data
        recovery requires a separate operator procedure.
      </p>
      <p>For a temporary access pause that preserves accounts and data, use Suspend instead.</p>
      {error ? (
        <>
          <ErrorAlert title={error} />
          <p>
            Deletion was not confirmed. Some changes may already have occurred. Close this dialog
            and refresh the tenant list before trying again.
          </p>
        </>
      ) : null}
    </Modal>
  );
}

export function TenantDeletionNotice({ result }: { result: TenantDeletionResult }) {
  return (
    <Alert
      className="tenant-deletion-notice"
      title={
        result.deleted
          ? `Deleted tenant ${result.name}`
          : `No tenant record was deleted for ${result.name}`
      }
      type={result.deleted ? "success" : "warning"}
      description={
        <>
          {result.quarantined.length ? (
            <>
              <p>Quarantined entries reported by the server:</p>
              <ul className="tenant-quarantine-list">
                {result.quarantined.map((entry) => (
                  <li key={entry}>
                    <code>{entry}</code>
                  </li>
                ))}
              </ul>
            </>
          ) : (
            <p>The server reported no quarantined entries.</p>
          )}
          <p>Recreating the name does not restore accounts, credentials or quarantined data.</p>
        </>
      }
    />
  );
}
