import { Form, Input, Modal, Select } from "antd";
import { useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { type TokenGrant, TTL_PRESETS, ttlSecondsFor } from "../lib/tokens.ts";
import { ErrorAlert } from "./ErrorAlert.tsx";

// Rendered conditionally by the parent (mount = open, unmount = closed).
interface CreateTokenDialogProps {
  api: CogniGraphApi;
  userKey: string;
  username: string;
  onClose: () => void;
  /// Receives the grant so the parent can hand it to TokenGrantDialog —
  /// the plaintext is shown exactly once.
  onIssued: (grant: TokenGrant) => void;
}

interface Fields {
  name: string;
  ttl: string;
}

export function CreateTokenDialog({
  api,
  userKey,
  username,
  onClose,
  onIssued,
}: CreateTokenDialogProps) {
  const [form] = Form.useForm<Fields>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  async function submit(values: Fields) {
    setBusy(true);
    setError("");
    try {
      const seconds = ttlSecondsFor(values.ttl);
      const grant = await api.post<TokenGrant>(`/users/${encodeURIComponent(userKey)}/tokens`, {
        name: values.name.trim(),
        // Omitted -> server default TTL; 0 -> explicitly non-expiring.
        ...(seconds !== undefined ? { expires_in_secs: seconds } : {}),
      });
      onIssued(grant);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Token creation failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      confirmLoading={busy}
      okText="Issue token"
      onCancel={onClose}
      onOk={() => form.submit()}
      open
      title={`New API token for ${username}`}
    >
      <Form<Fields>
        form={form}
        initialValues={{ ttl: "default" }}
        layout="vertical"
        onFinish={(values) => void submit(values)}
        requiredMark={false}
      >
        <Form.Item
          label="Token name"
          name="name"
          rules={[{ required: true, message: "Name the token after what will use it." }]}
        >
          <Input autoFocus placeholder="ci-pipeline" />
        </Form.Item>
        <Form.Item label="Expiry" name="ttl" rules={[{ required: true }]}>
          <Select
            options={TTL_PRESETS.map((preset) => ({ label: preset.label, value: preset.value }))}
          />
        </Form.Item>
      </Form>
      {error ? <ErrorAlert title={error} /> : null}
    </Modal>
  );
}
