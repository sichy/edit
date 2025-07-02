// Copyright (c) Pavel Sich.
// Licensed under the MIT License.

use edit::framebuffer::IndexedColor;
use edit::helpers::*;
use edit::tui::*;

use crate::state::*;

/// Store information about a tab for rendering
struct TabInfo {
    index: usize,
    filename: String,
    is_dirty: bool,
    is_active: bool,
}

/// Draw the tab bar showing all open documents
pub fn draw_tabbar(ctx: &mut Context, state: &mut State) {
    // Always show tabbar to provide consistent space for File Browser title
    
    // Collect tab information first to avoid borrowing conflicts
    let tab_infos: Vec<TabInfo> = if state.documents.len() > 1 {
        let active_index = state.documents.active_index();
        state.documents.iter().enumerate().map(|(index, doc)| {
            TabInfo {
                index,
                filename: doc.filename.clone(),
                is_dirty: doc.buffer.borrow().is_dirty(),
                is_active: index == active_index,
            }
        }).collect()
    } else {
        Vec::new()
    };

    ctx.block_begin("tabbar");
    ctx.attr_background_rgba(ctx.indexed(IndexedColor::Background));
    ctx.attr_foreground_rgba(ctx.indexed(IndexedColor::Foreground));
    ctx.attr_intrinsic_size(Size { width: 0, height: 1 });
    {
        // Calculate widths for split layout
        let screen_width = ctx.size().width;
        let has_folder_browser = state.folder_browser_visible;
        
        if has_folder_browser {
            let editor_width = (screen_width as f32 * 0.7) as CoordType;
            let folder_browser_width = screen_width - editor_width;
            
            ctx.table_begin("tabbar_layout");
            ctx.table_set_columns(&[editor_width, folder_browser_width]);
            ctx.table_next_row();
            
            // Left side: tabs
            ctx.block_begin("tabs_area");
            if !tab_infos.is_empty() {
                ctx.table_begin("tabs");
                ctx.table_set_cell_gap(Size { width: 0, height: 0 });
                ctx.table_next_row();
                
                for tab_info in &tab_infos {
                    draw_single_tab(ctx, state, tab_info);
                }
                
                ctx.table_end();
            }
            ctx.block_end();
            
            // Right side: File Browser title
            ctx.block_begin("file_browser_title");
            ctx.attr_background_rgba(ctx.indexed(IndexedColor::BrightBlue));
            ctx.attr_foreground_rgba(ctx.indexed(IndexedColor::BrightWhite));
            ctx.attr_padding(Rect::two(0, 1));
            ctx.attr_overflow(Overflow::TruncateTail);
            ctx.label("title", "File Browser");
            ctx.block_end();
            
            ctx.table_end();
        } else {
            // No folder browser: just show tabs across full width
            if !tab_infos.is_empty() {
                ctx.table_begin("tabs");
                ctx.table_set_cell_gap(Size { width: 0, height: 0 });
                ctx.table_next_row();
                
                for tab_info in &tab_infos {
                    draw_single_tab(ctx, state, tab_info);
                }
                
                ctx.table_end();
            }
        }
    }
    ctx.block_end();
}

fn draw_single_tab(ctx: &mut Context, state: &mut State, tab_info: &TabInfo) {
    // Create tab text with filename and dirty indicator
    let mut tab_text = String::new();
    tab_text.push_str(&tab_info.filename);
    if tab_info.is_dirty {
        tab_text.push('*');
    }
    
    tab_text = format!(" {} ", tab_text);
    
    // Create button ID by mixing the index into the class name
    ctx.next_block_id_mixin(tab_info.index as u64);
    
    if ctx.button("tab", &tab_text, ButtonStyle::default().bracketed(false)) {
        // Switch to this document
        state.documents.set_active_index(tab_info.index);
        ctx.needs_rerender();
    }
    
    // Style the active tab differently
    if tab_info.is_active {
        ctx.attr_background_rgba(ctx.indexed(IndexedColor::BrightBlue));
        ctx.attr_foreground_rgba(ctx.indexed(IndexedColor::BrightWhite));
    } else {
        ctx.attr_background_rgba(ctx.indexed(IndexedColor::BrightBlack));
        ctx.attr_foreground_rgba(ctx.indexed(IndexedColor::White));
    }
}