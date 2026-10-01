// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Host-provided native and inference seams (FR-003/006).
use crate::{
    BackendId, ErrorCode, Inputs, ModelRequest, ModelResponse, PrimitiveId, Result, SaphoError,
    Signature, Value,
};
use async_trait::async_trait;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

/// Context for cooperative host-native work.
#[derive(Debug, Clone)]
pub struct PrimitiveContext {
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}
impl PrimitiveContext {
    /// Supply a monotonic deadline and shared cancellation signal.
    pub fn new(deadline: Instant, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            deadline,
            cancelled,
        }
    }
    /// Refuse work after cancellation or deadline.
    pub fn check_cancelled(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Relaxed) || Instant::now() >= self.deadline {
            Err(SaphoError::new(
                ErrorCode::DeadlineExceeded,
                "Native work cancelled or expired",
            ))
        } else {
            Ok(())
        }
    }
}
/// A host-owned synchronous transformation with a captured port contract.
pub trait Primitive: Send + Sync {
    /// Describe exact inputs and outputs without executing domain work.
    fn signature(&self) -> Signature;
    /// Evaluate inputs; native implementations cooperate with cancellation.
    fn execute(
        &self,
        context: &PrimitiveContext,
        inputs: &Inputs,
        params: &BTreeMap<String, Value>,
    ) -> Result<Inputs>;
}
/// Named native implementations; duplicates are errors.
#[derive(Clone, Default)]
pub struct PrimitiveRegistry {
    entries: BTreeMap<PrimitiveId, Arc<dyn Primitive>>,
}
impl PrimitiveRegistry {
    /// Register an implementation without replacing existing behavior.
    pub fn register(&mut self, id: PrimitiveId, primitive: Arc<dyn Primitive>) -> Result<()> {
        id.validate()?;
        if self.entries.contains_key(&id) {
            return Err(SaphoError::new(
                ErrorCode::DuplicateId,
                "Primitive already registered",
            ));
        }
        self.entries.insert(id, primitive);
        Ok(())
    }
    /// Resolve an implementation at compilation time.
    pub fn get(&self, id: &PrimitiveId) -> Result<Arc<dyn Primitive>> {
        self.entries.get(id).cloned().ok_or_else(|| {
            SaphoError::new(
                ErrorCode::UnknownPrimitive,
                "Native primitive not registered",
            )
            .with_context("primitive", id.to_string())
        })
    }
}
/// Replaceable asynchronous model inference; no runtime or transport dependency.
#[async_trait]
pub trait ModelBackend: Send + Sync {
    /// Obtain a raw response; the executor validates it before scoring.
    async fn infer(&self, request: &ModelRequest) -> Result<ModelResponse>;
}
/// One host-configured model binding, with no serializable credentials.
#[derive(Clone)]
pub struct BackendBinding {
    /// Explicit policy for complete-distribution mass interpretation.
    pub distribution_policy: crate::DistributionPolicy,
    /// Actual implementation supplied by the host.
    pub backend: Arc<dyn ModelBackend>,
    /// Model requested by every call through this binding.
    pub model: String,
    /// Optional strict actual-model expectation.
    pub expected_model: Option<String>,
}
/// Named backend bindings with explicit collision refusal.
#[derive(Clone, Default)]
pub struct BackendRegistry {
    entries: BTreeMap<BackendId, BackendBinding>,
}
impl BackendRegistry {
    /// Register a backend and its caller-owned model selection.
    pub fn register(&mut self, id: BackendId, binding: BackendBinding) -> Result<()> {
        id.validate()?;
        binding.distribution_policy.validate()?;
        if binding.model.is_empty()
            || binding
                .expected_model
                .as_ref()
                .is_some_and(|m| m.is_empty())
        {
            return Err(SaphoError::new(
                ErrorCode::InvalidValue,
                "Empty backend model identity",
            ));
        }
        if self.entries.contains_key(&id) {
            return Err(SaphoError::new(
                ErrorCode::DuplicateId,
                "Backend already registered",
            ));
        }
        self.entries.insert(id, binding);
        Ok(())
    }
    /// Resolve a registered model binding without inference.
    pub fn get(&self, id: &BackendId) -> Result<BackendBinding> {
        self.entries.get(id).cloned().ok_or_else(|| {
            SaphoError::new(ErrorCode::UnknownBackend, "Backend not registered")
                .with_context("backend", id.to_string())
        })
    }
}
