import {
  Buildings,
  Circle,
  Code,
  Cube,
  Database,
  GearSix,
  Graph,
  House,
  MagnifyingGlass,
  Scales,
  SidebarSimple,
  UsersThree,
} from "@phosphor-icons/react";
import { NavLink } from "react-router";
import { routeAccess } from "../lib/access.ts";
import type { HealthSnapshot } from "../types.ts";
import { useAccess } from "./AccessBoundary.tsx";
import { BrandMark } from "./BrandMark.tsx";

interface SidebarProps {
  collapsed: boolean;
  health: HealthSnapshot;
  onToggle: () => void;
}

const navigation: Array<{ label: string; path: string; icon: typeof House }> = [
  { label: "Overview", path: "/overview", icon: House },
  { label: "Collections", path: "/collections", icon: Database },
  { label: "Query", path: "/query", icon: MagnifyingGlass },
  { label: "Graph", path: "/graph", icon: Graph },
  { label: "Review", path: "/review", icon: Scales },
  { label: "Construct", path: "/construct", icon: Cube },
  { label: "Lua", path: "/lua", icon: Code },
  { label: "Users", path: "/users", icon: UsersThree },
  { label: "Operations", path: "/operations", icon: GearSix },
];

// Tenant lifecycle is a host-admin capability (Scope::TenantAdmin); other
// roles never see the entry — the server guard is the real enforcement.
const tenantsEntry = { label: "Tenants", path: "/tenants", icon: Buildings };

export function Sidebar({ collapsed, health, onToggle }: SidebarProps) {
  const access = useAccess();
  const items = [...navigation, tenantsEntry].filter(
    ({ path }) => routeAccess(path, access).allowed,
  );
  return (
    <aside className={collapsed ? "sidebar collapsed" : "sidebar"}>
      <div aria-label="CogniGraph" className="brand" role="img">
        <BrandMark />
        <span className="brand-name">CogniGraph</span>
      </div>

      <nav className="primary-nav" aria-label="Primary navigation">
        {items.map(({ label, path, icon: Icon }) => (
          <NavLink
            className={({ isActive }) => (isActive ? "nav-item active" : "nav-item")}
            key={path}
            to={path}
          >
            <Icon aria-hidden="true" size={21} />
            <span>{label}</span>
          </NavLink>
        ))}
      </nav>

      <div className="sidebar-footer">
        <div className="health-line">
          <Circle aria-hidden="true" className="health-dot" size={9} weight="fill" />
          <span>
            {health.status === "online"
              ? "System healthy"
              : health.status === "checking"
                ? "Checking system"
                : "System offline"}
          </span>
        </div>
        <small>Version {health.version ?? "—"}</small>
        <button
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          className="collapse-button"
          onClick={onToggle}
          type="button"
        >
          <SidebarSimple aria-hidden="true" size={20} />
          <span>{collapsed ? "Expand" : "Collapse"}</span>
        </button>
      </div>
    </aside>
  );
}
