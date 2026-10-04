// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Bounded domain-owned inspection content, independent of domain schemas.
use sapho_campaign::{Error, ErrorCode, Result};
/// One read-only panel assembled by a host before terminal rendering.
#[derive(Debug, Clone)]
pub struct Panel {
    /// Human-readable title, bounded to256UTF8bytes.
    pub title: String,
    /// Complete host-supplied body, bounded to4MiB; never silently truncated.
    pub body: String,
}
/// Validate a bounded collection before retaining it for a frame.
pub(crate) fn validate(panels: &[Panel]) -> Result<()> {
    if panels.len() > 16
        || panels.iter().any(|p| {
            p.title.is_empty()
                || p.title.len() > 256
                || p.title.chars().any(char::is_control)
                || p.body.len() > 4 * 1_048_576
        })
    {
        return Err(Error::new(
            ErrorCode::Invalid,
            "inspection panel exceeds title/body/count bounds",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_panels_are_bounded_without_truncation() {
        // Trace: FR-052-AC-5
        let p = Panel {
            title: "Synthetic domain".into(),
            body: "full owning context".into(),
        };
        assert!(validate(std::slice::from_ref(&p)).is_ok());
        assert!(validate(&vec![p.clone(); 17]).is_err());
        assert!(
            validate(&[Panel {
                body: "x".repeat(4 * 1_048_576 + 1),
                ..p.clone()
            }])
            .is_err()
        );
        assert!(
            validate(&[Panel {
                title: "bad\nlabel".into(),
                ..p
            }])
            .is_err()
        );
    }
}
