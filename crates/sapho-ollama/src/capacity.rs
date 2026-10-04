// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Stable OS-backed capacity leases for cooperating local workers.
use crate::{ErrorCode, Result, error};
use fs2::FileExt;
use std::{
    fs::{File, OpenOptions},
    path::Path,
    time::Duration,
};

pub(super) fn prepare(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .mode(0o600);
    }
    #[cfg(not(unix))]
    return Err(error(
        ErrorCode::Config,
        "Shared capacity requires Unix no-follow file support",
    ));
    let file = options
        .open(path)
        .map_err(|_| error(ErrorCode::Config, "Cannot open shared capacity file"))?;
    if !file
        .metadata()
        .map_err(|_| error(ErrorCode::Config, "Cannot inspect capacity file"))?
        .is_file()
    {
        return Err(error(ErrorCode::Config, "Capacity file must be regular"));
    }
    Ok(file)
}

pub(super) struct Lease<'a>(&'a File);
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        let _ = FileExt::unlock(self.0);
    }
}

pub(super) async fn acquire(file: &File) -> Result<Lease<'_>> {
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(Lease(file)),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(_) => {
                return Err(error(
                    ErrorCode::BackendFailed,
                    "Shared capacity lease failed",
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn independent_handles_wait_and_cancel_without_releasing_owner() {
        // Trace: FR-048-AC-5
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capacity.lock");
        let a = prepare(&path).unwrap();
        let b = prepare(&path).unwrap();
        let owner = acquire(&a).await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(60), acquire(&b))
                .await
                .is_err()
        );
        assert!(b.try_lock_exclusive().is_err());
        drop(owner);
        let next = tokio::time::timeout(Duration::from_secs(1), acquire(&b))
            .await
            .unwrap()
            .unwrap();
        drop(next);
        assert!(a.try_lock_exclusive().is_ok());
        FileExt::unlock(&a).unwrap();
    }
    #[test]
    #[ignore = "subprocess helper invoked only by capacity test"]
    fn subprocess_owner() {
        use std::io::Write;
        let path = std::env::var_os("SAPHO_CAPACITY_TEST_PATH").unwrap();
        let file = prepare(Path::new(&path)).unwrap();
        file.try_lock_exclusive().unwrap();
        println!("CAPACITY_OWNED");
        std::io::stdout().flush().unwrap();
        loop {
            std::thread::park();
        }
    }

    #[tokio::test]
    async fn process_exit_releases_shared_capacity() {
        // Trace: FR-048-AC-5
        use std::io::BufRead;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("process.lock");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "capacity::tests::subprocess_owner",
                "--ignored",
                "--nocapture",
            ])
            .env("SAPHO_CAPACITY_TEST_PATH", &path)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let output = child.stdout.take().unwrap();
        let ready = std::thread::spawn(move || {
            for line in std::io::BufReader::new(output).lines() {
                if line.unwrap().contains("CAPACITY_OWNED") {
                    return true;
                }
            }
            false
        });
        // Bound helper observation so a broken helper does not leave a live process.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !ready.is_finished() && std::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        if !ready.is_finished() {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("capacity helper did not acquire its lease");
        }
        assert!(ready.join().unwrap());
        let competitor = prepare(&path).unwrap();
        let excluded = competitor.try_lock_exclusive().is_err();
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(excluded, "separate process must own exclusive capacity");
        let lease = tokio::time::timeout(Duration::from_secs(1), acquire(&competitor))
            .await
            .unwrap()
            .unwrap();
        drop(lease);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_and_directory_are_refused() {
        // Trace: FR-048-AC-6
        let dir = tempfile::tempdir().unwrap();
        let actual = dir.path().join("actual");
        let _file = prepare(&actual).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&actual, &link).unwrap();
        assert!(prepare(&link).is_err());
        assert!(prepare(dir.path()).is_err());
        assert!(prepare(&dir.path().join("missing/lock")).is_err());
    }
}
