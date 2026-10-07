// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Explicit synchronous bounded acquisition/persistence, outside async workers.
use crate::CliError;
use sapho_core::{ErrorCode, SaphoError, SourceId};
use sapho_graph::{GraphFormat, GraphSpec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Exact graph definition with its original location and content identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphArtifact {
    /// Supplied source path.
    pub path: PathBuf,
    /// Path plus SHA-256 of the original representation bytes.
    pub source: SourceId,
    /// Parsed graph, retained for reproducible evidence.
    pub definition: GraphSpec,
}
/// Select the explicit representation or infer a recognized extension; never fall back.
pub fn select_format(path: &Path, explicit: Option<GraphFormat>) -> Result<GraphFormat, CliError> {
    if let Some(format) = explicit {
        return Ok(format);
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some("yaml" | "yml") => Ok(GraphFormat::Yaml),
        Some("json") => Ok(GraphFormat::Json),
        _ => Err(CliError::Format(path.into())),
    }
}
/// Read a regular file or '-' stdin with a 30-second monotonic deadline.
pub fn read_bytes(path: &Path, max_bytes: usize) -> Result<Vec<u8>, CliError> {
    read_bytes_with_timeout(path, max_bytes, Duration::from_secs(30))
}
/// Read under explicit positive byte and time ceilings, before async execution.
/// Unix stdin uses readiness polling without changing shared descriptor flags.
pub fn read_bytes_with_timeout(
    path: &Path,
    max_bytes: usize,
    timeout: Duration,
) -> Result<Vec<u8>, CliError> {
    if max_bytes == 0 || timeout.is_zero() {
        return Err(SaphoError::new(ErrorCode::Config, "Input limits must be positive").into());
    }
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| SaphoError::new(ErrorCode::LimitExceeded, "Input deadline overflow"))?;
    let io_error = |source| CliError::Io {
        path: path.into(),
        source,
    };
    let mut reader: File;
    #[cfg(unix)]
    {
        use rustix::fs::{Mode, OFlags, open};
        reader = if path == Path::new("-") {
            File::from(rustix::io::dup(std::io::stdin()).map_err(|e| io_error(e.into()))?)
        } else {
            File::from(
                open(
                    path,
                    OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|e| io_error(e.into()))?,
            )
        };
    }
    #[cfg(not(unix))]
    {
        if path == Path::new("-") {
            return Err(CliError::UnsupportedStdin);
        }
        reader = File::open(path).map_err(io_error)?;
    }
    if path != Path::new("-") && !reader.metadata().map_err(io_error)?.is_file() {
        return Err(CliError::InputFileType(path.into()));
    }
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| {
                SaphoError::new(
                    ErrorCode::DeadlineExceeded,
                    "Input acquisition deadline exceeded",
                )
            })?;
        #[cfg(unix)]
        {
            use rustix::event::{PollFd, PollFlags, Timespec, poll};
            let timeout = Timespec::try_from(remaining).map_err(|_| {
                SaphoError::new(ErrorCode::LimitExceeded, "Input poll deadline overflow")
            })?;
            match poll(&mut [PollFd::new(&reader, PollFlags::IN)], Some(&timeout)) {
                Ok(0) => {
                    return Err(SaphoError::new(
                        ErrorCode::DeadlineExceeded,
                        "Input acquisition deadline exceeded",
                    )
                    .into());
                }
                Ok(_) => (),
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(io_error(error.into())),
            }
        }
        #[cfg(not(unix))]
        let _ = remaining;
        match reader.read(&mut chunk) {
            Ok(0) => return Ok(bytes),
            Ok(n) => {
                let length = bytes
                    .len()
                    .checked_add(n)
                    .filter(|length| *length <= max_bytes)
                    .ok_or_else(|| {
                        SaphoError::new(ErrorCode::LimitExceeded, "Input exceeds byte ceiling")
                    })?;
                bytes.reserve(length - bytes.len());
                bytes.extend_from_slice(chunk.get(..n).ok_or_else(|| {
                    SaphoError::new(ErrorCode::InvalidValue, "Invalid read length")
                })?);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
                ) =>
            {
                continue;
            }
            Err(error) => return Err(io_error(error)),
        }
    }
}
/// Load a graph from a recognized/explicit format, retaining its exact source identity.
pub fn load_graph(path: &Path, format: Option<GraphFormat>) -> Result<GraphArtifact, CliError> {
    let format = select_format(path, format)?;
    let bytes = read_bytes(path, 1_048_576)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| SaphoError::new(ErrorCode::Config, "Graph must be UTF-8"))?;
    let definition = GraphSpec::parse_with_format(text, format)?;
    let identity = serde_json::to_string(&(path, format!("{:x}", Sha256::digest(&bytes))))
        .map_err(|e| SaphoError::new(ErrorCode::Config, e.to_string()))?;
    Ok(GraphArtifact {
        path: path.into(),
        source: SourceId::new(identity)?,
        definition,
    })
}
/// An exclusively claimed explicit destination; finish consumes the handle exactly once.
pub struct ArtifactWriter {
    file: File,
    path: PathBuf,
}
impl ArtifactWriter {
    /// Claim a destination before evaluation; existing files are never replaced.
    pub fn create(path: &Path) -> Result<Self, CliError> {
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .map_err(|source| CliError::Io {
                path: path.into(),
                source,
            })?;
        Ok(Self {
            file,
            path: path.into(),
        })
    }
    /// Persist complete bounded bytes synchronously after evaluation.
    pub fn finish(mut self, bytes: &[u8], max_bytes: usize) -> Result<(), CliError> {
        if bytes.len() > max_bytes {
            return Err(
                SaphoError::new(ErrorCode::LimitExceeded, "Artifact exceeds byte ceiling").into(),
            );
        }
        self.file
            .write_all(bytes)
            .and_then(|()| self.file.sync_all())
            .map_err(|source| CliError::Io {
                path: self.path,
                source,
            })
    }
}
impl ArtifactWriter {
    /// Persist one JSON document by streaming it, so that a large report is never also held
    /// as one serialized buffer. The size is measured first, and a document above the ceiling
    /// is refused before anything is written.
    pub fn finish_json<T: serde::Serialize>(
        mut self,
        value: &T,
        max_bytes: usize,
    ) -> Result<(), CliError> {
        sapho_core::measured_json_bytes(value, max_bytes)?;
        let path = self.path.clone();
        let io_error = |source| CliError::Io {
            path: path.clone(),
            source,
        };
        let mut sink = std::io::BufWriter::new(&mut self.file);
        serde_json::to_writer(&mut sink, value).map_err(|e| io_error(e.into()))?;
        sink.flush().map_err(io_error)?;
        drop(sink);
        self.file.sync_all().map_err(io_error)
    }
}
/// Exclusive explicit artifact write; an existing destination is preserved.
pub fn write_new(path: &Path, bytes: &[u8], max_bytes: usize) -> Result<(), CliError> {
    ArtifactWriter::create(path)?.finish(bytes, max_bytes)
}
