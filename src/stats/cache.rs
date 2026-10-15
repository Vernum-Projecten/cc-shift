//! The statistics cache, `~/.claude-profiles/stats-cache.json`.
//!
//! It keeps the responses read from each transcript with the size and the
//! modification time the file had, so a later run reads again only the
//! files that changed. It holds token counts, models, times, session ids and
//! folder paths, never message content. A cache that is missing, damaged or
//! of another version is rebuilt from the transcripts.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::stats::Response;
use crate::stats::transcript;
use crate::store::profile::ProfilesRoot;

/// The version of the cache layout; a cache of another version is rebuilt.
const VERSION: u32 = 1;

/// The whole cache file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Contents {
    version: u32,
    files: BTreeMap<String, Entry>,
}

impl Default for Contents {
    fn default() -> Self {
        Self {
            version: VERSION,
            files: BTreeMap::new(),
        }
    }
}

/// What was read from one transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    size: u64,
    modified: u64,
    responses: Vec<Response>,
}

/// The statistics cache file.
#[derive(Debug, Clone)]
pub struct StatsCache {
    path: PathBuf,
}

impl StatsCache {
    /// Returns the cache file inside a profiles root.
    #[must_use]
    pub fn in_root(root: &ProfilesRoot) -> Self {
        Self {
            path: root.path().join("stats-cache.json"),
        }
    }

    /// Returns the responses of every file in `files`, in the order given.
    ///
    /// A file whose size and modification time match the cache is not read
    /// again. The cache is rewritten to hold exactly `files` when anything
    /// changed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::StatsRead`] when a transcript cannot be read, and
    /// [`Error::StatsCache`] when the cache cannot be written.
    pub fn responses(&self, files: &[PathBuf]) -> Result<Vec<Response>, Error> {
        let old = self.load();
        let mut new = Contents::default();
        for file in files {
            let read_error = |source| Error::StatsRead {
                path: file.clone(),
                source,
            };
            let meta = fs::metadata(file).map_err(read_error)?;
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .and_then(|d| u64::try_from(d.as_nanos()).ok());
            let key = file.to_string_lossy().into_owned();
            let cached = old
                .files
                .get(&key)
                .filter(|e| e.size == meta.len() && Some(e.modified) == modified);
            let entry = match (cached, modified) {
                (Some(entry), _) => entry.clone(),
                (None, modified) => {
                    let bytes = fs::read(file).map_err(read_error)?;
                    let responses = transcript::responses(&String::from_utf8_lossy(&bytes));
                    // NOTE: no specification governs this: our own design. A
                    // file without a modification time is cached under 0 and,
                    // as `modified` is then `None`, read again on every run.
                    Entry {
                        size: meta.len(),
                        modified: modified.unwrap_or(0),
                        responses,
                    }
                }
            };
            new.files.insert(key, entry);
        }
        if new != old {
            self.save(&new)?;
        }
        let mut all = Vec::new();
        for file in files {
            if let Some(entry) = new.files.get(&*file.to_string_lossy()) {
                all.extend(entry.responses.iter().cloned());
            }
        }
        Ok(all)
    }

    /// Returns the cached contents, or empty contents when there is no
    /// usable cache.
    fn load(&self) -> Contents {
        // NOTE: no specification governs this: our own design. The cache only
        // saves work, so one that cannot be read is rebuilt from the transcripts.
        fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Contents>(&bytes).ok())
            .filter(|c| c.version == VERSION)
            .unwrap_or_default()
    }

    /// Writes the cache to a temporary file and renames it into place.
    fn save(&self, contents: &Contents) -> Result<(), Error> {
        let cache_error = |source| Error::StatsCache {
            path: self.path.clone(),
            source,
        };
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(cache_error)?;
        }
        let text = serde_json::to_vec(contents).map_err(|e| cache_error(io::Error::other(e)))?;
        let temporary = temporary_path(&self.path);
        fs::write(&temporary, text).map_err(cache_error)?;
        fs::rename(&temporary, &self.path).map_err(cache_error)
    }
}

/// Returns the temporary file a cache is written to before the rename.
fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", std::process::id()));
    path.with_file_name(name)
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::StatsCache;
    use crate::error::Error;
    use std::fs;
    use std::path::PathBuf;

    fn line(id: &str, output: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"2026-10-15T10:00:00Z","cwd":"/w/a","sessionId":"s","message":{{"id":"{id}","model":"m","usage":{{"input_tokens":1,"output_tokens":{output},"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}}}}"#
        )
    }

    fn cache(dir: &tempfile::TempDir) -> StatsCache {
        StatsCache {
            path: dir.path().join("stats-cache.json"),
        }
    }

    #[test]
    fn reads_new_files_and_keeps_them() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::TempDir::new()?;
        let file = dir.path().join("s.jsonl");
        fs::write(&file, line("a", 5))?;
        let found = cache(&dir).responses(std::slice::from_ref(&file))?;
        assert_eq!(found.len(), 1, "{found:?}");
        let text = fs::read_to_string(dir.path().join("stats-cache.json"))?;
        assert!(text.contains("\"a\""), "{text}");
        assert!(!text.contains("content"), "{text}");
        Ok(())
    }

    #[test]
    fn an_unchanged_file_comes_from_the_cache() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::TempDir::new()?;
        let file = dir.path().join("s.jsonl");
        fs::write(&file, line("a", 5))?;
        let files = vec![file.clone()];
        cache(&dir).responses(&files)?;
        let cached = fs::read_to_string(dir.path().join("stats-cache.json"))?;
        fs::write(
            dir.path().join("stats-cache.json"),
            cached.replace("\"a\"", "\"from-cache\""),
        )?;
        let found = cache(&dir).responses(&files)?;
        assert_eq!(found[0].id, "from-cache");
        Ok(())
    }

    #[test]
    fn a_changed_file_is_read_again() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::TempDir::new()?;
        let file = dir.path().join("s.jsonl");
        fs::write(&file, line("a", 5))?;
        let files = vec![file.clone()];
        cache(&dir).responses(&files)?;
        fs::write(&file, format!("{}\n{}", line("a", 5), line("b", 7)))?;
        let found = cache(&dir).responses(&files)?;
        assert_eq!(found.len(), 2, "{found:?}");
        Ok(())
    }

    #[test]
    fn a_damaged_cache_is_rebuilt() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::TempDir::new()?;
        let file = dir.path().join("s.jsonl");
        fs::write(&file, line("a", 5))?;
        fs::write(dir.path().join("stats-cache.json"), "{ damaged")?;
        let found = cache(&dir).responses(&[file])?;
        assert_eq!(found.len(), 1, "{found:?}");
        Ok(())
    }

    #[test]
    fn an_unreadable_transcript_is_an_error() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::TempDir::new()?;
        let missing = PathBuf::from(dir.path()).join("gone.jsonl");
        let result = cache(&dir).responses(&[missing]);
        assert!(matches!(result, Err(Error::StatsRead { .. })), "{result:?}");
        Ok(())
    }
}
