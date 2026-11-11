//! The HTTP meaning of a CGQL execution error (CG-198).
//!
//! Both query routes answer through this one function. The match has no
//! catch-all on purpose: a new `ExecutionError` variant does not compile
//! until someone decides whose fault it is. Before this, every error a route
//! did not name answered 500, so a budget refusal or a type error in the
//! caller's own query looked like a server failure.

use cognigraph_core::CogniGraphError;
use cognigraph_query::{ExecutionBudget, ExecutionError};

use crate::error::AppError;

/// Map an execution error to the response the caller should see. `budget` is
/// the budget the query ran under; it supplies the time limit, which the
/// executor's error does not carry.
pub(crate) fn cgql_error(error: ExecutionError, budget: &ExecutionBudget) -> AppError {
    use ExecutionError as E;
    let message = error.to_string();
    let exceeded = |name: &str, limit: u64, unit: &str, message: String| {
        CogniGraphError::QueryBudgetExceeded {
            budget: name.into(),
            limit,
            unit: unit.into(),
            message,
        }
    };
    AppError(match error {
        // The query text or its bind variables are wrong: 400.
        E::Plan(_)
        | E::BindVariableNotFound(_)
        | E::MissingBindVariables(_)
        | E::UnexpectedBindVariables(_)
        | E::IdentifierNotFound(_)
        | E::UnsupportedFunction(_)
        | E::InvalidInput(_)
        // A value the query computed has the wrong type: 400.
        | E::ExpectedNumber
        | E::ExpectedBool
        | E::ExpectedVector
        | E::ExpectedArraySource
        // The caller named the wrong document (CG-106): 400.
        | E::InvalidDocumentKey
        | E::MutationKeyOutsideCollection { .. } => CogniGraphError::ValidationError(message),

        // The request that asked was dropped, so nobody reads this answer.
        E::Cancelled => CogniGraphError::QueryError(message),

        // The query names a collection or a document that does not exist: 404.
        E::CollectionNotFound(collection) => CogniGraphError::CollectionNotFound(collection),
        E::DocumentNotFound { collection, key } => {
            CogniGraphError::DocumentNotFound { collection, key }
        }

        // A mutation sent to the read-only route, or data the caller may not
        // read: 403.
        E::MutationNotAllowed => CogniGraphError::Forbidden(message),
        E::Forbidden(detail) => CogniGraphError::Forbidden(detail),

        E::Conflict(detail) => CogniGraphError::DocumentConflict(detail),
        E::UniqueViolation {
            collection,
            index,
            existing,
        } => CogniGraphError::UniqueViolation {
            collection,
            index,
            existing,
        },

        // The query is valid and was stopped at a limit: 422.
        E::RowBudgetExceeded(limit) => exceeded("source_rows", limit, "rows", message),
        E::IntermediateRowBudgetExceeded(limit) => {
            exceeded("intermediate_rows", limit, "rows", message)
        }
        E::MemoryBudgetExceeded(limit) => exceeded("memory", limit, "bytes", message),
        E::TimeBudgetExceeded => {
            let limit = budget.time_budget_ms.unwrap_or_default();
            exceeded("time", limit, "ms", format!("{message} ({limit} ms)"))
        }

        E::Connection(detail) => CogniGraphError::ConnectionError(detail),

        // Not the caller's doing: 500.
        E::Backend(_) | E::MutationUnsupported => CogniGraphError::QueryError(message),
    })
}

#[cfg(test)]
#[path = "cgql_errors_tests.rs"]
mod tests;
