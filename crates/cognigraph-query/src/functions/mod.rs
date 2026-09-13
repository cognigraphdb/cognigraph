//! CGQL built-in function registry.
//!
//! Function *names* and *arity* are checked at validation time; argument
//! *types* are lenient at execution time — a wrong type yields `null`
//! (document-store semantics, consistent with null-falsy filters).

mod date;
mod numeric;
mod regex;
mod string;
mod value;

use chrono::{Datelike, Timelike};
use serde_json::Value;

use date::{date_part, fn_date_add, fn_date_diff, fn_now};
use numeric::{
    fn_abs, fn_avg, fn_ceil, fn_cosine_similarity, fn_floor, fn_max, fn_min, fn_round, fn_sum,
};
use regex::{fn_regex_replace, fn_regex_test};
use string::{
    fn_concat, fn_contains, fn_lower, fn_normalize_nfc, fn_split, fn_starts_with, fn_substring,
    fn_trim, fn_upper,
};
use value::{
    fn_coalesce, fn_first, fn_flatten, fn_has, fn_intersection, fn_is_same_collection, fn_last,
    fn_length, fn_minus, fn_parse_identifier, fn_slice, fn_sorted, fn_sorted_unique, fn_to_bool,
    fn_to_number, fn_to_string, fn_typename, fn_unique,
};

pub struct FunctionDef {
    pub name: &'static str,
    pub min_args: usize,
    /// `None` = variadic.
    pub max_args: Option<usize>,
    eval: fn(&[Value]) -> Value,
}

impl FunctionDef {
    pub fn call(&self, args: &[Value]) -> Value {
        if args.len() < self.min_args || self.max_args.is_some_and(|max| args.len() > max) {
            return Value::Null;
        }
        (self.eval)(args)
    }

    pub fn arity_description(&self) -> String {
        match (self.min_args, self.max_args) {
            (min, Some(max)) if min == max => format!("{min}"),
            (min, Some(max)) => format!("{min}..{max}"),
            (min, None) => format!("{min}+"),
        }
    }
}

/// Look up a function by case-insensitive name.
pub fn lookup(name: &str) -> Option<&'static FunctionDef> {
    FUNCTIONS
        .iter()
        .find(|def| def.name.eq_ignore_ascii_case(name))
}

pub const FUNCTIONS: &[FunctionDef] = &[
    FunctionDef {
        name: "NORMALIZE_NFC",
        min_args: 1,
        max_args: Some(1),
        eval: fn_normalize_nfc,
    },
    FunctionDef {
        name: "COALESCE",
        min_args: 1,
        max_args: None,
        eval: fn_coalesce,
    },
    // AQL spells the same operation NOT_NULL; both are accepted so the reflex
    // works whichever the author learned.
    FunctionDef {
        name: "NOT_NULL",
        min_args: 1,
        max_args: None,
        eval: fn_coalesce,
    },
    FunctionDef {
        name: "IS_SAME_COLLECTION",
        min_args: 2,
        max_args: Some(2),
        eval: fn_is_same_collection,
    },
    FunctionDef {
        name: "PARSE_IDENTIFIER",
        min_args: 1,
        max_args: Some(1),
        eval: fn_parse_identifier,
    },
    FunctionDef {
        name: "LENGTH",
        min_args: 1,
        max_args: Some(1),
        eval: fn_length,
    },
    FunctionDef {
        name: "UPPER",
        min_args: 1,
        max_args: Some(1),
        eval: fn_upper,
    },
    FunctionDef {
        name: "LOWER",
        min_args: 1,
        max_args: Some(1),
        eval: fn_lower,
    },
    FunctionDef {
        name: "TRIM",
        min_args: 1,
        max_args: Some(1),
        eval: fn_trim,
    },
    FunctionDef {
        name: "CONTAINS",
        min_args: 2,
        max_args: Some(2),
        eval: fn_contains,
    },
    FunctionDef {
        name: "STARTS_WITH",
        min_args: 2,
        max_args: Some(2),
        eval: fn_starts_with,
    },
    FunctionDef {
        name: "SUBSTRING",
        min_args: 2,
        max_args: Some(3),
        eval: fn_substring,
    },
    FunctionDef {
        name: "CONCAT",
        min_args: 1,
        max_args: None,
        eval: fn_concat,
    },
    FunctionDef {
        name: "SPLIT",
        min_args: 2,
        max_args: Some(2),
        eval: fn_split,
    },
    FunctionDef {
        name: "ABS",
        min_args: 1,
        max_args: Some(1),
        eval: fn_abs,
    },
    FunctionDef {
        name: "FLOOR",
        min_args: 1,
        max_args: Some(1),
        eval: fn_floor,
    },
    FunctionDef {
        name: "CEIL",
        min_args: 1,
        max_args: Some(1),
        eval: fn_ceil,
    },
    FunctionDef {
        name: "ROUND",
        min_args: 1,
        max_args: Some(1),
        eval: fn_round,
    },
    FunctionDef {
        name: "MIN",
        min_args: 1,
        max_args: Some(1),
        eval: fn_min,
    },
    FunctionDef {
        name: "MAX",
        min_args: 1,
        max_args: Some(1),
        eval: fn_max,
    },
    FunctionDef {
        name: "SUM",
        min_args: 1,
        max_args: Some(1),
        eval: fn_sum,
    },
    FunctionDef {
        name: "AVG",
        min_args: 1,
        max_args: Some(1),
        eval: fn_avg,
    },
    FunctionDef {
        name: "FIRST",
        min_args: 1,
        max_args: Some(1),
        eval: fn_first,
    },
    FunctionDef {
        name: "LAST",
        min_args: 1,
        max_args: Some(1),
        eval: fn_last,
    },
    // Registered so name and arity validate like any other function; the
    // evaluator intercepts it, because it reads documents rather than values
    // and `fn(&[Value]) -> Value` cannot express that.
    FunctionDef {
        name: "DOCUMENT",
        min_args: 1,
        max_args: Some(1),
        eval: |_| serde_json::Value::Null,
    },
    FunctionDef {
        name: "SLICE",
        min_args: 2,
        max_args: Some(3),
        eval: fn_slice,
    },
    FunctionDef {
        name: "FLATTEN",
        min_args: 1,
        max_args: Some(2),
        eval: fn_flatten,
    },
    FunctionDef {
        name: "INTERSECTION",
        min_args: 2,
        max_args: None,
        eval: fn_intersection,
    },
    FunctionDef {
        name: "MINUS",
        min_args: 2,
        max_args: None,
        eval: fn_minus,
    },
    FunctionDef {
        name: "TO_NUMBER",
        min_args: 1,
        max_args: Some(1),
        eval: fn_to_number,
    },
    FunctionDef {
        name: "TO_STRING",
        min_args: 1,
        max_args: Some(1),
        eval: fn_to_string,
    },
    FunctionDef {
        name: "TO_BOOL",
        min_args: 1,
        max_args: Some(1),
        eval: fn_to_bool,
    },
    FunctionDef {
        name: "REGEX_TEST",
        min_args: 2,
        max_args: Some(3),
        eval: fn_regex_test,
    },
    FunctionDef {
        name: "REGEX_REPLACE",
        min_args: 3,
        max_args: Some(4),
        eval: fn_regex_replace,
    },
    FunctionDef {
        name: "DATE_ADD",
        min_args: 3,
        max_args: Some(3),
        eval: fn_date_add,
    },
    FunctionDef {
        name: "UNIQUE",
        min_args: 1,
        max_args: Some(1),
        eval: fn_unique,
    },
    FunctionDef {
        name: "SORTED",
        min_args: 1,
        max_args: Some(1),
        eval: fn_sorted,
    },
    FunctionDef {
        name: "SORTED_UNIQUE",
        min_args: 1,
        max_args: Some(1),
        eval: fn_sorted_unique,
    },
    FunctionDef {
        name: "HAS",
        min_args: 2,
        max_args: Some(2),
        eval: fn_has,
    },
    FunctionDef {
        name: "TYPENAME",
        min_args: 1,
        max_args: Some(1),
        eval: fn_typename,
    },
    FunctionDef {
        name: "NOW",
        min_args: 0,
        max_args: Some(0),
        eval: fn_now,
    },
    FunctionDef {
        name: "DATE_YEAR",
        min_args: 1,
        max_args: Some(1),
        eval: |args| date_part(args, |d| d.year() as i64),
    },
    FunctionDef {
        name: "DATE_MONTH",
        min_args: 1,
        max_args: Some(1),
        eval: |args| date_part(args, |d| d.month() as i64),
    },
    FunctionDef {
        name: "DATE_DAY",
        min_args: 1,
        max_args: Some(1),
        eval: |args| date_part(args, |d| d.day() as i64),
    },
    FunctionDef {
        name: "DATE_HOUR",
        min_args: 1,
        max_args: Some(1),
        eval: |args| date_part(args, |d| d.hour() as i64),
    },
    FunctionDef {
        name: "DATE_MINUTE",
        min_args: 1,
        max_args: Some(1),
        eval: |args| date_part(args, |d| d.minute() as i64),
    },
    FunctionDef {
        name: "DATE_SECOND",
        min_args: 1,
        max_args: Some(1),
        eval: |args| date_part(args, |d| d.second() as i64),
    },
    FunctionDef {
        name: "DATE_TIMESTAMP",
        min_args: 1,
        max_args: Some(1),
        eval: |args| date_part(args, |d| d.timestamp_millis()),
    },
    FunctionDef {
        name: "COSINE_SIMILARITY",
        min_args: 2,
        max_args: Some(2),
        eval: fn_cosine_similarity,
    },
    FunctionDef {
        name: "DATE_DIFF",
        min_args: 3,
        max_args: Some(3),
        eval: fn_date_diff,
    },
];
