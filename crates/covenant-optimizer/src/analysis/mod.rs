//! Analysis utilities for the optimizer
//!
//! This module provides data flow and control flow analysis that optimization
//! passes use to make decisions.

pub mod reachability;
pub mod usage;

pub use reachability::compute_reachable;
pub use usage::{analyze_usage, UsageAnalysis};
