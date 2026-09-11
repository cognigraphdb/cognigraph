//! Construction config.

use super::*;

impl ConstructionCandidateArtifact {
    /// Validate the complete deterministic M22 configuration shape without a
    /// corpus or promotion context and expose the exact runtime projection
    /// used by governed M25 ingestion. Corpus-dependent work bounds and the
    /// resolved configuration digest remain enforced by `resolve`.
    pub(crate) fn validate_semantic_repair_candidate(
        &self,
        target: &PromotionTarget,
    ) -> Result<(SpaceType, Vec<Neuron>), CogniGraphError> {
        if canonical_json_bytes(self)?.len() as u64 > MAX_M22_CANDIDATE_ARTIFACT_BYTES {
            return Err(CogniGraphError::CapacityExceeded(format!(
                "canonical construction candidate exceeds {MAX_M22_CANDIDATE_ARTIFACT_BYTES} bytes"
            )));
        }
        if self.kind != "semantic-neuron-bundle" {
            return Err(validation(
                "semantic repair candidate kind must be `semantic-neuron-bundle`",
            ));
        }
        self.validate_and_project(target, MAX_M22_CONFIG_ITEMS)
    }

    /// Resolve the exact M22/M23 construction configuration for a bounded
    /// materialized graph generation. Unlike the M25 runtime-only validator,
    /// this also proves the corpus-dependent grounding limits and the frozen
    /// construction-config digest carried by the selected promotion context.
    pub(crate) fn resolve_materialization(
        &self,
        context: &PromotionContext,
        chunks: &[Chunk],
    ) -> Result<(SpaceType, Vec<VetoRule>, String), CogniGraphError> {
        let plan = context
            .consumption_plan
            .as_deref()
            .and_then(|plan| plan.derivation.as_deref())
            .ok_or_else(|| {
                validation(
                    "verified materialization requires an M22/M23 derivation-capable context",
                )
            })?;
        self.resolve(context, plan, chunks)
    }

    pub(super) fn validate_and_project(
        &self,
        target: &PromotionTarget,
        max_config_items: usize,
    ) -> Result<(SpaceType, Vec<Neuron>), CogniGraphError> {
        if self.schema_version != 1
            || self.base_space_type.id != target.space_type
            || self.accepted_neurons.len() > max_config_items
        {
            return Err(validation(
                "construction candidate does not match its target or M22 limits",
            ));
        }
        for (label, value) in [
            ("candidate.kind", &self.kind),
            ("candidate.id", &self.id),
            ("candidate.revision", &self.revision),
            ("candidate.space.id", &self.base_space_type.id),
        ] {
            validate_text(label, value)?;
        }
        validate_optional_text(
            "candidate.space.name",
            &self.base_space_type.name,
            MAX_TEXT_BYTES,
        )?;
        validate_optional_text(
            "candidate.space.description",
            &self.base_space_type.description,
            MAX_TEXT_BYTES,
        )?;
        let mut item_count = self.base_space_type.entities.len()
            + self.base_space_type.relation_rules.len()
            + self.accepted_neurons.len();
        let mut entity_names = HashSet::new();
        let mut previous_entity: Option<&str> = None;
        for entity in &self.base_space_type.entities {
            validate_text("candidate.entity.name", &entity.name)?;
            validate_text("candidate.entity.type", &entity.entity_type)?;
            validate_sorted_unique_text("candidate.entity.aliases", &entity.aliases, false)?;
            if previous_entity.is_some_and(|previous| previous >= entity.name.as_str())
                || !entity_names.insert(entity.name.as_str())
            {
                return Err(validation(
                    "construction entities must be sorted by unique canonical name",
                ));
            }
            item_count = item_count
                .checked_add(entity.aliases.len())
                .ok_or_else(|| validation("construction configuration size overflowed"))?;
            previous_entity = Some(&entity.name);
        }
        let mut previous_rule: Option<(&str, &str, &str)> = None;
        for rule in &self.base_space_type.relation_rules {
            for (label, value) in [
                ("candidate.rule.source", &rule.source),
                ("candidate.rule.relation", &rule.relation),
                ("candidate.rule.target", &rule.target),
            ] {
                validate_text(label, value)?;
            }
            if !entity_names.contains(rule.source.as_str())
                || !entity_names.contains(rule.target.as_str())
            {
                return Err(validation(
                    "construction relation-rule endpoints must name configured entities",
                ));
            }
            validate_sorted_unique_triggers("candidate.rule.when_any", &rule.when_any)?;
            validate_sorted_unique_endpoints(&rule.require_in_sentence)?;
            let key = (
                rule.source.as_str(),
                rule.relation.as_str(),
                rule.target.as_str(),
            );
            if previous_rule.is_some_and(|previous| previous >= key) {
                return Err(validation(
                    "construction relation rules must be sorted by unique triple",
                ));
            }
            item_count = item_count
                .checked_add(rule.when_any.len())
                .and_then(|count| count.checked_add(rule.require_in_sentence.len()))
                .ok_or_else(|| validation("construction configuration size overflowed"))?;
            previous_rule = Some(key);
        }
        let mut previous_neuron: Option<&str> = None;
        let neurons = self
            .accepted_neurons
            .iter()
            .map(|neuron| {
                validate_text("candidate.neuron.id", neuron.id())?;
                if previous_neuron.is_some_and(|previous| previous >= neuron.id()) {
                    return Err(validation(
                        "accepted construction neurons must be sorted by unique id",
                    ));
                }
                validate_construction_neuron(neuron)?;
                previous_neuron = Some(neuron.id());
                Ok(neuron.to_neuron())
            })
            .collect::<Result<Vec<_>, CogniGraphError>>()?;
        item_count = item_count
            .checked_add(
                self.accepted_neurons
                    .iter()
                    .map(construction_neuron_item_count)
                    .sum::<usize>(),
            )
            .ok_or_else(|| validation("construction configuration size overflowed"))?;
        if item_count > max_config_items {
            return Err(CogniGraphError::CapacityExceeded(format!(
                "M22 construction configuration names {item_count} items, exceeding {max_config_items}"
            )));
        }
        let space = SpaceType {
            id: self.base_space_type.id.clone(),
            name: self.base_space_type.name.clone(),
            version: self.base_space_type.version,
            description: self.base_space_type.description.clone(),
            entities: self
                .base_space_type
                .entities
                .iter()
                .map(|entity| EntityDef {
                    name: entity.name.clone(),
                    entity_type: entity.entity_type.clone(),
                    aliases: entity.aliases.clone(),
                })
                .collect(),
            relation_rules: self
                .base_space_type
                .relation_rules
                .iter()
                .map(|rule| RelationRule {
                    source: rule.source.clone(),
                    relation: rule.relation.clone(),
                    target: rule.target.clone(),
                    when_any: rule.when_any.clone(),
                    require_in_sentence: rule.require_in_sentence.clone(),
                    trigger_provenance: BTreeMap::<String, TriggerProvenance>::new(),
                })
                .collect(),
        };
        validate_neurons(
            &NeuronSet {
                space_type: space.id.clone(),
                neurons: neurons.clone(),
            },
            &space,
        )
        .map_err(|error| validation(format!("invalid M22 construction candidate: {error}")))?;
        Ok((space, neurons))
    }

    pub(super) fn resolve(
        &self,
        context: &PromotionContext,
        plan: &CorpusGraphDerivationPlan,
        chunks: &[Chunk],
    ) -> Result<(SpaceType, Vec<VetoRule>, String), CogniGraphError> {
        if self.kind != context.candidate.kind
            || self.id != context.candidate.id
            || self.revision != context.candidate.revision
        {
            return Err(validation(
                "construction candidate does not match its M22 context or limits",
            ));
        }
        let (space, neurons) =
            self.validate_and_project(&context.target, plan.max_config_items as usize)?;
        let effective = effective_config(&space, &neurons);
        let vetoes = effective_vetoes(&neurons);
        validate_grounding_work(
            &effective,
            &vetoes,
            chunks,
            plan.max_grounding_work,
            plan.max_chunk_grounding_work,
        )?;
        let resolved = ResolvedConstructionConfig {
            schema_version: 1,
            space_type: &effective,
            vetoes: vetoes
                .iter()
                .map(|veto| ResolvedVeto {
                    source: &veto.source,
                    relation: &veto.relation,
                    target: &veto.target,
                    when_any: &veto.when_any,
                })
                .collect(),
        };
        let resolved_digest = canonical_digest(&resolved)?;
        if resolved_digest != context.effective_configuration.construction_config_digest {
            return Err(validation(
                "resolved M22 construction configuration digest does not match its context",
            ));
        }
        Ok((effective, vetoes, resolved_digest))
    }
}
