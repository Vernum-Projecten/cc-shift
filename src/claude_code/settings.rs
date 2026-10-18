// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift setup`: installs the status line and the rate-limit hook in
//! `~/.claude/settings.json`.
//!
//! The status line is a `statusLine` entry of type `command` with a
//! `refreshInterval`, so it also runs while the main session waits on
//! background subagents (<https://code.claude.com/docs/en/statusline>), and
//! the hook a
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

/// How often, in seconds, Claude Code runs the status line when nothing else
/// runs it, such as while the main session waits on background subagents.
const REFRESH_SECONDS: u64 = 30;

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
/// The status line gets a `refreshInterval` of 30 seconds, unless
/// it already runs at least that often. `cc-shift` names its own binary by the absolute path `cs_path`, so the
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
    let statusline = format!("{} statusline", shell_path(cs_path)?);
    let hook = hook_entry(cs_path);

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
    if let Some(line) = root.get_mut("statusLine").and_then(Value::as_object_mut) {
        let every = line.get("refreshInterval").and_then(Value::as_u64);
        if every.is_none_or(|seconds| seconds == 0 || seconds > REFRESH_SECONDS) {
            line.insert("refreshInterval".to_owned(), json!(REFRESH_SECONDS));
        }
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
        drop_older_hooks(list, Some(&hook));
    }
    let present = entries.as_array().is_some_and(|list| {
        list.iter()
            .filter_map(|entry| entry.get("hooks").and_then(Value::as_array))
            .flatten()
            .any(|h| same_hook(h, &hook))
    });
    if !present {
        let Value::Array(list) = entries else {
            return Err(not_an_object(HOOK_EVENT));
        };
        list.push(json!({
            "matcher": HOOK_MATCHER,
            "hooks": [hook]
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
            drop_older_hooks(entries, None);
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
    own_binary(binary)
}

/// Returns whether `binary` (a path, quoted or not) is this tool's binary
/// under any of its names, with `/` or `\` and with or without `.exe`.
fn own_binary(binary: &str) -> bool {
    let binary = binary.trim_matches(|c| c == '\'' || c == '"');
    let name = binary.rsplit(['/', '\\']).next().unwrap_or(binary);
    let name = name.strip_suffix(".exe").unwrap_or(name);
    OWN_NAMES.contains(&name)
}

/// Returns whether the hook `h` is this tool's rate-limit hook, in the shell
/// form (`command` alone) or the exec form (`command` and `args`).
fn own_hook(h: &Value) -> bool {
    let Some(command) = h.get("command").and_then(Value::as_str) else {
        return false;
    };
    match h.get("args") {
        Some(args) => own_binary(command) && *args == json!(["hook", "rate-limit"]),
        None => ours(command, "hook rate-limit"),
    }
}

/// Returns whether two hooks run the same command with the same arguments.
fn same_hook(a: &Value, b: &Value) -> bool {
    a.get("command") == b.get("command") && a.get("args") == b.get("args")
}

/// Returns the rate-limit hook for the binary at `cs_path`.
///
/// On Windows the hook uses the exec form, which runs the program directly
/// with `args` and needs no shell (<https://code.claude.com/docs/en/hooks>);
/// on macOS and Linux it is a command line for `sh -c`.
fn hook_entry(cs_path: &str) -> Value {
    if cfg!(windows) {
        json!({ "type": "command", "command": cs_path, "args": ["hook", "rate-limit"] })
    } else {
        json!({ "type": "command", "command": format!("{} hook rate-limit", quoted(cs_path)) })
    }
}

/// Returns `cs_path` as the status line command needs it.
///
/// On macOS and Linux it is quoted for `/bin/sh`. On Windows Claude Code
/// runs the status line through Git Bash, or PowerShell without it, and no
/// quoting works in both, so the path is written with `/` and with `~` for
/// the home folder (<https://code.claude.com/docs/en/statusline#windows-configuration>).
///
/// # Errors
///
/// Returns [`Error::Settings`] on Windows for a path that holds a space or
/// another character neither shell takes unquoted.
fn shell_path(cs_path: &str) -> Result<String, Error> {
    if cfg!(windows) {
        windows_shell_path(cs_path, std::env::var("USERPROFILE").ok().as_deref())
    } else {
        Ok(quoted(cs_path))
    }
}

/// Returns `path` with `/` for `\` and `~` for the leading `home`, or the
/// error when the result needs quoting in Git Bash or PowerShell.
fn windows_shell_path(path: &str, home: Option<&str>) -> Result<String, Error> {
    let slashed = path.replace('\\', "/");
    let short = home
        .map(|h| h.replace('\\', "/"))
        .filter(|h| !h.is_empty())
        .and_then(|h| {
            let rest = slashed.get(h.len()..)?;
            (slashed.get(..h.len())?.eq_ignore_ascii_case(&h) && rest.starts_with('/'))
                .then(|| format!("~{rest}"))
        })
        .unwrap_or(slashed);
    let plain = short
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+~:".contains(c));
    if plain {
        Ok(short)
    } else {
        Err(Error::Settings(format!(
            "the status line cannot run {path}: move cc-shift.exe to a folder without spaces, such as %USERPROFILE%\\.cargo\\bin"
        )))
    }
}

/// Removes this tool's rate-limit hooks other than `hook`, such as one left
/// by an older install or name, and entries left with no command.
fn drop_older_hooks(entries: &mut Vec<Value>, hook: Option<&Value>) {
    for entry in entries.iter_mut() {
        if let Some(Value::Array(commands)) = entry.get_mut("hooks") {
            commands.retain(|h| hook.is_some_and(|keep| same_hook(h, keep)) || !own_hook(h));
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
    let temporary = path.with_extension("json.cc-shift-tmp");
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

    /// Returns the rate-limit hook for `cs`: the exec form on Windows, which
    /// runs a program without a shell, and a shell line elsewhere
    /// (<https://code.claude.com/docs/en/hooks>).
    fn rate_limit_hook(cs: &str) -> serde_json::Value {
        if cfg!(windows) {
            json!({"type": "command", "command": cs, "args": ["hook", "rate-limit"]})
        } else {
            json!({"type": "command", "command": format!("{cs} hook rate-limit")})
        }
    }

    #[test]
    fn installs_into_empty_settings() -> Result<(), Error> {
        let done = install(json!({}), CS)?;
        assert_eq!(
            done.settings,
            json!({
                "statusLine": {
                    "type": "command",
                    "command": "/Users/someone/.cargo/bin/cc-shift statusline",
                    "refreshInterval": 30
                },
                "hooks": {"StopFailure": [{
                    "matcher": "rate_limit",
                    "hooks": [rate_limit_hook(CS)]
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
    fn an_install_without_a_refresh_interval_gains_one() -> Result<(), Error> {
        let first = install(json!({}), CS)?;
        let mut older = first.settings.clone();
        if let Some(line) = older["statusLine"].as_object_mut() {
            line.remove("refreshInterval");
        }
        let done = install(older, CS)?;
        assert!(done.changed, "changed");
        assert_eq!(done.settings["statusLine"]["refreshInterval"], json!(30));
        Ok(())
    }

    #[test]
    fn a_refresh_interval_that_is_too_long_is_shortened() -> Result<(), Error> {
        let mut settings = install(json!({}), CS)?.settings;
        settings["statusLine"]["refreshInterval"] = json!(300);
        let done = install(settings, CS)?;
        assert_eq!(done.settings["statusLine"]["refreshInterval"], json!(30));
        Ok(())
    }

    #[test]
    fn a_shorter_refresh_interval_is_kept() -> Result<(), Error> {
        let mut settings = install(json!({}), CS)?.settings;
        settings["statusLine"]["refreshInterval"] = json!(5);
        let done = install(settings, CS)?;
        assert!(!done.changed, "changed {:?}", done.settings);
        assert_eq!(done.settings["statusLine"]["refreshInterval"], json!(5));
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
            done.settings["hooks"]["StopFailure"][0]["hooks"][0],
            rate_limit_hook(CS)
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

    #[test]
    fn a_windows_status_line_path_uses_slashes_and_the_home_shorthand() {
        use super::windows_shell_path;
        assert_eq!(
            windows_shell_path(
                r"C:\Users\Ann\.cargo\bin\cc-shift.exe",
                Some(r"C:\Users\Ann")
            )
            .ok()
            .as_deref(),
            Some("~/.cargo/bin/cc-shift.exe")
        );
        assert_eq!(
            windows_shell_path(r"D:\tools\cc-shift.exe", Some(r"C:\Users\Ann"))
                .ok()
                .as_deref(),
            Some("D:/tools/cc-shift.exe"),
            "outside the home folder the full path stays"
        );
        assert!(
            windows_shell_path(r"C:\Program Files\cc-shift.exe", None).is_err(),
            "a space cannot be written for both shells"
        );
    }

    #[test]
    fn windows_paths_and_the_exec_form_hook_are_recognised_as_ours() {
        use super::{own_binary, own_hook};
        assert!(own_binary(r"C:\Users\Ann\.cargo\bin\cc-shift.exe"));
        assert!(own_binary("~/.cargo/bin/cc-shift.exe"));
        assert!(!own_binary(r"C:\tools\other.exe"));
        assert!(own_hook(&json!({
            "type": "command",
            "command": r"C:\Users\Ann\.cargo\bin\cc-shift.exe",
            "args": ["hook", "rate-limit"]
        })));
        assert!(!own_hook(&json!({
            "type": "command",
            "command": r"C:\Users\Ann\.cargo\bin\cc-shift.exe",
            "args": ["status"]
        })));
        assert!(own_hook(&json!({
            "type": "command",
            "command": "/Users/ann/.cargo/bin/cc-shift hook rate-limit"
        })));
    }
}
