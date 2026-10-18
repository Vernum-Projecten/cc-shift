// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift completion zsh|powershell`: prints a shell completion script.
//!
//! zsh is the default shell on macOS, and PowerShell the shell of Windows.
//! Each script completes the commands, the profile names after `use` and
//! `remove` (read from `cc-shift list` each time, so new profiles complete at
//! once) and the flags
//! (<https://zsh.sourceforge.io/Doc/Release/Completion-System.html>,
//! <https://learn.microsoft.com/powershell/module/microsoft.powershell.core/register-argumentcompleter>).

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
    'stats:show token totals per project and model'
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
    stats) compadd -- --since --all --project --cost --per-account --json ;;
    uninstall) compadd -- --all ;;
    log) compadd -- --since --all --verbose --json ;;
    completion) compadd -- zsh powershell ;;
  esac
}

compdef _cc_shift cc-shift
"#;

/// The PowerShell completion script, for Windows PowerShell 5.1 and
/// PowerShell 7.
///
/// The completer finds the position of the word under the cursor by counting
/// the command elements that end before the cursor.
pub const POWERSHELL: &str = r"# PowerShell completion for cc-shift. Load it with:
#   cc-shift completion powershell | Out-String | Invoke-Expression
# in your PowerShell profile ($PROFILE).

Register-ArgumentCompleter -Native -CommandName 'cc-shift', 'cc-shift.exe' -ScriptBlock {
  param($wordToComplete, $commandAst, $cursorPosition)
  $commands = [ordered]@{
    'add' = 'create a profile and run /login in it'
    'setup' = 'install cc-shift in Claude Code'
    'status' = 'show every account with its usage'
    'stats' = 'show token totals per project and model'
    'use' = 'make an account active now'
    'refresh' = 'refresh parked logins and read their usage'
    'list' = 'print the profiles'
    'doctor' = 'check the setup'
    'log' = 'show the history'
    'remove' = 'delete a profile and its login'
    'uninstall' = 'undo setup'
    'completion' = 'print the shell completion'
    'version' = 'print the version'
    'help' = 'print the commands'
  }
  $before = @($commandAst.CommandElements | Where-Object { $_.Extent.EndOffset -lt $cursorPosition })
  $position = $before.Count
  if ($position -lt 1) { return }
  $values = @()
  if ($position -eq 1) {
    $values = @($commands.Keys)
  } else {
    switch ($before[1].ToString()) {
      'use' { $values = @(cc-shift list 2>$null) }
      'remove' { $values = @(cc-shift list 2>$null) }
      'status' { $values = @('--json') }
      'stats' { $values = @('--since', '--all', '--project', '--cost', '--per-account', '--json') }
      'uninstall' { $values = @('--all') }
      'log' { $values = @('--since', '--all', '--verbose', '--json') }
      'completion' { $values = @('zsh', 'powershell') }
    }
  }
  $values | Where-Object {
    $_.StartsWith($wordToComplete, [System.StringComparison]::OrdinalIgnoreCase)
  } | ForEach-Object {
    $tip = $_
    if ($position -eq 1) { $tip = $commands[$_] }
    [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $tip)
  }
}
";

/// Prints the completion script for `shell`.
///
/// # Errors
///
/// Returns [`Error::Completion`] for a shell other than zsh and PowerShell.
pub fn run(shell: &str) -> Result<(), Error> {
    let script = match shell {
        "zsh" => ZSH,
        "powershell" => POWERSHELL,
        _ => return Err(Error::Completion(shell.to_owned())),
    };
    let _ignored: std::io::Result<()> = std::io::stdout().write_all(script.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{POWERSHELL, ZSH, run};
    use crate::error::Error;

    const COMMANDS: [&str; 14] = [
        "add",
        "setup",
        "status",
        "stats",
        "use",
        "refresh",
        "list",
        "doctor",
        "log",
        "remove",
        "uninstall",
        "completion",
        "version",
        "help",
    ];

    #[test]
    fn every_command_completes_in_powershell() {
        for command in COMMANDS {
            assert!(
                POWERSHELL.contains(&format!("'{command}' = '")),
                "{command}"
            );
        }
    }

    #[test]
    fn powershell_registers_a_native_completer() {
        assert!(
            POWERSHELL.contains(
                "Register-ArgumentCompleter -Native -CommandName 'cc-shift', 'cc-shift.exe'"
            ),
            "{POWERSHELL}"
        );
    }

    #[test]
    fn powershell_profile_names_come_from_cc_shift_list() {
        for command in ["use", "remove"] {
            assert!(
                POWERSHELL.contains(&format!(
                    "'{command}' {{ $values = @(cc-shift list 2>$null) }}"
                )),
                "{command}"
            );
        }
    }

    #[test]
    fn powershell_completes_the_flags_zsh_completes() {
        for flag in [
            "--json",
            "--since",
            "--all",
            "--project",
            "--cost",
            "--per-account",
            "--verbose",
        ] {
            assert!(ZSH.contains(flag), "{flag} in zsh");
            assert!(POWERSHELL.contains(&format!("'{flag}'")), "{flag}");
        }
    }

    #[test]
    fn neither_script_offers_the_early_start() {
        for word in ["early", "--at", "--accounts"] {
            assert!(!ZSH.contains(word), "{word} in zsh");
            assert!(!POWERSHELL.contains(word), "{word} in PowerShell");
        }
    }

    #[test]
    fn both_scripts_complete_both_shells() {
        assert!(
            ZSH.contains("completion) compadd -- zsh powershell ;;"),
            "{ZSH}"
        );
        assert!(
            POWERSHELL.contains("'completion' { $values = @('zsh', 'powershell') }"),
            "{POWERSHELL}"
        );
    }

    #[test]
    fn another_shell_is_an_error() {
        assert!(
            matches!(run("fish"), Err(Error::Completion(shell)) if shell == "fish"),
            "fish"
        );
    }

    #[test]
    fn every_command_completes() {
        for command in [
            "add",
            "setup",
            "status",
            "stats",
            "use",
            "refresh",
            "list",
            "doctor",
            "log",
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
