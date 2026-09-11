import { BracketsCurly } from "@phosphor-icons/react";
import { JsonCode } from "./JsonCode.tsx";

export function JsonResult({
  value,
  empty = "Run the request to inspect its response.",
}: {
  value?: unknown;
  empty?: string;
}) {
  if (value === undefined) {
    return (
      <div className="empty-state-base result-empty">
        <BracketsCurly aria-hidden="true" size={25} />
        <span>{empty}</span>
      </div>
    );
  }
  return <JsonCode value={JSON.stringify(value, null, 2)} />;
}
