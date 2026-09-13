import { CheckCircle, Info, WarningCircle, XCircle } from "@phosphor-icons/react";
import { App as AntApp } from "antd";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Navigate, Route, Routes, useLocation, useNavigate } from "react-router";
import { ApiError, CogniGraphApi } from "./api/client.ts";
import { AccessBoundary, AccessContext } from "./components/AccessBoundary.tsx";
import { ConnectionGate } from "./components/ConnectionGate.tsx";
import { Sidebar } from "./components/Sidebar.tsx";
import { TopBar } from "./components/TopBar.tsx";
import { consoleAccess, landingRoute, type SessionContext } from "./lib/access.ts";
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
  const [verified, setVerified] = useState<{ api: CogniGraphApi; value: SessionContext }>();
  const [gateError, setGateError] = useState("");
  const [gate, setGate] = useState<AuthGate>("checking");
  const [health, setHealth] = useState<HealthSnapshot>({ status: "checking" });
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const api = useMemo(() => new CogniGraphApi(config), [config]);
  const context = verified?.api === api ? verified.value : undefined;
  const session = context?.user ?? null;
  const access = context ? consoleAccess(context) : undefined;
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
      setVerified(undefined);
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
    let established = false;
    const verify = () =>
      api
        .sessionContext()
        .then((value) => {
          if (!cancelled) {
            established = true;
            setVerified({ api, value });
            if (value.user)
              sessionStorage.setItem("cognigraph-session", JSON.stringify(value.user));
            else {
              sessionStorage.removeItem("cognigraph-session");
              sessionStorage.removeItem("cognigraph-api-token");
              setConfig((current) => (current.token ? { ...current, token: "" } : current));
            }
            setGate("ready");
          }
        })
        .catch((error) => {
          if (!cancelled) {
            if (error instanceof ApiError && error.status === 401) {
              setGate("login");
              return;
            }
            if (established && (!(error instanceof ApiError) || error.status >= 500)) return;
            setGateError(error instanceof Error ? error.message : "Server verification failed.");
            setGate("unavailable");
          }
        });
    void verify();
    const timer = window.setInterval(() => void verify(), 15_000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [api]);

  const onAuthenticated = (baseUrl: string, token: string, next: AuthSession) => {
    sessionStorage.setItem("cognigraph-api-url", baseUrl);
    sessionStorage.setItem("cognigraph-api-token", token);
    sessionStorage.setItem("cognigraph-session", JSON.stringify(next));
    setVerified(undefined);
    setConfig({ baseUrl, token });
    setGate("checking");
    notify(`Signed in as ${next.username}`);
  };

  const logout = () => {
    sessionStorage.removeItem("cognigraph-api-token");
    sessionStorage.removeItem("cognigraph-session");
    setVerified(undefined);
    setConfig((current) => ({ ...current, token: "" }));
  };

  if (gate === "checking" || gate === "unavailable" || (gate === "ready" && !access)) {
    return (
      <ConnectionGate
        server={config.baseUrl}
        checking={gate !== "unavailable"}
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
                setVerified(undefined);
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

  if (!access) return null;

  return (
    <AccessContext.Provider value={access}>
      <div className={sidebarCollapsed ? "app-shell sidebar-collapsed" : "app-shell"}>
        <Sidebar
          collapsed={sidebarCollapsed}
          health={health}
          onToggle={() => setSidebarCollapsed((current) => !current)}
        />
        <TopBar
          health={health}
          server={config.baseUrl}
          tenant={session?.tenant ?? "default"}
          username={
            session?.username ??
            (config.token ? "Authenticated session" : "Authentication disabled")
          }
          onLogout={session ? logout : undefined}
        />
        <AccessBoundary>
          <Routes>
            <Route element={<Navigate replace to={landingRoute(access)} />} path="/" />
            <Route
              element={
                <OverviewScreen
                  api={api}
                  health={health}
                  onRefresh={refreshHealth}
                  session={session}
                />
              }
              path="/overview"
            />
            <Route
              element={<CollectionsIndexScreen api={api} notify={notify} />}
              path="/collections"
            />
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
                <UsersScreen
                  api={api}
                  notify={notify}
                  session={session}
                  edition={context?.edition}
                />
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
            <Route element={<Navigate replace to={landingRoute(access)} />} path="*" />
          </Routes>
        </AccessBoundary>
      </div>
    </AccessContext.Provider>
  );
}
