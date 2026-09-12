use std::{collections::HashSet, env, error::Error, io, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Paragraph, Wrap},
};
use tokio::task::JoinSet;
use tui_tree_widget::{Tree, TreeItem, TreeState};
use zadt::{Client, ReqwestTransport};
use zvfs::{Mount, Node, NodeId, NodeKind, VfsError, VirtualRepositoryTree};

// None identifies a placeholder under an unloaded folder. Real nodes retain
// their ZVFS identity, including when the widget's visible row indices change.
type State = TreeState<Option<NodeId>>;
type Item = TreeItem<'static, Option<NodeId>>;

#[derive(Clone, Copy)]
enum Action {
    Load,
    Refresh,
    Preload,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    println!("Connecting to SAP...");
    let transport = ReqwestTransport::builder()
        .destination(required_env("SAP_DESTINATION")?)
        .sap_client(required_env("SAP_CLIENT")?)
        .language(env::var("SAP_LANGUAGE").unwrap_or_else(|_| "EN".to_owned()))
        .basic_auth(required_env("SAP_USERNAME")?, required_env("SAP_PASSWORD")?)
        .danger_accept_invalid_certs(enabled("SAP_DANGER_ACCEPT_INVALID_CERTS"))
        .danger_accept_invalid_hostnames(enabled("SAP_DANGER_ACCEPT_INVALID_HOSTNAMES"))
        .build()?;
    let client = Client::new(transport).discover().await?;

    let mount = env::args()
        .nth(1)
        .map(Mount::package)
        .unwrap_or_else(|| Mount::system_library("System Library"));

    let tree = VirtualRepositoryTree::builder(client)
        .mount(mount)
        .build()
        .await?;

    // Ratatui installs a panic hook that restores the terminal as well.
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, tree).await;
    ratatui::restore();
    result
}

async fn run(
    terminal: &mut DefaultTerminal,
    tree: VirtualRepositoryTree,
) -> Result<(), Box<dyn Error>> {
    let mut state = State::default();
    if let Some(mount) = tree.children(tree.root()).await?.first() {
        state.select(vec![Some(mount.id)]);
        state.open(vec![Some(mount.id)]);
    }
    let mut jobs = JoinSet::new();
    let mut busy = None;
    let mut failed = HashSet::new();
    let mut message = String::from("Ready");
    let mut search = String::new();
    let mut searching = false;
    let mut dirty = true;

    loop {
        if let Some(completed) = jobs.try_join_next() {
            busy = None;
            match completed {
                Ok((id, action, Ok(()))) => {
                    failed.remove(&id);
                    if matches!(action, Action::Refresh) {
                        failed.clear();
                    }
                    message = "Ready".into();
                }
                Ok((id, _, Err(error))) => {
                    failed.insert(id);
                    message = format!("{error} — press r to retry");
                }
                Err(error) => return Err(error.into()),
            }
            let selected = state
                .selected()
                .iter()
                .copied()
                .take_while(|id| id.is_some_and(|id| tree.node(id).is_some()))
                .collect();
            state.select(selected);
            let stale: Vec<_> = state
                .opened()
                .iter()
                .filter(|path| {
                    path.iter()
                        .any(|id| id.is_none_or(|id| tree.node(id).is_none()))
                })
                .cloned()
                .collect();
            for path in stale {
                state.close(&path);
            }
            dirty = true;
        }

        // One request at a time keeps this example simple. Navigation and quit
        // remain responsive, and other opened folders load when it finishes.
        if jobs.is_empty() {
            let unloaded = state
                .opened()
                .iter()
                .filter_map(|path| path.last().copied().flatten())
                .find(|id| {
                    !failed.contains(id)
                        && tree.node(*id).is_some_and(|node| node.is_directory())
                        && matches!(tree.cached_children(*id), Ok(None))
                });
            if let Some(id) = unloaded {
                start(&mut jobs, &tree, id, Action::Load);
                busy = Some(id);
                message = format!(
                    "Loading {}...",
                    tree.node(id).map(|n| n.label).unwrap_or_default()
                );
                dirty = true;
            }
        }

        if dirty {
            let items = items(&tree, tree.root(), &state, &failed, &mut Vec::new())?;
            terminal.draw(|frame| {
                draw(
                    frame,
                    &tree,
                    &items,
                    &mut state,
                    &message,
                    searching.then_some(search.as_str()),
                    busy,
                )
            })?;
            dirty = false;
        }
        if !event::poll(Duration::ZERO)? {
            tokio::time::sleep(Duration::from_millis(30)).await;
            continue;
        }
        let event = event::read()?;
        dirty = true;
        let Event::Key(key) = event else { continue };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            break;
        }
        if searching {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => searching = false,
                KeyCode::Backspace => {
                    search.pop();
                }
                KeyCode::Char(ch) => search.push(ch),
                _ => {}
            }
            if searching && !search.is_empty() {
                find(&tree, &mut state, &failed, &search, false)?;
            }
            continue;
        }
        let page = usize::from(terminal.size()?.height.saturating_sub(7).max(1));
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Up | KeyCode::Char('k') => {
                state.key_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                state.key_down();
            }
            KeyCode::Left | KeyCode::Char('h') => {
                state.key_left();
            }
            KeyCode::Right | KeyCode::Char('l')
                if selected(&tree, &state).is_some_and(|n| n.is_directory()) =>
            {
                state.key_right();
            }
            KeyCode::Enter | KeyCode::Char(' ')
                if selected(&tree, &state).is_some_and(|n| n.is_directory()) =>
            {
                state.toggle_selected();
            }
            KeyCode::Home => {
                state.select_first();
            }
            KeyCode::End => {
                state.select_last();
            }
            KeyCode::PageUp => {
                state.select_relative(|i| i.unwrap_or_default().saturating_sub(page));
            }
            KeyCode::PageDown => {
                state.select_relative(|i| i.unwrap_or_default().saturating_add(page));
            }
            KeyCode::Char('/') => {
                search.clear();
                searching = true;
            }
            KeyCode::Char('n') if !search.is_empty() => {
                find(&tree, &mut state, &failed, &search, true)?;
            }
            KeyCode::Char('r' | 'p') if jobs.is_empty() => {
                if let Some(node) = selected(&tree, &state).filter(Node::is_directory) {
                    failed.remove(&node.id);
                    let action = if key.code == KeyCode::Char('p') {
                        Action::Preload
                    } else {
                        Action::Refresh
                    };
                    start(&mut jobs, &tree, node.id, action);
                    busy = Some(node.id);
                    message = format!(
                        "{} {}...",
                        if matches!(action, Action::Preload) {
                            "Preloading"
                        } else {
                            "Refreshing"
                        },
                        node.label
                    );
                }
            }
            _ => {}
        }
    }
    // Dropping JoinSet cancels any outstanding read before the UI exits.
    Ok(())
}

fn start(
    jobs: &mut JoinSet<(NodeId, Action, Result<(), VfsError>)>,
    tree: &VirtualRepositoryTree,
    id: NodeId,
    action: Action,
) {
    let tree = tree.clone();
    jobs.spawn(async move {
        let result = match action {
            Action::Load => tree.children(id).await.map(|_| ()),
            Action::Preload => tree.preload_all_children(id).await,
            Action::Refresh => match tree.refresh(id).await {
                Err(VfsError::StaleNode(_)) if tree.node(id).is_none() => Ok(()),
                result => result.map(|_| ()),
            },
        };
        (id, action, result)
    });
}

fn selected(tree: &VirtualRepositoryTree, state: &State) -> Option<Node> {
    state
        .selected()
        .iter()
        .rev()
        .find_map(|id| id.and_then(|id| tree.node(id)))
}

fn items(
    tree: &VirtualRepositoryTree,
    parent: NodeId,
    state: &State,
    failed: &HashSet<NodeId>,
    path: &mut Vec<Option<NodeId>>,
) -> io::Result<Vec<Item>> {
    let mut result = Vec::new();
    for node in tree
        .cached_children(parent)
        .ok()
        .flatten()
        .unwrap_or_default()
    {
        path.push(Some(node.id));
        let children = if node.is_directory() {
            if state.opened().contains(path)
                && tree.cached_children(node.id).ok().flatten().is_some()
            {
                items(tree, node.id, state, failed, path)?
            } else {
                vec![TreeItem::new_leaf(
                    None,
                    if failed.contains(&node.id) {
                        "Load failed — press r"
                    } else {
                        "Loading..."
                    },
                )]
            }
        } else {
            Vec::new()
        };
        let suffix = match &node.kind {
            NodeKind::Object { object } => format!("  {}", object.workbench_type),
            NodeKind::ObjectGroup { workbench_type, .. } => format!("  {workbench_type}"),
            _ => String::new(),
        };
        result.push(TreeItem::new(
            Some(node.id),
            format!("{}{suffix}", node.label),
            children,
        )?);
        path.pop();
    }
    Ok(result)
}

fn find(
    tree: &VirtualRepositoryTree,
    state: &mut State,
    failed: &HashSet<NodeId>,
    query: &str,
    next: bool,
) -> io::Result<()> {
    let items = items(tree, tree.root(), state, failed, &mut Vec::new())?;
    let rows = state.flatten(&items);
    let start = rows
        .iter()
        .position(|row| row.identifier == state.selected())
        .unwrap_or_default()
        + usize::from(next);
    let query = query.to_lowercase();
    if let Some(row) = rows
        .iter()
        .cycle()
        .skip(start)
        .take(rows.len())
        .find(|row| {
            row.identifier
                .last()
                .copied()
                .flatten()
                .and_then(|id| tree.node(id))
                .is_some_and(|node| node.label.to_lowercase().contains(&query))
        })
    {
        state.select(row.identifier.clone());
    }
    Ok(())
}

fn draw(
    frame: &mut Frame,
    tree: &VirtualRepositoryTree,
    items: &[Item],
    state: &mut State,
    message: &str,
    search: Option<&str>,
    busy: Option<NodeId>,
) {
    let [title, body, status, help] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(2),
        Constraint::Length(2),
    ])
    .areas(frame.area());
    let [browser, details] =
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(body);
    let node = selected(tree, state);
    let path = node
        .as_ref()
        .and_then(|node| tree.path(node.id).ok())
        .unwrap_or_default()
        .iter()
        .skip(1)
        .map(|node| node.label.as_str())
        .collect::<Vec<_>>()
        .join(" > ");
    frame.render_widget(
        Paragraph::new(format!(" ZVFS  {path}")).style(Style::default().fg(Color::Cyan)),
        title,
    );
    if let Ok(widget) = Tree::new(items) {
        frame.render_stateful_widget(
            widget
                .block(Block::bordered().title(" Repository "))
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> "),
            browser,
            state,
        );
    }
    frame.render_widget(
        Paragraph::new(node.as_ref().map(node_details).unwrap_or_default())
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(" Details ")),
        details,
    );
    let status_text = search
        .map(|query| format!("Find visible: /{query}"))
        .unwrap_or_else(|| message.to_owned());
    frame.render_widget(
        Paragraph::new(status_text)
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(if busy.is_some() {
                Color::Yellow
            } else {
                Color::White
            })),
        status,
    );
    frame.render_widget(Paragraph::new("↑/↓ or j/k: move  ←/→ or h/l: collapse/expand  Enter: toggle\n/: find visible  n: next  r: refresh  p: preload  PgUp/PgDn: page  q: quit"), help);
}

fn node_details(node: &Node) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(node.label.clone()), Line::from("")];
    match &node.kind {
        NodeKind::Object { object } => {
            lines.push(format!("Object: {}", object.workbench_type).into());
            if let Some(package) = &object.package {
                lines.push(format!("Package: {package}").into());
            }
            if let Some(description) = &object.description {
                lines.push(description.clone().into());
            }
            if let Some(uri) = &object.uri {
                lines.push(format!("URI: {uri}").into());
            }
            if !object.query.is_empty() {
                lines.push(format!("Query: {:?}", object.query).into());
            }
            if let Some(fragment) = &object.fragment {
                lines.push(format!("Fragment: {fragment}").into());
            }
            if let Some(version) = &object.version {
                lines.push(format!("Version: {version}").into());
            }
        }
        NodeKind::ObjectGroup {
            workbench_type,
            category,
        } => {
            lines.push(format!("Folder: {workbench_type}").into());
            lines.push(format!("Category: {category}").into());
        }
        NodeKind::Package {
            uri, object_count, ..
        } => {
            lines.push(format!("URI: {uri}").into());
            if let Some(count) = object_count {
                lines.push(format!("Objects: {count}").into());
            }
        }
        NodeKind::Facet {
            facet,
            value,
            object_count,
            ..
        } => {
            lines.push(format!("{facet}={value}").into());
            lines.push(format!("Objects: {object_count}").into());
        }
        NodeKind::Mount { .. } => lines.push("Repository selection".into()),
        NodeKind::Root => {}
    }
    lines.push(format!("Expandable: {}", node.is_directory()).into());
    lines
}

fn required_env(name: &str) -> Result<String, io::Error> {
    env::var(name).map_err(|source| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing required environment variable `{name}`: {source}"),
        )
    })
}

fn enabled(name: &str) -> bool {
    env::var(name).is_ok_and(|value| matches!(value.as_str(), "true" | "TRUE" | "1"))
}
