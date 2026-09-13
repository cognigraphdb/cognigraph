import { Button, Result } from "antd";
import { createContext, type ReactNode, useContext } from "react";
import { useLocation, useNavigate } from "react-router";
import { type ConsoleAccess, landingRoute, routeAccess } from "../lib/access.ts";

export const AccessContext = createContext<ConsoleAccess | null>(null);

export function useAccess(): ConsoleAccess {
  const access = useContext(AccessContext);
  if (!access) throw new Error("Console access must be verified before mounting a screen.");
  return access;
}

// The child is not mounted on denial, so direct URLs cannot start forbidden
// screen effects. The server remains the authorization boundary for every call.
export function AccessBoundary({ children }: { children: ReactNode }) {
  const access = useAccess();
  const { pathname } = useLocation();
  const navigate = useNavigate();
  const { allowed, reason } = routeAccess(pathname, access);
  return allowed ? (
    children
  ) : (
    <main className="page-workspace">
      <Result
        status="403"
        title="Page unavailable"
        subTitle={reason}
        extra={
          <Button type="primary" onClick={() => navigate(landingRoute(access))}>
            Open your workspace
          </Button>
        }
      />
    </main>
  );
}
