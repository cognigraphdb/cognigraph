import { CheckCircle, Info, WarningCircle, XCircle } from "@phosphor-icons/react";
import { App as AntApp } from "antd";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Navigate, Route, Routes, useLocation, useNavigate } from "react-router";
import { CogniGraphApi } from "./api/client.ts";
import { ConnectionGate } from "./components/ConnectionGate.tsx";
import { Sidebar } from "./components/Sidebar.tsx";
import { TopBar } from "./components/TopBar.tsx";
import { defaultApiOrigin } from "./lib/api-origin.ts";
import { CollectionsIndexScreen } from "./screens/CollectionsIndexScreen.tsx";
import { CollectionsScreen } from "./screens/CollectionsScreen.tsx";
import { ConstructScreen } from "./screens/ConstructScreen.tsx";
import { GraphScreen } from "./screens/GraphScreen.tsx";
import { LoginScreen } from "./screens/LoginScreen.tsx";
import { LuaScreen } from "./screens/LuaScreen.tsx";
import { OperationsScreen } from "./screens/OperationsScreen.tsx";
import { OverviewScreen } from "./screens/OverviewScreen.tsx";
import { QueryScreen } from "./screens/QueryScreen.tsx";
import { ReviewScreen } from "./screens/ReviewScreen.tsx";
import { TenantsScreen } from "./screens/TenantsScreen.tsx";
import { UserDetailScreen } from "./screens/UserDetailScreen.tsx";
import { UsersScreen } from "./screens/UsersScreen.tsx";
import type { ApiConfig, AuthSession, HealthSnapshot, Notify } from "./types.ts";

const readSession = (): AuthSession | null => {
  const raw = sessionStorage.getItem("cognigraph-session");
  if (!raw) return null;
  try {
    return JSON.parse(raw) as AuthSession;
  } catch {
    return null;
  }
};

type AuthGate = "checking" | "login" | "ready" | "unavailable";

const initialConfig = (): ApiConfig => ({
  baseUrl: sessionStorage.getItem("cognigraph-api-url") ?? defaultApiOrigin(),
  token: sessionStorage.getItem("cognigraph-api-token") ?? "",
});

const noticeIcons = {
  error: <XCircle weight="fill" />,
  info: <Info weight="fill" />,
  success: <CheckCircle weight="fill" />,
  warning: <WarningCircle weight="fill" />,
};

export function App() {
  const [config, setConfig] = useState(initialConfig);
  const [session, setSession] = useState<AuthSession | null>(readSession);
  const [gateError, setGateError] = useState("");
  const [gate, setGate] = useState<AuthGate>("checking");
  const [health, setHealth] = useState<HealthSnapshot>({ status: "checking" });
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const api = useMemo(() => new CogniGraphApi(config), [config]);
  const { message: messageApi } = AntApp.useApp();
  const navigate = useNavigate();
  const location = useLocation();

  // Every page has a copyable URL; keep the tab title in step with it.
  useEffect(() => {
    const segment = location.pathname.split("/")[1] || "collections";
    document.title = `CogniGraph — ${segment.charAt(0).toUpperCase()}${segment.slice(1)}`;
  }, [location.pathname]);

  const notify: Notify = useCallback(
    (content, type = "success") => {
      void messageApi.open({
        content,
        duration: 2.6,
        icon: noticeIcons[type],
        key: "cognigraph-notice",
        type,
      });
    },
    [messageApi],
  );

  // An expired/revoked session surfaces as a 401 on a token-bearing request.
  // Clear the dead token so the auth gate re-probes and lands on login.
  useEffect(() => {
    api.onUnauthorized = () => {
      sessionStorage.removeItem("cognigraph-api-token");
      sessionStorage.removeItem("cognigraph-session");
      setSession(null);
      setConfig((current) => (current.token ? { ...current, token: "" } : current));
      notify("Session expired — please sign in again", "warning");
    };
    return () => {
      api.onUnauthorized = undefined;
    };
  }, [api, notify]);

  const refreshHealth = useCallback(async () => {
    setHealth((current) => ({ ...current, status: "checking" }));
    setHealth(await api.health());
  }, [api]);

  useEffect(() => {
    void refreshHealth();
    const timer = window.setInterval(refreshHealth, 15_000);
    return () => window.clearInterval(timer);
  }, [refreshHealth]);

  // Decide whether to show the login screen: if the server enforces auth and the
  // current token can't satisfy it, gate on login. Re-runs whenever the client
  // (URL or token) changes, so a successful sign-in flips straight to ready and
  // a logout flips back to login.
  useEffect(() => {
    let cancelled = false;
    setGate("checking");
    setGateError("");
    void api
      .authRequired()
      .then((needed) => {
        if (!cancelled) setGate(needed ? "login" : "ready");
      })
      .catch((error) => {
        if (!cancelled) {
          setGateError(error instanceof Error ? error.message : "Server verification failed.");
          setGate("unavailable");
        }
      });
    return () => {
      cancelled = true;
    };
  }, [api]);

  const onAuthenticated = (baseUrl: string, token: string, next: AuthSession) => {
    sessionStorage.setItem("cognigraph-api-url", baseUrl);
    sessionStorage.setItem("cognigraph-api-token", token);
    sessionStorage.setItem("cognigraph-session", JSON.stringify(next));
    setSession(next);
    setConfig({ baseUrl, token });
    setGate("ready");
    notify(`Signed in as ${next.username}`);
  };

  const logout = () => {
    sessionStorage.removeItem("cognigraph-api-token");
    sessionStorage.removeItem("cognigraph-session");
    setSession(null);
    setConfig((current) => ({ ...current, token: "" }));
  };

  if (gate === "checking" || gate === "unavailable") {
    return (
      <ConnectionGate
        server={config.baseUrl}
        checking={gate === "checking"}
        error={gateError}
        onRetry={() => {
          setGate("checking");
          setConfig((current) => ({ ...current }));
        }}
        onReset={
          config.baseUrl !== defaultApiOrigin() || config.token
            ? () => {
                // A saved override and its credentials belong together. Never forward
                // the old bearer token when returning to the page's default server.
                setGate("checking");
                sessionStorage.removeItem("cognigraph-api-url");
                sessionStorage.removeItem("cognigraph-api-token");
                sessionStorage.removeItem("cognigraph-session");
                setSession(null);
                setConfig({ baseUrl: defaultApiOrigin(), token: "" });
              }
            : undefined
        }
      />
    );
  }

  if (gate === "login") {
    return <LoginScreen defaultBaseUrl={config.baseUrl} onAuthenticated={onAuthenticated} />;
  }

  return (
    <div className={sidebarCollapsed ? "app-shell sidebar-collapsed" : "app-shell"}>
      <Sidebar
        collapsed={sidebarCollapsed}
        health={health}
        onToggle={() => setSidebarCollapsed((current) => !current)}
        role={session?.role}
      />
      <TopBar
        health={health}
        server={config.baseUrl}
        tenant={session?.tenant ?? "default"}
        username={
          session?.username ?? (config.token ? "Authenticated session" : "Authentication disabled")
        }
        onLogout={session ? logout : undefined}
      />
      <Routes>
        <Route element={<Navigate replace to="/collections" />} path="/" />
        <Route
          element={
            <OverviewScreen api={api} health={health} onRefresh={refreshHealth} session={session} />
          }
          path="/overview"
        />
        <Route element={<CollectionsIndexScreen api={api} notify={notify} />} path="/collections" />
        <Route
          element={<CollectionsScreen api={api} connection={health.status} notify={notify} />}
          path="/collections/:collection"
        />
        <Route element={<QueryScreen api={api} notify={notify} />} path="/query" />
        <Route
          element={
            <GraphScreen
              api={api}
              notify={notify}
              onOpenDocument={(id) => {
                const [collection, key] = id.split("/");
                navigate(
                  `/collections/${encodeURIComponent(collection ?? "documents")}?doc=${encodeURIComponent(key ?? "")}`,
                );
              }}
            />
          }
          path="/graph"
        />
        <Route element={<ReviewScreen api={api} notify={notify} />} path="/review" />
        <Route element={<ConstructScreen api={api} notify={notify} />} path="/construct" />
        <Route element={<LuaScreen api={api} notify={notify} />} path="/lua" />
        <Route
          element={
            <UsersScreen api={api} notify={notify} session={session} edition={health.edition} />
          }
          path="/users"
        />
        <Route
          element={
            <UserDetailScreen api={api} currentUsername={session?.username} notify={notify} />
          }
          path="/users/:username"
        />
        <Route
          element={<OperationsScreen api={api} baseUrl={config.baseUrl} notify={notify} />}
          path="/operations"
        />
        <Route element={<TenantsScreen api={api} notify={notify} />} path="/tenants" />
        <Route element={<Navigate replace to="/collections" />} path="*" />
      </Routes>
    </div>
  );
}
