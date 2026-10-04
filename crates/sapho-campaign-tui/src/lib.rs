// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Agent-IX
//! Attached terminal UI. Rendering never performs inference or derives domain counts.
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Block, Paragraph, Wrap},
};
use sapho_campaign::{
    Result,
    adapter::DomainSnapshot,
    control::Control,
    lifecycle::{Attempt, CampaignSnapshot},
};
use std::{io::IsTerminal, time::Duration};
/// Read-only projection supplied by a generic or domain command host.
pub struct View {
    /// Generic authoritative snapshot.
    pub campaign: CampaignSnapshot,
    /// Domain metrics with their owning meanings.
    pub domain: DomainSnapshot,
    /// Retained attempts for inspection and guarded retry.
    pub attempts: Vec<Attempt>,
}
struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        ratatui::restore();
    }
}
/// Attach to a running or idle campaign. `read` uses snapshot readers; `submit`
/// queues a control and returns its receipt identity, without becoming a writer.
/// Closing the UI leaves the headless worker alone.
pub fn attach<R, C>(mut read: R, mut submit: C) -> Result<()>
where
    R: FnMut() -> Result<View>,
    C: FnMut(&Control) -> Result<String>,
{
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err(sapho_campaign::Error::new(
            sapho_campaign::ErrorCode::Refused,
            "dashboard requires a terminal; use status for non-TTY monitoring",
        ));
    }
    let _restore = Restore;
    let mut terminal = ratatui::try_init()?;
    let mut selected = 0usize;
    let mut scroll = 0u16;
    let mut detail_scroll = 0u16;
    let mut inspect_focus = false;
    let mut reason: Option<(
        sapho_campaign::lifecycle::AttemptId,
        sapho_campaign::lifecycle::AttemptState,
        String,
    )> = None;
    let mut notice = String::new();
    loop {
        let view = read()?;
        selected = selected.min(view.attempts.len().saturating_sub(1));
        terminal.draw(|frame| {
            let areas = Layout::vertical([
                Constraint::Length(3),
                Constraint::Percentage(35),
                Constraint::Min(4),
                Constraint::Length(5),
            ])
            .split(frame.area());
            let c = &view.campaign;
            frame.render_widget(
                Paragraph::new(format!(
                    "Campaign | jobs {} | event {} | control {} | paused {}",
                    c.jobs, c.sequence, c.control_revision, c.paused
                ))
                .block(Block::bordered()),
                areas[0],
            );
            let metrics = view
                .domain
                .metrics
                .values()
                .map(|m| {
                    format!(
                        "{}: {}{}",
                        m.label,
                        m.value,
                        m.target.map(|t| format!(" / {t}")).unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            frame.render_widget(
                Paragraph::new(format!(
                    "Attempts: {:?}\nDomain completion: {:?}\n{}",
                    c.attempts, view.domain.complete, metrics
                ))
                .scroll((scroll, 0))
                .block(Block::bordered().title("Observed counts / domain evidence")),
                areas[1],
            );
            let detail = view.attempts.get(selected)
                .map(|attempt| attempt_detail(attempt,selected,view.attempts.len()))
                .unwrap_or_else(|| "No attempts".into());
            render_inspection(frame,areas[2],&detail,detail_scroll,inspect_focus);
            let footer = reason
                .as_ref()
                .map(|(_, _, r)| format!("Retry reason: {r}\nEnter submit | Esc cancel"))
                .unwrap_or_else(|| {
                    let help = "q detach | p pause | arrows inspect | Tab focus | PgUp/Dn scroll | r retry";
                    if notice.is_empty() {
                        help.into()
                    } else {
                        format!("{notice}\n{help}")
                    }
                });
            frame.render_widget(
                Paragraph::new(footer)
                    .wrap(Wrap { trim: true })
                    .block(Block::bordered()),
                areas[3],
            );
        })?;
        if !crossterm::event::poll(Duration::from_millis(400))? {
            continue;
        }
        let Event::Key(key) = crossterm::event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if let Some((attempt, expected, text)) = reason.as_mut() {
            match key.code {
                KeyCode::Esc => reason = None,
                KeyCode::Backspace => {
                    text.pop();
                }
                KeyCode::Char(c) if !c.is_control() && text.len() + c.len_utf8() <= 1024 => {
                    text.push(c)
                }
                KeyCode::Enter => {
                    let control = Control::Retry {
                        attempt: *attempt,
                        expected: *expected,
                        reason: text.clone(),
                    };
                    notice = match submit(&control) {
                        Ok(id) => format!("queued {id}"),
                        Err(e) => e.to_string(),
                    };
                    reason = None;
                }
                _ => {}
            }
            continue;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Up => {
                selected = selected.saturating_sub(1);
                detail_scroll = 0;
            }
            KeyCode::Down => {
                selected = selected
                    .saturating_add(1)
                    .min(view.attempts.len().saturating_sub(1));
                detail_scroll = 0;
            }
            KeyCode::Tab => inspect_focus = !inspect_focus,
            KeyCode::Home => {
                if inspect_focus {
                    detail_scroll = 0;
                } else {
                    scroll = 0;
                }
            }
            KeyCode::PageDown => {
                if inspect_focus {
                    detail_scroll = detail_scroll.saturating_add(5);
                } else {
                    scroll = scroll.saturating_add(5);
                }
            }
            KeyCode::PageUp => {
                if inspect_focus {
                    detail_scroll = detail_scroll.saturating_sub(5);
                } else {
                    scroll = scroll.saturating_sub(5);
                }
            }
            KeyCode::Char('p') => {
                notice = match submit(&Control::Pause {
                    paused: !view.campaign.paused,
                    revision: view.campaign.control_revision,
                }) {
                    Ok(id) => format!("queued {id}"),
                    Err(e) => e.to_string(),
                };
            }
            KeyCode::Char('r')
                if view
                    .attempts
                    .get(selected)
                    .is_some_and(|a| a.state.retryable()) =>
            {
                if let Some(a) = view.attempts.get(selected) {
                    reason = Some((a.id, a.state, String::new()));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn attempt_detail(attempt: &Attempt, selected: usize, count: usize) -> String {
    format!(
        "Attempt {} ({} / {})\nJob {} | stage {} | {:?}\nRetry parent: {}\nRetry reason: {}\nRequest {}\nResult {}",
        attempt.id.get(),
        selected + 1,
        count,
        attempt.job.as_str(),
        attempt.stage.as_str(),
        attempt.state,
        attempt
            .parent
            .map(|parent| parent.get().to_string())
            .unwrap_or_else(|| "none".into()),
        attempt.reason.as_deref().unwrap_or("none"),
        attempt.request,
        attempt.response.as_deref().unwrap_or("not captured"),
    )
}
fn render_inspection(
    frame: &mut ratatui::Frame<'_>,
    area: ratatui::layout::Rect,
    detail: &str,
    scroll: u16,
    focused: bool,
) {
    frame.render_widget(
        Paragraph::new(detail)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0))
            .block(Block::bordered().title(if focused {
                "Attempt inspection [scroll]"
            } else {
                "Attempt inspection"
            })),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use sapho_campaign::lifecycle::{AttemptId, AttemptState, JobId, StageId};

    #[test]
    fn narrow_inspection_preserves_actual_identity_lineage_and_reachable_long_reason() {
        let attempt = Attempt {
            id: AttemptId::new(77).unwrap(),
            job: JobId::new("source-job").unwrap(),
            stage: StageId::new("classify").unwrap(),
            parent: Some(AttemptId::new(31).unwrap()),
            reason: Some(format!(
                "{}reason-end-marker",
                "source grounded reason ".repeat(30)
            )),
            state: AttemptState::Failed,
            request: "a".repeat(64),
            response: Some("b".repeat(64)),
        };
        let detail = attempt_detail(&attempt, 0, 1);
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(40, 8)).unwrap();
        let mut observed = String::new();
        for scroll in 0..60 {
            terminal
                .draw(|frame| render_inspection(frame, frame.area(), &detail, scroll, true))
                .unwrap();
            for cell in &terminal.backend().buffer().content {
                observed.push_str(cell.symbol());
            }
        }
        assert!(observed.contains("Attempt 77 (1 / 1)"));
        assert!(observed.contains("Retry parent: 31"));
        assert!(observed.contains("reason-end-marker"));
        assert!(observed.contains(&"a".repeat(38)));
        assert!(observed.contains(&"b".repeat(38)));
        assert!(observed.contains("[scroll]"));
    }
}
