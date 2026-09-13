import { Form, Input, Modal } from "antd";
import { useState } from "react";
import type { CogniGraphApi } from "../api/client.ts";
import type { UserAccount } from "../lib/users.ts";
import { ErrorAlert } from "./ErrorAlert.tsx";

export function CreateTenantDialog({
  api,
  onClose,
  onCreated,
}: {
  api: CogniGraphApi;
  onClose: () => void;
  onCreated: (name: string) => void;
}) {
  const [form] = Form.useForm<{ name: string }>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function submit({ name }: { name: string }) {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      const result = await api.post<{ name: string }>("/tenants", { name: name.trim() });
      if (result?.name !== name.trim())
        throw new Error("Tenant creation was not confirmed. Refresh the list before retrying.");
      onCreated(result.name);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Tenant creation failed");
    } finally {
      setBusy(false);
    }
  }
  return (
    <Modal
      cancelButtonProps={{ disabled: busy }}
      closable={!busy}
      confirmLoading={busy}
      keyboard={!busy}
      mask={{ closable: !busy }}
      okText="Create tenant"
      onCancel={() => !busy && onClose()}
      onOk={() => form.submit()}
      open
      title="Create a tenant"
    >
      <p className="dialog-hint">
        Create the tenant, then set up its first administrator. If you leave the next step, use Set
        up admin on the tenant row to continue.
      </p>
      <p className="dialog-hint">
        This form uses the server's default quotas. The server enforces max_active_jobs limits.
      </p>
      <Form
        form={form}
        layout="vertical"
        onFinish={(values) => void submit(values)}
        requiredMark={false}
      >
        <Form.Item
          label="Tenant name"
          name="name"
          rules={[{ required: true, whitespace: true, message: "Name the tenant." }]}
        >
          <Input autoFocus placeholder="acme" />
        </Form.Item>
      </Form>
      {error ? <ErrorAlert title={error} /> : null}
    </Modal>
  );
}

export function BootstrapTenantAdminDialog({
  api,
  name,
  onClose,
  onCreated,
}: {
  api: CogniGraphApi;
  name: string;
  onClose: () => void;
  onCreated: (user: UserAccount) => void;
}) {
  const [form] = Form.useForm<{ username: string; password: string }>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function submit(values: { username: string; password: string }) {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      const username = values.username.trim();
      const user = await api.post<UserAccount>(`/tenants/${encodeURIComponent(name)}/admin`, {
        username,
        password: values.password,
      });
      if (
        user?.tenant !== name ||
        user.role !== "admin" ||
        user.username !== username ||
        !user.key
      ) {
        throw new Error("Administrator creation was not confirmed. Check sign-in before retrying.");
      }
      form.resetFields(["password"]);
      onCreated(user);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Administrator setup failed");
    } finally {
      setBusy(false);
    }
  }
  return (
    <Modal
      cancelButtonProps={{ disabled: busy }}
      closable={!busy}
      confirmLoading={busy}
      keyboard={!busy}
      mask={{ closable: !busy }}
      okText="Create administrator"
      onCancel={() => !busy && onClose()}
      onOk={() => form.submit()}
      open
      title="Set up first administrator"
    >
      <p className="dialog-hint">
        Tenant: <strong>{name}</strong>
      </p>
      <p className="dialog-hint">
        Create the first tenant-local Admin. The server refuses this step if an Admin already
        exists. Further accounts must be created while signed in as that tenant's administrator.
        Closing this dialog leaves the tenant in place.
      </p>
      <Form
        form={form}
        layout="vertical"
        onFinish={(values) => void submit(values)}
        requiredMark={false}
      >
        <Form.Item
          label="Administrator username"
          name="username"
          rules={[{ required: true, whitespace: true, message: "Enter a username." }]}
        >
          <Input autoFocus autoComplete="off" placeholder="acme-admin" />
        </Form.Item>
        <Form.Item
          label="Administrator password"
          name="password"
          rules={[
            { required: true, message: "Enter a password." },
            { min: 8, message: "Use at least 8 characters." },
          ]}
        >
          <Input.Password autoComplete="new-password" />
        </Form.Item>
      </Form>
      {error ? <ErrorAlert title={error} /> : null}
    </Modal>
  );
}
