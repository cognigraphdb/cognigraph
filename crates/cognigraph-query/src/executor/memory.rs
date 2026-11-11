//! The per-query memory budget (CG-111).
//!
//! The meter counts the approximate memory of what a query builds: see
//! [`weight`]. It has two counters. `retained` holds what the query keeps: values bound by LET,
//! rows produced by a join, COLLECT and SORT keys, and results. `transient`
//! holds what the expression being evaluated has read so far, and starts
//! again with every top-level expression. A query fails as soon as the two
//! together pass the limit, which is before the next value is copied.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

use super::{Env, ExecutionError};

/// One JSON value: a scalar, or the fixed part of a string or container.
const VALUE: u64 = 32;
/// The bookkeeping and rounding of one heap allocation.
const ALLOCATION: u64 = 16;
/// One node of an object's field map. A node is allocated whole, however few
/// fields it holds; it has room for eleven and is counted per six.
const OBJECT_NODE: u64 = 640;
const FIELDS_PER_NODE: u64 = 6;
/// One variable of a row: its slot in the row's table and its share of the
/// table, besides the name and the value.
const BINDING: u64 = 160;

#[derive(Clone)]
pub(super) struct MemoryMeter {
    max: Option<u64>,
    retained: Arc<AtomicU64>,
    transient: Arc<AtomicU64>,
}

impl MemoryMeter {
    pub fn new(max: Option<u64>) -> Self {
        Self {
            max,
            retained: Arc::new(AtomicU64::new(0)),
            transient: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn enabled(&self) -> bool {
        self.max.is_some()
    }

    /// Start one top-level expression evaluation.
    pub fn begin(&self) {
        if self.max.is_some() {
            self.transient.store(0, Ordering::Relaxed);
        }
    }

    /// Count bytes the current expression is about to copy or has produced.
    pub fn read(&self, bytes: u64) -> Result<(), ExecutionError> {
        let Some(max) = self.max else {
            return Ok(());
        };
        let transient = self
            .transient
            .fetch_add(bytes, Ordering::Relaxed)
            .saturating_add(bytes);
        if self
            .retained
            .load(Ordering::Relaxed)
            .saturating_add(transient)
            > max
        {
            return Err(ExecutionError::MemoryBudgetExceeded(max));
        }
        Ok(())
    }

    /// Count the value an expression is about to copy.
    pub fn read_value(&self, value: &Value) -> Result<(), ExecutionError> {
        match value {
            // Scalars are bounded by the row and expression counts.
            Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
            _ if self.max.is_none() => Ok(()),
            _ => self.read(weight(value)),
        }
    }

    /// The error every over-budget path reports.
    pub fn exceeded(&self) -> ExecutionError {
        ExecutionError::MemoryBudgetExceeded(self.max.unwrap_or(u64::MAX))
    }

    /// Bytes the current expression may still produce.
    pub fn remaining(&self) -> u64 {
        match self.max {
            None => u64::MAX,
            Some(max) => max
                .saturating_sub(self.retained.load(Ordering::Relaxed))
                .saturating_sub(self.transient.load(Ordering::Relaxed)),
        }
    }

    /// Count bytes the query keeps.
    pub fn retain(&self, bytes: u64) -> Result<(), ExecutionError> {
        let Some(max) = self.max else {
            return Ok(());
        };
        let retained = self
            .retained
            .fetch_add(bytes, Ordering::Relaxed)
            .saturating_add(bytes);
        if retained > max {
            return Err(ExecutionError::MemoryBudgetExceeded(max));
        }
        Ok(())
    }

    /// Count a value the query keeps. Returns what was counted.
    pub fn retain_value(&self, value: &Value) -> Result<u64, ExecutionError> {
        if self.max.is_none() {
            return Ok(0);
        }
        let bytes = weight(value);
        self.retain(bytes)?;
        Ok(bytes)
    }

    /// Give back bytes that are no longer held.
    pub fn release(&self, bytes: u64) {
        if self.max.is_some() && bytes > 0 {
            self.retained
                .update(Ordering::Relaxed, Ordering::Relaxed, |held| {
                    held.saturating_sub(bytes)
                });
        }
    }

    /// The retained count, to hand back to [`Self::release_to`].
    pub fn mark(&self) -> u64 {
        self.retained.load(Ordering::Relaxed)
    }

    /// Drop everything retained since `mark`: the rows of a finished subquery.
    pub fn release_to(&self, mark: u64) {
        if self.max.is_some() {
            self.retained.store(mark, Ordering::Relaxed);
        }
    }
}

/// Approximate memory of a value, modelled on how the engine holds JSON:
/// 32 bytes per value; a string its length plus 16; an array 16 plus its
/// elements; an object 640 bytes per six fields (rounded up) plus, per field,
/// the key's length, 16, and whatever the value holds beyond its 32 bytes.
/// It is a deterministic estimate, not an allocator measurement.
pub(super) fn weight(value: &Value) -> u64 {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => VALUE,
        Value::String(text) => VALUE + ALLOCATION + text.len() as u64,
        Value::Array(items) if items.is_empty() => VALUE,
        Value::Array(items) => items.iter().fold(VALUE + ALLOCATION, |total, item| {
            total.saturating_add(weight(item))
        }),
        Value::Object(fields) if fields.is_empty() => VALUE,
        Value::Object(fields) => {
            let nodes = (fields.len() as u64).div_ceil(FIELDS_PER_NODE);
            fields
                .iter()
                .fold(VALUE + nodes * OBJECT_NODE, |total, (name, item)| {
                    total
                        .saturating_add(ALLOCATION + name.len() as u64)
                        // The value's own 32 bytes are part of the node.
                        .saturating_add(weight(item) - VALUE)
                })
        }
    }
}

/// Approximate memory of one row: per variable 160 bytes, its name's length
/// plus 16, and whatever the value holds beyond its 32 bytes. A row with no
/// variables weighs nothing.
pub(super) fn env_weight(env: &Env) -> u64 {
    env.iter().fold(0, |total, (name, value)| {
        total
            .saturating_add(BINDING + ALLOCATION + name.len() as u64)
            .saturating_add(weight(value) - VALUE)
    })
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;
