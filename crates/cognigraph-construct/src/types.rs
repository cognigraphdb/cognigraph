//! Configuration types — JSON-compatible with the research repo's YAML
//! (converted fixtures live under fixtures/semantic-neurons/).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceType {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub entities: Vec<EntityDef>,
    #[serde(default)]
    pub relation_rules: Vec<RelationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDef {
    pub name: String,
    #[serde(default, rename = "type")]
    pub entity_type: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// An endpoint reference for sentence-scoped gating. An enum (not a free
/// string) so a typo'd value fails loudly at deserialization instead of
/// silently not gating — set-but-ignored restraint config is the worst
/// failure mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointRef {
    Source,
    Target,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationRule {
    pub source: String,
    pub relation: String,
    pub target: String,
    /// Evidence triggers: the rule grounds when any phrase is affirmed.
    /// A trigger may contain `{source}`/`{target}` placeholders — it is
    /// then expanded against the endpoints' surfaces (name + aliases) at
    /// grounding time, matching only direction-faithful phrasings
    /// (decision_grounding_gates.md, D2).
    #[serde(default)]
    pub when_any: Vec<String>,
    /// Sentence-scoped endpoint gate (opt-in, per rule): when non-empty,
    /// the affirmed trigger's SENTENCE must also contain each listed
    /// endpoint's surface (name or alias). Measured
    /// (decision_grounding_gates.md, D1): closes cross-company leakage at
    /// zero recall cost when authors gate the leakage-prone rules; global
    /// defaults were rejected by measurement. Relation-hint neurons extend
    /// a rule's triggers and inherit its gate.
    #[serde(default)]
    pub require_in_sentence: Vec<EndpointRef>,
    /// Which neuron licensed which trigger, keyed by the trigger phrase AS
    /// AUTHORED (templates are keyed by the unexpanded phrase).
    ///
    /// Attribution has to be per-TRIGGER, not per-rule: a relation-hint neuron
    /// may EXTEND an authored rule, so one rule's `when_any` can mix triggers
    /// the ontology author wrote with triggers a reviewed neuron added. The fact
    /// is licensed by the specific trigger that matched, so that is the level at
    /// which "who approved this?" has an answer. A trigger absent from this map
    /// is authored base ontology — no neuron, and the reviewer is whoever owns
    /// the space.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub trigger_provenance: BTreeMap<String, TriggerProvenance>,
}

/// Who licensed a trigger, and who approved it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerProvenance {
    /// The neuron whose acceptance put this trigger into the effective config.
    pub neuron_id: String,
    /// The actor who accepted that neuron. `None` when the neuron was accepted
    /// outside the server's review route (e.g. a test or an offline harness) —
    /// absence means "not recorded", never "nobody".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_by: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeuronStatus {
    #[default]
    Proposed,
    Accepted,
    Rejected,
    Retired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeuronKind {
    Alias,
    #[default]
    RelationHint,
    /// Reweights `relation_score` in retrieval-trace ranking. Cannot create
    /// facts; boosts are additive, so there is no precedence problem —
    /// the roadmap's lowest-risk neuron type.
    RelationRankHint,
    /// Extraction-time veto: when any veto phrase appears in a chunk
    /// (plain casefold presence, deliberately not negation-aware), the
    /// targeted triple does not ground FROM THAT CHUNK. Unconditional
    /// "veto wins" keeps the config boolean and order-independent —
    /// see docs/decisions/decision_relation_blocker.md.
    RelationBlocker,
}

/// A governed edge-construction operator: ontology-bounded, evidence-bound,
/// reviewable, reversible. Only `accepted` neurons influence construction.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Neuron {
    pub id: String,
    /// The actor who accepted or rejected this neuron. Written by the server's
    /// review route; `None` when the neuron has not been through it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_by: Option<String>,
    #[serde(rename = "type")]
    pub kind: NeuronKind,
    #[serde(default)]
    pub status: NeuronStatus,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    // Alias neurons:
    #[serde(default)]
    pub entity: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    // Relation-hint neurons:
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub relation: String,
    #[serde(default)]
    pub target: String,
    // For relation hints these are grounding triggers; for relation
    // blockers they are veto phrases (`when_any` accepted as an alias to
    // mirror RelationRule's field name).
    #[serde(default, alias = "when_any")]
    pub triggers: Vec<String>,
    // Relation-rank-hint neurons: additive score delta for `relation`
    // (negative demotes). Finite and non-zero, enforced by validation.
    #[serde(default)]
    pub boost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuronSet {
    pub space_type: String,
    #[serde(default)]
    pub neurons: Vec<Neuron>,
}

/// One `subject --RELATION--> object` fact.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fact {
    pub source: String,
    pub relation: String,
    pub target: String,
}

impl Fact {
    /// Parse the research repo's `"A --REL--> B"` notation.
    pub fn parse(raw: &str) -> Option<Self> {
        let (source, rest) = raw.split_once("--")?;
        let (relation, target) = rest.split_once("-->")?;
        Some(Self {
            source: source.trim().to_string(),
            relation: relation.trim().to_string(),
            target: target.trim().to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSpec {
    pub space_id: String,
    #[serde(default)]
    pub questions: Vec<EvalQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalQuestion {
    pub id: String,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub expected_facts: Vec<String>,
    #[serde(default)]
    pub forbidden_facts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    #[serde(default)]
    pub title: String,
    /// Caller-supplied text. Construction canonicalizes it to NFC before
    /// grounding; persisted hashes and UTF-8 spans refer to that representation.
    pub text: String,
}
