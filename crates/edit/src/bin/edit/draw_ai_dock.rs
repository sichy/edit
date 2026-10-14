// Copyright (c) Pavel Sich.
// Licensed under the MIT License.

//! AI Assistant window for agentic AI commands

use edit::helpers::*;
use edit::input::{kbmod, vk};
use edit::oklab::StraightRgba;
use edit::tui::*;

use crate::dock::PanelId;
use crate::state::*;

/// Draws the assistant into a window of the given inner `size`.
pub fn draw_ai_assistant(ctx: &mut Context, state: &mut State, size: Size, grab_focus: bool) {
    ctx.block_begin("ai_assistant");
    ctx.attr_intrinsic_size(size);
    ctx.attr_padding(Rect::two(0, 1));
    let focused = ctx.contains_focus();
    {
        ctx.label("prompt_label", "Prompt:");
        ctx.attr_overflow(Overflow::TruncateTail);
        ctx.attr_foreground_rgba(StraightRgba::from_rgba(0xbbbbbbff)); // Gray label

        ctx.editline("ai_prompt_input", &mut state.ai_prompt);
        ctx.inherit_focus();
        ctx.attr_intrinsic_size(Size { width: size.width - 2, height: 1 });
        if grab_focus {
            ctx.steal_focus();
        }

        // Action buttons in a single row
        ctx.table_begin("button_table");
        ctx.table_set_cell_gap(Size { width: 1, height: 0 });
        ctx.attr_padding(Rect::three(1, 0, 0));
        {
            ctx.table_next_row();

            if ctx.button("send", "Send (Ctrl+Enter)", ButtonStyle::default()) {
                execute_ai_prompt(ctx, state);
            }

            if ctx.button("clear", "Clear", ButtonStyle::default()) {
                state.ai_prompt.clear();
                ctx.needs_rerender();
            }
        }
        ctx.table_end();

        if !state.ai_output.is_empty() {
            ctx.label("output_label", "Output:");
            ctx.attr_padding(Rect::three(1, 0, 0));
            ctx.attr_overflow(Overflow::TruncateTail);
            ctx.attr_foreground_rgba(StraightRgba::from_rgba(0xbbbbbbff)); // Gray label

            ctx.label("ai_output", &state.ai_output);
            ctx.attr_overflow(Overflow::TruncateTail);
            ctx.attr_foreground_rgba(StraightRgba::from_rgba(0x90ee90ff)); // Light green for output
        }
    }
    ctx.block_end();

    if focused && let Some(key) = ctx.keyboard_input() {
        if key == vk::ESCAPE {
            // Return to the editor, like leaving a dialog.
            state.windows.focus(PanelId::Editor);
        } else if key == kbmod::CTRL | vk::RETURN {
            execute_ai_prompt(ctx, state);
        } else {
            return;
        }
        ctx.needs_rerender();
        ctx.set_input_consumed();
    }
}

fn execute_ai_prompt(ctx: &mut Context, state: &mut State) {
    if state.ai_prompt.trim().is_empty() {
        return;
    }

    // For now, just simulate AI output with a simple response
    // TODO: Integrate with actual AI API
    state.ai_output = format!(
        "AI Response to: '{}'\n[This is a placeholder - AI integration coming soon!]",
        state.ai_prompt
    );

    // Optional: Insert output into current document
    if let Some(doc) = state.documents.active() {
        let mut tb = doc.buffer.borrow_mut();
        let output_text = format!("\n// AI Generated:\n// {}\n", state.ai_output);
        tb.write_canon(output_text.as_bytes());
    }

    ctx.needs_rerender();
}
