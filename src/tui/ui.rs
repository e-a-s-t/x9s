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
use serde_json::Value;
use tokio::sync::mpsc;

use crate::{
    core::{
        kubernetes::kube_context, relationships::RelationshipGraph, Condition, ConditionStatus,
        Kubernetes, Resource,
    },
    tui::{
        app::{App, InputMode, ProviderEntry, View},
        theme::{K9sDark, ThemeColor},
    },
};

enum Update {
    Loaded {
        client: Kubernetes,
        context: String,
        resources: Vec<Resource>,
        graph: RelationshipGraph,
        warnings: Vec<String>,
    },
    EventsLoaded {
        events: Vec<Value>,
    },
    EventsFailed {
        message: String,
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

fn request_refresh(
    tx: mpsc::UnboundedSender<Update>,
    context: Option<String>,
    connected: Option<Kubernetes>,
) {
    tokio::spawn(async move {
        let client_result = match connected {
            Some(client) => Ok(client),
            None => Kubernetes::connect(context.as_deref()).await,
        };
        match client_result {
            Ok(client) => match client.list_managed_resources().await {
                Ok(report) => {
                    let warnings = report
                        .errors
                        .iter()
                        .chain(report.relationship_errors.iter())
                        .map(|error| error.message.clone())
                        .collect();
                    let _ = tx.send(Update::Loaded {
                        context: client.context.clone(),
                        client,
                        resources: report.all_resources,
                        graph: report.graph,
                        warnings,
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

fn request_events(
    tx: mpsc::UnboundedSender<Update>,
    context: Option<String>,
    connected: Option<Kubernetes>,
    resource: Resource,
) {
    tokio::spawn(async move {
        let client_result = match connected {
            Some(client) => Ok(client),
            None => Kubernetes::connect(context.as_deref()).await,
        };
        match client_result {
            Ok(client) => match client.list_events(&resource).await {
                Ok(events) => {
                    let _ = tx.send(Update::EventsLoaded { events });
                }
                Err(error) => {
                    let _ = tx.send(Update::EventsFailed {
                        message: format!("{error:#}"),
                    });
                }
            },
            Err(error) => {
                let _ = tx.send(Update::EventsFailed {
                    message: format!("{error:#}"),
                });
            }
        }
    });
}

pub async fn run(context: Option<&str>) -> Result<()> {
    let _guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut app = App::new("loading kubeconfig".into());
    let mut kube_client = None;
    request_refresh(tx.clone(), context.map(str::to_owned), None);
    loop {
        terminal.draw(|frame| draw(frame, &app))?;
        while let Ok(update) = rx.try_recv() {
            match update {
                Update::Loaded {
                    client,
                    context,
                    resources,
                    graph,
                    warnings,
                } => {
                    app.context = context;
                    kube_client = Some(client);
                    app.replace_resources(resources, graph, warnings);
                    app.loading = false;
                    app.error = None;
                }
                Update::EventsLoaded { events } => {
                    app.events = events;
                    app.error = None;
                }
                Update::EventsFailed { message } => {
                    app.events.clear();
                    app.error = Some(message);
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
                    kube_client.clone(),
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
    connected: Option<Kubernetes>,
) -> bool {
    if app.input_mode != InputMode::None {
        match key {
            KeyCode::Esc => {
                app.input_mode = InputMode::None;
                app.input.clear();
            }
            KeyCode::Enter => {
                let submitted = app.input.trim().to_owned();
                match app.input_mode {
                    InputMode::Filter => app.filter = app.input.clone(),
                    InputMode::Command => {
                        execute_command(app, &submitted, tx.clone(), connected.clone())
                    }
                    InputMode::None => {}
                }
                app.input.clear();
                app.input_mode = InputMode::None;
                app.selected = 0;
            }
            KeyCode::Backspace => {
                app.input.pop();
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => app.input.push(ch),
            _ => {}
        }
        return false;
    }
    match app.view {
        View::Details | View::Yaml | View::Events | View::Describe | View::Help => match key {
            KeyCode::Esc | KeyCode::Left => app.back(),
            KeyCode::Char('q') => return true,
            KeyCode::Char(':') => {
                app.input_mode = InputMode::Command;
                app.input.clear();
            }
            KeyCode::Char('?') => {
                app.push_history();
                app.view = View::Help;
            }
            KeyCode::Down | KeyCode::Char('j') if app.view == View::Details => {
                let len = app.related_resource_ids().len();
                if len > 0 {
                    app.related_selected = (app.related_selected + 1).min(len - 1);
                }
            }
            KeyCode::Down | KeyCode::Char('j')
                if matches!(app.view, View::Yaml | View::Describe | View::Events) =>
            {
                app.detail_scroll = app.detail_scroll.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') if app.view == View::Details => {
                app.related_selected = app.related_selected.saturating_sub(1);
            }
            KeyCode::Up | KeyCode::Char('k')
                if matches!(app.view, View::Yaml | View::Describe | View::Events) =>
            {
                app.detail_scroll = app.detail_scroll.saturating_sub(1);
            }
            KeyCode::Enter if app.view == View::Details => {
                if let Some(target) = app
                    .related_resource_ids()
                    .get(app.related_selected)
                    .cloned()
                {
                    app.push_history();
                    app.selected_provider = app.graph.relationships.iter().find_map(|edge| {
                        (edge.relation == crate::core::RelationshipType::ManagedBy
                            && edge.from == target)
                            .then(|| edge.to.clone())
                            .flatten()
                    });
                    app.unassigned_only = false;
                    app.unresolved_only = false;
                    app.view = View::Resources;
                    app.selected = app
                        .visible_indices()
                        .iter()
                        .position(|index| app.resources[*index].id() == target)
                        .unwrap_or(0);
                    app.related_selected = 0;
                }
            }
            KeyCode::Char('y') if app.view == View::Details => {
                app.push_history();
                app.view = View::Yaml;
            }
            KeyCode::Char('e') if app.view == View::Details => {
                if let Some(resource) = app.selected_resource().cloned() {
                    app.events.clear();
                    app.push_history();
                    app.view = View::Events;
                    request_events(tx, context, connected.clone(), resource);
                }
            }
            KeyCode::Char('d') if app.view == View::Details => {
                app.push_history();
                app.view = View::Describe;
            }
            KeyCode::Char('p') => {
                app.push_history();
                app.view = View::Providers;
                app.selected = 0;
            }
            KeyCode::Char('r') => {
                app.loading = true;
                request_refresh(tx, context, connected.clone());
            }
            _ => {}
        },
        View::Providers => match key {
            KeyCode::Char('q') => return true,
            KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
            KeyCode::Enter => {
                app.push_history();
                match app.visible_provider_entries().get(app.selected) {
                    Some(ProviderEntry::All) => {
                        app.selected_provider = None;
                        app.unassigned_only = false;
                        app.unresolved_only = false;
                    }
                    Some(ProviderEntry::Unassigned) => {
                        app.selected_provider = None;
                        app.unassigned_only = true;
                        app.unresolved_only = false;
                    }
                    Some(ProviderEntry::Unresolved) => {
                        app.selected_provider = None;
                        app.unassigned_only = false;
                        app.unresolved_only = true;
                    }
                    Some(ProviderEntry::Config(provider)) => {
                        app.selected_provider = Some(provider.clone());
                        app.unassigned_only = false;
                        app.unresolved_only = false;
                    }
                    None => {}
                }
                app.view = View::Resources;
                app.selected = 0;
            }
            KeyCode::Char('/') => {
                app.input_mode = InputMode::Filter;
                app.input = app.filter.clone();
            }
            KeyCode::Char(':') => {
                app.input_mode = InputMode::Command;
                app.input.clear();
            }
            KeyCode::Char('r') => {
                app.loading = true;
                request_refresh(tx, context, connected.clone());
            }
            KeyCode::Char('?') => {
                app.push_history();
                app.view = View::Help;
            }
            KeyCode::Esc => app.back(),
            _ => {}
        },
        View::Resources | View::System => match key {
            KeyCode::Char('q') => return true,
            KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
            KeyCode::Enter => {
                if app.selected_resource().is_some() {
                    app.push_history();
                    app.view = View::Details;
                }
            }
            KeyCode::Char('/') => {
                app.input_mode = InputMode::Filter;
                app.input = app.filter.clone();
            }
            KeyCode::Char(':') => {
                app.input_mode = InputMode::Command;
                app.input.clear();
            }
            KeyCode::Char('p') => {
                app.push_history();
                app.view = View::Providers;
                app.selected = 0;
            }
            KeyCode::Char('t') => {
                set_tree_mode(app, !app.tree);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(row) = app.visible_tree_rows().get(app.selected) {
                    if row.has_children {
                        let id = app.resources[row.index].id();
                        app.collapsed.remove(&id);
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(row) = app.visible_tree_rows().get(app.selected).copied() {
                    let id = app.resources[row.index].id();
                    if row.has_children && !app.collapsed.contains(&id) {
                        app.collapsed.insert(id);
                    } else if let Some(parent) = tree_parent(app, &id) {
                        app.selected = app
                            .visible_indices()
                            .iter()
                            .position(|index| app.resources[*index].id() == parent)
                            .unwrap_or(app.selected);
                    }
                }
            }
            KeyCode::Char('r') => {
                app.loading = true;
                request_refresh(tx, context, connected.clone());
            }
            KeyCode::Char('?') => {
                app.push_history();
                app.view = View::Help;
            }
            KeyCode::Esc => app.back(),
            _ => {}
        },
    }
    false
}

fn execute_command(
    app: &mut App,
    command: &str,
    tx: mpsc::UnboundedSender<Update>,
    connected: Option<Kubernetes>,
) {
    let command = command.trim_start_matches(':').trim();
    if let Some(context) = command
        .strip_prefix("context ")
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        app.loading = true;
        app.error = None;
        request_refresh(
            tx,
            Some(context.to_owned()),
            connected.filter(|client| client.context == context),
        );
        return;
    }
    match command {
        "providers" => {
            app.view = View::Providers;
            app.selected = 0;
        }
        "all" => {
            app.view = View::Resources;
            app.selected_provider = None;
            app.unassigned_only = false;
            app.unresolved_only = false;
            app.health_only = false;
            app.selected = 0;
        }
        "system" => {
            app.view = View::System;
            app.selected_provider = None;
            app.unassigned_only = false;
            app.unresolved_only = false;
            app.selected = 0;
        }
        "tree" => set_tree_mode(app, true),
        "flat" => set_tree_mode(app, false),
        "health" => {
            app.view = View::Resources;
            app.selected_provider = None;
            app.unassigned_only = false;
            app.health_only = true;
            app.selected = 0;
        }
        "context" => {
            app.error = Some(format!(
                "Active context: {}. Use :context <name> to switch.",
                app.context
            ))
        }
        _ => {
            app.error = Some(
                "Unknown command. Try :providers, :all, :tree, :flat, :system, :health, :context"
                    .into(),
            )
        }
    }
}

fn draw(frame: &mut ratatui::Frame<'_>, app: &App) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(
            Style::default()
                .bg(K9sDark::color(ThemeColor::Background))
                .fg(K9sDark::color(ThemeColor::Foreground)),
        ),
        area,
    );
    if area.width < 40 || area.height < 8 {
        frame.render_widget(Paragraph::new("Resize terminal to at least 40x8"), area);
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(area);
    let context = format!(
        " Context: {}   {}",
        app.context,
        match app.view {
            View::Providers => "ProviderConfigs",
            View::Resources => "Resources",
            View::System => "System",
            _ => "Resource",
        }
    );
    frame.render_widget(
        Paragraph::new(context).style(Style::default().fg(K9sDark::color(ThemeColor::Accent))),
        chunks[0],
    );
    match app.view {
        View::Providers => draw_providers(frame, app, chunks[1]),
        View::Resources => draw_resources(frame, app, chunks[1]),
        View::System => draw_resources(frame, app, chunks[1]),
        View::Details => draw_details(frame, app, chunks[1]),
        View::Describe => draw_describe(frame, app, chunks[1]),
        View::Yaml => draw_yaml(frame, app, chunks[1]),
        View::Events => draw_events(frame, app, chunks[1]),
        View::Help => frame.render_widget(Paragraph::new("j/k move  Enter open  Esc back  p providers  / filter  : command  t tree/flat  y YAML  d details  e events  r refresh  ? help  q quit").wrap(Wrap { trim: true }), chunks[1]),
    }
    let footer = if app.input_mode == InputMode::Filter {
        format!("/{}", app.input)
    } else if app.input_mode == InputMode::Command {
        format!(":{}", app.input)
    } else if let Some(error) = &app.error {
        format!("{error}  [r] refresh")
    } else if app.loading {
        "Discovering resources…  [r] Refresh  [q] Quit".into()
    } else if !app.warnings.is_empty() {
        format!(
            "Partial discovery: {}  [/] Filter [r] Refresh [?] Help [q] Quit",
            app.warnings[0]
        )
    } else if app.view == View::Providers {
        "Enter open  / filter  : command  r refresh  q quit".into()
    } else {
        "j/k move  Enter details  p providers  / filter  t tree/flat  r refresh  Esc back".into()
    };
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(Color::Gray)),
        chunks[2],
    );
}

fn set_tree_mode(app: &mut App, tree: bool) {
    let selected = app.selected_resource().map(Resource::id);
    app.tree = tree;
    if let Some(selected) = selected {
        if let Some(position) = app
            .visible_indices()
            .iter()
            .position(|index| app.resources[*index].id() == selected)
        {
            app.selected = position;
        }
    }
}

fn draw_providers(frame: &mut ratatui::Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let entries = app.visible_provider_entries();
    let mut rows = Vec::new();
    for entry in &entries {
        let id = match entry {
            ProviderEntry::All => {
                let count = app
                    .resources
                    .iter()
                    .filter(|resource| {
                        resource.family != crate::core::ResourceFamily::ProviderConfig
                            && !resource.family.is_system()
                    })
                    .map(Resource::id)
                    .collect::<std::collections::HashSet<_>>()
                    .len();
                rows.push(Row::new(vec![
                    Cell::from("All Providers"),
                    Cell::from("-"),
                    Cell::from(count.to_string()),
                    Cell::from("-"),
                ]));
                continue;
            }
            ProviderEntry::Unassigned => {
                let count = app
                    .resources
                    .iter()
                    .filter(|resource| {
                        resource.family == crate::core::ResourceFamily::ManagedResource
                    })
                    .filter(|resource| {
                        resource
                            .original
                            .pointer("/spec/providerConfigRef")
                            .is_none()
                    })
                    .map(Resource::id)
                    .collect::<std::collections::HashSet<_>>()
                    .len();
                rows.push(Row::new(vec![
                    Cell::from("Unassigned"),
                    Cell::from("-"),
                    Cell::from(count.to_string()),
                    Cell::from("Unknown"),
                ]));
                continue;
            }
            ProviderEntry::Unresolved => {
                let count = app
                    .resources
                    .iter()
                    .filter(|resource| {
                        resource.family == crate::core::ResourceFamily::ManagedResource
                    })
                    .filter(|resource| {
                        app.graph.relationships.iter().any(|edge| {
                            edge.relation == crate::core::RelationshipType::ManagedBy
                                && edge.from == resource.id()
                                && edge.to.is_none()
                        })
                    })
                    .map(Resource::id)
                    .collect::<std::collections::HashSet<_>>()
                    .len();
                rows.push(Row::new(vec![
                    Cell::from("Unresolved ProviderConfig"),
                    Cell::from("-"),
                    Cell::from(count.to_string()),
                    Cell::from(if count == 0 { "Healthy" } else { "Degraded" }),
                ]));
                continue;
            }
            ProviderEntry::Config(id) => id,
        };
        let resource = app.resources.iter().find(|resource| resource.id() == *id);
        let label = format_resource_id(id);
        let provider = resource
            .and_then(|config| {
                config.provider_owner_uid.as_deref().and_then(|uid| {
                    app.resources
                        .iter()
                        .find(|candidate| candidate.uid.as_deref() == Some(uid))
                        .and_then(|revision| {
                            revision
                                .original
                                .pointer("/metadata/ownerReferences/0/name")
                                .and_then(Value::as_str)
                        })
                })
            })
            .unwrap_or("-");
        let count = app.provider_resource_count(id);
        rows.push(Row::new(vec![
            Cell::from(label),
            Cell::from(provider.to_owned()),
            Cell::from(count.to_string()),
            Cell::from(
                resource
                    .map(|item| item.ready.status.label())
                    .unwrap_or("Missing"),
            ),
        ]));
    }
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
        ],
    )
    .header(
        Row::new(["NAME", "PROVIDER", "RESOURCES", "STATUS"]).style(
            Style::default()
                .fg(K9sDark::color(ThemeColor::Heading))
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .title(format!(" PROVIDER CONFIGS [{}] ", app.providers.len()))
            .borders(Borders::NONE),
    )
    .row_highlight_style(
        Style::default()
            .bg(K9sDark::color(ThemeColor::Selection))
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");
    frame.render_stateful_widget(
        table,
        area,
        &mut TableState::default().with_selected((!entries.is_empty()).then_some(app.selected)),
    );
}

fn draw_resources(frame: &mut ratatui::Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let tree_rows = app.visible_tree_rows();
    if tree_rows.is_empty() && app.view == View::Resources && app.selected_provider.is_some() {
        frame.render_widget(
            Paragraph::new("No managed resources associated with this ProviderConfig."),
            area,
        );
        return;
    }
    let rows = tree_rows.iter().filter_map(|tree_row| {
        app.resources.get(tree_row.index).map(|resource| {
            let id = resource.id();
            let marker = if tree_row.has_children {
                if app.collapsed.contains(&id) {
                    "▶ "
                } else {
                    "▼ "
                }
            } else {
                "  "
            };
            let branch = if app.tree && tree_row.depth > 0 {
                format!(
                    "{}{}",
                    "  ".repeat(tree_row.depth - 1),
                    if tree_row.last {
                        "└── "
                    } else {
                        "├── "
                    }
                )
            } else {
                String::new()
            };
            let name = format!("{branch}{marker}{}/{}", resource.kind, resource.name);
            Row::new(vec![
                Cell::from(name),
                Cell::from(resource.namespace.clone().unwrap_or_else(|| "-".into())),
                Cell::from(condition_span(&resource.ready)),
                Cell::from(condition_span(&resource.synced)),
                Cell::from(age(resource)),
            ])
        })
    });
    let title = if app.view == View::System {
        " CROSSPLANE SYSTEM OBJECTS ".into()
    } else if app.unresolved_only {
        " RESOURCES (UNRESOLVED PROVIDERCONFIG) ".into()
    } else if app.unassigned_only {
        " RESOURCES (UNASSIGNED) ".into()
    } else {
        app.selected_provider
            .as_ref()
            .map(|provider| format!(" RESOURCES ({}) ", format_resource_id(provider)))
            .unwrap_or_else(|| " RESOURCES (ALL PROVIDERS) ".into())
    };
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(54),
            Constraint::Percentage(14),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
            Constraint::Percentage(12),
        ],
    )
    .header(
        Row::new(["RESOURCE", "NAMESPACE", "READY", "SYNCED", "AGE"]).style(
            Style::default()
                .fg(K9sDark::color(ThemeColor::Heading))
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(Block::default().title(title).borders(Borders::NONE))
    .row_highlight_style(
        Style::default()
            .bg(K9sDark::color(ThemeColor::Selection))
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");
    frame.render_stateful_widget(
        table,
        area,
        &mut TableState::default().with_selected((!tree_rows.is_empty()).then_some(app.selected)),
    );
}

fn draw_details(frame: &mut ratatui::Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let Some(resource) = app.selected_resource() else {
        frame.render_widget(
            Paragraph::new("Resource no longer available. Press Esc."),
            area,
        );
        return;
    };
    let mut lines = vec![
        Line::from(format!("Resource: {}/{}", resource.kind, resource.name)),
        Line::from(format!("API:       {}", resource.api_version)),
        Line::from(format!("Group:     {}", resource.group)),
        Line::from(format!(
            "Namespace: {}",
            resource.namespace.as_deref().unwrap_or("-")
        )),
        Line::from(format!(
            "UID:       {}",
            resource.uid.as_deref().unwrap_or("-")
        )),
        Line::from(format!(
            "External:  {}",
            resource.external_name.as_deref().unwrap_or("-")
        )),
        Line::from(""),
        Line::from("CONDITIONS"),
    ];
    for (name, condition) in [("Ready", &resource.ready), ("Synced", &resource.synced)] {
        lines.push(Line::from(format!(
            "{name}: {}{}",
            condition.status.label(),
            condition
                .reason
                .as_ref()
                .map(|r| format!(" ({r})"))
                .unwrap_or_default()
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("RELATIONSHIPS"));
    for edge in app
        .graph
        .relationships
        .iter()
        .filter(|edge| edge.from == resource.id() || edge.to.as_ref() == Some(&resource.id()))
    {
        let other = if edge.from == resource.id() {
            edge.to
                .as_ref()
                .and_then(|id| app.resources.iter().find(|resource| resource.id() == *id))
                .map(|r| format!("{}/{}", r.kind, r.name))
                .or_else(|| edge.unresolved.clone())
                .unwrap_or_else(|| "unresolved".into())
        } else {
            app.resources
                .iter()
                .find(|resource| resource.id() == edge.from)
                .map(|r| format!("{}/{}", r.kind, r.name))
                .unwrap_or_else(|| "unresolved".into())
        };
        let related = app.related_resource_ids();
        let index = edge
            .to
            .as_ref()
            .filter(|_| edge.from == resource.id())
            .or_else(|| (edge.to.as_ref() == Some(&resource.id())).then_some(&edge.from));
        let marker = index
            .and_then(|id| related.iter().position(|candidate| candidate == id))
            .map(|position| {
                if position == app.related_selected {
                    "▶ "
                } else {
                    "  "
                }
            })
            .unwrap_or("  ");
        lines.push(Line::from(format!(
            "{marker}{}: {other}",
            edge.relation.label()
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("y YAML   e Events   d Describe   Esc Back"));
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::NONE))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_yaml(frame: &mut ratatui::Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let text = app
        .selected_resource()
        .and_then(|resource| sanitized_yaml(&resource.original).ok())
        .unwrap_or_else(|| "Could not render resource YAML".into());
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((app.detail_scroll, 0)),
        area,
    );
}

fn draw_describe(frame: &mut ratatui::Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let Some(resource) = app.selected_resource() else {
        frame.render_widget(
            Paragraph::new("Resource no longer available. Press Esc."),
            area,
        );
        return;
    };
    let body =
        sanitized_yaml(&resource.original).unwrap_or_else(|_| "Could not describe resource".into());
    frame.render_widget(
        Paragraph::new(body)
            .block(
                Block::default()
                    .title(format!(" DESCRIBE {} {} ", resource.kind, resource.name))
                    .borders(Borders::NONE),
            )
            .wrap(Wrap { trim: false })
            .scroll((app.detail_scroll, 0)),
        area,
    );
}

fn draw_events(frame: &mut ratatui::Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    if let Some(error) = &app.error {
        frame.render_widget(
            Paragraph::new(error.clone()).wrap(Wrap { trim: true }),
            area,
        );
        return;
    }
    if app.events.is_empty() {
        frame.render_widget(
            Paragraph::new("No Events found. Press Esc to return."),
            area,
        );
        return;
    }
    let lines: Vec<_> = app
        .events
        .iter()
        .map(|event| {
            let reason = event
                .pointer("/reason")
                .and_then(Value::as_str)
                .unwrap_or("-");
            let kind = event
                .pointer("/type")
                .and_then(Value::as_str)
                .unwrap_or("-");
            let message = event
                .pointer("/message")
                .and_then(Value::as_str)
                .unwrap_or("");
            let time = event
                .pointer("/eventTime")
                .and_then(Value::as_str)
                .or_else(|| event.pointer("/lastTimestamp").and_then(Value::as_str))
                .unwrap_or("-");
            Line::from(format!("{time}  {kind:<8} {reason:<24} {message}"))
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((app.detail_scroll, 0)),
        area,
    );
}

fn sanitized_yaml(value: &Value) -> Result<String, serde_yaml::Error> {
    fn scrub(value: &Value) -> Value {
        if let Some(object) = value.as_object() {
            let is_secret = object.get("kind").and_then(Value::as_str) == Some("Secret");
            let mut result = serde_json::Map::new();
            for (key, child) in object {
                let normalized = key.to_lowercase();
                let sensitive = [
                    "password",
                    "token",
                    "apikey",
                    "accesskey",
                    "authorization",
                    "bearer",
                    "credential",
                    "privatekey",
                    "private_key",
                    "clientsecret",
                    "client_secret",
                    "secretdata",
                    "secret",
                    "connectiondetails",
                    "connectionstring",
                    "certificate",
                ]
                .iter()
                .any(|needle| normalized.contains(needle));
                if (is_secret && (key == "data" || key == "stringData")) || sensitive {
                    result.insert(key.clone(), Value::String("<redacted>".into()));
                } else {
                    result.insert(key.clone(), scrub(child));
                }
            }
            Value::Object(result)
        } else if let Some(array) = value.as_array() {
            Value::Array(array.iter().map(scrub).collect())
        } else {
            value.clone()
        }
    }
    serde_yaml::to_string(&scrub(value))
}

fn condition_span(condition: &Condition) -> Span<'static> {
    let color = match condition.status {
        ConditionStatus::True => K9sDark::color(ThemeColor::Healthy),
        ConditionStatus::False => K9sDark::color(ThemeColor::Error),
        ConditionStatus::Unknown => K9sDark::color(ThemeColor::Warning),
        ConditionStatus::Missing => K9sDark::color(ThemeColor::Muted),
    };
    Span::styled(
        condition.status.label().to_owned(),
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

fn format_resource_id(id: &crate::core::ResourceId) -> String {
    let group = if id.group.is_empty() {
        "core"
    } else {
        &id.group
    };
    let address = id
        .namespace
        .as_deref()
        .map(|namespace| format!("{namespace}/{}", id.name))
        .unwrap_or_else(|| id.name.clone());
    format!("{}/{} [{group}]", id.kind, address)
}

fn tree_parent(app: &App, id: &crate::core::ResourceId) -> Option<crate::core::ResourceId> {
    app.graph
        .relationships
        .iter()
        .find_map(|edge| match edge.relation {
            crate::core::RelationshipType::Composes | crate::core::RelationshipType::Owns
                if edge.to.as_ref() == Some(id) =>
            {
                Some(edge.from.clone())
            }
            crate::core::RelationshipType::BelongsTo if edge.from == *id => edge.to.clone(),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::{sanitized_yaml, set_tree_mode};
    use crate::core::{
        crossplane::{normalize, CrdInfo},
        models::{Resource, ResourceFamily},
        relationships::RelationshipGraph,
    };
    use serde_json::json;

    fn test_resource(name: &str) -> Resource {
        let crd = CrdInfo {
            group: "test.example.org".into(),
            kind: "Widget".into(),
            namespaced: false,
            schema: json!({}),
            labels: json!({}),
            annotations: json!({}),
            owner_references: json!([]),
            categories: vec![],
            family: ResourceFamily::Other,
            api_version: "test.example.org/v1".into(),
            plural: "widgets".into(),
            provider_config_default_kind: None,
            vcd_vapp_name_reference: false,
        };
        normalize(
            json!({"apiVersion":"test.example.org/v1","kind":"Widget","metadata":{"name":name}}),
            &crd,
        )
    }

    #[test]
    fn yaml_redacts_secret_payloads_and_sensitive_fields_recursively() {
        let value = json!({
            "apiVersion": "v1", "kind": "Secret",
            "data": {"password": "encoded-value"},
            "spec": {"accessToken": "token-value", "endpoint": "https://safe.example"}
        });
        let yaml = sanitized_yaml(&value).unwrap();
        assert!(!yaml.contains("encoded-value"));
        assert!(!yaml.contains("token-value"));
        assert!(yaml.contains("<redacted>"));
        assert!(yaml.contains("https://safe.example"));
    }

    #[test]
    fn switching_tree_mode_preserves_selected_resource() {
        let mut app = crate::tui::app::App::new("test".into());
        app.view = crate::tui::app::View::Resources;
        app.resources = vec![test_resource("alpha"), test_resource("beta")];
        app.graph = RelationshipGraph::default();
        app.selected = 1;
        let selected = app.selected_resource().unwrap().id();
        set_tree_mode(&mut app, false);
        assert_eq!(app.selected_resource().unwrap().id(), selected);
    }
}
