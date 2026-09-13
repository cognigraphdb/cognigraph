import { CircleNotch } from "@phosphor-icons/react";
import { Alert, Button } from "antd";
import { Link } from "react-router";
import type { useSpaceTypes } from "../hooks/useSpaceTypes.ts";
import { useAccess } from "./AccessBoundary.tsx";

interface Props {
  catalog: ReturnType<typeof useSpaceTypes>;
  screen: "review" | "construct";
  busy?: boolean;
}

// Loading and failures take precedence over an empty (or previously loaded)
// catalog. Edition and route-level role denials remain with AccessBoundary.
export function SpaceCatalogNotice({ catalog, screen, busy = false }: Props) {
  const { constructWrite } = useAccess();
  if (catalog.loading) {
    return (
      <Alert
        role="status"
        type="info"
        showIcon
        icon={<CircleNotch className="cg-spin" />}
        title="Loading spaces…"
      />
    );
  }
  if (catalog.error) {
    const denied = catalog.errorStatus === 403;
    return (
      <Alert
        type={denied ? "warning" : "error"}
        showIcon
        title={denied ? "Space access denied" : "Unable to load spaces"}
        description={
          denied
            ? "This account cannot read spaces in this tenant. Sign in with an authorized account, or ask your administrator to check access."
            : catalog.error
        }
        action={
          <Button disabled={busy} onClick={catalog.refresh}>
            Retry spaces
          </Button>
        }
      />
    );
  }
  if (catalog.spaces.length) return null;
  return (
    <Alert
      type="info"
      showIcon
      title="No accepted spaces in this tenant"
      description={
        constructWrite ? (
          <>
            {screen === "review" ? (
              <>
                <Link to="/construct">Open Construct</Link> to create or accept a space draft.{" "}
              </>
            ) : (
              <>Create a draft below or prepare one through the documents API. </>
            )}
            Review and edit it in <Link to="/collections/space_type_drafts">Space drafts</Link>,
            then use Accept draft in Construct. Acceptance makes the space available to Review and
            construction.
          </>
        ) : (
          "Ask a tenant editor or administrator to create and review a space draft, then accept it in Construct. Your account can inspect spaces after acceptance."
        )
      }
    />
  );
}
