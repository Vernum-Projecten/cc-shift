//! The `cs` binary: a thin entry point over [`cs::run`].

use std::process::ExitCode;

fn main() -> ExitCode {
    cs::run(std::env::args_os().skip(1))
}
