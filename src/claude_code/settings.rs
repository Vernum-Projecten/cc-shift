// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift setup`: installs the status line and the rate-limit hook in
//! `~/.claude/settings.json`.
//!
//! The status line is a `statusLine` entry of type `command`
//! (<https://code.claude.com/docs/en/statusline>), and the hook a
//! `StopFailure` entry with matcher `rate_limit`
//! (<https://code.claude.com/docs/en/hooks>). Every other setting is kept.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::error::Error;

/// The hook event and matcher `cc-shift` installs under.
const HOOK_EVENT: &str = "StopFailure";
const HOOK_MATCHER: &str = "rate_limit";

/// What [`install`] changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    /// The settings with the status line and the hook in place.
    pub settings: Value,
    /// The status line command the user had, to run after `cc-shift statusline`.
    pub previous_statusline: Option<String>,
    /// Whether anything changed.
    pub changed: bool,
}

/// Puts `cc-shift statusline` and `cc-shift hook rate-limit` into `settings`.
///
/// `cc-shift` names its own binary by the absolute path `cs_path`, so the
/// commands work whatever `PATH` Claude Code runs them with. A status line
/// the user had is returned to be chained; one that is already `cc-shift` is
/// left as it is. Running it on its own output changes nothing.
///
/// # Errors
///
/// Returns [`Error::Settings`] when `settings`, or its `hooks`, is not a
/// JSON object.
pub fn install(settings: Value, cs_path: &str) -> Result<Installed, Error> {
    let Value::Object(mut root) = settings else {
        return Err(not_an_object("the settings"));
    };
    let before = root.clone();
    let statusline = format!("{} statusline", quoted(cs_path));
    let hook = format!("{} hook rate-limit", quoted(cs_path));

    let previous_statusline = root
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(Value::as_str)
        .filter(|command| *command != statusline && !ours(command, "statusline"))
        .map(str::to_owned);
    let current = root
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(Value::as_str);
    if current != Some(statusline.as_str()) {
        root.insert(
            "statusLine".to_owned(),
            json!({ "type": "command", "command": statusline }),
        );
    }

    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| not_an_object("hooks"))?;
    let entries = hooks
        .entry(HOOK_EVENT)
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Value::Array(list) = entries {
        drop_older_hooks(list, &hook);
    }
    let present = entries.as_array().is_some_and(|list| {
        list.iter()
            .filter_map(|entry| entry.get("hooks").and_then(Value::as_array))
            .flatten()
            .any(|h| h.get("command").and_then(Value::as_str) == Some(hook.as_str()))
    });
    if !present {
        let Value::Array(list) = entries else {
            return Err(not_an_object(HOOK_EVENT));
        };
        list.push(json!({
            "matcher": HOOK_MATCHER,
            "hooks": [{ "type": "command", "command": hook }]
        }));
    }

    let changed = root != before;
    Ok(Installed {
        settings: Value::Object(root),
        previous_statusline,
        changed,
    })
}

/// Removes this tool's status line and rate-limit hook from `settings`.
///
/// Every other setting and hook is kept. The status line goes back to
/// `previous_statusline` when the user had one, and is removed otherwise.
/// A hook event or the `hooks` object left empty is removed too. Returns
/// the settings and whether anything changed.
///
/// # Errors
///
/// Returns [`Error::Settings`] when `settings` is not a JSON object.
pub fn uninstall(
    settings: Value,
    previous_statusline: Option<&str>,
) -> Result<(Value, bool), Error> {
    let Value::Object(mut root) = settings else {
        return Err(not_an_object("the settings"));
    };
    let before = root.clone();
    let our_statusline = root
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(Value::as_str)
        .is_some_and(|command| ours(command, "statusline"));
    if our_statusline {
        match previous_statusline {
            Some(command) => {
                root.insert(
                    "statusLine".to_owned(),
                    json!({ "type": "command", "command": command }),
                );
            }
            None => {
                root.remove("statusLine");
            }
        }
    }
    if let Some(Value::Object(hooks)) = root.get_mut("hooks") {
        if let Some(Value::Array(entries)) = hooks.get_mut(HOOK_EVENT) {
            drop_older_hooks(entries, "");
            if entries.is_empty() {
                hooks.remove(HOOK_EVENT);
            }
        }
        if hooks.is_empty() {
            root.remove("hooks");
        }
    }
    let changed = root != before;
    Ok((Value::Object(root), changed))
}

/// The names this tool's binary has had; a command run through any of them
/// is this tool's own.
const OWN_NAMES: [&str; 2] = ["cc-shift", "cs"];

/// Returns whether `command` is this tool's binary, at any path and under
/// any of its names, running `tail` (`statusline` or `hook rate-limit`).
fn ours(command: &str, tail: &str) -> bool {
    let Some(binary) = command.strip_suffix(tail).and_then(|c| c.strip_suffix(' ')) else {
        return false;
    };
    let binary = binary.trim_matches('\'');
    let name = binary.rsplit('/').next().unwrap_or(binary);
    OWN_NAMES.contains(&name)
}

/// Removes hook commands of this tool other than `hook`, such as one left by
/// an older install or name, and entries left with no command.
fn drop_older_hooks(entries: &mut Vec<Value>, hook: &str) {
    for entry in entries.iter_mut() {
        if let Some(Value::Array(commands)) = entry.get_mut("hooks") {
            commands.retain(|h| {
                h.get("command")
                    .and_then(Value::as_str)
                    .is_none_or(|c| c == hook || !ours(c, "hook rate-limit"))
            });
        }
    }
    entries.retain(|entry| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .is_none_or(|commands| !commands.is_empty())
    });
}

/// Quotes a path for `/bin/sh` when it holds anything but plain characters.
fn quoted(path: &str) -> String {
    let plain = path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c));
    if plain {
        path.to_owned()
    } else {
        format!("'{}'", path.replace('\'', r"'\''"))
    }
}

fn not_an_object(what: &str) -> Error {
    Error::Settings(format!(
        "{what} in ~/.claude/settings.json is not a JSON object"
    ))
}

/// Returns the settings file of `~/.claude` inside `home`.
#[must_use]
pub fn settings_path(home: &Path) -> PathBuf {
    home.join(".claude").join("settings.json")
}

/// Reads a settings file; a missing file is an empty object.
///
/// # Errors
///
/// Returns [`Error::Settings`] when the file cannot be read or parsed.
pub fn read_settings(path: &Path) -> Result<Value, Error> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| Error::Settings(format!("{} does not parse: {e}", path.display()))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Value::Object(Map::new())),
        Err(e) => Err(Error::Settings(format!(
            "cannot read {}: {e}",
            path.display()
        ))),
    }
}

/// Writes a settings file through a temporary file and one rename.
///
/// # Errors
///
/// Returns [`Error::Settings`] when the file cannot be written.
pub fn write_settings(path: &Path, settings: &Value) -> Result<(), Error> {
    let fail = |e: &dyn std::fmt::Display| {
        Error::Settings(format!("cannot write {}: {e}", path.display()))
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| fail(&e))?;
    }
    let mut text = serde_json::to_vec_pretty(settings).map_err(|e| fail(&e))?;
    text.push(b'\n');
    let temporary = path.with_extension("json.cs-tmp");
    fs::write(&temporary, text).map_err(|e| fail(&e))?;
    fs::rename(&temporary, path).map_err(|e| fail(&e))
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::{install, quoted};
    use crate::error::Error;
    use serde_json::json;

    const CS: &str = "/Users/someone/.cargo/bin/cc-shift";

    #[test]
    fn installs_into_empty_settings() -> Result<(), Error> {
        let done = install(json!({}), CS)?;
        assert_eq!(
            done.settings,
            json!({
                "statusLine": {"type": "command", "command": "/Users/someone/.cargo/bin/cc-shift statusline"},
                "hooks": {"StopFailure": [{
                    "matcher": "rate_limit",
                    "hooks": [{"type": "command", "command": "/Users/someone/.cargo/bin/cc-shift hook rate-limit"}]
                }]}
            })
        );
        assert_eq!(done.previous_statusline, None);
        assert!(done.changed, "changed");
        Ok(())
    }

    #[test]
    fn keeps_other_settings_and_hooks() -> Result<(), Error> {
        let settings = json!({
            "model": "opus",
            "hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "guard"}]}],
                "StopFailure": [{"matcher": "server_error", "hooks": [{"type": "command", "command": "notify"}]}]
            }
        });
        let done = install(settings, CS)?;
        assert_eq!(done.settings["model"], json!("opus"));
        assert_eq!(
            done.settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            json!("guard")
        );
        assert_eq!(
            done.settings["hooks"]["StopFailure"][0]["hooks"][0]["command"],
            json!("notify")
        );
        assert_eq!(
            done.settings["hooks"]["StopFailure"][1]["matcher"],
            json!("rate_limit")
        );
        Ok(())
    }

    #[test]
    fn chains_the_users_status_line() -> Result<(), Error> {
        let settings = json!({"statusLine": {"type": "command", "command": "~/bin/my-line"}});
        let done = install(settings, CS)?;
        assert_eq!(done.previous_statusline.as_deref(), Some("~/bin/my-line"));
        assert_eq!(
            done.settings["statusLine"]["command"],
            json!("/Users/someone/.cargo/bin/cc-shift statusline")
        );
        Ok(())
    }

    #[test]
    fn a_second_run_changes_nothing() -> Result<(), Error> {
        let first = install(
            json!({"statusLine": {"type": "command", "command": "mine"}}),
            CS,
        )?;
        let second = install(first.settings.clone(), CS)?;
        assert!(!second.changed, "second run changed {:?}", second.settings);
        assert_eq!(second.previous_statusline, None);
        assert_eq!(second.settings, first.settings);
        Ok(())
    }

    #[test]
    fn replaces_an_install_under_the_old_name() -> Result<(), Error> {
        let old = "/Users/someone/.cargo/bin/cs";
        let first = install(json!({}), old)?;
        let done = install(first.settings, CS)?;
        assert_eq!(
            done.previous_statusline, None,
            "the old line is not chained"
        );
        assert_eq!(
            done.settings["statusLine"]["command"],
            json!(format!("{CS} statusline"))
        );
        let entries = done.settings["hooks"]["StopFailure"]
            .as_array()
            .map_or(0, Vec::len);
        assert_eq!(entries, 1, "one hook: {}", done.settings);
        assert_eq!(
            done.settings["hooks"]["StopFailure"][0]["hooks"][0]["command"],
            json!(format!("{CS} hook rate-limit"))
        );
        Ok(())
    }

    #[test]
    fn a_users_own_command_is_never_ours() {
        assert!(!super::ours(
            "/usr/local/bin/my-line statusline",
            "statusline"
        ));
        assert!(!super::ours("cs-other statusline", "statusline"));
        assert!(super::ours("'/a b/cc-shift' statusline", "statusline"));
    }

    #[test]
    fn uninstall_removes_only_what_setup_added() -> Result<(), Error> {
        let settings = json!({
            "model": "opus",
            "statusLine": {"type": "command", "command": "mine"},
            "hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "guard"}]}]}
        });
        let installed = install(settings.clone(), CS)?;
        let (cleaned, changed) = super::uninstall(installed.settings, Some("mine"))?;
        assert!(changed, "changed");
        assert_eq!(cleaned, settings, "back to what it was");
        let (again, changed) = super::uninstall(cleaned, Some("mine"))?;
        assert!(!changed, "a second run changes nothing: {again}");
        Ok(())
    }

    #[test]
    fn uninstall_without_a_previous_line_removes_it() -> Result<(), Error> {
        let installed = install(json!({}), "/Users/someone/.cargo/bin/cs")?;
        let (cleaned, _) = super::uninstall(installed.settings, None)?;
        assert_eq!(cleaned, json!({}), "old name, nothing left");
        Ok(())
    }

    #[test]
    fn refuses_settings_that_are_not_an_object() {
        assert!(matches!(install(json!([]), CS), Err(Error::Settings(_))));
        assert!(matches!(
            install(json!({"hooks": 3}), CS),
            Err(Error::Settings(_))
        ));
    }

    #[test]
    fn quotes_a_path_with_spaces() {
        assert_eq!(quoted("/a b/cs"), "'/a b/cs'");
        assert_eq!(quoted("/a/it's/cs"), r"'/a/it'\''s/cs'");
        assert_eq!(quoted(CS), CS);
    }
}
