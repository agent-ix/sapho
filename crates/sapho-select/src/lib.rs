// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Explicit bounded host acquisition; never graph execution or inference.
mod files;
mod git;
mod json;
mod process;
pub use files::select_files;
pub use git::{GitMode, GitOptions, select_git};
pub use json::select_json;
use sapho_core::{Datum, Inputs, SaphoError, SourceId, SourceRef, Value, validate_name};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::PathBuf,
    time::{Duration, Instant},
};

/// Typed acquisition refusals; callers never interpret diagnostic text.
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum SelectionError {
    /// Shared schema, duplicate-key or value refusal.
    #[error(transparent)]
    Core(#[from] SaphoError),
    /// Filesystem operation failed for a named path.
    #[error("I/O at {path}: {source}")]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Underlying error.
        #[serde(skip)]
        source: std::io::Error,
    },
    /// Root is not an explicit ordinary directory.
    #[error("Invalid selection root {0}")]
    InvalidRoot(PathBuf),
    /// Path cannot be represented in the declared UTF-8 interchange.
    #[error("Non-UTF-8 path {0:?}")]
    PathEncoding(PathBuf),
    /// Glob syntax is invalid.
    #[error("Invalid glob {pattern}: {source}")]
    Glob {
        /// Original pattern.
        pattern: String,
        /// Parser refusal.
        #[serde(skip)]
        source: globset::Error,
    },
    /// File or patch contains NUL or invalid UTF-8.
    #[error("Non-text data at {0}")]
    NonText(String),
    /// A file changed into a non-regular object while opening.
    #[error("Unsupported selected file {0}")]
    UnsupportedFile(String),
    /// Positive finite limits were not supplied.
    #[error("Acquisition limits must be positive")]
    InvalidLimits,
    /// A specified resource ceiling was exceeded.
    #[error("Acquisition ceiling exceeded: {resource:?} ({limit})")]
    Limit {
        /// Resource identity.
        resource: Resource,
        /// Explicit ceiling.
        limit: usize,
    },
    /// Acquisition exceeded its monotonic time budget.
    #[error("Acquisition deadline exceeded")]
    Deadline,
    /// RFC 6901 syntax or escaping is invalid.
    #[error("Invalid JSON pointer {0}")]
    PointerSyntax(String),
    /// Object member is absent.
    #[error("JSON member absent: {0}")]
    MissingKey(String),
    /// Array index is invalid or outside the array.
    #[error("Invalid JSON array index {0}")]
    ArrayIndex(String),
    /// Pointer attempts to descend through a scalar.
    #[error("JSON pointer descends through a scalar")]
    PointerType,
    /// Git cannot be started.
    #[error("Git is unavailable: {0}")]
    GitUnavailable(
        #[source]
        #[serde(skip)]
        std::io::Error,
    ),
    /// Git version is missing, malformed or older than required.
    #[error("Git 2.34+ required; received {0}")]
    GitVersion(String),
    /// A Git process failed; diagnostics are bounded.
    #[error("Git failed with status {status:?}: {stderr}")]
    GitFailed {
        /// Exit status, absent after signal.
        status: Option<i32>,
        /// Bounded stderr.
        stderr: String,
    },
    /// A requested commit is invalid or unborn.
    #[error("Invalid Git revision {0}")]
    Revision(String),
    /// Git wire metadata could not be interpreted safely.
    #[error("Invalid Git metadata")]
    GitMetadata,
    /// Submodule changes cannot be represented as ordinary text patches.
    #[error("Submodule change at {0}")]
    Submodule(String),
    /// Nonblocking bounded Git pipes require a supported host.
    #[error("Bounded Git acquisition requires a Unix host")]
    UnsupportedPlatform,
}
/// Acquisition resources with distinct machine identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    /// Selected files or patches.
    Files,
    /// All visited directory entries.
    Entries,
    /// One file/process stdout.
    FileBytes,
    /// Accumulated retained file/patch bytes.
    TotalBytes,
    /// Git stderr.
    StderrBytes,
}
/// Caller-owned finite acquisition ceilings, shared across one selection.
#[derive(Debug, Clone)]
pub struct SelectionLimits {
    /// Selected file/patch ceiling.
    pub files: usize,
    /// Visited-entry ceiling.
    pub entries: usize,
    /// Per-file bytes, without cropping.
    pub file_bytes: usize,
    /// Total retained bytes.
    pub total_bytes: usize,
    /// Separate Git stderr ceiling.
    pub stderr_bytes: usize,
    /// Total monotonic acquisition budget.
    pub duration: Duration,
}
impl SelectionLimits {
    /// Refuse nonpositive or unrepresentable acquisition budgets before I/O.
    pub fn validate(&self) -> Result<(), SelectionError> {
        Budget::new(self).map(|_| ())
    }
}
impl Default for SelectionLimits {
    fn default() -> Self {
        Self {
            files: 1024,
            entries: 10000,
            file_bytes: 1_048_576,
            total_bytes: 4 * 1_048_576,
            stderr_bytes: 65536,
            duration: Duration::from_secs(30),
        }
    }
}
/// Root-relative slash-separated inclusion/exclusion patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patterns {
    /// Inclusion globs, defaulting to all paths.
    pub include: Vec<String>,
    /// Exclusion globs; `.git` directories are always skipped.
    pub exclude: Vec<String>,
}
impl Default for Patterns {
    fn default() -> Self {
        Self {
            include: vec!["**/*".into()],
            exclude: Vec::new(),
        }
    }
}
pub(crate) struct Matcher {
    include: globset::GlobSet,
    exclude: globset::GlobSet,
}
impl Matcher {
    fn new(patterns: &Patterns) -> Result<Self, SelectionError> {
        fn build(patterns: &[String]) -> Result<globset::GlobSet, SelectionError> {
            let mut builder = globset::GlobSetBuilder::new();
            for pattern in patterns {
                builder.add(
                    globset::GlobBuilder::new(pattern)
                        .literal_separator(true)
                        .build()
                        .map_err(|source| SelectionError::Glob {
                            pattern: pattern.clone(),
                            source,
                        })?,
                );
            }
            builder.build().map_err(|source| SelectionError::Glob {
                pattern: patterns.join(","),
                source,
            })
        }
        Ok(Self {
            include: build(&patterns.include)?,
            exclude: build(&patterns.exclude)?,
        })
    }
    fn matches(&self, path: &str) -> bool {
        self.include.is_match(path) && !self.exclude.is_match(path)
    }
}
pub(crate) struct Budget<'a> {
    limits: &'a SelectionLimits,
    deadline: Instant,
    files: usize,
    entries: usize,
    bytes: usize,
}
impl<'a> Budget<'a> {
    fn new(limits: &'a SelectionLimits) -> Result<Self, SelectionError> {
        if [
            limits.files,
            limits.entries,
            limits.file_bytes,
            limits.total_bytes,
            limits.stderr_bytes,
        ]
        .contains(&0)
            || limits.duration.is_zero()
        {
            return Err(SelectionError::InvalidLimits);
        }
        let deadline = Instant::now()
            .checked_add(limits.duration)
            .ok_or(SelectionError::InvalidLimits)?;
        Ok(Self {
            limits,
            deadline,
            files: 0,
            entries: 0,
            bytes: 0,
        })
    }
    fn check_time(&self) -> Result<(), SelectionError> {
        if Instant::now() >= self.deadline {
            Err(SelectionError::Deadline)
        } else {
            Ok(())
        }
    }
    fn entry(&mut self) -> Result<(), SelectionError> {
        self.check_time()?;
        self.entries = add(self.entries, 1, self.limits.entries, Resource::Entries)?;
        Ok(())
    }
    fn retain(&mut self, bytes: usize) -> Result<(), SelectionError> {
        self.check_time()?;
        self.files = add(self.files, 1, self.limits.files, Resource::Files)?;
        self.bytes = add(
            self.bytes,
            bytes,
            self.limits.total_bytes,
            Resource::TotalBytes,
        )?;
        Ok(())
    }
}
fn add(
    current: usize,
    more: usize,
    limit: usize,
    resource: Resource,
) -> Result<usize, SelectionError> {
    current
        .checked_add(more)
        .filter(|n| *n <= limit)
        .ok_or(SelectionError::Limit { resource, limit })
}
fn io(path: impl Into<PathBuf>, source: std::io::Error) -> SelectionError {
    SelectionError::Io {
        path: path.into(),
        source,
    }
}
fn read_bounded(
    mut reader: impl Read,
    name: &str,
    budget: &Budget<'_>,
) -> Result<Vec<u8>, SelectionError> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        budget.check_time()?;
        let count = reader.read(&mut chunk).map_err(|e| io(name, e))?;
        if count == 0 {
            break;
        }
        add(
            bytes.len(),
            count,
            budget.limits.file_bytes,
            Resource::FileBytes,
        )?;
        bytes.extend_from_slice(chunk.get(..count).ok_or(SelectionError::GitMetadata)?);
    }
    Ok(bytes)
}
fn text(bytes: Vec<u8>, name: &str) -> Result<String, SelectionError> {
    if bytes.contains(&0) {
        return Err(SelectionError::NonText(name.into()));
    }
    String::from_utf8(bytes).map_err(|_| SelectionError::NonText(name.into()))
}
fn source(kind: &str, location: &str, bytes: &[u8]) -> Result<SourceRef, SelectionError> {
    let hash = format!("{:x}", Sha256::digest(bytes));
    let identity =
        serde_json::to_string(&(kind, location, hash)).map_err(|_| SelectionError::GitMetadata)?;
    Ok(SourceRef {
        source: SourceId::new(identity)?,
        start: Some(0),
        end: Some(u64::try_from(bytes.len()).map_err(|_| SelectionError::InvalidLimits)?),
    })
}
fn unit(
    path: &str,
    status: &str,
    bytes: Vec<u8>,
    location: &str,
    kind: &str,
) -> Result<Datum, SelectionError> {
    let attribution = source(kind, location, &bytes)?;
    let value = Value::Record(std::collections::BTreeMap::from([
        ("path".into(), Value::Text(path.into())),
        ("text".into(), Value::Text(text(bytes, path)?)),
        ("status".into(), Value::Text(status.into())),
    ]));
    let mut datum = Datum::new(path, value)?;
    datum.inherit_sources([attribution]);
    Ok(datum)
}
fn list_inputs(port: &str, identity: &str, units: Vec<Datum>) -> Result<Inputs, SelectionError> {
    validate_name(port)?;
    let sources = units
        .iter()
        .flat_map(|d| d.sources.clone())
        .collect::<Vec<_>>();
    let mut datum = Datum::new(identity, Value::List(units))?;
    datum.inherit_sources(sources);
    Ok(Inputs::from([(port.into(), datum)]))
}
#[cfg(test)]
mod tests;
