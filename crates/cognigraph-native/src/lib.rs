//! Native CogniGraph backend.

mod derivative;
mod memory;
mod sidecar;
mod storage;
mod text_index;

pub use memory::{NativeBackend, StorageMode, VectorMode};
pub use text_index::validate_text_search_fields;
