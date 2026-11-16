// FastClaw · _support
//
// Drop-in helpers so migrated tool files only need a use-path change.

pub mod constants;
pub mod helpers;
pub mod process_util;
pub mod tasks;

// Re-export for convenience (so tools can `use crate::core::tools::_support::...`)
pub use constants::*;
