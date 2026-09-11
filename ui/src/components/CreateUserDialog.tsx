import { Form, Input, Modal, Select } from "antd";
import { useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import { ROLE_META, USER_ROLES, type UserRole } from "../lib/users.ts";
import type { Notify } from "../types.ts";
import { ErrorAlert } from "./ErrorAlert.tsx";

// Rendered conditionally by the parent (mount = open, unmount = closed) —
// the app-wide Modal convention: toggling `open` on a mounted Modal leaves
// the close transition hanging in the Bun dev bundle (see DocumentDialog).
interface CreateUserDialogProps {
  api: CogniGraphApi;
  notify: Notify;
  onClose: () => void;
  onCreated: () => void;
}

interface Fields {
  username: string;
  password: string;
  role: UserRole;
  tenant?: string;
}

export function CreateUserDialog({ api, notify, onClose, onCreated }: CreateUserDialogProps) {
  const [form] = Form.useForm<Fields>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const role = Form.useWatch("role", form) ?? "editor";

  async function submit(values: Fields) {
    setBusy(true);
    setError("");
    try {
      await api.post("/users", {
        username: values.username.trim(),
        password: values.password,
        role: values.role,
        // Omitted -> the server files the user under the default tenant.
        ...(values.tenant?.trim() ? { tenant: values.tenant.trim() } : {}),
      });
      notify(`Created ${values.username.trim()}`);
      onCreated();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "User creation failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      confirmLoading={busy}
      okText="Create user"
      onCancel={onClose}
      onOk={() => form.submit()}
      open
      title="Create a user"
    >
      <Form<Fields>
        form={form}
        initialValues={{ role: "editor" }}
        layout="vertical"
        onFinish={(values) => void submit(values)}
        requiredMark={false}
      >
        <Form.Item
          label="Username"
          name="username"
          rules={[{ required: true, message: "Enter a username." }]}
        >
          <Input autoComplete="off" autoFocus placeholder="vera" />
        </Form.Item>
        <Form.Item
          label="Password"
          name="password"
          rules={[
            { required: true, message: "Enter a password." },
            { min: 8, message: "Use at least 8 characters." },
          ]}
        >
          <Input.Password autoComplete="new-password" placeholder="At least 8 characters" />
        </Form.Item>
        <Form.Item label="Role" name="role" rules={[{ required: true }]}>
          <Select
            options={USER_ROLES.map((value) => ({
              label: ROLE_META[value].label,
              value,
            }))}
          />
        </Form.Item>
        <p className="dialog-hint role-hint">{ROLE_META[role].scopes}</p>
        <Form.Item label="Tenant (optional)" name="tenant">
          <Input placeholder="default" />
        </Form.Item>
      </Form>
      {error ? <ErrorAlert title={error} /> : null}
    </Modal>
  );
}
