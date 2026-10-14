// Copyright (c) Pavel Sich.
// Licensed under the MIT License.

//! A Turbo Vision style window manager.
//!
//! Windows are either docked into a tiling layout or float above it. They are moved by
//! dragging their title bar and docked by dropping them onto a screen edge or onto a side
//! of another docked window. The borders between docked windows and the edges of floating
//! windows can be dragged to resize them. Any window can be zoomed to fill the desktop.

use std::fmt::Write as _;
use std::fs;
use std::time::{Duration, Instant};

use edit::arena::scratch_arena;
use edit::helpers::*;
use edit::input::{InputMouseState, kbmod, vk};
use edit::json;
use edit::tui::Context;

use crate::settings;

const LAYOUT_FILE_NAME: &str = "layout.json";
/// How much of the desktop a window takes up when it gets docked to a screen edge.
const EDGE_DOCK_FRACTION: f32 = 0.3;
/// How close to a docked window's border a drop must be to dock next to it.
const SIDE_ZONE_FRACTION: f32 = 0.25;
const DOUBLE_CLICK_TIME: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PanelId {
    Editor,
    Assistant,
    Files,
}

impl PanelId {
    fn key(self) -> &'static str {
        match self {
            PanelId::Editor => "editor",
            PanelId::Assistant => "assistant",
            PanelId::Files => "files",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "editor" => Some(PanelId::Editor),
            "assistant" => Some(PanelId::Assistant),
            "files" => Some(PanelId::Files),
            _ => None,
        }
    }

    /// The smallest outer size (including the frame) the window can be shrunk to.
    fn min_size(self) -> Size {
        match self {
            PanelId::Editor => Size { width: 20, height: 5 },
            PanelId::Assistant | PanelId::Files => Size { width: 16, height: 4 },
        }
    }

    pub fn is_closable(self) -> bool {
        self != PanelId::Editor
    }

    fn default_dock_side(self) -> Side {
        match self {
            PanelId::Editor => Side::Left,
            PanelId::Assistant => Side::Bottom,
            PanelId::Files => Side::Right,
        }
    }
}

/// `Horizontal` places the children side by side, `Vertical` stacks them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    fn key(self) -> &'static str {
        match self {
            Axis::Horizontal => "horizontal",
            Axis::Vertical => "vertical",
        }
    }

    fn rect_length(self, rect: Rect) -> CoordType {
        match self {
            Axis::Horizontal => rect.width(),
            Axis::Vertical => rect.height(),
        }
    }

    fn size_length(self, size: Size) -> CoordType {
        match self {
            Axis::Horizontal => size.width,
            Axis::Vertical => size.height,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    fn axis(self) -> Axis {
        match self {
            Side::Left | Side::Right => Axis::Horizontal,
            Side::Top | Side::Bottom => Axis::Vertical,
        }
    }

    /// Whether a window docked to this side becomes the first child of the split.
    fn is_first(self) -> bool {
        matches!(self, Side::Left | Side::Top)
    }

    /// Returns the part of `rect` at this side that is `length` cells wide or high.
    fn strip(self, rect: Rect, length: CoordType) -> Rect {
        match self {
            Side::Left => Rect { right: rect.left + length, ..rect },
            Side::Right => Rect { left: rect.right - length, ..rect },
            Side::Top => Rect { bottom: rect.top + length, ..rect },
            Side::Bottom => Rect { top: rect.bottom - length, ..rect },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockNode {
    Panel(PanelId),
    Split(Box<Split>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Split {
    axis: Axis,
    /// The first child's share of the available space. [0, 1]
    ratio: f32,
    first: DockNode,
    second: DockNode,
}

impl DockNode {
    /// Creates a split with `new` docked at `side` of `existing`.
    /// `new` receives `fraction` of the space.
    fn split(side: Side, fraction: f32, new: DockNode, existing: DockNode) -> Self {
        let (ratio, first, second) = if side.is_first() {
            (fraction, new, existing)
        } else {
            (1.0 - fraction, existing, new)
        };
        DockNode::Split(Box::new(Split { axis: side.axis(), ratio, first, second }))
    }

    fn min_size(&self) -> Size {
        match self {
            DockNode::Panel(panel) => panel.min_size(),
            DockNode::Split(split) => {
                let a = split.first.min_size();
                let b = split.second.min_size();
                match split.axis {
                    Axis::Horizontal => {
                        Size { width: a.width + b.width, height: a.height.max(b.height) }
                    }
                    Axis::Vertical => {
                        Size { width: a.width.max(b.width), height: a.height + b.height }
                    }
                }
            }
        }
    }

    fn contains(&self, panel: PanelId) -> bool {
        match self {
            DockNode::Panel(p) => *p == panel,
            DockNode::Split(split) => split.first.contains(panel) || split.second.contains(panel),
        }
    }

    fn panels(&self, out: &mut Vec<PanelId>) {
        match self {
            DockNode::Panel(p) => out.push(*p),
            DockNode::Split(split) => {
                split.first.panels(out);
                split.second.panels(out);
            }
        }
    }

    /// Returns the path from this node to `panel`. `false` means first child, `true` second.
    fn path_of(&self, panel: PanelId) -> Option<Vec<bool>> {
        match self {
            DockNode::Panel(p) => (*p == panel).then(Vec::new),
            DockNode::Split(split) => {
                for (is_second, child) in [(false, &split.first), (true, &split.second)] {
                    if let Some(mut path) = child.path_of(panel) {
                        path.insert(0, is_second);
                        return Some(path);
                    }
                }
                None
            }
        }
    }

    fn split_at_path(&mut self, path: &[bool]) -> Option<&mut Split> {
        let DockNode::Split(split) = self else {
            return None;
        };
        match path.split_first() {
            None => Some(split),
            Some((&false, rest)) => split.first.split_at_path(rest),
            Some((&true, rest)) => split.second.split_at_path(rest),
        }
    }

    /// Replaces the `target` panel with a split of `target` and `new`, with `new` at `side`.
    fn dock_next_to(&mut self, target: PanelId, new: PanelId, side: Side) -> bool {
        if *self == DockNode::Panel(target) {
            *self = DockNode::split(side, 0.5, DockNode::Panel(new), DockNode::Panel(target));
            return true;
        }
        match self {
            DockNode::Panel(_) => false,
            DockNode::Split(split) => {
                split.first.dock_next_to(target, new, side)
                    || split.second.dock_next_to(target, new, side)
            }
        }
    }

    /// Returns this tree without `panel`. A split that loses a child is replaced by the other child.
    fn without(self, panel: PanelId) -> Option<DockNode> {
        match self {
            DockNode::Panel(p) => (p != panel).then_some(DockNode::Panel(p)),
            DockNode::Split(split) => {
                let Split { axis, ratio, first, second } = *split;
                match (first.without(panel), second.without(panel)) {
                    (Some(first), Some(second)) => {
                        Some(DockNode::Split(Box::new(Split { axis, ratio, first, second })))
                    }
                    (first, second) => first.or(second),
                }
            }
        }
    }
}

impl Split {
    /// The length of the first child along the split axis, honoring the minimum sizes.
    fn first_length(&self, rect: Rect) -> CoordType {
        let total = self.axis.rect_length(rect);
        let preferred = (total as f32 * self.ratio).round() as CoordType;
        self.clamp_first_length(preferred, total)
    }

    fn clamp_first_length(&self, length: CoordType, total: CoordType) -> CoordType {
        let min_first = self.axis.size_length(self.first.min_size());
        let min_second = self.axis.size_length(self.second.min_size());
        // If there isn't enough space for both, the first child wins.
        length.min(total - min_second).max(min_first).min(total).max(0)
    }

    fn set_first_length(&mut self, length: CoordType, rect: Rect) {
        let total = self.axis.rect_length(rect);
        if total > 0 {
            self.ratio = self.clamp_first_length(length, total) as f32 / total as f32;
        }
    }

    fn child_rects(&self, rect: Rect) -> (Rect, Rect) {
        let length = self.first_length(rect);
        match self.axis {
            Axis::Horizontal => {
                let x = rect.left + length;
                (Rect { right: x, ..rect }, Rect { left: x, ..rect })
            }
            Axis::Vertical => {
                let y = rect.top + length;
                (Rect { bottom: y, ..rect }, Rect { top: y, ..rect })
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloatingWindow {
    panel: PanelId,
    rect: Rect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    docked: Option<DockNode>,
    /// Ordered from back to front.
    floating: Vec<FloatingWindow>,
    zoomed: Option<PanelId>,
}

impl Default for Layout {
    /// The editor at the top left, the file browser to its right and the assistant below both.
    fn default() -> Self {
        let top = DockNode::split(
            Side::Right,
            0.28,
            DockNode::Panel(PanelId::Files),
            DockNode::Panel(PanelId::Editor),
        );
        let root = DockNode::split(Side::Bottom, 0.25, DockNode::Panel(PanelId::Assistant), top);
        Layout { docked: Some(root), floating: Vec::new(), zoomed: None }
    }
}

impl Layout {
    fn panels(&self) -> Vec<PanelId> {
        let mut panels = Vec::new();
        if let Some(root) = &self.docked {
            root.panels(&mut panels);
        }
        panels.extend(self.floating.iter().map(|w| w.panel));
        panels
    }

    fn is_valid(&self) -> bool {
        let panels = self.panels();
        let unique = panels.iter().enumerate().all(|(i, p)| !panels[..i].contains(p));
        unique && panels.contains(&PanelId::Editor)
    }

    pub fn to_json(&self) -> String {
        let mut out = String::from("{\n  \"docked\": ");
        match &self.docked {
            Some(root) => node_to_json(root, &mut out),
            None => out.push_str("null"),
        }
        out.push_str(",\n  \"floating\": [");
        for (i, w) in self.floating.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "\n    {{ \"panel\": \"{}\", \"left\": {}, \"top\": {}, \"width\": {}, \"height\": {} }}",
                w.panel.key(),
                w.rect.left,
                w.rect.top,
                w.rect.width(),
                w.rect.height(),
            );
        }
        if !self.floating.is_empty() {
            out.push_str("\n  ");
        }
        out.push_str("]\n}\n");
        out
    }

    pub fn from_json(text: &str) -> Option<Self> {
        let scratch = scratch_arena(None);
        let value = json::parse(&scratch, text).ok()?;
        let root = value.as_object()?;

        let docked = match root.get("docked") {
            Some(value) if !value.is_null() => Some(node_from_json(value, 0)?),
            _ => None,
        };

        let mut floating = Vec::new();
        for value in root.get_array("floating").unwrap_or(&[]) {
            let object = value.as_object()?;
            let number = |key: &str| object.get_number(key).map(|n| n as CoordType);
            let panel = PanelId::from_key(object.get_str("panel")?)?;
            let left = number("left")?;
            let top = number("top")?;
            let rect =
                Rect { left, top, right: left + number("width")?, bottom: top + number("height")? };
            floating.push(FloatingWindow { panel, rect });
        }

        let layout = Layout { docked, floating, zoomed: None };
        layout.is_valid().then_some(layout)
    }
}

fn node_to_json(node: &DockNode, out: &mut String) {
    match node {
        DockNode::Panel(panel) => {
            let _ = write!(out, "{{ \"panel\": \"{}\" }}", panel.key());
        }
        DockNode::Split(split) => {
            let _ = write!(
                out,
                "{{ \"axis\": \"{}\", \"ratio\": {:.3}, \"first\": ",
                split.axis.key(),
                split.ratio
            );
            node_to_json(&split.first, out);
            out.push_str(", \"second\": ");
            node_to_json(&split.second, out);
            out.push_str(" }");
        }
    }
}

fn node_from_json(value: &json::Value, depth: usize) -> Option<DockNode> {
    // There are only 3 panels, so any deeper nesting is invalid.
    if depth > 3 {
        return None;
    }
    let object = value.as_object()?;
    if let Some(key) = object.get_str("panel") {
        return PanelId::from_key(key).map(DockNode::Panel);
    }
    let axis = match object.get_str("axis")? {
        "horizontal" => Axis::Horizontal,
        "vertical" => Axis::Vertical,
        _ => return None,
    };
    let ratio = (object.get_number("ratio")? as f32).clamp(0.05, 0.95);
    let first = node_from_json(object.get("first")?, depth + 1)?;
    let second = node_from_json(object.get("second")?, depth + 1)?;
    Some(DockNode::Split(Box::new(Split { axis, ratio, first, second })))
}

/// A window as it appears on screen during this frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Frame {
    pub panel: PanelId,
    pub rect: Rect,
    pub floating: bool,
}

impl Frame {
    /// The `[■]` button at the left of the title bar.
    pub fn close_button(&self) -> Option<Rect> {
        (self.panel.is_closable() && self.rect.width() >= 12).then(|| Rect {
            left: self.rect.left + 2,
            top: self.rect.top,
            right: self.rect.left + 5,
            bottom: self.rect.top + 1,
        })
    }

    /// The `[↑]` button at the right of the title bar.
    pub fn zoom_button(&self) -> Option<Rect> {
        (self.rect.width() >= 8).then(|| Rect {
            left: self.rect.right - 5,
            top: self.rect.top,
            right: self.rect.right - 2,
            bottom: self.rect.top + 1,
        })
    }

    fn button_at(&self, pos: Point) -> Option<TitleButton> {
        if self.close_button().is_some_and(|r| r.contains(pos)) {
            Some(TitleButton::Close)
        } else if self.zoom_button().is_some_and(|r| r.contains(pos)) {
            Some(TitleButton::Zoom)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TitleButton {
    Close,
    Zoom,
}

/// The border between the two children of a docked split.
struct Splitter {
    path: Vec<bool>,
    axis: Axis,
    rect: Rect,
    /// The first cell of the second child along the split axis.
    position: CoordType,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DropTarget {
    Edge(Side),
    Panel(PanelId, Side),
    Float(Rect),
}

struct MoveDrag {
    panel: PanelId,
    floating: bool,
    grab: Point,
    /// Where the window was grabbed, relative to its top left corner.
    offset: Point,
    /// The size of the window once it floats.
    size: Size,
    started: bool,
    target: DropTarget,
}

#[derive(Clone, Copy)]
struct ResizeEdges {
    left: bool,
    right: bool,
    bottom: bool,
}

enum Drag {
    Splitter { path: Vec<bool>, grab_delta: CoordType },
    Move(MoveDrag),
    Resize { panel: PanelId, origin: Rect, grab: Point, edges: ResizeEdges },
}

pub struct WindowManager {
    layout: Layout,
    active: PanelId,
    focus_request: Option<PanelId>,
    keyboard_mode: Option<PanelId>,
    layout_changed: bool,

    drag: Option<Drag>,
    pressed_button: Option<(PanelId, TitleButton)>,
    last_title_click: Option<(PanelId, Point, Instant)>,
    prev_mouse_state: InputMouseState,
    prev_mouse_position: Point,

    // Computed by `begin_frame()`.
    work_area: Rect,
    frames: Vec<Frame>,
    splitters: Vec<Splitter>,
    preview: Option<Rect>,
}

impl WindowManager {
    /// Restores the layout saved by a previous session.
    pub fn load() -> Self {
        let layout = settings::config_file_path(LAYOUT_FILE_NAME)
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|text| Layout::from_json(&text))
            .unwrap_or_default();
        Self::new(layout)
    }

    pub fn new(layout: Layout) -> Self {
        Self {
            layout,
            active: PanelId::Editor,
            focus_request: Some(PanelId::Editor),
            keyboard_mode: None,
            layout_changed: false,

            drag: None,
            pressed_button: None,
            last_title_click: None,
            prev_mouse_state: InputMouseState::None,
            prev_mouse_position: Point::MIN,

            work_area: Rect::default(),
            frames: Vec::new(),
            splitters: Vec::new(),
            preview: None,
        }
    }

    /// Writes the layout to disk if it changed since the last call.
    /// Persisting the layout is a convenience, so failures are ignored.
    pub fn save_if_changed(&mut self) {
        if !self.layout_changed {
            return;
        }
        self.layout_changed = false;
        if let Some(path) = settings::config_file_path(LAYOUT_FILE_NAME) {
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(path, self.layout.to_json());
        }
    }

    /// Lays out the windows and handles mouse dragging and the keyboard move/size mode.
    /// Must be called at the start of every frame, before any window is drawn.
    pub fn begin_frame(&mut self, ctx: &mut Context) {
        let size = ctx.size();
        // Between the menubar and the statusbar.
        self.work_area =
            Rect { left: 0, top: 1, right: size.width, bottom: (size.height - 1).max(1) };
        self.compute_frames();

        // E.g. closing a menu or clicking a window border leaves nothing focused.
        if !ctx.is_anything_focused() && self.focus_request.is_none() {
            self.focus_request = Some(self.active);
        }

        let keyboard_changed = self.handle_keyboard_mode(ctx);
        let mouse_changed = self.handle_mouse(ctx);
        if keyboard_changed || mouse_changed {
            self.compute_frames();
            ctx.needs_rerender();
        }
    }

    /// The visible windows, ordered from back to front.
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// The area a dragged window would occupy if it was dropped now.
    pub fn preview(&self) -> Option<Rect> {
        self.preview
    }

    pub fn active(&self) -> PanelId {
        self.active
    }

    pub fn set_active(&mut self, panel: PanelId) {
        self.active = panel;
    }

    pub fn is_zoomed(&self, panel: PanelId) -> bool {
        self.layout.zoomed == Some(panel)
    }

    pub fn is_open(&self, panel: PanelId) -> bool {
        self.layout.docked.as_ref().is_some_and(|root| root.contains(panel))
            || self.layout.floating.iter().any(|w| w.panel == panel)
    }

    pub fn keyboard_mode(&self) -> Option<PanelId> {
        self.keyboard_mode
    }

    /// Moves the keyboard focus into the given window during the next frame.
    pub fn focus(&mut self, panel: PanelId) {
        self.active = panel;
        self.focus_request = Some(panel);
        self.raise(panel);
    }

    /// Returns true once if the window should grab the keyboard focus.
    pub fn take_focus_request(&mut self, panel: PanelId) -> bool {
        if self.focus_request == Some(panel) {
            self.focus_request = None;
            true
        } else {
            false
        }
    }

    pub fn toggle(&mut self, panel: PanelId) {
        if self.is_open(panel) { self.close(panel) } else { self.open(panel) }
    }

    pub fn open(&mut self, panel: PanelId) {
        if !self.is_open(panel) {
            self.dock(panel, DropTarget::Edge(panel.default_dock_side()));
        }
        self.focus(panel);
    }

    pub fn close(&mut self, panel: PanelId) {
        if !panel.is_closable() {
            return;
        }
        self.remove(panel);
        if self.keyboard_mode == Some(panel) {
            self.keyboard_mode = None;
        }
        if self.active == panel {
            self.focus(PanelId::Editor);
        }
        self.layout_changed = true;
    }

    pub fn toggle_zoom(&mut self, panel: PanelId) {
        if self.is_open(panel) {
            self.layout.zoomed = if self.is_zoomed(panel) { None } else { Some(panel) };
            self.focus(panel);
        }
    }

    pub fn reset_layout(&mut self) {
        self.layout = Layout::default();
        self.keyboard_mode = None;
        self.layout_changed = true;
        self.focus(PanelId::Editor);
    }

    /// Activates the next (or previous) visible window.
    pub fn cycle(&mut self, forward: bool) {
        let panels: Vec<PanelId> = self.frames.iter().map(|f| f.panel).collect();
        if panels.is_empty() {
            return;
        }
        let current = panels.iter().position(|&p| p == self.active).unwrap_or(0);
        let next = if forward {
            (current + 1) % panels.len()
        } else {
            (current + panels.len() - 1) % panels.len()
        };
        self.focus(panels[next]);
    }

    /// Enters the Ctrl+F5 mode in which the arrow keys move and size the active window.
    pub fn start_keyboard_mode(&mut self) {
        if self.layout.zoomed.is_none() && self.is_open(self.active) {
            self.keyboard_mode = Some(self.active);
        }
    }

    fn compute_frames(&mut self) {
        self.frames.clear();
        self.splitters.clear();

        let work = self.work_area;
        for w in &mut self.layout.floating {
            w.rect = clamp_floating(w.rect, w.panel.min_size(), work);
        }

        if let Some(panel) = self.layout.zoomed {
            if self.is_open(panel) {
                self.frames.push(Frame { panel, rect: work, floating: false });
                return;
            }
            self.layout.zoomed = None;
        }

        if let Some(root) = &self.layout.docked {
            layout_node(root, work, &mut Vec::new(), &mut self.frames, &mut self.splitters);
        }
        self.frames.extend(self.layout.floating.iter().map(|w| Frame {
            panel: w.panel,
            rect: w.rect,
            floating: true,
        }));
    }

    fn frame(&self, panel: PanelId) -> Option<Frame> {
        self.frames.iter().find(|f| f.panel == panel).copied()
    }

    fn floating_mut(&mut self, panel: PanelId) -> Option<&mut FloatingWindow> {
        self.layout.floating.iter_mut().find(|w| w.panel == panel)
    }

    fn raise(&mut self, panel: PanelId) {
        if let Some(index) = self.layout.floating.iter().position(|w| w.panel == panel) {
            let window = self.layout.floating.remove(index);
            self.layout.floating.push(window);
        }
    }

    fn remove(&mut self, panel: PanelId) {
        self.layout.floating.retain(|w| w.panel != panel);
        self.layout.docked = self.layout.docked.take().and_then(|root| root.without(panel));
        if self.layout.zoomed == Some(panel) {
            self.layout.zoomed = None;
        }
    }

    fn dock(&mut self, panel: PanelId, target: DropTarget) {
        self.remove(panel);
        let target = match target {
            DropTarget::Panel(other, side)
                if self
                    .layout
                    .docked
                    .as_mut()
                    .is_some_and(|r| r.dock_next_to(other, panel, side)) =>
            {
                None
            }
            // The target window is gone. This shouldn't happen, but docking to its side is close enough.
            DropTarget::Panel(_, side) => Some(DropTarget::Edge(side)),
            target => Some(target),
        };
        match target {
            Some(DropTarget::Edge(side)) => {
                let new = DockNode::Panel(panel);
                self.layout.docked = Some(match self.layout.docked.take() {
                    Some(root) => DockNode::split(side, EDGE_DOCK_FRACTION, new, root),
                    None => new,
                });
            }
            Some(DropTarget::Float(rect)) => {
                self.layout.floating.push(FloatingWindow { panel, rect });
            }
            Some(DropTarget::Panel(..)) | None => {}
        }
        self.layout.zoomed = None;
        self.layout_changed = true;
        self.focus(panel);
    }

    fn handle_keyboard_mode(&mut self, ctx: &mut Context) -> bool {
        let Some(panel) = self.keyboard_mode else {
            return false;
        };
        if !self.is_open(panel) || self.layout.zoomed.is_some() {
            self.keyboard_mode = None;
            return true;
        }
        let Some(key) = ctx.keyboard_input() else {
            return false;
        };
        // While moving and sizing, all keys are swallowed so that they don't edit the document.
        ctx.set_input_consumed();

        if key == vk::RETURN || key == vk::ESCAPE {
            self.keyboard_mode = None;
            self.layout_changed = true;
            return true;
        }

        let arrows = [(vk::LEFT, -1, 0), (vk::RIGHT, 1, 0), (vk::UP, 0, -1), (vk::DOWN, 0, 1)];
        let Some((resize, dx, dy)) = arrows.iter().find_map(|&(arrow, dx, dy)| {
            if key == arrow {
                Some((false, dx, dy))
            } else if key == kbmod::SHIFT | arrow {
                Some((true, dx, dy))
            } else {
                None
            }
        }) else {
            return false;
        };

        let work = self.work_area;
        if let Some(window) = self.floating_mut(panel) {
            let min = panel.min_size();
            let r = &mut window.rect;
            if resize {
                r.right = (r.right + dx).max(r.left + min.width).min(work.right);
                r.bottom = (r.bottom + dy).max(r.top + min.height).min(work.bottom);
            } else {
                *r = clamp_floating(
                    Rect {
                        left: r.left + dx,
                        top: r.top + dy,
                        right: r.right + dx,
                        bottom: r.bottom + dy,
                    },
                    min,
                    work,
                );
            }
        } else {
            // Docked windows can only be resized: move the nearest border along the arrow's axis.
            let axis = if dx != 0 { Axis::Horizontal } else { Axis::Vertical };
            self.nudge_splitter(panel, axis, dx + dy);
        }
        true
    }

    fn nudge_splitter(&mut self, panel: PanelId, axis: Axis, delta: CoordType) {
        let Some(root) = &mut self.layout.docked else {
            return;
        };
        let Some(panel_path) = root.path_of(panel) else {
            return;
        };
        let Some(splitter) = self
            .splitters
            .iter()
            .filter(|s| s.axis == axis && panel_path.starts_with(&s.path))
            .max_by_key(|s| s.path.len())
        else {
            return;
        };
        let start = match axis {
            Axis::Horizontal => splitter.rect.left,
            Axis::Vertical => splitter.rect.top,
        };
        if let Some(split) = root.split_at_path(&splitter.path) {
            split.set_first_length(splitter.position - start + delta, splitter.rect);
        }
    }

    fn handle_mouse(&mut self, ctx: &Context) -> bool {
        let state = ctx.mouse_state();
        let pos = ctx.mouse_position();
        let prev = std::mem::replace(&mut self.prev_mouse_state, state);
        let prev_pos = std::mem::replace(&mut self.prev_mouse_position, pos);

        if state == InputMouseState::Left {
            if prev != InputMouseState::Left {
                self.on_press(ctx, pos)
            } else if pos != prev_pos {
                // Every input is drawn several times until the UI settles.
                // Reacting to an unchanged position would keep it from ever settling.
                self.on_drag(pos)
            } else {
                false
            }
        } else if self.drag.is_some() || self.pressed_button.is_some() {
            self.on_release(pos)
        } else {
            false
        }
    }

    fn on_press(&mut self, ctx: &Context, pos: Point) -> bool {
        if ctx.is_modal_open() || !self.work_area.contains(pos) {
            return false;
        }

        // Floating windows are on top of everything else.
        if let Some(frame) = self.frames.iter().rev().find(|f| f.floating && f.rect.contains(pos)) {
            let frame = *frame;
            self.focus(frame.panel);
            let r = frame.rect;
            if pos.y == r.top {
                self.press_title(frame, pos);
            } else {
                let edges = ResizeEdges {
                    left: pos.x == r.left,
                    right: pos.x == r.right - 1,
                    bottom: pos.y == r.bottom - 1,
                };
                if edges.left || edges.right || edges.bottom {
                    self.drag =
                        Some(Drag::Resize { panel: frame.panel, origin: r, grab: pos, edges });
                }
            }
            return true;
        }

        if let Some(splitter) = self.splitters.iter().find(|s| s.hit(pos)) {
            let grab = match splitter.axis {
                Axis::Horizontal => pos.x,
                Axis::Vertical => pos.y,
            };
            self.drag = Some(Drag::Splitter {
                path: splitter.path.clone(),
                grab_delta: grab - splitter.position,
            });
            // Clicking a border takes the focus away from the active window. Give it back.
            self.focus(self.active);
            return true;
        }

        if let Some(frame) = self.frames.iter().rev().find(|f| f.rect.contains(pos)) {
            let frame = *frame;
            self.focus(frame.panel);
            if pos.y == frame.rect.top {
                self.press_title(frame, pos);
            }
            return true;
        }

        false
    }

    fn press_title(&mut self, frame: Frame, pos: Point) {
        if let Some(button) = frame.button_at(pos) {
            self.pressed_button = Some((frame.panel, button));
            return;
        }

        let now = Instant::now();
        let double_click = self.last_title_click.is_some_and(|(panel, last_pos, time)| {
            panel == frame.panel && last_pos == pos && now - time < DOUBLE_CLICK_TIME
        });
        if double_click {
            self.last_title_click = None;
            self.toggle_zoom(frame.panel);
            return;
        }
        self.last_title_click = Some((frame.panel, pos, now));

        if self.layout.zoomed.is_some() {
            return;
        }

        let work = self.work_area;
        let size = if frame.floating {
            Size { width: frame.rect.width(), height: frame.rect.height() }
        } else {
            // Docked windows tend to be large. Make them float at a more manageable size.
            let min = frame.panel.min_size();
            Size {
                width: frame.rect.width().min(work.width() * 3 / 5).max(min.width),
                height: frame.rect.height().min(work.height() * 3 / 5).max(min.height),
            }
        };
        let offset = Point { x: (pos.x - frame.rect.left).min(size.width - 6).max(1), y: 0 };
        self.drag = Some(Drag::Move(MoveDrag {
            panel: frame.panel,
            floating: frame.floating,
            grab: pos,
            offset,
            size,
            started: false,
            target: DropTarget::Float(frame.rect),
        }));
    }

    fn on_drag(&mut self, pos: Point) -> bool {
        let Some(mut drag) = self.drag.take() else {
            return false;
        };
        let changed = match &mut drag {
            Drag::Splitter { path, grab_delta } => self.drag_splitter(path, *grab_delta, pos),
            Drag::Move(m) => self.drag_move(m, pos),
            Drag::Resize { panel, origin, grab, edges } => {
                self.drag_resize(*panel, *origin, *grab, *edges, pos)
            }
        };
        self.drag = Some(drag);
        changed
    }

    fn drag_splitter(&mut self, path: &[bool], grab_delta: CoordType, pos: Point) -> bool {
        let Some(splitter) = self.splitters.iter().find(|s| s.path == path) else {
            return false;
        };
        let (mouse, start) = match splitter.axis {
            Axis::Horizontal => (pos.x, splitter.rect.left),
            Axis::Vertical => (pos.y, splitter.rect.top),
        };
        let rect = splitter.rect;
        let Some(split) = self.layout.docked.as_mut().and_then(|r| r.split_at_path(path)) else {
            return false;
        };
        split.set_first_length(mouse - grab_delta - start, rect);
        true
    }

    fn drag_move(&mut self, m: &mut MoveDrag, pos: Point) -> bool {
        if !m.started && pos == m.grab {
            return false;
        }
        m.started = true;

        let left = pos.x - m.offset.x;
        let top = pos.y - m.offset.y;
        let float_rect = clamp_floating(
            Rect { left, top, right: left + m.size.width, bottom: top + m.size.height },
            m.panel.min_size(),
            self.work_area,
        );
        if m.floating
            && let Some(window) = self.floating_mut(m.panel)
        {
            window.rect = float_rect;
        }

        m.target = self.drop_target(m.panel, pos, float_rect);
        self.preview = match m.target {
            // A floating window that keeps floating is moved live and needs no preview.
            DropTarget::Float(_) if m.floating => None,
            target => Some(self.target_rect(m.panel, target)),
        };
        true
    }

    fn drag_resize(
        &mut self,
        panel: PanelId,
        origin: Rect,
        grab: Point,
        edges: ResizeEdges,
        pos: Point,
    ) -> bool {
        let work = self.work_area;
        let min = panel.min_size();
        let dx = pos.x - grab.x;
        let dy = pos.y - grab.y;
        let Some(window) = self.floating_mut(panel) else {
            return false;
        };
        let mut r = origin;
        if edges.left {
            r.left = (origin.left + dx).min(origin.right - min.width).max(work.left);
        }
        if edges.right {
            r.right = (origin.right + dx).max(origin.left + min.width).min(work.right);
        }
        if edges.bottom {
            r.bottom = (origin.bottom + dy).max(origin.top + min.height).min(work.bottom);
        }
        window.rect = r;
        true
    }

    fn on_release(&mut self, pos: Point) -> bool {
        self.preview = None;

        if let Some((panel, button)) = self.pressed_button.take() {
            if self.frame(panel).and_then(|f| f.button_at(pos)) == Some(button) {
                match button {
                    TitleButton::Close => self.close(panel),
                    TitleButton::Zoom => self.toggle_zoom(panel),
                }
            }
            return true;
        }

        match self.drag.take() {
            Some(Drag::Move(m)) if m.started => match m.target {
                DropTarget::Float(_) if m.floating => self.layout_changed = true,
                target => self.dock(m.panel, target),
            },
            Some(Drag::Move(_)) | None => {}
            Some(Drag::Splitter { .. } | Drag::Resize { .. }) => self.layout_changed = true,
        }
        true
    }

    fn drop_target(&self, moving: PanelId, pos: Point, float_rect: Rect) -> DropTarget {
        let work = self.work_area;
        if pos.x <= work.left {
            return DropTarget::Edge(Side::Left);
        }
        if pos.x >= work.right - 1 {
            return DropTarget::Edge(Side::Right);
        }
        if pos.y < work.top {
            return DropTarget::Edge(Side::Top);
        }
        if pos.y >= work.bottom {
            return DropTarget::Edge(Side::Bottom);
        }

        let docked = self.frames.iter().filter(|f| !f.floating && f.panel != moving);
        if let Some(frame) = docked.into_iter().find(|f| f.rect.contains(pos)) {
            let r = frame.rect;
            let fx = (pos.x - r.left) as f32 / r.width().max(1) as f32;
            let fy = (pos.y - r.top) as f32 / r.height().max(1) as f32;
            let sides = [
                (Side::Left, fx),
                (Side::Right, 1.0 - fx),
                (Side::Top, fy),
                (Side::Bottom, 1.0 - fy),
            ];
            let (side, distance) =
                sides.into_iter().min_by(|a, b| a.1.total_cmp(&b.1)).unwrap_or((Side::Left, 1.0));
            if distance < SIDE_ZONE_FRACTION {
                return DropTarget::Panel(frame.panel, side);
            }
        }

        DropTarget::Float(float_rect)
    }

    fn target_rect(&self, panel: PanelId, target: DropTarget) -> Rect {
        match target {
            DropTarget::Edge(side) => {
                let work = self.work_area;
                let axis = side.axis();
                let total = axis.rect_length(work);
                let length = ((total as f32 * EDGE_DOCK_FRACTION).round() as CoordType)
                    .max(axis.size_length(panel.min_size()))
                    .min(total);
                side.strip(work, length)
            }
            DropTarget::Panel(other, side) => match self.frame(other) {
                Some(frame) => {
                    let length = side.axis().rect_length(frame.rect) / 2;
                    side.strip(frame.rect, length)
                }
                None => self.work_area,
            },
            DropTarget::Float(rect) => rect,
        }
    }
}

impl Splitter {
    /// Between side by side windows, both adjacent borders form the splitter.
    /// Between stacked windows, only the upper window's bottom border does,
    /// because the lower window's top border is its title bar.
    fn hit(&self, pos: Point) -> bool {
        if !self.rect.contains(pos) {
            return false;
        }
        match self.axis {
            Axis::Horizontal => pos.x == self.position - 1 || pos.x == self.position,
            Axis::Vertical => pos.y == self.position - 1,
        }
    }
}

fn layout_node(
    node: &DockNode,
    rect: Rect,
    path: &mut Vec<bool>,
    frames: &mut Vec<Frame>,
    splitters: &mut Vec<Splitter>,
) {
    match node {
        DockNode::Panel(panel) => frames.push(Frame { panel: *panel, rect, floating: false }),
        DockNode::Split(split) => {
            let (first, second) = split.child_rects(rect);
            let position = match split.axis {
                Axis::Horizontal => second.left,
                Axis::Vertical => second.top,
            };
            splitters.push(Splitter { path: path.clone(), axis: split.axis, rect, position });

            path.push(false);
            layout_node(&split.first, first, path, frames, splitters);
            path.pop();

            path.push(true);
            layout_node(&split.second, second, path, frames, splitters);
            path.pop();
        }
    }
}

/// Keeps a floating window inside the desktop and at least at its minimum size.
fn clamp_floating(rect: Rect, min: Size, work: Rect) -> Rect {
    let width = rect.width().clamp(min.width.min(work.width()), work.width().max(0));
    let height = rect.height().clamp(min.height.min(work.height()), work.height().max(0));
    let left = rect.left.clamp(work.left, work.right - width);
    let top = rect.top.clamp(work.top, work.bottom - height);
    Rect { left, top, right: left + width, bottom: top + height }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work() -> Rect {
        Rect { left: 0, top: 1, right: 100, bottom: 31 }
    }

    fn manager(layout: Layout) -> WindowManager {
        let mut wm = WindowManager::new(layout);
        wm.work_area = work();
        wm.compute_frames();
        wm
    }

    #[test]
    fn test_default_layout_fills_desktop() {
        let wm = manager(Layout::default());
        let area: CoordType = wm.frames().iter().map(|f| f.rect.width() * f.rect.height()).sum();
        assert_eq!(area, work().width() * work().height());
        assert_eq!(wm.frames().len(), 3);
    }

    #[test]
    fn test_close_and_reopen() {
        let mut wm = manager(Layout::default());
        wm.close(PanelId::Files);
        wm.close(PanelId::Editor);
        assert!(!wm.is_open(PanelId::Files));
        assert!(wm.is_open(PanelId::Editor));

        wm.open(PanelId::Files);
        wm.compute_frames();
        let files = wm.frame(PanelId::Files).unwrap();
        assert_eq!(files.rect.right, work().right);
        assert_eq!(files.rect.top, work().top);
    }

    #[test]
    fn test_dock_next_to_panel() {
        let mut wm = manager(Layout::default());
        wm.dock(PanelId::Assistant, DropTarget::Panel(PanelId::Editor, Side::Left));
        wm.compute_frames();
        let editor = wm.frame(PanelId::Editor).unwrap();
        let assistant = wm.frame(PanelId::Assistant).unwrap();
        assert_eq!(assistant.rect.right, editor.rect.left);
        assert_eq!(assistant.rect.top, editor.rect.top);
        assert_eq!(assistant.rect.bottom, editor.rect.bottom);
    }

    #[test]
    fn test_float_and_clamp() {
        let mut wm = manager(Layout::default());
        let rect = Rect { left: 90, top: 25, right: 130, bottom: 40 };
        wm.dock(PanelId::Files, DropTarget::Float(rect));
        wm.compute_frames();
        let files = wm.frame(PanelId::Files).unwrap();
        assert!(files.floating);
        assert_eq!(files.rect.right, work().right);
        assert_eq!(files.rect.bottom, work().bottom);
        assert_eq!(files.rect.width(), 40);
    }

    #[test]
    fn test_splitter_respects_min_size() {
        let mut wm = manager(Layout::default());
        let path = wm.splitters.iter().find(|s| s.axis == Axis::Horizontal).unwrap().path.clone();
        wm.drag_splitter(&path, 0, Point { x: 2, y: 5 });
        wm.compute_frames();
        let editor = wm.frame(PanelId::Editor).unwrap();
        assert_eq!(editor.rect.width(), PanelId::Editor.min_size().width);
    }

    #[test]
    fn test_json_roundtrip() {
        let mut layout = Layout::default();
        layout.docked = layout.docked.take().and_then(|r| r.without(PanelId::Assistant));
        layout.floating.push(FloatingWindow {
            panel: PanelId::Assistant,
            rect: Rect { left: 5, top: 6, right: 45, bottom: 16 },
        });

        let parsed = Layout::from_json(&layout.to_json()).unwrap();
        assert_eq!(parsed.floating, layout.floating);
        let mut panels = parsed.panels();
        panels.sort_by_key(|p| p.key());
        assert_eq!(panels, [PanelId::Assistant, PanelId::Editor, PanelId::Files]);
    }

    #[test]
    fn test_json_rejects_invalid_layouts() {
        assert!(Layout::from_json("{\"docked\": null, \"floating\": []}").is_none());
        assert!(
            Layout::from_json(
                "{\"docked\": {\"axis\": \"vertical\", \"ratio\": 0.5, \
                 \"first\": {\"panel\": \"editor\"}, \"second\": {\"panel\": \"editor\"}}}"
            )
            .is_none()
        );
    }
}
