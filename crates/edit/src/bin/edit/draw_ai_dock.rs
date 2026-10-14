// Copyright (c) Pavel Sich.
// Licensed under the MIT License.

//! AI Assistant window: a header with the chat title, the chat history with the newest
//! message at the bottom, and the prompt composer below it.

use edit::framebuffer::IndexedColor;
use edit::helpers::*;
use edit::input::vk;
use edit::tui::*;
use edit::unicode::MeasurementConfig;

use crate::dock::PanelId;
use crate::draw_windows::truncate;
use crate::state::*;

const HEADER_HEIGHT: CoordType = 1;
/// The composer's border and its footer row, without the prompt lines.
const COMPOSER_CHROME_HEIGHT: CoordType = 3;
/// The prompt grows with its contents up to this many lines, then scrolls.
const PROMPT_MAX_LINES: CoordType = 6;
const NEW_CHAT_LABEL: &str = "+ New";
/// The width of [`NEW_CHAT_LABEL`] including the button's brackets.
const NEW_CHAT_WIDTH: CoordType = 7;
const SEND_LABEL: &str = "↑ Send";
/// The width of [`SEND_LABEL`] including the button's brackets.
const SEND_WIDTH: CoordType = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub text: String,
}

/// Draws the assistant into a window of the given inner `size`.
pub fn draw_ai_assistant(ctx: &mut Context, state: &mut State, size: Size, grab_focus: bool) {
    ctx.block_begin("ai_assistant");
    ctx.attr_intrinsic_size(size);
    let focused = ctx.contains_focus();
    {
        let prompt_lines = state.ai_prompt.borrow().visual_line_count().clamp(1, PROMPT_MAX_LINES);
        let history_height =
            (size.height - HEADER_HEIGHT - COMPOSER_CHROME_HEIGHT - prompt_lines).max(0);

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
    ctx.attr_background_rgba(ctx.indexed_alpha(IndexedColor::BrightBlue, 1, 3));
    {
        ctx.table_next_row();

        // The title is padded to its full width, so that the button sits at the right edge.
        let (title, title_len) = truncate(&format!("✦ {title}"), title_width.max(0) as usize);
        let padding = (title_width.max(0) as usize).saturating_sub(title_len);
        ctx.label("title", &format!("{title}{:padding$}", ""));
        ctx.attr_foreground_rgba(ctx.indexed(IndexedColor::BrightWhite));

        if has_button && ctx.button("new_chat", NEW_CHAT_LABEL, ButtonStyle::default()) {
            state.ai_messages.clear();
            ctx.needs_rerender();
        }
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
        if state.ai_messages.is_empty() {
            ctx.label("empty", "Ask about your code, or anything else.");
            ctx.attr_padding(Rect::three(1, 1, 0));
            ctx.attr_overflow(Overflow::TruncateTail);
            ctx.attr_foreground_rgba(ctx.indexed(IndexedColor::BrightBlack));
        }

        // The scrollbar takes up the last column and each message has 1 column of margin per side.
        let text_width = size.width - 3;
        let bubble = ctx.indexed_alpha(IndexedColor::BrightBlack, 1, 2);
        let accent = ctx.indexed(IndexedColor::BrightBlue);
        for (index, message) in state.ai_messages.iter().enumerate() {
            ctx.next_block_id_mixin(index as u64);
            ctx.block_begin("message");
            ctx.attr_padding(Rect::three(1, 1, 0));
            match message.role {
                ChatRole::User => {
                    ctx.block_begin("bubble");
                    ctx.attr_padding(Rect::two(0, 1));
                    ctx.attr_background_rgba(bubble);
                    draw_lines(ctx, &message.text, text_width - 2);
                    ctx.block_end();
                }
                ChatRole::Assistant => {
                    ctx.label("role", "✦ Assistant");
                    ctx.attr_foreground_rgba(accent);
                    draw_lines(ctx, &message.text, text_width);
                }
            }
            ctx.block_end();
        }
    }
    ctx.scrollarea_end();
}

fn draw_lines(ctx: &mut Context, text: &str, width: CoordType) {
    for (index, line) in wrap_text(text, width).iter().enumerate() {
        ctx.next_block_id_mixin(index as u64);
        ctx.label("line", line);
    }
}

/// A card with the word-wrapped prompt and a footer with the key hints and the send button.
/// The border lights up while the composer has the focus.
fn draw_composer(
    ctx: &mut Context,
    state: &mut State,
    width: CoordType,
    prompt_lines: CoordType,
    grab_focus: bool,
) {
    let inner_width = (width - 2).max(0);
    let has_text = state.ai_prompt.borrow().text_length() > 0;

    ctx.block_begin("composer");
    ctx.attr_border();
    ctx.attr_intrinsic_size(Size {
        width: inner_width,
        height: prompt_lines + COMPOSER_CHROME_HEIGHT - 2,
    });
    ctx.attr_background_rgba(ctx.indexed_alpha(IndexedColor::BrightBlack, 1, 4));
    let focused = ctx.contains_focus();
    let border = if focused { IndexedColor::BrightBlue } else { IndexedColor::BrightBlack };
    ctx.attr_foreground_rgba(ctx.indexed(border));
    {
        // The prompt is multi-line, so it would otherwise insert a newline on Enter
        // and swallow Escape. Alt+Enter still inserts a newline.
        let mut send = focused && ctx.consume_shortcut(vk::RETURN);
        if focused && ctx.consume_shortcut(vk::ESCAPE) {
            state.windows.focus(PanelId::Editor);
            ctx.needs_rerender();
        }

        ctx.textarea("prompt", state.ai_prompt.clone());
        ctx.inherit_focus();
        ctx.attr_intrinsic_size(Size { width: inner_width, height: prompt_lines });
        if ctx.attr_fit_height(PROMPT_MAX_LINES) != prompt_lines {
            // The history above was sized for the old prompt height.
            ctx.needs_rerender();
        }
        ctx.attr_foreground_rgba(ctx.indexed(IndexedColor::Foreground));
        ctx.attr_placeholder("Ask anything…");
        if grab_focus {
            ctx.steal_focus();
        }

        ctx.table_begin("footer");
        ctx.table_set_cell_gap(Size { width: 1, height: 0 });
        {
            ctx.table_next_row();

            let hint_width = (inner_width - 1 - SEND_WIDTH).max(0) as usize;
            let (hint, hint_len) = truncate("Alt+⏎ new line", hint_width);
            let padding = hint_width.saturating_sub(hint_len);
            ctx.label("hint", &format!("{hint}{:padding$}", ""));
            ctx.attr_foreground_rgba(ctx.indexed(IndexedColor::BrightBlack));

            send |= ctx.button("send", SEND_LABEL, ButtonStyle::default());
            let send_color =
                if has_text { IndexedColor::BrightBlue } else { IndexedColor::BrightBlack };
            ctx.attr_foreground_rgba(ctx.indexed(send_color));
        }
        ctx.table_end();

        if send && send_prompt(state) {
            // Clicking the send button moved the focus away from the prompt.
            state.windows.focus(PanelId::Assistant);
            ctx.needs_rerender();
        }
    }
    ctx.block_end();
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
        "You asked: \"{prompt}\"\n\nAI integration isn't connected yet, so this is a placeholder reply."
    );
    state.ai_messages.push(ChatMessage { role: ChatRole::User, text: prompt.to_string() });
    state.ai_messages.push(ChatMessage { role: ChatRole::Assistant, text: reply });
    state.ai_prompt.borrow_mut().copy_from_str(&String::new());
    state.ai_scroll_to_bottom = true;
    true
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
}
