use std::{io, time::Duration};

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
    Terminal,
};
use tokio::sync::mpsc;

use crate::{
    core::{kubernetes::kube_context, Condition, ConditionStatus, Kubernetes, Resource},
    tui::app::{App, View},
};

enum Update {
    Loaded {
        context: String,
        resources: Vec<Resource>,
        warnings: Vec<String>,
    },
    Failed {
        context: Option<String>,
        message: String,
    },
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode().context("Could not enable terminal raw mode")?;
        if let Err(error) = execute!(
            io::stdout(),
            EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        ) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        );
    }
}

fn request_refresh(tx: mpsc::UnboundedSender<Update>, context: Option<String>) {
    tokio::spawn(async move {
        match Kubernetes::connect(context.as_deref()).await {
            Ok(client) => match client.list_managed_resources().await {
                Ok(report) => {
                    let _ = tx.send(Update::Loaded {
                        context: client.context,
                        resources: report.resources,
                        warnings: report
                            .errors
                            .iter()
                            .map(|error| error.message.clone())
                            .collect(),
                    });
                }
                Err(error) => {
                    let _ = tx.send(Update::Failed {
                        context: Some(client.context),
                        message: format!("{error:#}"),
                    });
                }
            },
            Err(error) => {
                let context = kube_context(context.as_deref()).await.ok();
                let _ = tx.send(Update::Failed {
                    context,
                    message: format!("{error:#}"),
                });
            }
        }
    });
}

pub async fn run(context: Option<&str>) -> Result<()> {
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut app = App {
        resources: Vec::new(),
        selected: 0,
        filter: String::new(),
        filtering: false,
        view: View::List,
        loading: true,
        error: None,
        warnings: Vec::new(),
        context: "loading kubeconfig".into(),
    };
    request_refresh(tx.clone(), context.map(str::to_owned));

    loop {
        terminal.draw(|frame| draw(frame, &mut app))?;
        while let Ok(update) = rx.try_recv() {
            match update {
                Update::Loaded {
                    context,
                    resources,
                    warnings,
                } => {
                    app.context = context;
                    app.replace_resources(resources, warnings);
                    app.loading = false;
                    app.error = None;
                }
                Update::Failed { context, message } => {
                    if let Some(context) = context {
                        app.context = context;
                    }
                    app.loading = false;
                    app.error = Some(message);
                }
            }
        }
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }
                if handle_key(
                    &mut app,
                    key.code,
                    key.modifiers,
                    tx.clone(),
                    context.map(str::to_owned),
                ) {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn handle_key(
    app: &mut App,
    key: KeyCode,
    modifiers: KeyModifiers,
    tx: mpsc::UnboundedSender<Update>,
    context: Option<String>,
) -> bool {
    if app.filtering {
        match key {
            KeyCode::Esc => {
                app.filtering = false;
                app.filter.clear();
                app.selected = 0;
            }
            KeyCode::Enter => app.filtering = false,
            KeyCode::Backspace => {
                app.filter.pop();
                app.selected = 0;
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                app.filter.push(ch);
                app.selected = 0;
            }
            _ => {}
        }
        return false;
    }
    if app.view == View::Details {
        if key == KeyCode::Esc {
            app.view = View::List;
        } else if key == KeyCode::Char('q') {
            return true;
        }
        return false;
    }
    match key {
        KeyCode::Char('q') => return true,
        KeyCode::Char('/') => {
            app.filtering = true;
        }
        KeyCode::Char('r') if !app.loading => {
            app.loading = true;
            app.error = None;
            request_refresh(tx, context);
        }
        KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
        KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
        KeyCode::Enter if app.selected_resource().is_some() => app.view = View::Details,
        KeyCode::Esc => {
            app.filter.clear();
            app.selected = 0;
        }
        _ => {}
    }
    false
}

fn draw(frame: &mut ratatui::Frame<'_>, app: &mut App) {
    let area = frame.area();
    if area.width < 60 || area.height < 10 {
        frame.render_widget(
            Paragraph::new("Terminal too small; resize to at least 60x10")
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }
    match app.view {
        View::List => draw_list(frame, app),
        View::Details => draw_details(frame, app),
    }
}

fn draw_list(frame: &mut ratatui::Frame<'_>, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(4),
            Constraint::Length(2),
        ])
        .split(frame.area());
    let title = format!(
        " x9s — Crossplane Resource Explorer     Context: {}     Namespace: All",
        app.context
    );
    frame.render_widget(
        Paragraph::new(title).style(Style::default().add_modifier(Modifier::BOLD)),
        chunks[0],
    );

    let headers = Row::new(["KIND", "NAME", "NAMESPACE", "SYNCED", "READY", "AGE"])
        .style(Style::default().add_modifier(Modifier::BOLD));
    let visible = app.visible_indices();
    let rows = visible
        .iter()
        .filter_map(|index| app.resources.get(*index))
        .map(resource_row);
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(26),
            Constraint::Percentage(25),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
            Constraint::Percentage(9),
        ],
    )
    .header(headers)
    .block(Block::default().borders(Borders::ALL))
    .row_highlight_style(
        Style::default()
            .bg(Color::Blue)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");
    let mut state =
        TableState::default().with_selected((!visible.is_empty()).then_some(app.selected));
    frame.render_stateful_widget(table, chunks[1], &mut state);

    let status = if app.filtering {
        format!("Filter: {}█", app.filter)
    } else if app.loading {
        "Loading resources…  [r] Refresh  [q] Quit".into()
    } else if let Some(error) = &app.error {
        format!("Error: {error}  [r] Retry  [q] Quit")
    } else if !app.warnings.is_empty() {
        let more = app.warnings.len().saturating_sub(1);
        let suffix = if more > 0 {
            format!(" (+{more} more)")
        } else {
            String::new()
        };
        format!(
            "Partial results: {}{suffix}  [/] Filter  [r] Refresh  [q] Quit",
            app.warnings[0]
        )
    } else if !app.filter.is_empty() {
        format!(
            "No resources match filter: {}  [/] Edit  [r] Refresh  [q] Quit",
            app.filter
        )
    } else if visible.is_empty() {
        "No confirmed Managed Resources found  [/] Filter  [r] Refresh  [q] Quit".into()
    } else {
        "[Enter] Details  [/] Filter  [r] Refresh  [q] Quit".into()
    };
    frame.render_widget(Paragraph::new(status).wrap(Wrap { trim: true }), chunks[2]);
}

fn resource_row(resource: &Resource) -> Row<'static> {
    Row::new(vec![
        Cell::from(resource.kind.clone()),
        Cell::from(resource.name.clone()),
        Cell::from(resource.namespace.clone().unwrap_or_else(|| "-".into())),
        Cell::from(condition_span(&resource.synced)),
        Cell::from(condition_span(&resource.ready)),
        Cell::from(age(resource)),
    ])
}

fn condition_span(condition: &Condition) -> Span<'static> {
    let (mark, color) = match condition.status {
        ConditionStatus::True => ("✓", Color::Green),
        ConditionStatus::False => ("✗", Color::Red),
        ConditionStatus::Unknown => ("?", Color::Yellow),
        ConditionStatus::Missing => ("–", Color::DarkGray),
    };
    Span::styled(
        format!("{mark} {}", condition.status.label()),
        Style::default().fg(color),
    )
}

fn age(resource: &Resource) -> String {
    resource
        .created_at
        .map(|created| {
            let seconds = (chrono::Utc::now() - created).num_seconds().max(0);
            if seconds < 3600 {
                format!("{}m", seconds / 60)
            } else if seconds < 86400 {
                format!("{}h", seconds / 3600)
            } else {
                format!("{}d", seconds / 86400)
            }
        })
        .unwrap_or_else(|| "-".into())
}

fn draw_details(frame: &mut ratatui::Frame<'_>, app: &App) {
    let Some(resource) = app.selected_resource() else {
        frame.render_widget(
            Paragraph::new("Selected resource is no longer available. Press Esc to return."),
            frame.area(),
        );
        return;
    };
    let mut lines = vec![
        Line::from(Span::styled(
            format!("Resource: {}/{}", resource.kind, resource.name),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("API Version:    {}", resource.api_version)),
        Line::from(format!("Family:         {}", resource.family.label())),
        Line::from(format!("Kind:           {}", resource.kind)),
        Line::from(format!(
            "Namespace:      {}",
            resource.namespace.as_deref().unwrap_or("-")
        )),
        Line::from(format!("Name:           {}", resource.name)),
        Line::from(format!(
            "UID:            {}",
            resource.uid.as_deref().unwrap_or("-")
        )),
        Line::from(format!(
            "Created:        {}",
            resource
                .created_at
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "-".into())
        )),
        Line::from(format!(
            "External Name:  {}",
            resource.external_name.as_deref().unwrap_or("-")
        )),
        Line::from(""),
        Line::from("Conditions:"),
    ];
    for (name, condition) in [("Synced", &resource.synced), ("Ready", &resource.ready)] {
        lines.push(Line::from(format!(
            "  {name}: {}",
            condition.status.label()
        )));
        if let Some(reason) = &condition.reason {
            lines.push(Line::from(format!("    Reason: {reason}")));
        }
        if let Some(message) = &condition.message {
            lines.push(Line::from(format!("    Message: {message}")));
        }
        if let Some(transition) = &condition.last_transition_time {
            lines.push(Line::from(format!("    Last Transition: {transition}")));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from("[Esc] Back"));
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        frame.area(),
    );
}
