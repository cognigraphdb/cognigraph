//! CogniGraph Query Language parser, planner, and executor.
//!
//! This crate owns the storage-agnostic CGQL pipeline: parsing, validation,
//! planning, in-memory execution, and execution through `GraphBackend`.

pub mod ast;
pub mod executor;
pub mod functions;
mod parser;
pub mod planner;
pub mod validation;

pub use ast::*;
pub use executor::{
    ExecutionBudget, QueryMode, parse_and_execute_backend_with_mode,
    parse_and_execute_backend_with_options,
};
pub use executor::{ExecutionError, InMemoryDataset, execute_plan, parse_and_execute};
pub use executor::{execute_backend_plan, parse_and_execute_backend};
pub use parser::{ParseError, parse_query};
pub use planner::{LogicalPlan, PlanError, PlanSource, parse_and_plan, plan_query};
pub use validation::{
    ValidationError, ValidationOptions, validate_query, validate_query_with_options,
};
