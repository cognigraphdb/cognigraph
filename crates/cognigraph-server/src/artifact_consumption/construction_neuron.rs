//! Construction neuron.

use super::*;

impl ConstructionNeuron {
    pub(crate) fn id(&self) -> &str {
        match self {
            Self::Alias { id, .. }
            | Self::RelationHint { id, .. }
            | Self::RelationBlocker { id, .. } => id,
        }
    }

    pub(crate) fn to_neuron(&self) -> Neuron {
        let mut neuron = Neuron {
            id: self.id().into(),
            status: NeuronStatus::Accepted,
            confidence: 1.0,
            ..Neuron::default()
        };
        match self {
            Self::Alias {
                reviewed_by,
                evidence,
                entity,
                aliases,
                ..
            } => {
                neuron.reviewed_by = reviewed_by.clone();
                neuron.kind = NeuronKind::Alias;
                neuron.evidence.clone_from(evidence);
                neuron.entity.clone_from(entity);
                neuron.aliases.clone_from(aliases);
            }
            Self::RelationHint {
                reviewed_by,
                evidence,
                source,
                relation,
                target,
                triggers,
                ..
            } => {
                neuron.reviewed_by = reviewed_by.clone();
                neuron.kind = NeuronKind::RelationHint;
                neuron.evidence.clone_from(evidence);
                neuron.source.clone_from(source);
                neuron.relation.clone_from(relation);
                neuron.target.clone_from(target);
                neuron.triggers.clone_from(triggers);
            }
            Self::RelationBlocker {
                reviewed_by,
                evidence,
                source,
                relation,
                target,
                triggers,
                ..
            } => {
                neuron.reviewed_by = reviewed_by.clone();
                neuron.kind = NeuronKind::RelationBlocker;
                neuron.evidence.clone_from(evidence);
                neuron.source.clone_from(source);
                neuron.relation.clone_from(relation);
                neuron.target.clone_from(target);
                neuron.triggers.clone_from(triggers);
            }
        }
        neuron
    }
}
