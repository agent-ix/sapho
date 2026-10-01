// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Explicit tracked/index/revision patch acquisition, with literal path arguments.
use crate::{
    Budget, Matcher, Patterns, SelectionError, SelectionLimits, io, list_inputs, process, text,
    unit,
};
use sapho_core::Inputs;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// Explicit comparison source; no untracked files are implied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GitMode {
    /// Tracked working/index state compared with HEAD.
    WorkingTree,
    /// Index compared with HEAD.
    Staged,
    /// Two explicitly resolved commits.
    Revisions {
        /// Base commit reference.
        base: String,
        /// Head commit reference.
        head: String,
    },
}
/// Git process executable and explicit comparison selection.
#[derive(Debug, Clone)]
pub struct GitOptions {
    /// Program path, defaulting to git from PATH.
    pub program: PathBuf,
    /// Selected comparison.
    pub mode: GitMode,
}
impl Default for GitOptions {
    fn default() -> Self {
        Self {
            program: "git".into(),
            mode: GitMode::WorkingTree,
        }
    }
}
struct Git<'a> {
    root: &'a Path,
    program: &'a Path,
    args: Vec<OsString>,
    budget: Budget<'a>,
}
impl Git<'_> {
    fn output(&self, args: &[OsString]) -> Result<process::Output, SelectionError> {
        let mut full = self.args.clone();
        full.extend_from_slice(args);
        process::execute(self.program, self.root, &full, &self.budget)
    }
    fn run(&self, args: &[OsString]) -> Result<Vec<u8>, SelectionError> {
        let output = self.output(args)?;
        if output.status != Some(0) {
            return Err(SelectionError::GitFailed {
                status: output.status,
                stderr: output.stderr,
            });
        }
        Ok(output.bytes)
    }
    fn resolve(&self, reference: &str) -> Result<String, SelectionError> {
        if reference.is_empty() {
            return Err(SelectionError::Revision(reference.into()));
        }
        let output = self.output(&[
            "rev-parse".into(),
            "--verify".into(),
            "--end-of-options".into(),
            format!("{reference}^{{commit}}").into(),
        ])?;
        if output.status != Some(0) {
            return Err(SelectionError::Revision(reference.into()));
        }
        let commit = text(output.bytes, reference)?.trim().to_owned();
        if commit.is_empty() || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(SelectionError::GitMetadata);
        }
        Ok(commit)
    }
}
/// Gather complete text patches with exact comparison/patch attribution under finite limits.
pub fn select_git(
    root: &Path,
    options: &GitOptions,
    patterns: &Patterns,
    limits: &SelectionLimits,
    port: &str,
) -> Result<Inputs, SelectionError> {
    let root = std::fs::canonicalize(root).map_err(|e| io(root, e))?;
    if !root.is_dir() {
        return Err(SelectionError::InvalidRoot(root));
    }
    let matcher = Matcher::new(patterns)?;
    let mut git = Git {
        root: &root,
        program: &options.program,
        args: vec![
            "--literal-pathspecs".into(),
            "-c".into(),
            "core.fsmonitor=false".into(),
            "-c".into(),
            "core.attributesfile=/dev/null".into(),
            "-c".into(),
            "core.hooksPath=/dev/null".into(),
        ],
        budget: Budget::new(limits)?,
    };
    let version = text(git.run(&["--version".into()])?, "git version")?;
    let mut numbers = version
        .strip_prefix("git version ")
        .unwrap_or("")
        .split('.');
    let major = numbers.next().and_then(|n| n.parse::<u32>().ok());
    let minor = numbers.next().and_then(|n| n.parse::<u32>().ok());
    if !matches!((major,minor),(Some(major),Some(minor)) if major>2 || (major==2 && minor>=34)) {
        return Err(SelectionError::GitVersion(version.trim().into()));
    }
    let top = text(
        git.run(&["rev-parse".into(), "--show-toplevel".into()])?,
        "git root",
    )?;
    if std::fs::canonicalize(top.trim()).map_err(|e| io(top.trim(), e))? != root {
        return Err(SelectionError::InvalidRoot(root));
    }
    // Disable repository filter commands before asking Git to read working content.
    let filters = git.output(&[
        "config".into(),
        "--local".into(),
        "--null".into(),
        "--name-only".into(),
        "--get-regexp".into(),
        r"^filter\..*\.(clean|smudge|process|required)$".into(),
    ])?;
    if !matches!(filters.status, Some(0 | 1)) {
        return Err(SelectionError::GitFailed {
            status: filters.status,
            stderr: filters.stderr,
        });
    }
    for key in filters
        .bytes
        .split(|b| *b == 0)
        .filter(|key| !key.is_empty())
    {
        let key = std::str::from_utf8(key).map_err(|_| SelectionError::GitMetadata)?;
        git.args.push("-c".into());
        git.args.push(
            format!(
                "{key}={}",
                if key.ends_with(".required") {
                    "false"
                } else {
                    ""
                }
            )
            .into(),
        );
    }
    let mut diff = vec![
        "diff".into(),
        "--no-ext-diff".into(),
        "--no-textconv".into(),
        "--no-renames".into(),
        "--ignore-submodules=none".into(),
        "--no-color".into(),
        "--diff-algorithm=myers".into(),
    ];
    let comparison = match &options.mode {
        GitMode::WorkingTree => {
            let head = git.resolve("HEAD")?;
            diff.push(head.clone().into());
            format!("working_tree:{head}")
        }
        GitMode::Staged => {
            let head = git.resolve("HEAD")?;
            diff.push("--cached".into());
            diff.push(head.clone().into());
            format!("staged:{head}")
        }
        GitMode::Revisions { base, head } => {
            let base = git.resolve(base)?;
            let head = git.resolve(head)?;
            diff.push(base.clone().into());
            diff.push(head.clone().into());
            format!("revisions:{base}:{head}")
        }
    };
    let mut raw_args = diff.clone();
    raw_args.extend([
        "--raw".into(),
        "--no-abbrev".into(),
        "-z".into(),
        "--".into(),
    ]);
    let raw = git.run(&raw_args)?;
    let mut tokens = raw.split(|b| *b == 0).filter(|bytes| !bytes.is_empty());
    let mut paths = std::collections::BTreeMap::new();
    while let Some(meta) = tokens.next() {
        git.budget.entry()?;
        let path = tokens.next().ok_or(SelectionError::GitMetadata)?;
        let path = std::str::from_utf8(path).map_err(|_| {
            SelectionError::PathEncoding(PathBuf::from(String::from_utf8_lossy(path).into_owned()))
        })?;
        let meta = std::str::from_utf8(meta).map_err(|_| SelectionError::GitMetadata)?;
        let fields = meta.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 5 || !meta.starts_with(':') {
            return Err(SelectionError::GitMetadata);
        }
        if !matcher.matches(path) {
            continue;
        }
        if fields.first() == Some(&":160000") || fields.get(1) == Some(&"160000") {
            return Err(SelectionError::Submodule(path.into()));
        }
        let status = match fields.get(4).copied() {
            Some("A") => "added",
            Some("D") => "deleted",
            Some("M") => "modified",
            Some("T") => "type_changed",
            _ => return Err(SelectionError::GitMetadata),
        };
        paths.insert(path.to_owned(), status);
    }
    let mut units = Vec::new();
    for (path, status) in paths {
        let mut numstat = diff.clone();
        numstat.extend([
            "--numstat".into(),
            "-z".into(),
            "--".into(),
            path.clone().into(),
        ]);
        if git.run(&numstat)?.starts_with(b"-\t-\t") {
            return Err(SelectionError::NonText(path));
        }
        let mut patch = diff.clone();
        patch.extend([
            "--patch".into(),
            "--full-index".into(),
            "--unified=3".into(),
            "--src-prefix=a/".into(),
            "--dst-prefix=b/".into(),
            "--".into(),
            path.clone().into(),
        ]);
        let bytes = git.run(&patch)?;
        if bytes.len() > limits.file_bytes {
            return Err(SelectionError::Limit {
                resource: crate::Resource::FileBytes,
                limit: limits.file_bytes,
            });
        }
        git.budget.retain(bytes.len())?;
        let location = serde_json::to_string(&(
            root.to_str()
                .ok_or_else(|| SelectionError::PathEncoding(root.clone()))?,
            &comparison,
            &path,
        ))
        .map_err(|_| SelectionError::GitMetadata)?;
        units.push(unit(&path, status, bytes, &location, "git_patch")?);
    }
    git.budget.check_time()?;
    list_inputs(port, "git", units)
}
