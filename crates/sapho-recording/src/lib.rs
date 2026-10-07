// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Caller-controlled recording and exact replay (FR-027/028/029).
//! Synchronous file methods are explicit host actions, never invoked by infer.
//!
//! # Bounded serialization and offline replay
//!
//! Wrap a host backend in [`RecordingBackend`], retain its handle and call
//! [`RecordingBackend::snapshot`] after evaluation. Only successful validated
//! exchanges become recordings; raw invalid responses remain in runtime traces.
//!
//! ```
//! use sapho_recording::{Recording, ReplayBackend};
//! let recording = Recording::default(); // A graph with no model calls.
//! let bytes = recording.to_json(1024)?;
//! let loaded = Recording::from_json(&bytes, 1024)?;
//! assert_eq!(loaded, recording);
//! let _backend = ReplayBackend::new(&loaded, 1024)?;
//! # Ok::<(), sapho_core::SaphoError>(())
//! ```
//!
//! Replay matches exact model, binding, policy, state and ordered questions.
//! It has no live fallback. Threshold-only changes can reuse unchanged requests.
//! File helpers are synchronous host actions and refuse overwriting existing files.
use async_trait::async_trait;
use sapho_core::{
    ErrorCode, ModelBackend, ModelRequest, ModelResponse, Result, SaphoError, bounded_json,
    measured_json_bytes, validate_response,
};
use serde::{Deserialize, Serialize, ser::SerializeSeq};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex},
};

/// One successful backend exchange, with exact ordered question definitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exchange {
    /// Exact reconstructed request and binding identity.
    pub request: ModelRequest,
    /// Raw response, validated before retention.
    pub response: ModelResponse,
}
/// Typed recording export; no model outputs or private examples are bundled.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recording {
    /// Explicitly retained successful exchanges.
    pub exchanges: Vec<Exchange>,
}
impl Recording {
    /// Export original typed exchanges under a byte ceiling.
    pub fn to_json(&self, max_bytes: usize) -> Result<Vec<u8>> {
        self.validate()?;
        bounded_json(self, max_bytes)
    }
    /// Load bytes only after checking their explicit size ceiling.
    pub fn from_json(bytes: &[u8], max_bytes: usize) -> Result<Self> {
        if bytes.len() > max_bytes {
            return Err(limit("Recording exceeds byte ceiling"));
        }
        let recording: Self = sapho_core::decode_json(bytes, max_bytes).map_err(|e| {
            if e.code == ErrorCode::Config {
                SaphoError::new(ErrorCode::RecordingMismatch, e.message)
            } else {
                e
            }
        })?;
        recording.validate()?;
        Ok(recording)
    }
    /// Validate exchange semantics without trusting the saved answers.
    pub fn validate(&self) -> Result<()> {
        for e in &self.exchanges {
            validate_response(&e.request, &e.response).map_err(|e| {
                SaphoError::new(ErrorCode::RecordingMismatch, "Invalid recorded exchange")
                    .with_context("cause", format!("{:?}", e.code))
            })?;
        }
        Ok(())
    }
    /// Explicit synchronous exclusive write; an existing path is never overwritten.
    pub fn write_new(&self, path: impl AsRef<Path>, max_bytes: usize) -> Result<()> {
        let bytes = self.to_json(max_bytes)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(io_error)?;
        file.write_all(&bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)
    }
    /// Explicit synchronous read with a bounded buffer, for use outside async workers.
    pub fn read(path: impl AsRef<Path>, max_bytes: usize) -> Result<Self> {
        let max = max_bytes
            .checked_add(1)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| limit("Recording byte ceiling overflow"))?;
        let file = std::fs::File::open(path).map_err(io_error)?;
        let mut bytes = Vec::new();
        file.take(max).read_to_end(&mut bytes).map_err(io_error)?;
        Self::from_json(&bytes, max_bytes)
    }
}
/// Backend decorator retaining only successful validated exchanges in bounded memory.
pub struct RecordingBackend {
    delegate: Arc<dyn ModelBackend>,
    recording: Mutex<Recording>,
    max_bytes: usize,
}
impl RecordingBackend {
    /// Configure the live/scripted delegate and a finite recording memory ceiling.
    pub fn new(delegate: Arc<dyn ModelBackend>, max_bytes: usize) -> Result<Self> {
        if max_bytes == 0 {
            return Err(limit("Recording ceiling is zero"));
        }
        Ok(Self {
            delegate,
            recording: Mutex::new(Recording::default()),
            max_bytes,
        })
    }
    /// Obtain a typed snapshot; the caller decides whether and where to export it.
    pub fn snapshot(&self) -> Result<Recording> {
        self.recording
            .lock()
            .map(|r| r.clone())
            .map_err(|_| SaphoError::new(ErrorCode::RecordingIo, "Recording lock poisoned"))
    }
}
struct Appended<'a> {
    current: &'a [Exchange],
    next: &'a Exchange,
}
impl Serialize for Appended<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(None)?;
        for exchange in self.current {
            sequence.serialize_element(exchange)?;
        }
        sequence.serialize_element(self.next)?;
        sequence.end()
    }
}
#[derive(Serialize)]
struct Preview<'a> {
    exchanges: Appended<'a>,
}
#[async_trait]
impl ModelBackend for RecordingBackend {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        request.validate()?;
        let response = self.delegate.infer(request).await?;
        // Invalid raw answers still reach the executor, where their evidence is retained.
        if validate_response(request, &response).is_err() {
            return Ok(response);
        }
        let exchange = Exchange {
            request: request.clone(),
            response: response.clone(),
        };
        let mut recording = self
            .recording
            .lock()
            .map_err(|_| SaphoError::new(ErrorCode::RecordingIo, "Recording lock poisoned"))?;
        measured_json_bytes(
            &Preview {
                exchanges: Appended {
                    current: &recording.exchanges,
                    next: &exchange,
                },
            },
            self.max_bytes,
        )?;
        recording.exchanges.push(exchange);
        Ok(response)
    }
}
/// Offline backend with no live delegate, credential lookup or transport path.
pub struct ReplayBackend {
    exchanges: BTreeMap<Fingerprint, Vec<u8>>,
    max_bytes: usize,
}
/// SHA-256 of a request's canonical JSON; the index never holds the requests themselves.
type Fingerprint = [u8; 32];
fn fingerprint(request: &ModelRequest, max_bytes: usize) -> Result<Fingerprint> {
    use sha2::{Digest, Sha256};
    Ok(Sha256::digest(bounded_json(request, max_bytes)?).into())
}
fn stored(response: &ModelResponse, max_bytes: usize) -> Result<Vec<u8>> {
    bounded_json(response, max_bytes)
}
fn restored(bytes: &[u8]) -> Result<ModelResponse> {
    serde_json::from_slice(bytes).map_err(|_| {
        SaphoError::new(
            ErrorCode::RecordingMismatch,
            "Recorded response is unreadable",
        )
    })
}
impl ReplayBackend {
    /// Validate and index recorded requests; conflicting duplicate responses refuse loading.
    pub fn new(recording: &Recording, max_bytes: usize) -> Result<Self> {
        recording.validate()?;
        measured_json_bytes(recording, max_bytes)?;
        let mut index = Index::new(max_bytes, true);
        for exchange in &recording.exchanges {
            index.insert(exchange)?;
        }
        Ok(index.finish())
    }
    /// Load a recording straight into the replay index, one exchange at a time.
    ///
    /// The result is the same as [`Recording::from_json`] followed by [`ReplayBackend::new`],
    /// without ever holding the whole typed [`Recording`], so a recording of many exchanges
    /// costs memory for its index only. `each` sees every exchange as it is read, which lets a
    /// caller infer binding identities without a second pass.
    pub fn from_json(
        bytes: &[u8],
        max_bytes: usize,
        each: impl FnMut(&Exchange) -> Result<()>,
    ) -> Result<Self> {
        load(bytes, max_bytes, each, true).map(Index::finish)
    }
    /// Check that a recording is well formed and that every exchange is valid, exactly as
    /// [`Recording::from_json`] does, without indexing anything. Conflicts between exchanges
    /// are the indexing step's refusals and are not found here.
    pub fn check_json(bytes: &[u8], max_bytes: usize) -> Result<()> {
        load(bytes, max_bytes, |_| Ok(()), false).map(|_| ())
    }
}
fn load(
    bytes: &[u8],
    max_bytes: usize,
    mut each: impl FnMut(&Exchange) -> Result<()>,
    indexed: bool,
) -> Result<Index> {
    {
        if bytes.len() > max_bytes {
            return Err(limit("Recording exceeds byte ceiling"));
        }
        let mismatch = |e: SaphoError| {
            if e.code == ErrorCode::Config {
                SaphoError::new(ErrorCode::RecordingMismatch, e.message)
            } else {
                e
            }
        };
        // Depth, duplicate keys and malformed JSON are refused exactly as `from_json` does.
        sapho_core::decode_json::<serde::de::IgnoredAny>(bytes, max_bytes).map_err(mismatch)?;
        let mut decoder = serde_json::Deserializer::from_slice(bytes);
        decoder.disable_recursion_limit();
        let failure = std::cell::RefCell::new(None);
        let loaded = serde::de::Deserializer::deserialize_struct(
            &mut decoder,
            "Recording",
            &["exchanges"],
            Load {
                index: std::cell::RefCell::new(Index::new(max_bytes, indexed)),
                each: std::cell::RefCell::new(&mut each),
                failure: &failure,
            },
        );
        if let Some(error) = failure.into_inner() {
            return Err(error);
        }
        loaded
            .and_then(|index| decoder.end().map(|()| index))
            .map_err(|e| mismatch(SaphoError::new(ErrorCode::Config, e.to_string())))
    }
}
/// The request-keyed answers of a recording while it is being read.
struct Index {
    exchanges: BTreeMap<Fingerprint, Vec<u8>>,
    max_bytes: usize,
    /// Whether exchanges are indexed; a check only validates them.
    indexed: bool,
}
impl Index {
    fn new(max_bytes: usize, indexed: bool) -> Self {
        Self {
            exchanges: BTreeMap::new(),
            max_bytes,
            indexed,
        }
    }
    fn insert(&mut self, exchange: &Exchange) -> Result<()> {
        if !self.indexed {
            return Ok(());
        }
        let key = fingerprint(&exchange.request, self.max_bytes)?;
        if let Some(previous) = self.exchanges.get(&key) {
            // Provider bodies differ between identical requests (timestamps, durations),
            // so the retained raw exchange is not part of the answer being compared.
            let bare = |r: &ModelResponse| ModelResponse {
                raw: None,
                ..r.clone()
            };
            if bare(&restored(previous)?) != bare(&exchange.response) {
                return Err(SaphoError::new(
                    ErrorCode::RecordingMismatch,
                    "Conflicting response for identical request",
                ));
            }
        } else {
            self.exchanges
                .insert(key, stored(&exchange.response, self.max_bytes)?);
        }
        Ok(())
    }
    fn finish(self) -> ReplayBackend {
        ReplayBackend {
            exchanges: self.exchanges,
            max_bytes: self.max_bytes,
        }
    }
}
/// Reads the one `exchanges` member of a recording, indexing each exchange as it arrives.
struct Load<'a, F> {
    index: std::cell::RefCell<Index>,
    each: std::cell::RefCell<&'a mut F>,
    failure: &'a std::cell::RefCell<Option<SaphoError>>,
}
impl<'de, F: FnMut(&Exchange) -> Result<()>> serde::de::Visitor<'de> for Load<'_, F> {
    type Value = Index;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a recording")
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(
        self,
        mut map: A,
    ) -> std::result::Result<Index, A::Error> {
        let mut seen = false;
        while let Some(key) = map.next_key::<String>()? {
            if key != "exchanges" {
                return Err(serde::de::Error::unknown_field(&key, &["exchanges"]));
            }
            seen = true;
            map.next_value_seed(Exchanges(&self))?;
        }
        if !seen {
            return Err(serde::de::Error::missing_field("exchanges"));
        }
        Ok(self.index.into_inner())
    }
}
struct Exchanges<'a, 'b, F>(&'a Load<'b, F>);
impl<'de, F: FnMut(&Exchange) -> Result<()>> serde::de::DeserializeSeed<'de>
    for Exchanges<'_, '_, F>
{
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        decoder: D,
    ) -> std::result::Result<(), D::Error> {
        decoder.deserialize_seq(self)
    }
}
impl<'de, F: FnMut(&Exchange) -> Result<()>> serde::de::Visitor<'de> for Exchanges<'_, '_, F> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a sequence of exchanges")
    }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> std::result::Result<(), A::Error> {
        while let Some(exchange) = seq.next_element::<Exchange>()? {
            let outcome = validate_response(&exchange.request, &exchange.response)
                .map(|_| ())
                .map_err(|e| {
                    SaphoError::new(ErrorCode::RecordingMismatch, "Invalid recorded exchange")
                        .with_context("cause", format!("{:?}", e.code))
                })
                .and_then(|()| (self.0.each.borrow_mut())(&exchange))
                .and_then(|()| self.0.index.borrow_mut().insert(&exchange));
            if let Err(error) = outcome {
                *self.0.failure.borrow_mut() = Some(error);
                return Err(serde::de::Error::custom("Recording refused"));
            }
        }
        Ok(())
    }
}
#[async_trait]
impl ModelBackend for ReplayBackend {
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse> {
        request.validate()?;
        let key = fingerprint(request, self.max_bytes)?;
        let recorded = self.exchanges.get(&key).ok_or_else(|| {
            SaphoError::new(ErrorCode::ReplayMiss, "No exact recorded request matches")
        })?;
        restored(recorded)
    }
}
fn limit(message: &str) -> SaphoError {
    SaphoError::new(ErrorCode::LimitExceeded, message)
}
fn io_error(error: std::io::Error) -> SaphoError {
    SaphoError::new(ErrorCode::RecordingIo, error.to_string())
}
