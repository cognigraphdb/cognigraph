use std::sync::Arc;

use cognigraph_core::{CollectionType, GraphBackend};
use cognigraph_governance::SigningKeyMaterial;
use cognigraph_native::NativeBackend;

use super::*;
use crate::state::AppState;

const TENANT: &str = "default";
const INCARNATION: &str = "default";

mod fixtures;
use fixtures::*;

mod domains;

mod key_resolution;

mod authority_recovery;
