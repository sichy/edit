// Copyright (c) Pavel Sich.
// Licensed under the MIT License.

//! AI Assistant window, modeled after Cursor's chat: a header with the chat title,
//! the history with the newest message at the bottom, and the composer below it.
//!
//! The composer is a card with the attached files, the prompt and a toolbar
//! with the mode and model pickers and the send button.

use std::{env, fs};

use edit::helpers::*;
use edit::input::vk;
use edit::tui::*;
use edit::unicode::MeasurementConfig;

use crate::dock::PanelId;
use crate::draw_windows::truncate;
use crate::state::*;
use crate::theme::Theme;

const HEADER_HEIGHT: CoordType = 1;
/// The prompt starts out this tall, so that it doesn't feel cramped.
const PROMPT_MIN_LINES: CoordType = 2;
/// The prompt grows with its contents up to this many lines, then scrolls.
const PROMPT_MAX_LINES: CoordType = 8;
const NEW_CHAT_LABEL: &str = " + New ";
/// The width of [`NEW_CHAT_LABEL`] including its padding.
const NEW_CHAT_WIDTH: CoordType = 7;
const ADD_CONTEXT_LABEL: &str = " + ";
const SEND_LABEL: &str = " ↑ ";
/// The width of the round buttons at both ends of the toolbar.
const ROUND_BUTTON_WIDTH: CoordType = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub text: String,
    /// The files that were attached to a user message.
    pub context: Vec<String>,
}

/// What the assistant is asked to do with a prompt, like in Cursor.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ChatMode {
    #[default]
    Agent,
    Ask,
    Plan,
}

impl ChatMode {
    const ALL: [ChatMode; 3] = [ChatMode::Agent, ChatMode::Ask, ChatMode::Plan];

    fn label(self) -> &'static str {
        match self {
            ChatMode::Agent => "Agent",
            ChatMode::Ask => "Ask",
            ChatMode::Plan => "Plan",
        }
    }

    fn placeholder(self) -> &'static str {
        match self {
            ChatMode::Agent => "Describe a change…",
            ChatMode::Ask => "Ask a question…",
            ChatMode::Plan => "Describe what to plan…",
        }
    }
}

pub struct Model {
    /// Shown in the toolbar, where space is tight.
    short: &'static str,
    name: &'static str,
}

pub const MODELS: [Model; 4] = [
    Model { short: "Auto", name: "Auto" },
    Model { short: "Opus", name: "Claude Opus 5.5" },
    Model { short: "Sonnet", name: "Claude Sonnet 5.5" },
    Model { short: "GPT", name: "GPT-5.6" },
];

/// The composer's menu that is currently open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChatPicker {
    Mode,
    Model,
}

/// Draws the assistant into a window of the given inner `size`.
pub fn draw_ai_assistant(ctx: &mut Context, state: &mut State, size: Size, grab_focus: bool) {
    ctx.block_begin("ai_assistant");
    ctx.attr_intrinsic_size(size);
    let focused = ctx.contains_focus();
    {
        let prompt_lines =
            state.ai_prompt.borrow().visual_line_count().clamp(PROMPT_MIN_LINES, PROMPT_MAX_LINES);
        let history_height =
            (size.height - HEADER_HEIGHT - composer_height(state, prompt_lines)).max(0);

        draw_header(ctx, state, size.width);
        draw_history(ctx, state, Size { width: size.width, height: history_height });
        draw_composer(ctx, state, size.width, prompt_lines, grab_focus);
    }
    ctx.block_end();

    if focused && ctx.consume_shortcut(vk::ESCAPE) {
        // Return to the editor, like leaving a dialog.
        state.windows.focus(PanelId::Editor);
        ctx.needs_rerender();
    }
}

/// The card's border, its rows and the line with the branch below it.
fn composer_height(state: &State, prompt_lines: CoordType) -> CoordType {
    let chips = !state.ai_context.is_empty() as CoordType;
    let branch = state.ai_branch.is_some() as CoordType;
    2 + chips + prompt_lines + 1 + branch
}

/// A tinted bar with the chat's title and the button to start a new chat.
fn draw_header(ctx: &mut Context, state: &mut State, width: CoordType) {
    let title = state
        .ai_messages
        .iter()
        .find(|m| m.role == ChatRole::User)
        .map_or("New Chat", |m| m.text.lines().next().unwrap_or_default());
    let has_button = !state.ai_messages.is_empty();
    let title_width = width - 2 - if has_button { NEW_CHAT_WIDTH + 1 } else { 0 };

    ctx.table_begin("header");
    ctx.table_set_cell_gap(Size { width: 1, height: 0 });
    ctx.attr_padding(Rect::two(0, 1));
    ctx.attr_background_rgba(state.theme.chrome);
    {
        ctx.table_next_row();

        // The title is padded to its full width, so that the button sits at the right edge.
        let (title, title_len) = truncate(&format!("✦ {title}"), title_width.max(0) as usize);
        let padding = (title_width.max(0) as usize).saturating_sub(title_len);
        ctx.label("title", &format!("{title}{:padding$}", ""));
        ctx.attr_foreground_rgba(state.theme.text);

        if has_button
            && ctx.button("new_chat", NEW_CHAT_LABEL, ButtonStyle::default().bracketed(false))
        {
            state.ai_messages.clear();
            ctx.needs_rerender();
        }
        ctx.attr_foreground_rgba(state.theme.text_dim);
    }
    ctx.table_end();
}

fn draw_history(ctx: &mut Context, state: &mut State, size: Size) {
    if size.height <= 0 {
        return;
    }

    ctx.scrollarea_begin("history", size);
    if state.ai_scroll_to_bottom {
        state.ai_scroll_to_bottom = false;
        // The scroll offset is clamped during layout, so this lands on the last line.
        ctx.scrollarea_scroll_to(Point { x: 0, y: CoordType::MAX });
    }
    {
        // The scrollbar takes up the last column and each message has 1 column of margin per side.
        let text_width = size.width - 3;
        let theme = state.theme;

        if state.ai_messages.is_empty() {
            draw_empty_state(ctx, &theme, text_width);
        }

        for (index, message) in state.ai_messages.iter().enumerate() {
            ctx.next_block_id_mixin(index as u64);
            ctx.block_begin("message");
            ctx.attr_padding(Rect::three(1, 1, 0));
            match message.role {
                ChatRole::User => {
                    ctx.block_begin("bubble");
                    ctx.attr_padding(Rect::two(0, 1));
                    ctx.attr_background_rgba(theme.elevated);
                    draw_lines(ctx, &message.text, text_width - 2);
                    if !message.context.is_empty() {
                        ctx.label("context", &format!("@ {}", message.context.join(", ")));
                        ctx.attr_overflow(Overflow::TruncateTail);
                        ctx.attr_foreground_rgba(theme.text_dim);
                    }
                    ctx.block_end();
                }
                ChatRole::Assistant => {
                    ctx.label("role", "✦ Assistant");
                    ctx.attr_foreground_rgba(theme.accent);
                    draw_lines(ctx, &message.text, text_width);
                }
            }
            ctx.block_end();
        }
    }
    ctx.scrollarea_end();
}

/// Explains the composer's keys, since they're not visible anywhere else.
fn draw_empty_state(ctx: &mut Context, theme: &Theme, width: CoordType) {
    const TIPS: [&str; 3] = [
        "Enter sends, Alt+Enter starts a new line.",
        "+ attaches the open file.",
        "↑ in an empty prompt brings back your last message.",
    ];

    ctx.block_begin("empty");
    ctx.attr_padding(Rect::three(1, 1, 0));
    {
        ctx.label("title", "Ask about your code");
        ctx.attr_foreground_rgba(theme.text);
        for (index, tip) in TIPS.iter().enumerate() {
            ctx.next_block_id_mixin(index as u64);
            ctx.block_begin("tip");
            ctx.attr_padding(Rect::three(1, 0, 0));
            ctx.attr_foreground_rgba(theme.text_dim);
            draw_lines(ctx, tip, width);
            ctx.block_end();
        }
    }
    ctx.block_end();
}

fn draw_lines(ctx: &mut Context, text: &str, width: CoordType) {
    for (index, line) in wrap_text(text, width).iter().enumerate() {
        ctx.next_block_id_mixin(index as u64);
        ctx.label("line", line);
    }
}

/// The card with the attached files, the prompt and the toolbar,
/// followed by the current git branch. The border lights up while it has the focus.
fn draw_composer(
    ctx: &mut Context,
    state: &mut State,
    width: CoordType,
    prompt_lines: CoordType,
    grab_focus: bool,
) {
    let theme = state.theme;
    // 1 column of margin, the border and 1 column of padding on each side.
    let content_width = (width - 6).max(0);
    let has_text = state.ai_prompt.borrow().text_length() > 0;
    let chips = !state.ai_context.is_empty() as CoordType;

    ctx.block_begin("composer_area");
    ctx.attr_padding(Rect::two(0, 1));
    {
        ctx.block_begin("composer");
        ctx.attr_border();
        ctx.attr_padding(Rect::two(0, 1));
        ctx.attr_intrinsic_size(Size { width: content_width, height: chips + prompt_lines + 1 });
        ctx.attr_background_rgba(theme.elevated);
        let focused = ctx.contains_focus();
        ctx.attr_foreground_rgba(if focused { theme.accent } else { theme.border });
        {
            if focused && ctx.consume_shortcut(vk::ESCAPE) {
                if state.ai_picker.take().is_some() {
                    state.ai_focus_prompt = true;
                } else {
                    state.windows.focus(PanelId::Editor);
                }
                ctx.needs_rerender();
            }

            // The prompt is multi-line, so it would otherwise insert a newline on Enter.
            // Alt+Enter still inserts a newline. An open menu needs Enter for itself.
            let prompt_keys = focused && state.ai_picker.is_none();
            let mut send = prompt_keys && ctx.consume_shortcut(vk::RETURN);
            if prompt_keys && !has_text && ctx.consume_shortcut(vk::UP) {
                recall_last_prompt(state);
                ctx.needs_rerender();
            }

            if chips > 0 {
                draw_context_chips(ctx, state, content_width);
            }

            ctx.textarea("prompt", state.ai_prompt.clone());
            ctx.inherit_focus();
            ctx.attr_intrinsic_size(Size { width: content_width, height: prompt_lines });
            if ctx.attr_fit_height(PROMPT_MIN_LINES, PROMPT_MAX_LINES) != prompt_lines {
                // The history above was sized for the old prompt height.
                ctx.needs_rerender();
            }
            ctx.attr_foreground_rgba(theme.text);
            ctx.attr_placeholder(state.ai_mode.placeholder());
            if grab_focus || std::mem::take(&mut state.ai_focus_prompt) {
                ctx.steal_focus();
            }

            send |= draw_toolbar(ctx, state, content_width, has_text);

            if send && send_prompt(state) {
                // Clicking the send button moved the focus away from the prompt.
                state.ai_focus_prompt = true;
                ctx.needs_rerender();
            }
        }
        ctx.block_end();

        if let Some(branch) = &state.ai_branch {
            ctx.label("branch", &format!("git:{branch}"));
            ctx.attr_padding(Rect::two(0, 2));
            ctx.attr_overflow(Overflow::TruncateTail);
            ctx.attr_foreground_rgba(theme.text_dim);
        }
    }
    ctx.block_end();
}

/// The attached files as pills. Clicking one detaches it.
fn draw_context_chips(ctx: &mut Context, state: &mut State, width: CoordType) {
    let theme = state.theme;
    let mut removed = None;

    ctx.table_begin("chips");
    ctx.table_set_cell_gap(Size { width: 1, height: 0 });
    ctx.attr_intrinsic_size(Size { width, height: 1 });
    {
        ctx.table_next_row();
        for (index, name) in state.ai_context.iter().enumerate() {
            ctx.next_block_id_mixin(index as u64);
            let label = format!(" {name} × ");
            if ctx.button("chip", &label, ButtonStyle::default().bracketed(false)) {
                removed = Some(index);
            }
            ctx.attr_background_rgba(theme.chrome);
            ctx.attr_foreground_rgba(theme.text);
        }
    }
    ctx.table_end();

    if let Some(index) = removed {
        state.ai_context.remove(index);
        state.ai_focus_prompt = true;
        ctx.needs_rerender();
    }
}

/// `+` on the left, then the mode and model pickers and the send button on the right.
/// Returns true if the send button was pressed.
fn draw_toolbar(ctx: &mut Context, state: &mut State, width: CoordType, has_text: bool) -> bool {
    let theme = state.theme;
    let style = ButtonStyle::default().bracketed(false);
    let mode_label = format!("{} ▾", state.ai_mode.label());
    let model_label = format!("{} ▾", MODELS[state.ai_model].short);
    let layout = toolbar_layout(width, text_width(&mode_label), text_width(&model_label));
    let send;

    ctx.table_begin("toolbar");
    ctx.table_set_cell_gap(Size { width: 1, height: 0 });
    {
        ctx.table_next_row();

        if ctx.button("add_context", ADD_CONTEXT_LABEL, style) {
            attach_active_file(state);
            state.ai_focus_prompt = true;
            ctx.needs_rerender();
        }
        ctx.attr_background_rgba(theme.chrome);
        ctx.attr_foreground_rgba(theme.text);

        if layout.show_mode {
            if ctx.button("mode", &mode_label, style) {
                toggle_picker(state, ChatPicker::Mode);
                ctx.needs_rerender();
            }
            ctx.attr_foreground_rgba(theme.text_dim);
            if state.ai_picker == Some(ChatPicker::Mode) {
                let labels = ChatMode::ALL.map(ChatMode::label);
                let current = ChatMode::ALL.iter().position(|&m| m == state.ai_mode);
                let picked = draw_picker(ctx, state, &labels, current.unwrap_or(0), None);
                if let Some(index) = picked {
                    state.ai_mode = ChatMode::ALL[index];
                }
            }
        }

        ctx.label("spacer", &" ".repeat(layout.spacer as usize));

        if layout.show_model {
            if ctx.button("model", &model_label, style) {
                toggle_picker(state, ChatPicker::Model);
                ctx.needs_rerender();
            }
            ctx.attr_foreground_rgba(theme.text_dim);
            if state.ai_picker == Some(ChatPicker::Model) {
                let labels = MODELS.each_ref().map(|m| m.name);
                let anchor_width = text_width(&model_label);
                let picked = draw_picker(ctx, state, &labels, state.ai_model, Some(anchor_width));
                if let Some(index) = picked {
                    state.ai_model = index;
                }
            }
        }

        // The send button fills in with the accent once there's something to send.
        send = ctx.button("send", SEND_LABEL, style);
        if has_text {
            ctx.attr_background_rgba(theme.accent);
            ctx.attr_foreground_rgba(theme.on_accent);
        } else {
            ctx.attr_background_rgba(theme.chrome);
            ctx.attr_foreground_rgba(theme.text_dim);
        }
    }
    ctx.table_end();

    send
}

struct ToolbarLayout {
    show_mode: bool,
    show_model: bool,
    /// The empty space that pushes the model picker and the send button to the right.
    spacer: CoordType,
}

/// Fits the toolbar into `width` columns. The pickers are hidden when space runs out,
/// the mode picker first, while `+` and the send button always stay.
fn toolbar_layout(
    width: CoordType,
    mode_width: CoordType,
    model_width: CoordType,
) -> ToolbarLayout {
    let fixed = 2 * ROUND_BUTTON_WIDTH;
    // Every item is followed by a 1 column gap, except for the last one.
    let spacer = |items: &[CoordType]| {
        let gaps = items.len() as CoordType + 2;
        width - fixed - items.iter().sum::<CoordType>() - gaps
    };

    for (show_mode, show_model) in [(true, true), (false, true), (false, false)] {
        let items: Vec<CoordType> = [(show_mode, mode_width), (show_model, model_width)]
            .iter()
            .filter(|(show, _)| *show)
            .map(|&(_, width)| width)
            .collect();
        let spacer = spacer(&items);
        if spacer >= 0 || !show_model {
            return ToolbarLayout { show_mode, show_model, spacer: spacer.max(0) };
        }
    }
    unreachable!()
}

fn toggle_picker(state: &mut State, picker: ChatPicker) {
    state.ai_picker = if state.ai_picker == Some(picker) { None } else { Some(picker) };
}

/// Shows `items` in a menu that opens upwards from the button drawn last.
/// With `align_right` set to the button's width, the menu lines up with its right edge.
/// Returns the item that was picked. The menu closes after a click, Enter or Escape,
/// or once it loses the focus.
fn draw_picker(
    ctx: &mut Context,
    state: &mut State,
    items: &[&str],
    current: usize,
    align_right: Option<CoordType>,
) -> Option<usize> {
    let mut picked = None;
    let mut close = false;

    ctx.block_begin("picker");
    ctx.attr_float(FloatSpec {
        anchor: Anchor::Last,
        gravity_x: if align_right.is_some() { 1.0 } else { 0.0 },
        gravity_y: 1.0,
        offset_x: align_right.unwrap_or(0) as f32,
        offset_y: 0.0,
    });
    ctx.attr_border();
    ctx.attr_padding(Rect::two(0, 1));
    ctx.attr_background_rgba(state.theme.elevated);
    ctx.attr_foreground_rgba(state.theme.text);
    {
        ctx.list_begin("items");
        ctx.focus_on_first_present();
        for (index, item) in items.iter().enumerate() {
            ctx.next_block_id_mixin(index as u64);
            match ctx.list_item(index == current, item) {
                ListSelection::Unchanged => {}
                ListSelection::Selected => {
                    picked = Some(index);
                    // Arrow keys move through the items, a click picks one.
                    close = ctx.was_mouse_down();
                }
                ListSelection::Activated => {
                    picked = Some(index);
                    close = true;
                }
            }
        }
        ctx.list_end();
    }
    ctx.block_end();

    if close {
        state.ai_picker = None;
        state.ai_focus_prompt = true;
        ctx.needs_rerender();
    } else if !ctx.contains_focus() {
        state.ai_picker = None;
        ctx.needs_rerender();
    }
    picked
}

/// Attaches the file open in the editor to the next prompt.
fn attach_active_file(state: &mut State) {
    let Some(doc) = state.documents.active() else {
        return;
    };
    if !state.ai_context.contains(&doc.filename) {
        let name = doc.filename.clone();
        state.ai_context.push(name);
    }
}

fn recall_last_prompt(state: &mut State) {
    if let Some(message) = state.ai_messages.iter().rev().find(|m| m.role == ChatRole::User) {
        let text = message.text.clone();
        state.ai_prompt.borrow_mut().copy_from_str(&text);
    }
}

/// Moves the prompt into the chat history. Returns false if the prompt was empty.
fn send_prompt(state: &mut State) -> bool {
    let mut text = String::new();
    state.ai_prompt.borrow_mut().save_as_string(&mut text);
    let prompt = text.trim();
    if prompt.is_empty() {
        return false;
    }

    let reply = format!(
        "{} mode with {} isn't connected yet, so this is a placeholder reply.",
        state.ai_mode.label(),
        MODELS[state.ai_model].name,
    );
    let context = std::mem::take(&mut state.ai_context);
    state.ai_messages.push(ChatMessage { role: ChatRole::User, text: prompt.to_string(), context });
    state.ai_messages.push(ChatMessage {
        role: ChatRole::Assistant,
        text: reply,
        context: Vec::new(),
    });
    state.ai_prompt.borrow_mut().copy_from_str(&String::new());
    state.ai_scroll_to_bottom = true;
    // The branch may have changed since the last prompt.
    state.ai_branch = git_branch();
    true
}

/// Returns the branch of the git repository the editor was started in,
/// or the abbreviated commit if the `HEAD` is detached.
pub fn git_branch() -> Option<String> {
    let cwd = env::current_dir().ok()?;
    let dot_git = cwd.ancestors().map(|dir| dir.join(".git")).find(|path| path.exists())?;
    // In worktrees and submodules `.git` is a file that points to the actual git directory.
    let git_dir = if dot_git.is_file() {
        let pointer = fs::read_to_string(&dot_git).ok()?;
        let path = pointer.trim().strip_prefix("gitdir:")?.trim();
        dot_git.parent()?.join(path)
    } else {
        dot_git
    };
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    branch_from_head(&head)
}

fn branch_from_head(head: &str) -> Option<String> {
    let head = head.trim();
    match head.strip_prefix("ref: ") {
        Some(reference) => {
            Some(reference.strip_prefix("refs/heads/").unwrap_or(reference).to_string())
        }
        None if head.len() >= 7 => Some(head[..7].to_string()),
        None => None,
    }
}

fn text_width(text: &str) -> CoordType {
    MeasurementConfig::new(&text.as_bytes()).goto_offset(text.len()).visual_pos.x
}

/// Word-wraps `text` to lines of at most `width` columns.
/// Newlines start a new line and words longer than `width` are broken up.
fn wrap_text(text: &str, width: CoordType) -> Vec<String> {
    if width <= 0 {
        return text.lines().map(str::to_string).collect();
    }

    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        let mut line_width = 0;

        for word in paragraph.split(' ').filter(|w| !w.is_empty()) {
            let mut word = word;
            let mut word_width = text_width(word);

            let needed = if line.is_empty() { word_width } else { line_width + 1 + word_width };
            if needed <= width {
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
                line_width = needed;
                continue;
            }

            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            while word_width > width {
                let bytes = word.as_bytes();
                let cut = MeasurementConfig::new(&bytes).goto_visual(Point { x: width, y: 0 });
                // A character wider than the line still has to go somewhere.
                let split = if cut.offset > 0 {
                    cut.offset
                } else {
                    word.chars().next().map_or(word.len(), char::len_utf8)
                };
                lines.push(word[..split].to_string());
                word = &word[split..];
                word_width = text_width(word);
            }
            line.push_str(word);
            line_width = word_width;
        }

        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_text_breaks_at_words() {
        assert_eq!(wrap_text("the quick brown fox", 10), ["the quick", "brown fox"]);
    }

    #[test]
    fn test_wrap_text_keeps_newlines() {
        assert_eq!(wrap_text("a\n\nb", 10), ["a", "", "b"]);
    }

    #[test]
    fn test_wrap_text_splits_long_words() {
        assert_eq!(wrap_text("abcdefghij xy", 4), ["abcd", "efgh", "ij", "xy"]);
    }

    #[test]
    fn test_toolbar_layout_hides_pickers_when_narrow() {
        // + Agent ▾ ⎵ Opus ▾ ↑ needs 3 + 7 + 6 + 3 columns plus 4 gaps.
        let fits = toolbar_layout(30, 7, 6);
        assert!(fits.show_mode && fits.show_model);
        assert_eq!(fits.spacer, 30 - 23);

        let no_mode = toolbar_layout(20, 7, 6);
        assert!(!no_mode.show_mode && no_mode.show_model);

        let minimal = toolbar_layout(8, 7, 6);
        assert!(!minimal.show_mode && !minimal.show_model);
        assert_eq!(minimal.spacer, 0);
    }

    #[test]
    fn test_branch_from_head() {
        assert_eq!(branch_from_head("ref: refs/heads/main\n").as_deref(), Some("main"));
        assert_eq!(branch_from_head("ref: refs/heads/feat/x").as_deref(), Some("feat/x"));
        assert_eq!(branch_from_head("6998a69c0ffee\n").as_deref(), Some("6998a69"));
        assert_eq!(branch_from_head("").as_deref(), None);
    }
}
