// Domain model + pure helpers for the Review workspace. Shapes mirror the
// server contract in crates/cognigraph-server/src/routes/neurons.rs: neurons
// are stored documents (Neuron struct + _key/space_type/provenance stamps),
// lifecycle transitions are explicit endpoints, and graduation is flags-only.

export type NeuronStatus = "proposed" | "accepted" | "rejected" | "retired";

export type NeuronKind = "alias" | "relation_hint" | "relation_rank_hint" | "relation_blocker";

export type GraduationReason = "covered_by_base" | "covered_by_others" | "inert_blocker";

/// A neuron document as returned by GET /api/neurons. Unused per-kind fields
/// arrive as empty strings/arrays (the server serializes defaults), so every
/// field is present but only the kind-relevant ones are meaningful.
export interface NeuronDoc {
  _key: string;
  id: string;
  type: NeuronKind;
  status: NeuronStatus;
  space_type: string;
  confidence: number;
  rationale: string;
  evidence: string[];
  // Alias neurons:
  entity: string;
  aliases: string[];
  // Relation hint/blocker neurons:
  source: string;
  relation: string;
  target: string;
  triggers: string[];
  // Rank-hint neurons:
  boost: number;
  // Provenance stamps (QW1 — the audit trail is the product claim):
  proposed_by?: string;
  proposed_at?: number;
  reviewed_by?: string;
  reviewed_at?: number;
  review_note?: string;
}

export interface NeuronListResponse {
  neurons: NeuronDoc[];
  count: number;
}

export interface GraduationCandidate {
  neuron_id: string;
  fact: string;
  reason: GraduationReason;
  review: { reviewed_by: string | null; reviewed_at: number | null } | null;
}

export interface GraduationResponse {
  space_type: string;
  candidates: GraduationCandidate[];
  count: number;
}

export interface TransitionResponse {
  _key: string;
  status: NeuronStatus;
  space_type: string;
  reviewed_by: string;
}

export const NEURON_STATUSES: NeuronStatus[] = ["proposed", "accepted", "rejected", "retired"];

export const STATUS_META: Record<NeuronStatus, { label: string; color: string }> = {
  proposed: { label: "Proposed", color: "gold" },
  accepted: { label: "Accepted", color: "green" },
  rejected: { label: "Rejected", color: "red" },
  retired: { label: "Retired", color: "default" },
};

export const KIND_META: Record<NeuronKind, { label: string; hint: string }> = {
  alias: { label: "Alias", hint: "Maps alternate names onto a known entity." },
  relation_hint: {
    label: "Relation hint",
    hint: "Licenses grounding a fact when a trigger phrase appears.",
  },
  relation_rank_hint: {
    label: "Rank hint",
    hint: "Reweights a relation in retrieval ranking. Cannot create facts.",
  },
  relation_blocker: {
    label: "Blocker",
    hint: "Vetoes grounding a fact from chunks containing a veto phrase.",
  },
};

export const GRADUATION_REASON_META: Record<GraduationReason, string> = {
  covered_by_base: "Covered by base rules — the space grounds this fact without the neuron.",
  covered_by_others: "Covered by other accepted neurons — removing it changes nothing.",
  inert_blocker: "Inert blocker — its veto phrase never fires on the stored chunks.",
};

/// One-line rendering of what a neuron asserts, by kind.
export function factOf(neuron: NeuronDoc): string {
  switch (neuron.type) {
    case "alias":
      return `${neuron.entity || "?"} ≡ ${neuron.aliases.join(", ") || "?"}`;
    case "relation_rank_hint": {
      const boost = neuron.boost > 0 ? `+${neuron.boost}` : String(neuron.boost);
      return `${neuron.relation || "?"} rank ${boost}`;
    }
    default:
      return `${neuron.source || "?"} —${neuron.relation || "?"}→ ${neuron.target || "?"}`;
  }
}

/// The phrases a hint grounds on / a blocker vetoes on. Rank hints and
/// aliases have none.
export function triggersOf(neuron: NeuronDoc): string[] {
  return neuron.type === "relation_hint" || neuron.type === "relation_blocker"
    ? neuron.triggers
    : [];
}

/// Server timestamps are unix seconds; 0/undefined means "never".
export function formatUnixSeconds(seconds: number | null | undefined): string {
  if (!seconds) return "—";
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(seconds * 1000),
  );
}

/// The lifecycle transitions available FROM a status. Mirrors the server's
/// route surface: accept/reject review a proposal; retire withdraws an
/// accepted neuron. Rejected/retired are terminal in this UI (re-propose
/// instead of resurrecting — provenance stays honest).
export function verdictsFor(status: NeuronStatus): Array<"accept" | "reject" | "retire"> {
  switch (status) {
    case "proposed":
      return ["accept", "reject"];
    case "accepted":
      return ["retire"];
    default:
      return [];
  }
}
