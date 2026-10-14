// Copyright (c) Pavel Sich.
// Licensed under the MIT License.

//! The code completion popup. Its suggestions come from the language servers in [`crate::lsp`].

use std::mem;
use std::path::{Path, PathBuf};

use edit::arena::scratch_arena;
use edit::buffer::TextBuffer;
use edit::framebuffer::IndexedColor;
use edit::fuzzy::score_fuzzy;
use edit::helpers::*;
use edit::input::{kbmod, vk};
use edit::tui::*;

use crate::lsp::{CompletionItem, CompletionList, CompletionTrigger, LspManager, Position};
use crate::state::State;

const MAX_VISIBLE_ITEMS: usize = 10;
const MAX_LABEL_WIDTH: usize = 40;
const MAX_DETAIL_WIDTH: usize = 40;
const KIND_WIDTH: usize = 7;

#[derive(Default)]
pub struct CompletionState {
    /// The document being completed in. `None` while no completion is active.
    path: Option<PathBuf>,
    /// Byte offset where the word being completed starts.
    word_start: usize,
    /// The part of the word typed so far. Used to filter `items`.
    prefix: String,
    /// A request is in flight.
    waiting: bool,
    is_incomplete: bool,
    items: Vec<CompletionItem>,
    /// Indices into `items` that match `prefix`, best match first.
    filtered: Vec<usize>,
    selected: usize,
    scroll: usize,
    /// Whether the textarea had the focus during the last frame.
    textarea_focused: bool,
    /// Where the popup goes during this frame. See [`draw_completion_popup`].
    popup_anchor: Option<Point>,
}

impl CompletionState {
    fn is_visible(&self) -> bool {
        self.path.is_some() && !self.filtered.is_empty()
    }

    fn close(&mut self) {
        *self = Self {
            textarea_focused: self.textarea_focused,
            popup_anchor: self.popup_anchor,
            ..Default::default()
        };
    }

    fn receive(&mut self, list: CompletionList) {
        self.waiting = false;
        self.is_incomplete = list.is_incomplete;
        self.items = list.items;
        self.refilter();
    }

    fn refilter(&mut self) {
        let mut scored: Vec<(i32, usize)> = Vec::with_capacity(self.items.len());
        for (index, item) in self.items.iter().enumerate() {
            if self.prefix.is_empty() {
                scored.push((0, index));
                continue;
            }
            let scratch = scratch_arena(None);
            let (score, _) = score_fuzzy(&scratch, &item.filter_text, &self.prefix, true);
            if score > 0 {
                scored.push((score, index));
            }
        }

        let items = &self.items;
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0).then_with(|| items[a.1].sort_text.cmp(&items[b.1].sort_text))
        });
        self.filtered = scored.into_iter().map(|(_, index)| index).collect();
        self.selected = 0;
        self.scroll = 0;
    }

    fn select(&mut self, index: usize) {
        self.selected = index;
        if index < self.scroll {
            self.scroll = index;
        } else if index >= self.scroll + MAX_VISIBLE_ITEMS {
            self.scroll = index + 1 - MAX_VISIBLE_ITEMS;
        }
    }
}

/// The textarea's state before it processed this frame's input.
pub struct TextareaSnapshot {
    generation: u32,
    cursor_offset: usize,
    text_length: usize,
    /// Ctrl+Space was pressed.
    invoked: bool,
}

/// Must be called before the textarea, so that the popup gets to handle
/// keys like Up, Down and Enter before the textarea does.
pub fn completion_before_textarea(
    ctx: &mut Context,
    state: &mut State,
) -> Option<TextareaSnapshot> {
    let poll = state.lsp.poll();
    if poll.changed {
        ctx.needs_rerender();
    }
    if let Some(list) = poll.completion
        && state.completion.waiting
    {
        state.completion.receive(list);
    }

    let mut invoked = false;
    if state.completion.textarea_focused {
        if state.completion.is_visible() && handle_popup_keys(ctx, state) {
            ctx.needs_rerender();
        }
        invoked = ctx.consume_shortcut(kbmod::CTRL | vk::SPACE);
    }

    let tb = state.documents.active()?.buffer.borrow();
    Some(TextareaSnapshot {
        generation: tb.generation(),
        cursor_offset: tb.cursor_offset(),
        text_length: tb.text_length(),
        invoked,
    })
}

/// Must be called right after the textarea was created.
pub fn completion_after_textarea(
    state: &mut State,
    before: Option<TextareaSnapshot>,
    focused: bool,
    cursor_screen: Option<Point>,
) {
    state.completion.textarea_focused = focused;

    let open: Vec<&Path> = state.documents.iter().filter_map(|doc| doc.path.as_deref()).collect();
    state.lsp.close_documents_except(&open);

    let Some(doc) = state.documents.active() else {
        state.completion.close();
        return;
    };
    let Some(path) = doc.path.as_deref() else {
        state.completion.close();
        return;
    };
    let tb = doc.buffer.borrow();
    state.lsp.sync_document(path, tb.generation(), || buffer_text(&tb));

    let completion = &mut state.completion;
    if !focused || completion.path.as_deref().is_some_and(|p| p != path) {
        completion.close();
    }

    let cursor = tb.cursor_offset();

    // Keep filtering while the user types the rest of the word,
    // and close the popup once the cursor leaves the word.
    if completion.path.is_some() {
        let prefix = (cursor >= completion.word_start)
            .then(|| read_text(&tb, completion.word_start, cursor));
        match prefix {
            Some(prefix) if prefix.chars().all(is_word_char) => {
                if prefix != completion.prefix {
                    completion.prefix = prefix;
                    completion.refilter();
                    if completion.is_incomplete && !completion.waiting {
                        completion.waiting =
                            request(&mut state.lsp, &tb, path, CompletionTrigger::Incomplete);
                    }
                }
            }
            _ => completion.close(),
        }
    }

    let typed = before.as_ref().and_then(|before| typed_char(&tb, before));
    let trigger = match typed {
        Some(ch) if state.lsp.trigger_characters(path).contains(&ch) => {
            Some(CompletionTrigger::Character(ch))
        }
        _ if before.as_ref().is_some_and(|before| before.invoked) => {
            Some(CompletionTrigger::Invoked)
        }
        _ => None,
    };
    if let Some(trigger) = trigger
        && focused
        && request(&mut state.lsp, &tb, path, trigger)
    {
        let word_start = word_start(&tb, cursor);
        state.completion = CompletionState {
            path: Some(path.to_path_buf()),
            word_start,
            prefix: read_text(&tb, word_start, cursor),
            waiting: true,
            textarea_focused: focused,
            ..Default::default()
        };
    }

    drop(tb);
    state.completion.popup_anchor = cursor_screen;
}

/// Draws the popup on top of all windows. Must be called after all windows were drawn.
pub fn draw_completion_popup(ctx: &mut Context, state: &mut State) {
    if let Some(cursor_screen) = state.completion.popup_anchor.take()
        && state.completion.is_visible()
    {
        draw_popup(ctx, &state.completion, cursor_screen);
    }
}

/// Returns true if a key was handled.
fn handle_popup_keys(ctx: &mut Context, state: &mut State) -> bool {
    let completion = &mut state.completion;
    let count = completion.filtered.len();
    let selected = completion.selected;

    if ctx.consume_shortcut(vk::DOWN) {
        completion.select((selected + 1) % count);
    } else if ctx.consume_shortcut(vk::UP) {
        completion.select((selected + count - 1) % count);
    } else if ctx.consume_shortcut(vk::NEXT) {
        completion.select((selected + MAX_VISIBLE_ITEMS).min(count - 1));
    } else if ctx.consume_shortcut(vk::PRIOR) {
        completion.select(selected.saturating_sub(MAX_VISIBLE_ITEMS));
    } else if ctx.consume_shortcut(vk::RETURN) || ctx.consume_shortcut(vk::TAB) {
        accept(state);
    } else if ctx.consume_shortcut(vk::ESCAPE) {
        completion.close();
    } else {
        return false;
    }
    true
}

fn request(lsp: &mut LspManager, tb: &TextBuffer, path: &Path, trigger: CompletionTrigger) -> bool {
    let cursor = tb.cursor_offset();
    let line_start = line_start(tb, cursor);
    let position = Position {
        line: tb.cursor_logical_pos().y as u32,
        character: read_text(tb, line_start, cursor).encode_utf16().count() as u32,
    };
    lsp.request_completion(path, position, trigger)
}

/// Inserts the selected item, replacing the part of the word typed so far.
fn accept(state: &mut State) {
    let completion = mem::take(&mut state.completion);
    state.completion.textarea_focused = completion.textarea_focused;

    let Some(item) = completion.filtered.get(completion.selected).map(|&i| &completion.items[i])
    else {
        return;
    };
    let Some(doc) = state.documents.active() else {
        return;
    };
    let mut tb = doc.buffer.borrow_mut();
    let text = read_bytes(&tb, 0, tb.text_length());
    let cursor = tb.cursor_offset();

    // The user may have typed more since the request was sent,
    // which is why the replaced range always extends to the cursor.
    let (start, end, new_text) = match &item.text_edit {
        Some(edit) => {
            let end = offset_at(&text, edit.range.end).max(cursor);
            (offset_at(&text, edit.range.start).min(end), end, edit.new_text.as_str())
        }
        None => (completion.word_start.min(cursor), cursor, item.insert_text.as_str()),
    };

    // (start, end, new_text, is_main_edit)
    let mut edits = vec![(start, end, new_text, true)];
    for edit in &item.additional_edits {
        let edit_start = offset_at(&text, edit.range.start);
        let edit_end = offset_at(&text, edit.range.end);
        if edit_end <= start || edit_start >= end {
            edits.push((edit_start, edit_end, edit.new_text.as_str(), false));
        }
    }
    // Applying edits back to front keeps the offsets of the remaining edits valid.
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));

    tb.clear_selection();
    let mut cursor_after = None;
    for (start, end, new_text, is_main) in edits {
        let length_before = tb.text_length();
        tb.cursor_move_to_offset(start);
        tb.selection_update_offset(end);
        tb.write_canon(new_text.as_bytes());

        if is_main {
            cursor_after = Some(tb.cursor_offset());
        } else if let Some(offset) = &mut cursor_after {
            // An edit before the main one shifted it.
            *offset = (*offset + tb.text_length()).saturating_sub(length_before);
        }
    }
    if let Some(offset) = cursor_after {
        tb.cursor_move_to_offset(offset);
    }
    tb.make_cursor_visible();
}

fn draw_popup(ctx: &mut Context, completion: &CompletionState, cursor_screen: Point) {
    let items = &completion.items;
    let rows = completion.filtered.len().min(MAX_VISIBLE_ITEMS);
    let visible = &completion.filtered[completion.scroll..completion.scroll + rows];

    // Measured across all matches, so that the popup doesn't change its width while scrolling.
    let matches = || completion.filtered.iter().map(|&i| &items[i]);
    let widest =
        |widths: &mut dyn Iterator<Item = usize>, max: usize| widths.max().unwrap_or(0).min(max);
    let label_width =
        widest(&mut matches().map(|item| item.label.chars().count()), MAX_LABEL_WIDTH);
    let detail_width =
        widest(&mut matches().map(|item| item.detail.chars().count()), MAX_DETAIL_WIDTH);

    let screen = ctx.size();
    let inner_width = (label_width + KIND_WIDTH + detail_width + 5) as CoordType;
    let width = (inner_width + 2).min(screen.width);
    let height = rows as CoordType + 2;

    // Align the list with the start of the word, below the cursor if it fits.
    let prefix_width = completion.prefix.chars().count() as CoordType;
    let x = (cursor_screen.x - prefix_width).min(screen.width - width).max(0);
    let y = if cursor_screen.y + 1 + height <= screen.height {
        cursor_screen.y + 1
    } else {
        (cursor_screen.y - height).max(0)
    };

    ctx.block_begin("completion");
    ctx.attr_float(FloatSpec {
        anchor: Anchor::Root,
        offset_x: x as f32,
        offset_y: y as f32,
        ..Default::default()
    });
    ctx.attr_border();
    ctx.attr_intrinsic_size(Size { width: width - 2, height: rows as CoordType });
    {
        for (row, &index) in visible.iter().enumerate() {
            let item = &items[index];
            let text = format!(
                " {} {} {} ",
                fit(&item.label, label_width),
                fit(kind_name(item.kind), KIND_WIDTH),
                fit(&item.detail, detail_width),
            );

            ctx.next_block_id_mixin(row as u64);
            ctx.label("item", &text);
            ctx.attr_overflow(Overflow::TruncateTail);
            if completion.scroll + row == completion.selected {
                let bg = ctx.indexed(IndexedColor::BrightBlue);
                ctx.attr_background_rgba(bg);
                ctx.attr_foreground_rgba(ctx.contrasted(bg));
            }
        }
    }
    ctx.block_end();
}

/// Pads or truncates `text` to exactly `width` characters.
fn fit(text: &str, width: usize) -> String {
    let length = text.chars().count();
    if length <= width {
        format!("{text:<width$}")
    } else {
        let mut truncated: String = text.chars().take(width.saturating_sub(1)).collect();
        truncated.push('…');
        truncated
    }
}

fn kind_name(kind: u32) -> &'static str {
    match kind {
        1 => "text",
        2 => "method",
        3 => "fn",
        4 => "ctor",
        5 => "field",
        6 => "var",
        7 => "class",
        8 => "iface",
        9 => "module",
        10 => "prop",
        11 => "unit",
        12 => "value",
        13 => "enum",
        14 => "keyword",
        15 => "snippet",
        16 => "color",
        17 => "file",
        18 => "ref",
        19 => "folder",
        20 => "variant",
        21 => "const",
        22 => "struct",
        23 => "event",
        24 => "op",
        25 => "type",
        _ => "",
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '$'
}

/// Returns the last character that was just typed, if the last edit was typing.
/// Fast typing can arrive as several characters at once, e.g. `::`.
fn typed_char(tb: &TextBuffer, before: &TextareaSnapshot) -> Option<char> {
    const MAX_TYPED_BYTES: usize = 16;

    if tb.generation() == before.generation {
        return None;
    }
    let inserted = tb.text_length().checked_sub(before.text_length)?;
    if !(1..=MAX_TYPED_BYTES).contains(&inserted)
        || tb.cursor_offset() != before.cursor_offset + inserted
    {
        return None;
    }

    let text = read_text(tb, before.cursor_offset, tb.cursor_offset());
    if text.contains('\n') {
        return None;
    }
    text.chars().next_back()
}

fn word_start(tb: &TextBuffer, cursor: usize) -> usize {
    let line = read_text(tb, line_start(tb, cursor), cursor);
    let word_length: usize =
        line.chars().rev().take_while(|&ch| is_word_char(ch)).map(char::len_utf8).sum();
    cursor - word_length
}

fn line_start(tb: &TextBuffer, offset: usize) -> usize {
    let mut end = offset;
    while end > 0 {
        let chunk = tb.read_backward(end);
        if chunk.is_empty() {
            break;
        }
        if let Some(newline) = chunk.iter().rposition(|&b| b == b'\n') {
            return end - chunk.len() + newline + 1;
        }
        end -= chunk.len();
    }
    0
}

fn read_bytes(tb: &TextBuffer, start: usize, end: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(end.saturating_sub(start));
    let mut offset = start;
    while offset < end {
        let chunk = tb.read_forward(offset);
        if chunk.is_empty() {
            break;
        }
        let take = chunk.len().min(end - offset);
        bytes.extend_from_slice(&chunk[..take]);
        offset += take;
    }
    bytes
}

fn read_text(tb: &TextBuffer, start: usize, end: usize) -> String {
    String::from_utf8_lossy(&read_bytes(tb, start, end)).into_owned()
}

fn buffer_text(tb: &TextBuffer) -> String {
    read_text(tb, 0, tb.text_length())
}

/// Converts an LSP position (line and UTF-16 offset) into a byte offset into `text`.
fn offset_at(text: &[u8], position: Position) -> usize {
    let mut line_start = 0;
    for _ in 0..position.line {
        match text[line_start..].iter().position(|&b| b == b'\n') {
            Some(newline) => line_start += newline + 1,
            None => return text.len(),
        }
    }

    let line_end =
        text[line_start..].iter().position(|&b| b == b'\n').map_or(text.len(), |i| line_start + i);
    let line = String::from_utf8_lossy(&text[line_start..line_end]);
    let line = line.strip_suffix('\r').unwrap_or(&line);

    let mut offset = line_start;
    let mut units = 0;
    for ch in line.chars() {
        if units >= position.character as usize {
            break;
        }
        units += ch.len_utf16();
        offset += ch.len_utf8();
    }
    offset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_at() {
        let text = "fn main() {\r\n    p.na\n    é😀x\n".as_bytes();
        let at = |line, character| offset_at(text, Position { line, character });
        assert_eq!(at(0, 0), 0);
        assert_eq!(at(0, 100), 11);
        assert_eq!(at(1, 6), 19);
        // "é" is 1 UTF-16 unit and 2 bytes, "😀" is 2 units and 4 bytes.
        assert_eq!(at(2, 5), 28);
        assert_eq!(at(2, 7), 32);
        assert_eq!(at(9, 0), text.len());
    }

    #[test]
    fn test_fit() {
        assert_eq!(fit("abc", 5), "abc  ");
        assert_eq!(fit("abcdef", 4), "abc…");
    }
}
