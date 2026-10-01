// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Owned bounded Git processes; nonblocking pipes avoid stranded reader threads.
use crate::{Budget, SelectionError};
use std::{ffi::OsString, path::Path};
pub(crate) struct Output {
    pub(crate) bytes: Vec<u8>,
    pub(crate) status: Option<i32>,
    pub(crate) stderr: String,
}
#[cfg(unix)]
pub(crate) fn execute(
    program: &Path,
    root: &Path,
    args: &[OsString],
    budget: &Budget<'_>,
) -> Result<Output, SelectionError> {
    use crate::{Resource, add, io};
    use rustix::{
        fs::{OFlags, fcntl_getfl, fcntl_setfl},
        process::{Pid, Signal, kill_process_group},
    };
    use std::{
        io::Read,
        os::unix::process::CommandExt,
        process::{Child, Command, Stdio},
        time::Duration,
    };
    struct OwnedChild {
        child: Child,
        group: Option<Pid>,
        armed: bool,
    }
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            if self.armed {
                if let Some(group) = self.group {
                    let _ = kill_process_group(group, Signal::KILL);
                }
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }
    fn drain(
        reader: &mut impl Read,
        bytes: &mut Vec<u8>,
        limit: usize,
        resource: Resource,
        budget: &Budget<'_>,
    ) -> Result<bool, SelectionError> {
        let mut chunk = [0u8; 8192];
        loop {
            budget.check_time()?;
            match reader.read(&mut chunk) {
                Ok(0) => return Ok(true),
                Ok(n) => {
                    add(bytes.len(), n, limit, resource)?;
                    bytes.extend_from_slice(chunk.get(..n).ok_or(SelectionError::GitMetadata)?);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(io("git pipe", e)),
            }
        }
    }
    budget.check_time()?;
    let child = Command::new(program)
        .current_dir(root)
        .args(args)
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env_remove("GIT_CONFIG_COUNT")
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env_remove("GIT_CONFIG")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .env_remove("GIT_EXTERNAL_DIFF")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(SelectionError::GitUnavailable)?;
    let mut owned = OwnedChild {
        child,
        group: None,
        armed: true,
    };
    owned.group = Some(
        i32::try_from(owned.child.id())
            .ok()
            .and_then(Pid::from_raw)
            .ok_or(SelectionError::GitMetadata)?,
    );
    let mut stdout = owned
        .child
        .stdout
        .take()
        .ok_or(SelectionError::GitMetadata)?;
    let mut stderr = owned
        .child
        .stderr
        .take()
        .ok_or(SelectionError::GitMetadata)?;
    for flags in [
        fcntl_getfl(&stdout).and_then(|flags| fcntl_setfl(&stdout, flags | OFlags::NONBLOCK)),
        fcntl_getfl(&stderr).and_then(|flags| fcntl_setfl(&stderr, flags | OFlags::NONBLOCK)),
    ] {
        flags.map_err(|e| io("git pipe", e.into()))?;
    }
    let mut bytes = Vec::new();
    let mut diagnostics = Vec::new();
    loop {
        let a = drain(
            &mut stdout,
            &mut bytes,
            budget.limits.total_bytes,
            Resource::TotalBytes,
            budget,
        )?;
        let b = drain(
            &mut stderr,
            &mut diagnostics,
            budget.limits.stderr_bytes,
            Resource::StderrBytes,
            budget,
        )?;
        if a && b
            && let Some(status) = owned.child.try_wait().map_err(|e| io("git child", e))?
        {
            owned.armed = false;
            return Ok(Output {
                bytes,
                status: status.code(),
                stderr: String::from_utf8_lossy(&diagnostics).into_owned(),
            });
        }
        budget.check_time()?;
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[cfg(not(unix))]
pub(crate) fn execute(
    _program: &Path,
    _root: &Path,
    _args: &[OsString],
    _budget: &Budget<'_>,
) -> Result<Output, SelectionError> {
    Err(SelectionError::UnsupportedPlatform)
}
