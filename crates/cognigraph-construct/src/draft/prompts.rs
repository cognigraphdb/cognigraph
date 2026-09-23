//! System prompts and response schemas for the two drafting passes.

use super::*;

pub(super) const ENTITY_SYSTEM: &str = "You draft the ENTITY catalogue of a knowledge-graph ontology from \
document excerpts. Propose the distinct real-world entities the documents are about — \
organizations, people, products, platforms, standards, markets — each with a short lowercase \
type and the alternative surface forms (aliases) the text itself uses. STRICT RULES: every \
name and every alias must occur VERBATIM somewhere in the excerpts (they will be checked and \
dropped otherwise); do not invent canonical names the text never uses; do not include generic \
concepts that could not be an endpoint of a specific factual relation. Respond in the required \
JSON schema.";

pub(super) const RULE_SYSTEM: &str = "You draft the RELATION RULES of a knowledge-graph ontology. You are \
given a fixed entity catalogue and document excerpts. Propose rules of the form `source \
--RELATION--> target` where source and target are entity NAMES from the catalogue (never \
anything else), RELATION is a short UPPER_SNAKE label of your choice, and `when_any` lists 1-3 \
short trigger phrases copied VERBATIM from sentences that explicitly assert that specific \
relation between those specific entities (they will be checked against the corpus and dropped \
otherwise — paraphrases do not survive). Only propose relations the text plainly asserts; a \
rule constructs a fact wherever a trigger appears affirmed, so a trigger that could appear in \
text about OTHER entities is a bad trigger. Respond in the required JSON schema.";

pub(super) fn entity_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "entities": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "type": { "type": "string" },
                        "aliases": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["name", "type", "aliases"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["entities"],
        "additionalProperties": false
    })
}

pub(super) fn rule_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "rules": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "source": { "type": "string" },
                        "relation": { "type": "string" },
                        "target": { "type": "string" },
                        "when_any": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["source", "relation", "target", "when_any"],
                    "additionalProperties": true
                }
            }
        },
        "required": ["rules"],
        "additionalProperties": false
    })
}
