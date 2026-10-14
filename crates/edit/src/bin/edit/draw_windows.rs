// Copyright (c) Pavel Sich.
// Licensed under the MIT License.

//! Draws the windows laid out by [`crate::dock::WindowManager`] with Turbo Vision style frames.

use edit::framebuffer::IndexedColor;
use edit::helpers::*;
use edit::oklab::StraightRgba;
use edit::tui::*;
use edit::unicode::MeasurementConfig;

use crate::dock::{Frame, PanelId};
use crate::draw_ai_dock::draw_ai_assistant;
use crate::draw_editor::draw_editor;
use crate::draw_folder_browser::draw_folder_browser;
use crate::state::*;

/// Draws all windows from back to front. Must be called before the menubar,
/// so that the menus open on top of the windows.
pub fn draw_windows(ctx: &mut Context, state: &mut State) {
    let frames = state.windows.frames().to_vec();
    for frame in frames {
        draw_window(ctx, state, frame);
    }
    if let Some(preview) = state.windows.preview() {
        draw_drop_preview(ctx, preview);
    }
}

fn draw_window(ctx: &mut Context, state: &mut State, frame: Frame) {
    let rect = frame.rect;
    if rect.width() < 3 || rect.height() < 3 {
        return;
    }
    let inner = Size { width: rect.width() - 2, height: rect.height() - 2 };

    ctx.next_block_id_mixin(frame.panel as u64);
    ctx.block_begin("window");
    ctx.attr_float(FloatSpec {
        anchor: Anchor::Root,
        offset_x: rect.left as f32,
        offset_y: rect.top as f32,
        ..Default::default()
    });

    let has_focus = ctx.contains_focus();
    if has_focus {
        state.windows.set_active(frame.panel);
    }
    let grab_focus = state.windows.take_focus_request(frame.panel) && !has_focus;
    let active = has_focus || grab_focus;

    if active {
        ctx.attr_border_double();
    } else {
        ctx.attr_border();
    }
    let frame_color = frame_color(ctx, active);
    ctx.attr_intrinsic_size(inner);
    ctx.attr_background_rgba(ctx.indexed(IndexedColor::Background));
    ctx.attr_foreground_rgba(frame_color);
    {
        ctx.block_begin("content");
        ctx.attr_intrinsic_size(inner);
        ctx.attr_foreground_rgba(ctx.indexed(IndexedColor::Foreground));
        match frame.panel {
            PanelId::Editor => draw_editor(ctx, state, inner, grab_focus),
            PanelId::Assistant => draw_ai_assistant(ctx, state, inner, grab_focus),
            PanelId::Files => draw_folder_browser(ctx, state, inner, grab_focus),
        }
        ctx.block_end();
    }
    ctx.block_end();

    draw_title_bar(ctx, state, frame, frame_color);
}

fn frame_color(ctx: &Context, active: bool) -> StraightRgba {
    ctx.indexed(if active { IndexedColor::BrightWhite } else { IndexedColor::BrightBlack })
}

/// The title, the `[■]` and `[↑]` buttons and the `◢` grip are drawn on top of the frame.
fn draw_title_bar(ctx: &mut Context, state: &State, frame: Frame, color: StraightRgba) {
    let rect = frame.rect;
    let decoration = |ctx: &mut Context, classname: &'static str, x, y, text: &str| {
        ctx.next_block_id_mixin(frame.panel as u64);
        ctx.block_begin(classname);
        ctx.attr_float(FloatSpec {
            anchor: Anchor::Root,
            offset_x: x as f32,
            offset_y: y as f32,
            ..Default::default()
        });
        ctx.attr_background_rgba(ctx.indexed(IndexedColor::Background));
        ctx.attr_foreground_rgba(color);
        ctx.label("text", text);
        ctx.block_end();
    };

    // Leave room for the buttons and the frame corners on both sides.
    let max_title_width = rect.width() - 14;
    if max_title_width > 0 {
        let title = window_title(state, frame.panel);
        let (title, width) = truncate(&title, max_title_width as usize);
        let text = format!(" {title} ");
        let width = width as CoordType + 2;
        decoration(ctx, "title", rect.left + (rect.width() - width) / 2, rect.top, &text);
    }

    if let Some(r) = frame.close_button() {
        decoration(ctx, "close", r.left, r.top, "[■]");
    }
    if let Some(r) = frame.zoom_button() {
        let icon = if state.windows.is_zoomed(frame.panel) { "[↕]" } else { "[↑]" };
        decoration(ctx, "zoom", r.left, r.top, icon);
    }
    if frame.floating {
        decoration(ctx, "grip", rect.right - 1, rect.bottom - 1, "◢");
    }
}

fn window_title(state: &State, panel: PanelId) -> String {
    if state.windows.keyboard_mode() == Some(panel) {
        return "Move/Size: arrows, Shift+arrows, Enter".to_string();
    }
    match panel {
        PanelId::Editor => match state.documents.active() {
            Some(doc) if doc.buffer.borrow().is_dirty() => format!("{} *", doc.filename),
            Some(doc) => doc.filename.clone(),
            None => "Editor".to_string(),
        },
        PanelId::Assistant => "AI Assistant".to_string(),
        PanelId::Files => {
            let dir = state.folder_browser_current_dir.as_path();
            let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("/");
            format!("Files: {name}")
        }
    }
}

/// Truncates `text` to `max_width` columns, adding an ellipsis if needed.
/// Returns the result and its width in columns.
pub fn truncate(text: &str, max_width: usize) -> (String, usize) {
    let bytes = text.as_bytes();
    let full = MeasurementConfig::new(&bytes).goto_offset(text.len()).visual_pos.x as usize;
    if full <= max_width {
        return (text.to_string(), full);
    }
    let mut cfg = MeasurementConfig::new(&bytes);
    let cut = cfg.goto_visual(Point { x: max_width.saturating_sub(1) as CoordType, y: 0 });
    (format!("{}…", &text[..cut.offset]), cut.visual_pos.x as usize + 1)
}

/// Shows where a dragged window would end up if it was dropped now.
fn draw_drop_preview(ctx: &mut Context, rect: Rect) {
    if rect.width() < 3 || rect.height() < 3 {
        return;
    }
    ctx.block_begin("drop-preview");
    ctx.attr_float(FloatSpec {
        anchor: Anchor::Root,
        offset_x: rect.left as f32,
        offset_y: rect.top as f32,
        ..Default::default()
    });
    ctx.attr_border_double();
    ctx.attr_intrinsic_size(Size { width: rect.width() - 2, height: rect.height() - 2 });
    ctx.attr_background_rgba(ctx.indexed_alpha(IndexedColor::BrightBlue, 1, 2));
    ctx.attr_foreground_rgba(ctx.indexed(IndexedColor::BrightWhite));
    ctx.block_end();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("main.rs", 10), ("main.rs".to_string(), 7));
        assert_eq!(truncate("a_long_file_name.rs", 8), ("a_long_…".to_string(), 8));
    }
}
