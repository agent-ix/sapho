// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Selector tests use temporary files and repositories through the real acquisition paths.
use super::*;
use sapho_core::{ErrorCode, ValueType};
use std::{path::Path, process::Command};
fn units(inputs: &Inputs) -> &[Datum] {
    let Value::List(units) = &inputs["items"].value else {
        panic!("list expected")
    };
    units
}
fn field<'a>(datum: &'a Datum, name: &str) -> &'a str {
    let Value::Record(fields) = &datum.value else {
        panic!("record expected")
    };
    let Value::Text(text) = &fields[name] else {
        panic!("text expected")
    };
    text
}
/// Trace: FR-039-AC-1, FR-039-AC-2, FR-039-AC-3
#[test]
fn files_are_sorted_deduplicated_attributed_and_bounded() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("nested")).unwrap();
    std::fs::create_dir(root.path().join(".git")).unwrap();
    std::fs::write(root.path().join("b.rs"), "second\n").unwrap();
    std::fs::write(root.path().join("a.rs"), "first 🐈\n").unwrap();
    std::fs::write(root.path().join("nested/skip.rs"), "skip").unwrap();
    std::fs::write(root.path().join(".git/private"), "secret").unwrap();
    let patterns = Patterns {
        include: vec!["**/*.rs".into(), "a.rs".into()],
        exclude: vec!["nested/*".into()],
    };
    let inputs =
        select_files(root.path(), &patterns, &SelectionLimits::default(), "items").unwrap();
    assert_eq!(
        units(&inputs)
            .iter()
            .map(|d| field(d, "path"))
            .collect::<Vec<_>>(),
        vec!["a.rs", "b.rs"]
    );
    assert_eq!(field(&units(&inputs)[0], "text"), "first 🐈\n");
    assert_eq!(field(&units(&inputs)[0], "status"), "present");
    assert_eq!(units(&inputs)[0].sources[0].start, Some(0));
    assert_eq!(units(&inputs)[0].sources[0].end, Some(11));
    assert_eq!(
        select_files(root.path(), &patterns, &SelectionLimits::default(), "items").unwrap(),
        inputs
    );
    let empty = select_files(
        root.path(),
        &Patterns {
            include: vec!["missing".into()],
            exclude: vec![],
        },
        &SelectionLimits::default(),
        "items",
    )
    .unwrap();
    assert!(units(&empty).is_empty());
    let limit = SelectionLimits {
        files: 1,
        ..SelectionLimits::default()
    };
    assert!(matches!(
        select_files(root.path(), &patterns, &limit, "items"),
        Err(SelectionError::Limit {
            resource: Resource::Files,
            ..
        })
    ));
    let limit = SelectionLimits {
        file_bytes: 1,
        ..SelectionLimits::default()
    };
    assert!(matches!(
        select_files(root.path(), &patterns, &limit, "items"),
        Err(SelectionError::Limit {
            resource: Resource::FileBytes,
            ..
        })
    ));
    let limit = SelectionLimits {
        entries: 1,
        ..SelectionLimits::default()
    };
    assert!(matches!(
        select_files(root.path(), &patterns, &limit, "items"),
        Err(SelectionError::Limit {
            resource: Resource::Entries,
            ..
        })
    ));
    let limit = SelectionLimits {
        total_bytes: 1,
        ..SelectionLimits::default()
    };
    assert!(matches!(
        select_files(root.path(), &patterns, &limit, "items"),
        Err(SelectionError::Limit {
            resource: Resource::TotalBytes,
            ..
        })
    ));
    let limit = SelectionLimits {
        duration: Duration::from_nanos(1),
        ..SelectionLimits::default()
    };
    assert!(matches!(
        select_files(root.path(), &patterns, &limit, "items"),
        Err(SelectionError::Deadline)
    ));
    std::fs::write(root.path().join("binary.rs"), [0, 1, 2]).unwrap();
    assert!(matches!(
        select_files(root.path(), &patterns, &SelectionLimits::default(), "items"),
        Err(SelectionError::NonText(_))
    ));
}
/// Trace: FR-039-AC-2
#[cfg(unix)]
#[test]
fn filesystem_symlinks_never_expose_outside_content() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), "private").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("linked")).unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret"), root.path().join("file")).unwrap();
    assert!(
        units(
            &select_files(
                root.path(),
                &Patterns::default(),
                &SelectionLimits::default(),
                "items"
            )
            .unwrap()
        )
        .is_empty()
    );
    assert!(matches!(
        select_files(
            &root.path().join("linked"),
            &Patterns::default(),
            &SelectionLimits::default(),
            "items"
        ),
        Err(SelectionError::InvalidRoot(_))
    ));
}
/// Trace: FR-041-AC-1, FR-041-AC-2, FR-041-AC-3
#[test]
fn json_pointer_escapes_types_and_whole_document_sources_are_checked() {
    let bytes = br#"{"a/b":{"~key":[0.2,0.8]},"empty":[]}"#;
    let schema = ValueType::list(ValueType::Probability);
    let selected = select_json(
        bytes,
        "/a~1b/~0key",
        &schema,
        "occurrence:/[]",
        "items",
        "local",
        1024,
    )
    .unwrap();
    let entries = units(&selected);
    assert_eq!(entries.len(), 2);
    assert_ne!(entries[0].id, entries[1].id);
    assert_eq!(
        entries[0].value,
        Value::Probability(sapho_core::Probability::new(0.2).unwrap())
    );
    assert_eq!(
        entries[0].sources[0].end,
        Some(u64::try_from(bytes.len()).unwrap())
    );
    assert!(
        units(&select_json(bytes, "/empty", &schema, "empty", "items", "local", 1024).unwrap())
            .is_empty()
    );
    assert!(matches!(
        select_json(bytes, "/missing", &schema, "x", "items", "local", 1024),
        Err(SelectionError::MissingKey(_))
    ));
    assert!(matches!(
        select_json(bytes, "/a~2b", &schema, "x", "items", "local", 1024),
        Err(SelectionError::PointerSyntax(_))
    ));
    assert!(matches!(
        select_json(
            bytes,
            "/a~1b/~0key/01",
            &schema,
            "x",
            "items",
            "local",
            1024
        ),
        Err(SelectionError::ArrayIndex(_))
    ));
    assert!(matches!(
        select_json(bytes, "/a~1b/~0key/0", &schema, "x", "items", "local", 1024),
        Err(SelectionError::Core(SaphoError {
            code: ErrorCode::TypeMismatch,
            ..
        }))
    ));
    assert!(matches!(
        select_json(
            br#"{"x":1,"x":2}"#,
            "",
            &ValueType::Number,
            "x",
            "items",
            "local",
            1024
        ),
        Err(SelectionError::Core(SaphoError {
            code: ErrorCode::Config,
            ..
        }))
    ));
    assert!(matches!(
        select_json(bytes, "", &schema, "x", "items", "local", 1),
        Err(SelectionError::Core(SaphoError {
            code: ErrorCode::LimitExceeded,
            ..
        }))
    ));
}
fn git(root: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .current_dir(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{:?}: {}",
        args,
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
}
fn repository() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    git(root.path(), &["init", "--quiet"]);
    git(root.path(), &["config", "user.name", "Sapho test"]);
    git(
        root.path(),
        &["config", "user.email", "test@example.invalid"],
    );
    std::fs::write(root.path().join("-dash.txt"), "base\n").unwrap();
    git(root.path(), &["add", "--", "-dash.txt"]);
    git(root.path(), &["commit", "--quiet", "-m", "base"]);
    root
}
/// Trace: FR-040-AC-1, FR-040-AC-2, FR-040-AC-3
#[cfg(unix)]
#[test]
fn git_modes_keep_complete_patches_and_literal_punctuation_paths() {
    let root = repository();
    let base = git(root.path(), &["rev-parse", "HEAD"]);
    std::fs::write(root.path().join("-dash.txt"), "staged\n").unwrap();
    git(root.path(), &["add", "--", "-dash.txt"]);
    std::fs::write(root.path().join("-dash.txt"), "working\n").unwrap();
    std::fs::write(root.path().join("untracked.txt"), "never selected").unwrap();
    let staged = select_git(
        root.path(),
        &GitOptions {
            mode: GitMode::Staged,
            ..GitOptions::default()
        },
        &Patterns::default(),
        &SelectionLimits::default(),
        "items",
    )
    .unwrap();
    assert_eq!(units(&staged).len(), 1);
    assert_eq!(field(&units(&staged)[0], "path"), "-dash.txt");
    assert!(field(&units(&staged)[0], "text").contains("+staged"));
    assert!(!field(&units(&staged)[0], "text").contains("+working"));
    let working = select_git(
        root.path(),
        &GitOptions::default(),
        &Patterns::default(),
        &SelectionLimits::default(),
        "items",
    )
    .unwrap();
    assert_eq!(units(&working).len(), 1);
    assert!(field(&units(&working)[0], "text").contains("+working"));
    git(root.path(), &["commit", "--quiet", "-m", "staged"]);
    let head = git(root.path(), &["rev-parse", "HEAD"]);
    let revisions = select_git(
        root.path(),
        &GitOptions {
            mode: GitMode::Revisions { base, head },
            ..GitOptions::default()
        },
        &Patterns::default(),
        &SelectionLimits::default(),
        "items",
    )
    .unwrap();
    assert!(field(&units(&revisions)[0], "text").contains("+staged"));
    assert!(matches!(
        select_git(
            root.path(),
            &GitOptions {
                mode: GitMode::Revisions {
                    base: "missing-ref".into(),
                    head: "HEAD".into()
                },
                ..GitOptions::default()
            },
            &Patterns::default(),
            &SelectionLimits::default(),
            "items"
        ),
        Err(SelectionError::Revision(_))
    ));
    std::fs::write(root.path().join("-dash.txt"), [0, 1, 2]).unwrap();
    assert!(matches!(
        select_git(
            root.path(),
            &GitOptions::default(),
            &Patterns::default(),
            &SelectionLimits::default(),
            "items"
        ),
        Err(SelectionError::NonText(_))
    ));
    assert!(matches!(
        select_git(
            root.path(),
            &GitOptions {
                program: root.path().join("no-git"),
                ..GitOptions::default()
            },
            &Patterns::default(),
            &SelectionLimits::default(),
            "items"
        ),
        Err(SelectionError::GitUnavailable(_))
    ));
}
/// Trace: FR-040-AC-2
#[cfg(unix)]
#[test]
fn git_submodules_refuse_and_repository_clean_filters_never_execute() {
    let root = repository();
    let head = git(root.path(), &["rev-parse", "HEAD"]);
    git(
        root.path(),
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "160000",
            &head,
            "module",
        ],
    );
    assert!(matches!(
        select_git(
            root.path(),
            &GitOptions {
                mode: GitMode::Staged,
                ..GitOptions::default()
            },
            &Patterns::default(),
            &SelectionLimits::default(),
            "items"
        ),
        Err(SelectionError::Submodule(_))
    ));
    git(root.path(), &["reset", "--quiet", "--", "module"]);
    std::fs::write(root.path().join(".gitattributes"), "*.txt filter=probe\n").unwrap();
    let marker = root.path().join("executed");
    let command = format!("touch {}; cat", marker.display());
    git(root.path(), &["config", "filter.probe.clean", &command]);
    std::fs::write(root.path().join("-dash.txt"), "changed\n").unwrap();
    select_git(
        root.path(),
        &GitOptions::default(),
        &Patterns::default(),
        &SelectionLimits::default(),
        "items",
    )
    .unwrap();
    assert!(!marker.exists());
    git(root.path(), &["config", "--unset", "filter.probe.clean"]);
    let included = root.path().join(".git/filters.conf");
    std::fs::write(
        &included,
        format!("[filter \"probe\"]\n    clean = {command}\n"),
    )
    .unwrap();
    git(
        root.path(),
        &["config", "include.path", included.to_str().unwrap()],
    );
    select_git(
        root.path(),
        &GitOptions::default(),
        &Patterns::default(),
        &SelectionLimits::default(),
        "items",
    )
    .unwrap();
    assert!(
        !marker.exists(),
        "included repository filter must not execute"
    );
}
/// Trace: FR-040-AC-3
#[cfg(target_os = "linux")]
#[test]
fn overproducing_owned_git_process_is_killed_and_reaped() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let program = root.path().join("producer");
    let pid = root.path().join("pid");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\necho $$ > '{}'\nwhile :; do printf 'abcdefghijklmnopqrstuvwxyz'; done\n",
            pid.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let limits = SelectionLimits {
        total_bytes: 64,
        ..SelectionLimits::default()
    };
    assert!(matches!(
        select_git(
            root.path(),
            &GitOptions {
                program,
                ..GitOptions::default()
            },
            &Patterns::default(),
            &limits,
            "items"
        ),
        Err(SelectionError::Limit {
            resource: Resource::TotalBytes,
            ..
        })
    ));
    let process = std::fs::read_to_string(pid).unwrap();
    assert!(!Path::new("/proc").join(process.trim()).exists());
}

/// Trace: FR-040-AC-3
#[cfg(target_os = "linux")]
#[test]
fn started_git_child_is_killed_and_reaped_when_its_deadline_expires() {
    use std::{os::unix::fs::PermissionsExt, time::Duration};
    let root = tempfile::tempdir().unwrap();
    let program = root.path().join("waiting-git");
    let pid = root.path().join("started");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\necho $$ > '{}'\nwhile :; do :; done\n",
            pid.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let result = select_git(
        root.path(),
        &GitOptions {
            program,
            ..GitOptions::default()
        },
        &Patterns::default(),
        &SelectionLimits {
            duration: Duration::from_secs(1),
            ..SelectionLimits::default()
        },
        "items",
    );
    assert!(matches!(result, Err(SelectionError::Deadline)));
    let identity = std::fs::read_to_string(pid).unwrap(); // positive proof the owned child started
    assert!(!Path::new("/proc").join(identity.trim()).exists());
}
