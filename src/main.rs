// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The `cc-shift` binary: a thin entry point over [`cc_shift::run`].

use std::process::ExitCode;

fn main() -> ExitCode {
    cc_shift::run(std::env::args_os().skip(1))
}
