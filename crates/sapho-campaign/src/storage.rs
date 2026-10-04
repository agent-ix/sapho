// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Synced immutable artifacts, append-only events and one held writer fence.
use crate::{Error, ErrorCode, Result};
use fs2::FileExt;
use rusqlite::{Connection, OpenFlags, Transaction, params};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NONCE: AtomicU64 = AtomicU64::new(0);
/// Default per-artifact ceiling; callers can explicitly select another bounded policy.
pub const DEFAULT_ARTIFACT_BYTES: usize = 64 * 1_048_576;
/// Content identity for immutable artifacts.
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn regular(path: &Path, write: bool, create: bool) -> Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(write)
        .create(create)
        .truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::new(
            ErrorCode::Refused,
            "artifact is not a regular file",
        ));
    }
    Ok(file)
}
// This field drops after the connection. Explicit unlock prevents a duplicate
// descriptor in a concurrently spawned child prolonging a graceful writer close.
struct WriterFence(File);
impl Drop for WriterFence {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}
/// Fenced local SQLite ledger. Synchronous storage must stay outside async workers.
pub struct Ledger {
    root: PathBuf,
    conn: Connection,
    lock: Option<WriterFence>,
    max_bytes: usize,
}
impl Ledger {
    /// Open the common ledger; only a writer may initialize directories or schema.
    /// `database` is one filename, permitting explicit consumer migration without aliases.
    pub fn open(root: &Path, database: &str, write: bool, max_bytes: usize) -> Result<Self> {
        if max_bytes == 0
            || Path::new(database).components().count() != 1
            || !matches!(
                Path::new(database).components().next(),
                Some(std::path::Component::Normal(_))
            )
        {
            return Err(Error::new(
                ErrorCode::Invalid,
                "positive artifact bound and a database filename required",
            ));
        }
        let root = root.canonicalize()?;
        let lock = if write {
            let f = regular(&root.join("writer.lock"), true, true)?;
            f.try_lock_exclusive()
                .map_err(|_| Error::new(ErrorCode::Conflict, "another writer holds the ledger"))?;
            Some(WriterFence(f))
        } else {
            None
        };
        let db = root.join(database);
        if db.exists() && (!std::fs::symlink_metadata(&db)?.is_file()) {
            return Err(Error::new(
                ErrorCode::Refused,
                "database is not a regular file",
            ));
        }
        let conn = if write {
            Connection::open_with_flags(
                &db,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_NOFOLLOW,
            )?
        } else {
            Connection::open_with_flags(
                &db,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
            )?
        };
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let blobs = root.join("blobs");
        if write {
            if !blobs.exists() {
                std::fs::create_dir(&blobs)?;
            }
            conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS events(seq INTEGER PRIMARY KEY,kind TEXT NOT NULL,subject TEXT NOT NULL,evidence TEXT NOT NULL);")?;
        }
        if !std::fs::symlink_metadata(&blobs)?.is_dir() {
            return Err(Error::new(
                ErrorCode::Refused,
                "blob directory is not a real directory",
            ));
        }
        Ok(Self {
            root,
            conn,
            lock,
            max_bytes,
        })
    }
    /// Canonical state root.
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Borrow a snapshot connection for a trusted compiled domain projection.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }
    /// Borrow the fenced writer connection for a trusted compiled extension.
    /// Extensions must use short transactions and never hold them across await.
    pub fn connection_mut(&mut self) -> Result<&mut Connection> {
        self.require_writer()?;
        Ok(&mut self.conn)
    }
    /// Require held writer ownership before a mutation.
    pub fn require_writer(&self) -> Result<()> {
        if self.lock.is_none() {
            Err(Error::new(ErrorCode::Refused, "read-only ledger"))
        } else {
            Ok(())
        }
    }
    /// Append an event in the same transaction as the operation it evidences.
    pub fn event_tx(tx: &Transaction<'_>, kind: &str, subject: &str, evidence: &str) -> Result<()> {
        tx.execute(
            "INSERT INTO events(kind,subject,evidence) VALUES (?,?,?)",
            params![kind, subject, evidence],
        )?;
        Ok(())
    }
    /// Persist bounded immutable bytes before committing references.
    pub fn blob(&self, bytes: &[u8]) -> Result<String> {
        self.require_writer()?;
        if bytes.len() > self.max_bytes {
            return Err(Error::new(
                ErrorCode::Refused,
                "artifact byte limit exceeded",
            ));
        }
        let hash = digest(bytes);
        let target = self.root.join("blobs").join(&hash);
        if target.try_exists()? {
            self.load(&hash)?;
            return Ok(hash);
        }
        let temp = self.root.join("blobs").join(format!(
            ".{hash}-{}-{}.tmp",
            std::process::id(),
            NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| -> Result<()> {
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
            std::fs::rename(&temp, &target)?;
            File::open(self.root.join("blobs"))?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result?;
        Ok(hash)
    }
    /// Read a bounded regular artifact and verify its content identity.
    pub fn load(&self, hash: &str) -> Result<Vec<u8>> {
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::new(ErrorCode::Invalid, "invalid artifact digest"));
        }
        let file = regular(&self.root.join("blobs").join(hash), false, false)?;
        if file.metadata()?.len() > self.max_bytes as u64 {
            return Err(Error::new(
                ErrorCode::Refused,
                "artifact byte limit exceeded",
            ));
        }
        let mut bytes = Vec::new();
        file.take(self.max_bytes as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > self.max_bytes {
            return Err(Error::new(
                ErrorCode::Refused,
                "artifact grew beyond byte limit",
            ));
        }
        if digest(&bytes) != hash {
            return Err(Error::new(ErrorCode::Storage, "artifact hash mismatch"));
        }
        Ok(bytes)
    }
}

/// Read a bounded regular configuration or inbox file without following symlinks.
pub fn read_file(path: &Path, max_bytes: usize) -> Result<Vec<u8>> {
    if max_bytes == 0 {
        return Err(Error::new(
            ErrorCode::Invalid,
            "positive file bound required",
        ));
    }
    let file = regular(path, false, false)?;
    if file.metadata()?.len() > max_bytes as u64 {
        return Err(Error::new(ErrorCode::Refused, "file byte limit exceeded"));
    }
    let mut bytes = Vec::new();
    file.take(max_bytes as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(Error::new(
            ErrorCode::Refused,
            "file grew beyond byte limit",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writer_lifetime_releases_fence_despite_duplicated_descriptor() {
        // Trace: FR-049-AC-1
        // A descriptor inherited during process spawning has the same OS lock owner.
        let dir = tempfile::tempdir().unwrap();
        let writer =
            Ledger::open(dir.path(), "campaign.sqlite", true, DEFAULT_ARTIFACT_BYTES).unwrap();
        let duplicate = writer.lock.as_ref().unwrap().0.try_clone().unwrap();
        assert!(Ledger::open(dir.path(), "campaign.sqlite", true, DEFAULT_ARTIFACT_BYTES).is_err());
        let reader =
            Ledger::open(dir.path(), "campaign.sqlite", false, DEFAULT_ARTIFACT_BYTES).unwrap();
        drop(reader);
        drop(writer);
        let reopened = Ledger::open(dir.path(), "campaign.sqlite", true, DEFAULT_ARTIFACT_BYTES);
        drop(duplicate);
        assert!(
            reopened.is_ok(),
            "a duplicate descriptor must not prolong a closed ledger's writer lifetime"
        );
    }
}
