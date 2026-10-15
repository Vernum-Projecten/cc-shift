//! The single integration-test binary for `cs`.

#![allow(clippy::panic_in_result_fn, reason = "test assertions")]

mod add;
mod cli;
mod hook;
mod list;
mod sandbox;
mod setup;
mod status;
mod statusline;
