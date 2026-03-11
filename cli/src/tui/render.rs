use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, TableState,
        Wrap,
    },
    Frame,
};

use super::app::{App, Focus, Mode, PROJECT_FIELDS, SECRET_FIELDS};

pub fn render(f: &mut Frame, app: &App) {
    match &app.mode {
        Mode::Invite => render_invite(f, app),
        Mode::ProjectSecrets => render_project_secrets(f, app),
        Mode::NewSecret | Mode::EditSecret | Mode::NewProject | Mode::EditProject => {
            render_form(f, app)
        }
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
            Constraint::Length(1), // header
            Constraint::Min(0),    // body
            Constraint::Length(1), // footer
        ])
        .split(area);

    render_header(f, app, main_chunks[0]);

    // Body: members row on top, then projects | secrets
    let body_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(main_chunks[1]);

    render_members(f, app, body_chunks[0]);

    // Projects | Secrets split
    let pane_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(body_chunks[1]);

    render_projects(f, app, pane_chunks[0]);
    render_secrets(f, app, pane_chunks[1]);

    render_footer(f, app, main_chunks[2]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let sync_span = if app.syncing {
        Span::styled("↑ syncing…", Style::default().fg(Color::Yellow))
    } else {
        Span::styled("✓ saved", Style::default().fg(Color::DarkGray))
    };

    let line = Line::from(vec![
        Span::styled("envi", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(
            // Show workspace name if we can
            "",
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw("  "),
        sync_span,
    ]);

    f.render_widget(Paragraph::new(line), area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    // Member confirmation dialogs take priority
    if let Some(id) = &app.member_to_delete {
        let email = app.members.iter().find(|m| &m.id == id)
            .map(|m| m.email.as_str())
            .unwrap_or("member");
        let line = Line::from(vec![
            Span::styled(
                format!("Remove {email}? [y] Yes  [n] No"),
                Style::default().fg(Color::Yellow),
            ),
        ]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }
    if let Some(id) = &app.member_to_grant {
        let email = app.members.iter().find(|m| &m.id == id)
            .map(|m| m.email.as_str())
            .unwrap_or("member");
        let line = Line::from(vec![
            Span::styled(
                format!("Grant access to {email}? [y] Yes  [n] No"),
                Style::default().fg(Color::Yellow),
            ),
        ]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }

    let hint = match &app.focus {
        Focus::Projects => "[n] New  [e] Edit  [s] Secrets  [d] Delete  [Tab] Switch  [q] Quit",
        Focus::Secrets => {
            if app.show_values {
                "[n] New  [e] Edit  [d] Delete  [v] Hide values  [Tab] Switch  [q] Quit"
            } else {
                "[n] New  [e] Edit  [d] Delete  [v] Show values  [Tab] Switch  [q] Quit"
            }
        }
        Focus::Members => "[g] Grant access  [d] Remove  [i] Invite  [Tab] Switch  [q] Quit",
    };

    f.render_widget(
        Paragraph::new(hint).style(Style::default().fg(Color::DarkGray)),
        area,
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

    let block = Block::default()
        .title(" Members ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let mut state = ListState::default();
    if focused && !app.members.is_empty() {
        state.select(Some(app.member_idx));
    }

    f.render_stateful_widget(List::new(items).block(block), area, &mut state);
}

fn render_projects(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Projects;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let items: Vec<ListItem> = app
        .projects
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let is_selected = i == app.proj_idx && focused;
            let secret_count = p.secret_ids.len();
            let label = format!("{} ({})", p.name, secret_count);

            let style = if is_selected {
                Style::default().bg(Color::Cyan).fg(Color::Black)
            } else {
                Style::default()
            };

            ListItem::new(label).style(style)
        })
        .collect();

    let block = Block::default()
        .title(" Projects ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let mut state = ListState::default();
    if focused && !app.projects.is_empty() {
        state.select(Some(app.proj_idx));
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

    let header_cells = ["Name", "Value", "Tags", "Projects"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(0);

    let rows: Vec<Row> = app
        .secrets
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let is_selected = i == app.sec_idx && focused;

            let value_display = if app.show_values {
                s.value.clone()
            } else {
                "••••••••".to_string()
            };

            let tags = s.tags.join(", ");

            // Count how many projects contain this secret
            let project_count = app
                .projects
                .iter()
                .filter(|p| p.secret_ids.contains(&s.id))
                .count();
            let proj_display = project_count.to_string();

            let style = if is_selected {
                Style::default().bg(Color::Cyan).fg(Color::Black)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(s.name.clone()),
                Cell::from(value_display),
                Cell::from(tags),
                Cell::from(proj_display),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Percentage(30),
        Constraint::Percentage(35),
        Constraint::Percentage(25),
        Constraint::Percentage(10),
    ];

    let block = Block::default()
        .title(" Secrets ")
        .borders(Borders::ALL)
        .border_style(border_style);

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
        Mode::NewProject => " New Project ",
        Mode::EditProject => " Edit Project ",
        _ => " Form ",
    };

    let fields = if app.mode == Mode::NewSecret || app.mode == Mode::EditSecret {
        SECRET_FIELDS
    } else {
        PROJECT_FIELDS
    };

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
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
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
            let at_cursor = display.chars().nth(cursor_pos).map(|c| c.to_string()).unwrap_or_else(|| " ".to_string());
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

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    f.render_widget(Paragraph::new(lines).block(block), area);
}

// --- Project-secrets checklist view ---

fn render_project_secrets(f: &mut Frame, app: &App) {
    let area = f.area();

    let proj_name = app
        .editing_id
        .as_ref()
        .and_then(|id| app.projects.iter().find(|p| &p.id == id))
        .map(|p| p.name.as_str())
        .unwrap_or("project");

    let items: Vec<ListItem> = app
        .secrets
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let checked = app.ps_selected_ids.contains(&s.id);
            let is_cursor = i == app.ps_cursor;
            let checkbox = if checked { "[x]" } else { "[ ]" };
            let label = format!("{checkbox} {}", s.name);

            let style = if is_cursor {
                Style::default().bg(Color::Cyan).fg(Color::Black)
            } else if checked {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            ListItem::new(label).style(style)
        })
        .collect();

    let block = Block::default()
        .title(format!(" Secrets for project '{proj_name}' "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let mut state = ListState::default();
    state.select(Some(app.ps_cursor));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    f.render_stateful_widget(List::new(items).block(block), chunks[0], &mut state);
    f.render_widget(
        Paragraph::new("[Space] Toggle  [Enter] Save  [Esc] Cancel")
            .style(Style::default().fg(Color::DarkGray)),
        chunks[1],
    );
}

// --- Invite view ---

fn render_invite(f: &mut Frame, app: &App) {
    let area = f.area();

    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Invite Link",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
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
        Line::from(app.invite_link.clone()),
        Line::from(""),
        Line::from(vec![
            Span::styled("They should run:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("envi setup {}", app.invite_link),
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

    f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: false }), area);
}
