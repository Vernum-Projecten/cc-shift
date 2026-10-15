//! `cc-shift hook rate-limit`: run by Claude Code's `StopFailure` hook when a
//! turn ends on a usage limit.

use std::io::Read;

use crate::command::automatic;
use crate::error::Error;
use crate::switching::auto::Trigger;

/// Reads the hook input and switches away from the full account, unless a
/// switch happened in the last few minutes.
///
/// It never fails: a `cc-shift` problem must not block a Claude Code turn.
pub fn run() {
    let mut input = Vec::new();
    let _ignored: std::io::Result<usize> = std::io::stdin().read_to_end(&mut input);
    let _switched: Result<Option<String>, Error> = automatic(Trigger::LimitHit);
}
