use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, TableState, Wrap,
    },
    Frame,
};

use super::app::{App, Focus, Mode, SECRET_FIELDS, TAG_FIELDS};

pub fn render(f: &mut Frame, app: &App) {
    match &app.mode {
        Mode::Invite => render_invite(f, app),
        Mode::NewSecret | Mode::EditSecret | Mode::NewTag | Mode::EditTag => render_form(f, app),
        Mode::List => render_list(f, app),
    }
}

// --- List (main) view ---

fn render_list(f: &mut Frame, app: &App) {
    let area = f.area();

    // Outer vertical split: header | body | footer
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(f, app, main_chunks[0]);

    // Body: secrets on top, members on bottom
    let body_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(8)])
        .split(main_chunks[1]);

    // Secrets (left) | Tags (right, hidden when no secrets)
    let pane_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(if app.secrets.is_empty() {
            vec![Constraint::Percentage(100), Constraint::Percentage(0)]
        } else {
            vec![Constraint::Percentage(75), Constraint::Percentage(25)]
        })
        .split(body_chunks[0]);

    render_secrets(f, app, pane_chunks[0]);
    if !app.secrets.is_empty() {
        render_tags(f, app, pane_chunks[1]);
    }
    render_members(f, app, body_chunks[1]);

    render_footer(f, app, main_chunks[2]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    let lines = vec![
        Line::from(Span::styled(
            format!("Envi · v{VERSION}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::raw(app.device_name.clone())),
        Line::from(Span::raw(format!(
            "{} · {}",
            app.vault_name.clone(),
            app.storage_backend.clone()
        ))),
    ];
    let block = Block::default()
        .title(" Envisible ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    if let Some(tag) = &app.tag_to_delete {
        let line = Line::from(vec![Span::styled(
            format!("Delete tag '{tag}'? [y] Yes  [n] No"),
            Style::default().fg(Color::Yellow),
        )]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }
    if let Some(id) = &app.secret_to_delete {
        let name = app
            .secrets
            .iter()
            .find(|s| &s.id == id)
            .map(|s| s.name.as_str())
            .unwrap_or("secret");
        let line = Line::from(vec![Span::styled(
            format!("Delete secret '{name}'? [y] Yes  [n] No"),
            Style::default().fg(Color::Yellow),
        )]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }

    // Member confirmation dialogs take priority
    if let Some(id) = &app.member_to_delete {
        let email = app
            .members
            .iter()
            .find(|m| &m.id == id)
            .map(|m| m.email.as_str())
            .unwrap_or("member");
        let line = Line::from(vec![Span::styled(
            format!("Remove {email}? [y] Yes  [n] No"),
            Style::default().fg(Color::Yellow),
        )]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }
    if app.confirming_rotate {
        let line = Line::from(vec![Span::styled(
            "Rotate DEK? All secrets will be re-encrypted. [y] Yes  [n] No",
            Style::default().fg(Color::Yellow),
        )]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }
    if let Some(id) = &app.member_to_grant {
        let email = app
            .members
            .iter()
            .find(|m| &m.id == id)
            .map(|m| m.email.as_str())
            .unwrap_or("member");
        let line = Line::from(vec![Span::styled(
            format!("Grant access to {email}? [y] Yes  [n] No"),
            Style::default().fg(Color::Yellow),
        )]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }

    let hint = match &app.focus {
        Focus::Secrets => {
            if app.editing_tag.is_some() {
                "[Space] Toggle  [Enter] Save  [Esc] Cancel"
            } else {
                "[n] New  [e] Edit  [d] Delete  [c] Copy value  [v] Hold to reveal  [Tab] Switch  [q] Quit"
            }
        }
        Focus::Tags => "[n] New  [e] Rename  [s] Secrets  [d] Delete  [Tab] Switch  [q] Quit",
        Focus::Members => {
            let selected_is_pending = app
                .members
                .get(app.member_idx)
                .map(|m| m.wrapped_dek.is_empty())
                .unwrap_or(false);
            if selected_is_pending {
                "[g] Grant access  [d] Remove selected  [i] Invite new member  [Tab] Switch  [q] Quit"
            } else {
                "[d] Remove selected  [i] Invite new member  [Tab] Switch  [q] Quit"
            }
        }
    };

    let copied_recently = app
        .copied_at
        .map(|t| t.elapsed().as_secs() < 2)
        .unwrap_or(false);

    let (sync_text, sync_color) = if copied_recently {
        ("✓ copied", Color::Green)
    } else if app.syncing {
        ("↑ syncing…", Color::Yellow)
    } else {
        ("✓ saved", Color::DarkGray)
    };
    // Reserve enough width for the longer of the two status strings.
    let status_width = 10u16;

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(status_width)])
        .split(area);

    f.render_widget(
        Paragraph::new(hint).style(Style::default().fg(Color::DarkGray)),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new(sync_text)
            .style(Style::default().fg(sync_color))
            .alignment(Alignment::Right),
        chunks[1],
    );
}

fn render_members(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Members;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let items: Vec<ListItem> = app
        .members
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let is_me = m.id == app.session.member_id;
            let is_selected = i == app.member_idx && focused;
            let pending = m.wrapped_dek.is_empty();

            let label = if is_me {
                format!("{} (you)", m.email)
            } else if pending {
                format!("{} [pending]", m.email)
            } else {
                m.email.clone()
            };

            let style = if is_selected {
                Style::default().bg(Color::Cyan).fg(Color::Black)
            } else if pending {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            ListItem::new(label).style(style)
        })
        .collect();

    let scroll = scroll_indicators(app.member_idx, app.members.len(), area.height as usize, 2);
    let block = Block::default()
        .title(format!(" Members ({}) {}", app.members.len(), scroll))
        .borders(Borders::ALL)
        .border_style(border_style);

    let mut state = ListState::default();
    if focused && !app.members.is_empty() {
        state.select(Some(app.member_idx));
    }

    f.render_stateful_widget(List::new(items).block(block), area, &mut state);
}

fn render_tags(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Tags;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let items: Vec<ListItem> = app
        .tags
        .iter()
        .enumerate()
        .map(|(i, tag)| {
            let is_selected = i == app.tag_idx && focused;
            let count = app.secrets.iter().filter(|s| s.tags.contains(tag)).count();
            let label = format!("{tag} ({count})");
            let style = if is_selected {
                Style::default().bg(Color::Cyan).fg(Color::Black)
            } else {
                Style::default()
            };
            ListItem::new(label).style(style)
        })
        .collect();

    let scroll = scroll_indicators(app.tag_idx, app.tags.len(), area.height as usize, 2);
    let block = Block::default()
        .title(format!(" Tags ({}) {}", app.tags.len(), scroll))
        .borders(Borders::ALL)
        .border_style(border_style);

    let mut state = ListState::default();
    if focused && !app.tags.is_empty() {
        state.select(Some(app.tag_idx));
    }

    f.render_stateful_widget(List::new(items).block(block), area, &mut state);
}

fn render_secrets(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Secrets;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let header_cells = ["Name", "Value", "Tags"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(0);

    let assigning = app.editing_tag.is_some();

    // Inner width = pane minus left+right border (2) and column spacing (2 gaps of 1).
    // Value column gets 40% of what remains. Subtract 2 more to account for ratatui's
    // integer rounding — we'd rather truncate slightly early than let ratatui clip silently.
    let value_col_width = (area.width.saturating_sub(4) as usize) * 40 / 100 - 2;

    let rows: Vec<Row> = app
        .secrets
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let is_selected = i == app.sec_idx && focused;

            let value_display = if app.show_values {
                if s.value.chars().count() > value_col_width {
                    let truncated: String = s.value.chars().take(value_col_width).collect();
                    format!("{truncated}…")
                } else {
                    s.value.clone()
                }
            } else {
                "••••••••".to_string()
            };

            let tags = s.tags.join(", ");

            let name_cell = if assigning {
                let checked = app.ts_selected_ids.contains(&s.id);
                let checkbox = if checked { "[x] " } else { "[ ] " };
                Cell::from(format!("{checkbox}{}", s.name))
            } else {
                Cell::from(s.name.clone())
            };

            let style = if is_selected {
                Style::default().bg(Color::Cyan).fg(Color::Black)
            } else {
                Style::default()
            };

            Row::new(vec![name_cell, Cell::from(value_display), Cell::from(tags)])
                .style(style)
        })
        .collect();

    let widths = [
        Constraint::Percentage(30),
        Constraint::Percentage(40),
        Constraint::Percentage(30),
    ];

    // Secrets table has a header row, so subtract an extra row from the viewport.
    let scroll = scroll_indicators(app.sec_idx, app.secrets.len(), area.height as usize, 3);
    let title = if let Some(tag) = &app.editing_tag {
        format!(" Secrets — assigning '{}' {} ", tag, scroll)
    } else {
        format!(" Secrets ({}) {} ", app.secrets.len(), scroll)
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    if app.secrets.is_empty() {
        let placeholder = Paragraph::new(Span::styled(
            "press n to add a new secret",
            Style::default().fg(Color::DarkGray),
        ))
        .block(block)
        .alignment(ratatui::layout::Alignment::Left);
        f.render_widget(placeholder, area);
        return;
    }

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black));

    let mut state = TableState::default();
    if focused && !app.secrets.is_empty() {
        state.select(Some(app.sec_idx));
    }

    f.render_stateful_widget(table, area, &mut state);
}

// --- Form view ---

fn render_form(f: &mut Frame, app: &App) {
    let area = f.area();

    let title = match &app.mode {
        Mode::NewSecret => " New Secret ",
        Mode::EditSecret => " Edit Secret ",
        Mode::NewTag => " New Tag ",
        Mode::EditTag => " Edit Tag ",
        _ => " Form ",
    };

    let fields = if matches!(app.mode, Mode::NewTag | Mode::EditTag) {
        TAG_FIELDS
    } else {
        SECRET_FIELDS
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    if app.is_textarea_field() {
        let inner = block.inner(area);
        f.render_widget(block, area);
        render_textarea_form(f, app, fields, inner);
        return;
    }

    let mut lines: Vec<Line> = vec![Line::from("")];

    for (i, field) in fields.iter().enumerate() {
        if i < app.field_idx {
            // Completed field
            let value = app.collected_values.get(i).cloned().unwrap_or_default();
            let display = if field.secret {
                "••••••••".to_string()
            } else {
                value
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", field.label),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(display, Style::default().fg(Color::Green)),
            ]));
        } else if i == app.field_idx {
            // Current field
            lines.push(Line::from(vec![Span::styled(
                format!("▶ {} ", field.label),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]));

            // Input line with cursor
            let input = &app.field_input;
            let display = if field.secret {
                "•".repeat(input.len())
            } else {
                input.clone()
            };

            let cursor_pos = app.cursor;
            let before_cursor = if field.secret {
                "•".repeat(cursor_pos)
            } else {
                input.chars().take(cursor_pos).collect::<String>()
            };
            let at_cursor = display
                .chars()
                .nth(cursor_pos)
                .map(|c| c.to_string())
                .unwrap_or_else(|| " ".to_string());
            let after_cursor = if field.secret {
                let after_len = input.len().saturating_sub(cursor_pos + 1);
                "•".repeat(after_len)
            } else {
                input.chars().skip(cursor_pos + 1).collect::<String>()
            };

            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::raw(before_cursor),
                Span::styled(
                    at_cursor,
                    Style::default().bg(Color::White).fg(Color::Black),
                ),
                Span::raw(after_cursor),
            ]));

            // Tag autocomplete dropdown
            if matches!(app.mode, Mode::NewSecret | Mode::EditSecret)
                && i == 3
                && !app.tag_ac_matches.is_empty()
            {
                const MAX_VISIBLE: usize = 5;
                for (j, tag) in app.tag_ac_matches.iter().take(MAX_VISIBLE).enumerate() {
                    let selected = app.tag_ac_idx == Some(j);
                    if selected {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(
                                format!("▶ {tag}"),
                                Style::default().bg(Color::Cyan).fg(Color::Black),
                            ),
                        ]));
                    } else {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(
                                format!("  {tag}"),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    }
                }
                if app.tag_ac_matches.len() > MAX_VISIBLE {
                    lines.push(Line::from(Span::styled(
                        format!("  … {} more", app.tag_ac_matches.len() - MAX_VISIBLE),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
        } else {
            // Future field (dimmed)
            lines.push(Line::from(vec![Span::styled(
                format!("  {} ", field.label),
                Style::default().fg(Color::DarkGray),
            )]));
        }

        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![Span::styled(
        "  [Enter] Next/Submit  [Esc] Cancel",
        Style::default().fg(Color::DarkGray),
    )]));

    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_textarea_form(f: &mut Frame, app: &App, fields: &[super::app::FormField], area: Rect) {
    // Header: blank line + completed fields (2 lines each) + current field label
    let header_height = (1 + app.field_idx * 2 + 1) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Length(6), // textarea: 4 visible lines + 2 borders
            Constraint::Min(0),
        ])
        .split(area);

    // Header
    let mut header_lines: Vec<Line> = vec![Line::from("")];
    for i in 0..app.field_idx {
        let field = &fields[i];
        let value = app.collected_values.get(i).cloned().unwrap_or_default();
        let display = if field.secret {
            "••••••••".to_string()
        } else {
            value
        };
        header_lines.push(Line::from(vec![
            Span::styled(
                format!("  {} ", field.label),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(display, Style::default().fg(Color::Green)),
        ]));
        header_lines.push(Line::from(""));
    }
    header_lines.push(Line::from(vec![Span::styled(
        format!("▶ {} ", fields[app.field_idx].label),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]));
    f.render_widget(Paragraph::new(header_lines), chunks[0]);

    // Textarea
    render_textarea_widget(f, app, chunks[1]);

    // Footer
    let mut footer_lines: Vec<Line> = vec![];
    for i in (app.field_idx + 1)..fields.len() {
        footer_lines.push(Line::from(vec![Span::styled(
            format!("  {} ", fields[i].label),
            Style::default().fg(Color::DarkGray),
        )]));
    }
    footer_lines.push(Line::from(""));
    footer_lines.push(Line::from(vec![Span::styled(
        "  [Enter] Newline  [Tab] Next  [Esc] Cancel",
        Style::default().fg(Color::DarkGray),
    )]));
    f.render_widget(Paragraph::new(footer_lines), chunks[2]);
}

fn render_textarea_widget(f: &mut Frame, app: &App, area: Rect) {
    let scroll = app.ta_scroll;
    // Subtract 2 for the block borders
    let viewport = area.height.saturating_sub(2) as usize;

    let can_up = scroll > 0;
    let can_down = scroll + viewport < app.ta_lines.len();
    let scroll_indicator = match (can_up, can_down) {
        (true, true) => " ▲▼",
        (true, false) => " ▲",
        (false, true) => " ▼",
        (false, false) => "",
    };

    let block = Block::default()
        .title(format!(" Value{scroll_indicator} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let visible: Vec<Line> = (scroll..scroll + viewport)
        .map(|row| {
            if row >= app.ta_lines.len() {
                return Line::from("");
            }
            let text = &app.ta_lines[row];

            if row != app.ta_row {
                return Line::from(text.clone());
            }

            // Cursor row: highlight the character under the cursor
            let col = app.ta_col;
            let before: String = text.chars().take(col).collect();
            let at: String = text
                .chars()
                .nth(col)
                .map(|c| c.to_string())
                .unwrap_or_else(|| " ".to_string());
            let after: String = text.chars().skip(col + 1).collect();

            Line::from(vec![
                Span::raw(before),
                Span::styled(at, Style::default().bg(Color::White).fg(Color::Black)),
                Span::raw(after),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(visible), inner);
}

// --- Invite view ---

fn render_invite(f: &mut Frame, app: &App) {
    let area = f.area();

    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Invite Token",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Share this with the person you want to invite.",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            "It contains your storage config (no credentials).",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(""),
        Line::from(app.invite_token.clone()),
        Line::from(""),
        Line::from(vec![
            Span::styled("They should run:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("envi setup {}", app.invite_token),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        if app.clipboard_ok {
            Line::from(vec![Span::styled(
                "Copied to clipboard!",
                Style::default().fg(Color::Green),
            )])
        } else {
            Line::from(vec![Span::styled(
                "Could not access clipboard — copy the link above manually.",
                Style::default().fg(Color::Yellow),
            )])
        },
        Line::from(""),
        Line::from(vec![Span::styled(
            "[Esc] Close",
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    let block = Block::default()
        .title(" Invite ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

// --- Helpers ---

/// Returns a scroll indicator string ("▲", "▼", "▲▼", or "") based on whether
/// the list overflows the visible area and where the selection sits.
///
/// `overhead` = rows consumed by non-item chrome (borders, header rows, etc.)
fn scroll_indicators(
    selected: usize,
    total: usize,
    area_height: usize,
    overhead: usize,
) -> &'static str {
    let visible = area_height.saturating_sub(overhead);
    if total <= visible {
        return "";
    }
    let can_up = selected > 0;
    let can_down = selected + 1 < total;
    match (can_up, can_down) {
        (true, true) => "▲▼",
        (true, false) => "▲",
        (false, true) => "▼",
        (false, false) => "",
    }
}
