pub mod admin;
pub mod auth;
pub mod batch;
pub mod cache;
pub mod collections;
#[cfg(feature = "enterprise")]
pub mod construct;
#[cfg(test)]
mod document_lookup_tests;
pub mod documents;
#[cfg(feature = "enterprise")]
pub mod governance;
pub mod graph;
pub mod health;
#[cfg(feature = "enterprise")]
pub mod jobs;
pub mod lua;
#[cfg(feature = "enterprise")]
pub mod materialized_repairs;
#[cfg(all(test, feature = "enterprise"))]
mod neuron_lifecycle_tests;
#[cfg(feature = "enterprise")]
pub mod neurons;
#[cfg(feature = "enterprise")]
pub mod promotions;
pub mod query;
pub mod search;
#[cfg(feature = "enterprise")]
pub mod semantic_repairs;
#[cfg(feature = "enterprise")]
pub mod sideviews;
#[cfg(feature = "enterprise")]
pub mod tenants;
#[cfg(not(feature = "enterprise"))]
#[path = "tenants_community.rs"]
pub mod tenants;
pub mod users;
