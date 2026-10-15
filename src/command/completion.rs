//! `cc-shift completion zsh`: prints the zsh completion script.
//!
//! zsh is the default shell on macOS. The script completes the commands, the
//! profile names after `use` and `remove` (read from `cc-shift list` each
//! time, so new profiles complete at once) and the flags
//! (<https://zsh.sourceforge.io/Doc/Release/Completion-System.html>).

use std::io::Write;

use crate::error::Error;

/// The zsh completion script.
pub const ZSH: &str = r#"#compdef cc-shift
# zsh completion for cc-shift. Load it with:
#   source <(cc-shift completion zsh)
# in ~/.zshrc, after compinit.

_cc_shift() {
  local -a commands
  commands=(
    'add:create a profile and run /login in it'
    'setup:install cc-shift in Claude Code'
    'status:show every account with its usage'
    'use:make an account active now'
    'refresh:refresh parked logins and read their usage'
    'list:print the profiles'
    'doctor:check the setup'
    'log:show the history'
    'remove:delete a profile and its login'
    'uninstall:undo setup'
    'completion:print the shell completion'
    'version:print the version'
    'help:print the commands'
  )
  if (( CURRENT == 2 )); then
    _describe 'command' commands
    return
  fi
  case ${words[2]} in
    use|remove) compadd -- ${(f)"$(cc-shift list 2>/dev/null)"} ;;
    status) compadd -- --json ;;
    uninstall) compadd -- --all ;;
    log) compadd -- --since --all --verbose --json ;;
    completion) compadd -- zsh ;;
  esac
}

compdef _cc_shift cc-shift
"#;

/// Prints the completion script for `shell`.
///
/// # Errors
///
/// Returns [`Error::Completion`] for a shell other than zsh.
pub fn run(shell: &str) -> Result<(), Error> {
    if shell != "zsh" {
        return Err(Error::Completion(shell.to_owned()));
    }
    let _ignored: std::io::Result<()> = std::io::stdout().write_all(ZSH.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ZSH;

    #[test]
    fn every_command_completes() {
        for command in [
            "add",
            "setup",
            "status",
            "use",
            "refresh",
            "list",
            "doctor",
            "remove",
            "uninstall",
            "completion",
            "help",
        ] {
            assert!(ZSH.contains(&format!("'{command}:")), "{command}");
        }
    }

    #[test]
    fn profile_names_come_from_cc_shift_list() {
        assert!(
            ZSH.contains("use|remove) compadd -- ${(f)\"$(cc-shift list"),
            "{ZSH}"
        );
    }
}
