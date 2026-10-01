// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Capability-relative no-follow filesystem selection.
use crate::{
    Budget, Matcher, Patterns, SelectionError, SelectionLimits, io, list_inputs, read_bounded, unit,
};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use sapho_core::Inputs;
use std::path::Path;

/// Gather ordinary UTF-8 files under a canonical explicit directory, without following symlinks.
pub fn select_files(
    root: &Path,
    patterns: &Patterns,
    limits: &SelectionLimits,
    port: &str,
) -> Result<Inputs, SelectionError> {
    let mut budget = Budget::new(limits)?;
    let matcher = Matcher::new(patterns)?;
    let meta = std::fs::symlink_metadata(root).map_err(|e| io(root, e))?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(SelectionError::InvalidRoot(root.into()));
    }
    let root = std::fs::canonicalize(root).map_err(|e| io(root, e))?;
    let location = root
        .to_str()
        .ok_or_else(|| SelectionError::PathEncoding(root.clone()))?;
    let dir = Dir::open_ambient_dir(&root, ambient_authority()).map_err(|e| io(&root, e))?;
    let mut pending = vec![(String::new(), dir)];
    let mut units = std::collections::BTreeMap::new();
    while let Some((prefix, dir)) = pending.pop() {
        budget.check_time()?;
        let mut entries = Vec::new();
        for entry in dir.entries().map_err(|e| io(&root, e))? {
            budget.entry()?;
            entries.push(entry.map_err(|e| io(&root, e))?);
        }
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            budget.check_time()?;
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| SelectionError::PathEncoding(root.join(&name)))?;
            if name == ".git" {
                continue;
            }
            let path = if prefix.is_empty() {
                name.into()
            } else {
                format!("{prefix}/{name}")
            };
            let ty = entry.file_type().map_err(|e| io(&path, e))?;
            if ty.is_symlink() {
                continue;
            }
            if ty.is_dir() {
                pending.push((path, dir.open_dir_nofollow(name).map_err(|e| io(name, e))?));
            } else if ty.is_file() && matcher.matches(&path) {
                let mut options = OpenOptions::new();
                options.read(true).follow(FollowSymlinks::No).nonblock(true);
                let file = entry.open_with(&options).map_err(|e| io(&path, e))?;
                if !file.metadata().map_err(|e| io(&path, e))?.is_file() {
                    return Err(SelectionError::UnsupportedFile(path));
                }
                let bytes = read_bounded(file, &path, &budget)?;
                budget.retain(bytes.len())?;
                let identity = serde_json::to_string(&(location, &path))
                    .map_err(|_| SelectionError::GitMetadata)?;
                units.insert(
                    path.clone(),
                    unit(&path, "present", bytes, &identity, "file")?,
                );
            }
        }
    }
    budget.check_time()?;
    list_inputs(port, "files", units.into_values().collect())
}
