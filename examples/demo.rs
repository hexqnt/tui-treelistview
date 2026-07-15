use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Local};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Cell;
use ratatui::{DefaultTerminal, Frame};
use smallvec::smallvec;

use tui_treelistview::{
    ColumnDef, ColumnWidth, TreeChangeSet, TreeChildren, TreeColumnSet, TreeEditCommand,
    TreeEditRequest, TreeEditor, TreeEvent, TreeInsertPosition, TreeIntent, TreeLabelPrefix,
    TreeLabelProvider, TreeListView, TreeListViewState, TreeListViewStyle, TreeModel, TreeQuery,
    TreeRevision, TreeRowContext, TreeSelectionUpdate,
};

struct Node {
    name: String,
    parent: Option<usize>,
    children: Vec<usize>,
    size: String,
    perms: String,
    modified: String,
    alive: bool,
}

struct FsModel {
    nodes: Vec<Node>,
    root: Option<usize>,
    revision: TreeRevision,
}

impl FsModel {
    const fn new() -> Self {
        Self {
            nodes: Vec::new(),
            root: None,
            revision: TreeRevision::INITIAL,
        }
    }

    fn push_node(&mut self, node: Node) -> usize {
        let id = self.nodes.len();
        self.nodes.push(node);
        id
    }

    fn add_synthetic_child(&mut self, parent: usize) -> Option<usize> {
        if parent >= self.nodes.len() {
            return None;
        }
        let id = self.nodes.len();
        let name = format!("new-node-{id}");
        let node = Node {
            name,
            parent: Some(parent),
            children: Vec::new(),
            size: "-".to_string(),
            perms: placeholder_permissions(false),
            modified: now_string(),
            alive: true,
        };
        self.nodes.push(node);
        self.nodes[parent].children.push(id);
        Some(id)
    }

    fn rename_node(&mut self, id: usize) -> bool {
        if let Some(node) = self.nodes.get_mut(id) {
            if !node.name.ends_with(" [edited]") {
                node.name.push_str(" [edited]");
            }
            node.modified = now_string();
            return true;
        }
        false
    }

    fn detach_from_parent(&mut self, id: usize) -> Option<usize> {
        let parent = self.nodes.get(id)?.parent?;
        self.nodes[parent].children.retain(|child| *child != id);
        self.nodes[id].parent = None;
        Some(parent)
    }

    fn is_descendant(&self, root: usize, target: usize) -> bool {
        if root == target {
            return true;
        }
        let mut stack = vec![root];
        let mut visited = vec![false; self.nodes.len()];
        while let Some(id) = stack.pop() {
            if id >= self.nodes.len() || visited[id] {
                continue;
            }
            visited[id] = true;
            for &child in &self.nodes[id].children {
                if child == target {
                    return true;
                }
                stack.push(child);
            }
        }
        false
    }
}

impl TreeModel for FsModel {
    type Id = usize;

    fn roots(&self) -> impl Iterator<Item = Self::Id> + '_ {
        self.root
            .filter(|root| self.nodes.get(*root).is_some_and(|node| node.alive))
            .into_iter()
    }

    fn children(&self, id: Self::Id) -> TreeChildren<'_, Self::Id> {
        if self.nodes[id].alive {
            TreeChildren::loaded(&self.nodes[id].children)
        } else {
            TreeChildren::Leaf
        }
    }

    fn revision(&self) -> TreeRevision {
        self.revision
    }

    fn size_hint(&self) -> usize {
        self.nodes.len()
    }
}

impl TreeEditor for FsModel {
    type Error = &'static str;

    fn apply(
        &mut self,
        command: TreeEditCommand<Self::Id>,
    ) -> Result<TreeChangeSet<Self::Id>, Self::Error> {
        let mut changes = TreeChangeSet::default();
        match command {
            TreeEditCommand::CreateChild { parent } => {
                let child = self.add_synthetic_child(parent).ok_or("invalid parent")?;
                changes.inserted.push(child);
                changes.selection = TreeSelectionUpdate::Select(child);
            }
            TreeEditCommand::Rename { node } => {
                if !self.rename_node(node) {
                    return Err("invalid node");
                }
                changes.selection = TreeSelectionUpdate::Select(node);
            }
            TreeEditCommand::Move {
                nodes,
                parent,
                position,
            } => {
                if parent >= self.nodes.len() || !self.nodes[parent].alive {
                    return Err("invalid destination parent");
                }
                for node in nodes.iter().copied() {
                    if self.root == Some(node) || self.is_descendant(node, parent) {
                        return Err("move would violate tree invariants");
                    }
                }
                for node in nodes.iter().copied() {
                    self.detach_from_parent(node);
                }
                let index = position
                    .index_in(&self.nodes[parent].children)
                    .ok_or("insertion anchor is missing")?;
                for (offset, node) in nodes.iter().copied().enumerate() {
                    self.nodes[parent].children.insert(index + offset, node);
                    self.nodes[node].parent = Some(parent);
                    changes.moved.push(node);
                }
                changes.selection = nodes
                    .last()
                    .copied()
                    .map_or(TreeSelectionUpdate::Keep, TreeSelectionUpdate::Select);
            }
            TreeEditCommand::Detach { nodes } => {
                for node in nodes {
                    if self.root == Some(node) {
                        return Err("cannot detach root");
                    }
                    if self.detach_from_parent(node).is_some() {
                        changes.moved.push(node);
                    }
                }
            }
            TreeEditCommand::Delete { nodes } => {
                for node in nodes {
                    if self.root == Some(node) {
                        return Err("cannot delete root");
                    }
                    self.detach_from_parent(node);
                    let mut stack = vec![node];
                    while let Some(id) = stack.pop() {
                        stack.extend(self.nodes[id].children.iter().copied());
                        self.nodes[id].alive = false;
                        self.nodes[id].parent = None;
                        self.nodes[id].children.clear();
                        changes.removed.push(id);
                    }
                }
            }
        }
        self.revision.advance();
        Ok(changes)
    }
}

struct Label;

impl TreeLabelProvider<FsModel> for Label {
    fn label_parts<'a>(&'a self, model: &'a FsModel, id: usize) -> TreeLabelPrefix<'a> {
        TreeLabelPrefix::borrowed(&model.nodes[id].name)
    }
}

struct DemoArgs {
    root: PathBuf,
    max_depth: usize,
}

impl DemoArgs {
    fn usage() {
        eprintln!("Usage: demo [PATH] [DEPTH]");
        eprintln!("  PATH   Root directory (default: current dir)");
        eprintln!("  DEPTH  Max depth from root (default: 2)");
        eprintln!("Options:");
        eprintln!("  -d, --depth <N>  Max depth from root");
        eprintln!("  -h, --help       Show this help");
    }

    fn parse() -> Self {
        let mut path: Option<PathBuf> = None;
        let mut depth: Option<usize> = None;

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => {
                    Self::usage();
                    std::process::exit(0);
                }
                "-d" | "--depth" => {
                    if let Some(value) = args.next() {
                        depth = value.parse().ok();
                    }
                }
                _ => {
                    if path.is_none() {
                        path = Some(PathBuf::from(arg));
                    } else if depth.is_none() {
                        depth = arg.parse().ok();
                    }
                }
            }
        }

        let root =
            path.unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let max_depth = depth.unwrap_or(2);

        Self { root, max_depth }
    }
}

struct EntryInfo {
    name: String,
    path: PathBuf,
    metadata: fs::Metadata,
    is_dir: bool,
}

fn size_cell<'a>(model: &'a FsModel, id: usize, _: &TreeRowContext<'_>) -> Cell<'a> {
    Cell::from(model.nodes[id].size.as_str())
}

fn perms_cell<'a>(model: &'a FsModel, id: usize, _: &TreeRowContext<'_>) -> Cell<'a> {
    Cell::from(model.nodes[id].perms.as_str())
}

fn modified_cell<'a>(model: &'a FsModel, id: usize, _: &TreeRowContext<'_>) -> Cell<'a> {
    Cell::from(model.nodes[id].modified.as_str())
}

fn build_model(root: &Path, max_depth: usize) -> io::Result<FsModel> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let metadata = fs::symlink_metadata(&root)?;

    let mut model = FsModel::new();
    let root_id = model.push_node(node_from_meta(root.display().to_string(), None, &metadata));
    model.root = Some(root_id);

    if metadata.is_dir() {
        build_children(&mut model, root_id, &root, 0, max_depth);
    }

    Ok(model)
}

fn build_children(
    model: &mut FsModel,
    parent_id: usize,
    path: &Path,
    depth: usize,
    max_depth: usize,
) {
    if depth >= max_depth {
        return;
    }

    let mut entries: Vec<EntryInfo> = match fs::read_dir(path) {
        Ok(read_dir) => read_dir
            .filter_map(std::result::Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                let metadata = fs::symlink_metadata(&path).ok()?;
                let is_dir = metadata.is_dir();
                let name = entry.file_name().to_string_lossy().to_string();
                Some(EntryInfo {
                    name,
                    path,
                    metadata,
                    is_dir,
                })
            })
            .collect(),
        Err(_) => return,
    };

    entries.sort_by(|a, b| {
        if a.is_dir == b.is_dir {
            a.name.cmp(&b.name)
        } else if a.is_dir {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });

    for entry in entries {
        let node_id = model.push_node(node_from_meta(entry.name, Some(parent_id), &entry.metadata));
        model.nodes[parent_id].children.push(node_id);

        if entry.is_dir {
            build_children(model, node_id, &entry.path, depth + 1, max_depth);
        }
    }
}

fn node_from_meta(name: String, parent: Option<usize>, metadata: &fs::Metadata) -> Node {
    let is_dir = metadata.is_dir();
    Node {
        name,
        parent,
        children: Vec::new(),
        size: if is_dir {
            "-".to_string()
        } else {
            format_size(metadata.len())
        },
        perms: format_permissions(metadata),
        modified: format_modified(metadata),
        alive: true,
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes;
    let mut unit = 0usize;
    while value >= 1024 && unit + 1 < UNITS.len() {
        value /= 1024;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        let mut scale = 1_u64;
        for _ in 0..unit {
            scale = scale.saturating_mul(1024);
        }
        let value_x10 = bytes.saturating_mul(10) / scale;
        format!("{}.{} {}", value_x10 / 10, value_x10 % 10, UNITS[unit])
    }
}

#[cfg(unix)]
fn format_permissions(metadata: &fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;

    let mode = metadata.permissions().mode();
    let mut out = String::with_capacity(10);
    out.push(if metadata.is_dir() { 'd' } else { '-' });

    for shift in [6, 3, 0] {
        let bits = (mode >> shift) & 0b111;
        out.push(if bits & 0b100 != 0 { 'r' } else { '-' });
        out.push(if bits & 0b010 != 0 { 'w' } else { '-' });
        out.push(if bits & 0b001 != 0 { 'x' } else { '-' });
    }

    out
}

#[cfg(not(unix))]
fn format_permissions(metadata: &fs::Metadata) -> String {
    let prefix = if metadata.is_dir() { "d" } else { "-" };
    let mode = if metadata.permissions().readonly() {
        "ro"
    } else {
        "rw"
    };
    format!("{prefix}{mode}")
}

fn format_modified(metadata: &fs::Metadata) -> String {
    metadata.modified().map_or_else(
        |_| "-".to_string(),
        |time| {
            let datetime: DateTime<Local> = DateTime::from(time);
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        },
    )
}

fn now_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn placeholder_permissions(is_dir: bool) -> String {
    if is_dir {
        "d---------".to_string()
    } else {
        "----------".to_string()
    }
}

fn expand_all(state: &mut TreeListViewState<usize>, model: &FsModel) {
    let _ = state.expand_all(model);
}

fn render(
    frame: &mut Frame,
    model: &FsModel,
    query: &TreeQuery,
    label: &Label,
    columns: &TreeColumnSet<'_, FsModel>,
    state: &mut TreeListViewState<usize>,
    style: &TreeListViewStyle<'_>,
) {
    let widget = TreeListView::new(model, query, label, columns, style.clone());
    frame.render_stateful_widget(widget, frame.area(), state);
}

fn edit_command(
    model: &FsModel,
    request: TreeEditRequest<usize>,
    clipboard: &mut Option<usize>,
) -> Option<TreeEditCommand<usize>> {
    match request {
        TreeEditRequest::ReorderUp { node, parent } => {
            let siblings = &model.nodes[parent].children;
            let index = siblings.iter().position(|id| *id == node)?;
            let previous = index.checked_sub(1).and_then(|index| siblings.get(index))?;
            Some(TreeEditCommand::Move {
                nodes: smallvec![node],
                parent,
                position: TreeInsertPosition::Before(*previous),
            })
        }
        TreeEditRequest::ReorderDown { node, parent } => {
            let siblings = &model.nodes[parent].children;
            let index = siblings.iter().position(|id| *id == node)?;
            let next = siblings.get(index + 1)?;
            Some(TreeEditCommand::Move {
                nodes: smallvec![node],
                parent,
                position: TreeInsertPosition::After(*next),
            })
        }
        TreeEditRequest::AddChild { parent } => Some(TreeEditCommand::CreateChild { parent }),
        TreeEditRequest::Rename { node } => Some(TreeEditCommand::Rename { node }),
        TreeEditRequest::Detach { node, .. } => Some(TreeEditCommand::Detach {
            nodes: smallvec![node],
        }),
        TreeEditRequest::Delete { node } => Some(TreeEditCommand::Delete {
            nodes: smallvec![node],
        }),
        TreeEditRequest::Yank { node } => {
            *clipboard = Some(node);
            None
        }
        TreeEditRequest::Paste { parent } => {
            let node = clipboard.filter(|node| model.nodes[*node].alive)?;
            Some(TreeEditCommand::Move {
                nodes: smallvec![node],
                parent,
                position: TreeInsertPosition::Last,
            })
        }
    }
}

fn run_app(
    mut terminal: DefaultTerminal,
    mut model: FsModel,
    columns: &TreeColumnSet<'_, FsModel>,
    style: &TreeListViewStyle<'_>,
) -> io::Result<()> {
    let query = TreeQuery::new();
    let label = Label;
    let mut state = TreeListViewState::with_capacity(model.size_hint());
    let mut clipboard: Option<usize> = None;
    expand_all(&mut state, &model);
    if let Some(root_id) = model.roots().next() {
        let _ = state.select_by_id(&model, &query, root_id);
    }

    loop {
        terminal.draw(|frame| {
            render(frame, &model, &query, &label, columns, &mut state, style);
        })?;

        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    _ => {
                        let event = state.handle_key(&model, &query, columns, key);
                        if let TreeEvent::Intent(TreeIntent::Edit(request)) = event
                            && let Some(command) = edit_command(&model, request, &mut clipboard)
                            && let Err(error) = state.apply_edit(&mut model, &query, command)
                        {
                            eprintln!("Edit failed: {error}");
                        }
                    }
                },
                _ => {}
            }
        }
    }

    Ok(())
}

fn main() -> io::Result<()> {
    let args = DemoArgs::parse();
    if !args.root.is_dir() {
        eprintln!("Path is not a directory: {}", args.root.display());
        return Ok(());
    }

    let model = build_model(&args.root, args.max_depth)?;

    let columns = TreeColumnSet::new([
        ColumnDef::tree(
            "Name",
            ColumnWidth::flexible(16, 48).expect("valid static column width"),
        ),
        ColumnDef::data("Size", ColumnWidth::fixed(10), size_cell),
        ColumnDef::data("Perms", ColumnWidth::fixed(10), perms_cell),
        ColumnDef::data("Modified", ColumnWidth::fixed(19), modified_cell),
    ])
    .expect("exactly one tree column")
    .header_style(
        Style::default()
            .fg(Color::Rgb(229, 201, 133))
            .add_modifier(Modifier::BOLD),
    );

    let style = TreeListViewStyle {
        title: Some(Line::from(format!(
            "{} (depth {})",
            args.root.display(),
            args.max_depth
        ))),
        block_style: Style::default()
            .fg(Color::Rgb(221, 227, 235))
            .bg(Color::Rgb(24, 28, 36)),
        border_style: Style::default().fg(Color::Rgb(92, 110, 140)),
        highlight_style: Style::default()
            .fg(Color::Rgb(255, 255, 255))
            .bg(Color::Rgb(52, 66, 96))
            .add_modifier(Modifier::BOLD),
        marked_style: Style::default()
            .fg(Color::Rgb(136, 192, 208))
            .add_modifier(Modifier::BOLD),
        line_style: Style::default().fg(Color::Rgb(86, 98, 120)),
        ..TreeListViewStyle::default()
    };

    let terminal = ratatui::init();
    let result = run_app(terminal, model, &columns, &style);
    ratatui::restore();
    result
}
