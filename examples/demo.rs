use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Local};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::Constraint;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Cell;
use ratatui::{DefaultTerminal, Frame};

use tui_treelistview::{
    ColumnDef, SimpleColumns, TreeAction, TreeEdit, TreeEvent, TreeLabelPrefix, TreeLabelProvider,
    TreeListView, TreeListViewState, TreeListViewStyle, TreeModel,
};

struct Node {
    name: String,
    parent: Option<usize>,
    children: Vec<usize>,
    size: String,
    perms: String,
    modified: String,
}

struct FsModel {
    nodes: Vec<Node>,
    root: Option<usize>,
}

impl FsModel {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            root: None,
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
        };
        self.nodes.push(node);
        self.nodes[parent].children.push(id);
        Some(id)
    }

    fn rename_node(&mut self, id: usize) {
        if let Some(node) = self.nodes.get_mut(id) {
            if !node.name.ends_with(" [edited]") {
                node.name.push_str(" [edited]");
            }
            node.modified = now_string();
        }
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

    fn root(&self) -> Option<Self::Id> {
        self.root
    }

    fn children(&self, id: Self::Id) -> &[Self::Id] {
        &self.nodes[id].children
    }

    fn contains(&self, id: Self::Id) -> bool {
        id < self.nodes.len()
    }

    fn size_hint(&self) -> usize {
        self.nodes.len()
    }
}

impl TreeEdit for FsModel {
    fn is_root(&self, id: Self::Id) -> bool {
        self.root == Some(id)
    }

    fn move_child_up(&mut self, parent: Self::Id, child: Self::Id) -> bool {
        let children = &mut self.nodes[parent].children;
        if let Some(idx) = children.iter().position(|&id| id == child) {
            if idx == 0 {
                return false;
            }
            children.swap(idx, idx - 1);
            return true;
        }
        false
    }

    fn move_child_down(&mut self, parent: Self::Id, child: Self::Id) -> bool {
        let children = &mut self.nodes[parent].children;
        if let Some(idx) = children.iter().position(|&id| id == child) {
            if idx + 1 >= children.len() {
                return false;
            }
            children.swap(idx, idx + 1);
            return true;
        }
        false
    }

    fn remove_child(&mut self, parent: Self::Id, child: Self::Id) {
        if parent >= self.nodes.len() {
            return;
        }
        self.nodes[parent].children.retain(|&id| id != child);
        if let Some(node) = self.nodes.get_mut(child) {
            node.parent = None;
        }
    }

    fn delete_node(&mut self, id: Self::Id) {
        if id >= self.nodes.len() {
            return;
        }

        if self.root == Some(id) {
            self.root = None;
        }

        if let Some(parent) = self.nodes[id].parent {
            if let Some(node) = self.nodes.get_mut(parent) {
                node.children.retain(|&child_id| child_id != id);
            }
        }

        let children = self.nodes[id].children.clone();
        for child in children {
            if let Some(node) = self.nodes.get_mut(child) {
                node.parent = None;
            }
        }

        self.nodes[id].children.clear();
        self.nodes[id].parent = None;
    }

    fn add_child(&mut self, parent: Self::Id, child: Self::Id) {
        if parent >= self.nodes.len() || child >= self.nodes.len() {
            return;
        }
        if self.is_descendant(child, parent) {
            return;
        }
        if let Some(old_parent) = self.nodes[child].parent {
            if let Some(node) = self.nodes.get_mut(old_parent) {
                node.children.retain(|&id| id != child);
            }
        }
        self.nodes[child].parent = Some(parent);
        let children = &mut self.nodes[parent].children;
        if !children.contains(&child) {
            children.push(child);
        }
    }
}

struct Label;

impl TreeLabelProvider<FsModel> for Label {
    fn label_parts<'a>(&'a self, model: &'a FsModel, id: usize) -> TreeLabelPrefix<'a> {
        TreeLabelPrefix {
            name: model.nodes[id].name.as_str(),
            prefix: None,
        }
    }
}

fn size_cell<'a>(model: &'a FsModel, id: usize) -> Cell<'a> {
    Cell::from(model.nodes[id].size.as_str())
}

fn perms_cell<'a>(model: &'a FsModel, id: usize) -> Cell<'a> {
    Cell::from(model.nodes[id].perms.as_str())
}

fn modified_cell<'a>(model: &'a FsModel, id: usize) -> Cell<'a> {
    Cell::from(model.nodes[id].modified.as_str())
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

        let mut args = env::args().skip(1).peekable();
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
            .filter_map(|entry| entry.ok())
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
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
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
    match metadata.modified() {
        Ok(time) => {
            let datetime: DateTime<Local> = DateTime::from(time);
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        }
        Err(_) => "-".to_string(),
    }
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
    for (id, node) in model.nodes.iter().enumerate() {
        if !node.children.is_empty() {
            state.set_expanded(id, node.parent, true);
        }
    }
}

fn render(
    frame: &mut Frame,
    model: &FsModel,
    label: &Label,
    columns: &SimpleColumns<3, FsModel>,
    state: &mut TreeListViewState<usize>,
    style: &TreeListViewStyle<'_>,
) {
    let widget = TreeListView::new(model, label, columns, style.clone());
    frame.render_stateful_widget(widget, frame.area(), state);
}

fn run_app(
    mut terminal: DefaultTerminal,
    mut model: FsModel,
    columns: SimpleColumns<3, FsModel>,
    style: TreeListViewStyle<'_>,
) -> io::Result<()> {
    let label = Label;
    let mut state = TreeListViewState::with_capacity(model.size_hint());
    let mut clipboard: Option<usize> = None;
    expand_all(&mut state, &model);
    if let Some(root_id) = model.root() {
        state.select_by_id(&model, root_id);
    }

    loop {
        terminal.draw(|frame| render(frame, &model, &label, &columns, &mut state, &style))?;

        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    _ => {
                        let event = state.handle_key(&model, key);
                        if let TreeEvent::Action(action) = event {
                            match action {
                                TreeAction::AddChild => {
                                    if let Some(parent_id) = state.selected_id() {
                                        if let Some(new_id) = model.add_synthetic_child(parent_id) {
                                            state.invalidate_all();
                                            let _ = state.select_by_id(&model, new_id);
                                        }
                                    }
                                }
                                TreeAction::EditNode => {
                                    if let Some(id) = state.selected_id() {
                                        model.rename_node(id);
                                        state.invalidate_all();
                                    }
                                }
                                TreeAction::ReorderUp
                                | TreeAction::ReorderDown
                                | TreeAction::DeleteNode
                                | TreeAction::DetachNode
                                | TreeAction::YankNode
                                | TreeAction::PasteNode => {
                                    let _ = state.handle_edit_action(
                                        &mut model,
                                        action,
                                        &mut clipboard,
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                },
                Event::Resize(_, _) => {}
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

    let columns = SimpleColumns::new(
        Constraint::Fill(1),
        "Name",
        [
            ColumnDef::new("Size", Constraint::Length(10), size_cell),
            ColumnDef::new("Perms", Constraint::Length(10), perms_cell),
            ColumnDef::new("Modified", Constraint::Length(19), modified_cell),
        ],
    )
    .header_style(
        Style::default()
            .fg(Color::Rgb(229, 201, 133))
            .add_modifier(Modifier::BOLD),
    );

    let mut style = TreeListViewStyle::default();
    style.block_style = Style::default()
        .fg(Color::Rgb(221, 227, 235))
        .bg(Color::Rgb(24, 28, 36));
    style.border_style = Style::default().fg(Color::Rgb(92, 110, 140));
    style.line_style = Style::default().fg(Color::Rgb(86, 98, 120));
    style.mark_style = Style::default()
        .fg(Color::Rgb(136, 192, 208))
        .add_modifier(Modifier::BOLD);
    style.highlight_style = Style::default()
        .fg(Color::Rgb(255, 255, 255))
        .bg(Color::Rgb(52, 66, 96))
        .add_modifier(Modifier::BOLD);
    style.title = Some(Line::from(format!(
        "{} (depth {})",
        args.root.display(),
        args.max_depth
    )));

    let terminal = ratatui::init();
    let result = run_app(terminal, model, columns, style);
    ratatui::restore();
    result
}
