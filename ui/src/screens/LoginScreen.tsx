import { LockKey, UserCircle } from "@phosphor-icons/react";
import { Button, Form, Input } from "antd";
import { useState } from "react";
import { ApiError, CogniGraphApi } from "../api/client.ts";
import { BrandMark } from "../components/BrandMark.tsx";
import { ErrorAlert } from "../components/ErrorAlert.tsx";
import type { AuthSession } from "../types.ts";

interface LoginScreenProps {
  defaultBaseUrl: string;
  onAuthenticated: (baseUrl: string, token: string, session: AuthSession) => void;
}

interface Fields {
  username: string;
  password: string;
}

// The sign-in target is NOT user-editable: the API base derives from the
// origin the console was loaded from, and this screen only appears when that
// server is reachable and enforcing auth. Showing (rather than asking for)
// the server keeps credentials from being redirected by a mistyped or
// malicious URL; operators can still point at another server from Overview
// after signing in.
export function LoginScreen({ defaultBaseUrl, onAuthenticated }: LoginScreenProps) {
  const [form] = Form.useForm<Fields>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();

  async function submit(values: Fields) {
    const username = values.username.trim();
    setBusy(true);
    setError(undefined);
    try {
      // A fresh, token-less client aimed at the derived server.
      const api = new CogniGraphApi({ baseUrl: defaultBaseUrl, token: "" });
      const { token, role, tenant } = await api.login(username, values.password);
      onAuthenticated(defaultBaseUrl, token, { username, role, tenant });
    } catch (err) {
      setError(
        err instanceof ApiError && err.status === 401
          ? "Invalid username or password."
          : err instanceof Error
            ? err.message
            : "Sign-in failed.",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="auth-screen">
      <div className="auth-card">
        <div className="auth-brand">
          <BrandMark />
          <div>
            <h1>CogniGraph Console</h1>
            <p>Sign in to continue.</p>
          </div>
        </div>

        <Form<Fields>
          form={form}
          layout="vertical"
          requiredMark={false}
          initialValues={{ username: "", password: "" }}
          onFinish={submit}
          onValuesChange={() => error && setError(undefined)}
        >
          <Form.Item
            label="Username"
            name="username"
            rules={[{ required: true, message: "Enter your username." }]}
          >
            <Input
              autoFocus
              autoComplete="username"
              prefix={<UserCircle size={16} />}
              placeholder="admin"
            />
          </Form.Item>

          <Form.Item
            label="Password"
            name="password"
            rules={[{ required: true, message: "Enter your password." }]}
          >
            <Input.Password
              autoComplete="current-password"
              prefix={<LockKey size={16} />}
              placeholder="Your password"
              onPressEnter={() => form.submit()}
            />
          </Form.Item>

          {error ? <ErrorAlert title={error} /> : null}

          <Button type="primary" htmlType="submit" block loading={busy} className="auth-submit">
            Sign in
          </Button>
        </Form>

        <p className="auth-foot">
          Signing in to <strong>{defaultBaseUrl.replace(/^https?:\/\//, "")}</strong> — your session
          token stays in this browser and expires automatically.
        </p>
      </div>
    </div>
  );
}
