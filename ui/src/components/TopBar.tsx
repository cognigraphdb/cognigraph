import { Circle, Clock, SignOut, UserCircle } from "@phosphor-icons/react";
import { Tooltip } from "antd";
import type { HealthSnapshot } from "../types.ts";

interface TopBarProps {
  tenant: string;
  server: string;
  health: HealthSnapshot;
  username: string;
  onLogout?: () => void;
}

export function TopBar({ tenant, server, health, username, onLogout }: TopBarProps) {
  return (
    <header className="topbar">
      <Tooltip title="Tenant is determined by the authenticated token">
        <div className="context-field">
          <span>Tenant</span>
          <strong>{tenant}</strong>
        </div>
      </Tooltip>

      <div className="context-divider" />

      <div className="context-field">
        <span>Server</span>
        <strong>{server.replace(/^https?:\/\//, "")}</strong>
      </div>
      <div className="health-line top-health">
        <Circle aria-hidden="true" className="health-dot" size={9} weight="fill" />
        <span>
          {health.status === "online"
            ? "Healthy"
            : health.status === "checking"
              ? "Checking"
              : "Offline"}
        </span>
      </div>

      <div className="topbar-spacer" />

      <div className="topbar-meta">
        <Clock aria-hidden="true" size={17} />
        <span>
          {new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(
            new Date(),
          )}
        </span>
        <span className="meta-divider" />
        <UserCircle aria-hidden="true" size={20} />
        <span>{username}</span>
        {onLogout ? (
          <button type="button" className="topbar-logout" onClick={onLogout} title="Sign out">
            <SignOut aria-hidden="true" size={17} />
            <span>Sign out</span>
          </button>
        ) : null}
      </div>
    </header>
  );
}
