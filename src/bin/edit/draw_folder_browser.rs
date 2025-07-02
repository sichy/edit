// Copyright (c) Pavel Sich.
// Licensed under the MIT License.

use std::cmp::Ordering;
use std::fs;

use edit::framebuffer::IndexedColor;
use edit::helpers::*;
use edit::input::{kbmod, vk};
use edit::tui::*;
use edit::icu;

use crate::state::*;

pub fn draw_folder_browser(ctx: &mut Context, state: &mut State) {
    // Only show folder browser if visible
    if !state.folder_browser_visible {
        return;
    }
    
    if state.folder_browser_entries.is_none() {
        refresh_folder_browser(state);
    }

    ctx.block_begin("folder_browser");
    ctx.attr_background_rgba(ctx.indexed(IndexedColor::Background));
    ctx.attr_foreground_rgba(ctx.indexed(IndexedColor::Foreground));

    // do not drw border around the browser
    //ctx.attr_border();

    {
        // Header with current directory
        ctx.block_begin("header");
        ctx.attr_background_rgba(ctx.indexed(IndexedColor::BrightBlue));
        ctx.attr_foreground_rgba(ctx.indexed(IndexedColor::BrightWhite));
        ctx.attr_intrinsic_size(Size { width: 0, height: 1 });
        ctx.attr_padding(Rect::two(0, 1));
        {
            let dir_name = state.folder_browser_current_dir.as_path()
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Root");

            ctx.label("current_dir", &format!("📁 {}", dir_name));

            ctx.attr_overflow(Overflow::TruncateTail);
        }
        ctx.block_end();

        // File list using same structure as file picker
        ctx.next_block_id_mixin(state.folder_browser_current_dir_revision);
        ctx.list_begin("files");
        ctx.inherit_focus();
        
        let mut activated = false;
        let mut selected_entry = None;
        let mut current_item_index = 0;
        
        if let Some(dirs_files) = &state.folder_browser_entries {
            for entries in dirs_files {
                for entry in entries {
                    let is_selected = state.folder_browser_selected == current_item_index;
                    match ctx.list_item(is_selected, entry.as_str()) {
                        ListSelection::Unchanged => {}
                        ListSelection::Selected => {
                            state.folder_browser_selected = current_item_index;
                            selected_entry = Some(entry.clone());
                        }
                        ListSelection::Activated => {
                            selected_entry = Some(entry.clone());
                            activated = true;
                        }
                    }
                    ctx.attr_overflow(Overflow::TruncateMiddle);
                    current_item_index += 1;
                }
            }
        }
        
        ctx.list_end();
        
        // Handle navigation
        if ctx.contains_focus() && (ctx.consume_shortcut(vk::BACK) || ctx.consume_shortcut(kbmod::ALT | vk::UP)) {
            selected_entry = Some(DisplayablePathBuf::from(".."));
            activated = true;
        }
        
        if activated && let Some(entry) = selected_entry {
            if let Some(path) = update_folder_browser_path(state, entry) {
                // It's a file, open it
                if let Err(err) = state.documents.add_file_path(&path) {
                    error_log_add(ctx, state, err);
                } else {
                    ctx.needs_rerender();
                }
            }
        }
    }
    ctx.block_end();
}

// Returns Some(path) if the path refers to a file
fn update_folder_browser_path(state: &mut State, entry: DisplayablePathBuf) -> Option<std::path::PathBuf> {
    let old_path = state.folder_browser_current_dir.as_path();
    let path = old_path.join(entry.as_path());
    let path = edit::path::normalize(&path);

    let (dir, name) = if path.is_dir() {
        let dir = if cfg!(windows)
            && entry.as_str() == ".."
            // It's unnecessary to check the contents of the paths.
            && old_path.as_os_str().len() == path.as_os_str().len()
        {
            std::path::Path::new("")
        } else {
            path.as_path()
        };
        (dir, std::path::PathBuf::new())
    } else {
        let dir = path.parent().unwrap_or(&path);
        let name = path.file_name().map_or(Default::default(), |s| s.into());
        (dir, name)
    };
    
    if dir != state.folder_browser_current_dir.as_path() {
        state.folder_browser_current_dir = DisplayablePathBuf::from_path(dir.to_path_buf());
        state.folder_browser_current_dir_revision = state.folder_browser_current_dir_revision.wrapping_add(1);
        state.folder_browser_entries = None;
        state.folder_browser_selected = 0; // Reset selection when changing directories
    }

    if name.as_os_str().is_empty() { None } else { Some(path) }
}

fn refresh_folder_browser(state: &mut State) {
    let dir = state.folder_browser_current_dir.as_path();
    // ["..", directories, files]
    let mut dirs_files = [Vec::new(), Vec::new(), Vec::new()];

    #[cfg(windows)]
    if dir.as_os_str().is_empty() {
        // If the path is empty, we are at the drive picker.
        // Add all drives as entries.
        for drive in edit::sys::drives() {
            dirs_files[1].push(DisplayablePathBuf::from_string(format!("{drive}:\\")))
        }

        state.folder_browser_entries = Some(dirs_files);
        return;
    }

    if cfg!(windows) || dir.parent().is_some() {
        dirs_files[0].push(DisplayablePathBuf::from(".."));
    }

    if let Ok(iter) = fs::read_dir(dir) {
        for entry in iter.flatten() {
            if let Ok(metadata) = entry.metadata() {
                let mut name = entry.file_name();
                let dir = metadata.is_dir()
                    || (metadata.is_symlink()
                        && fs::metadata(entry.path()).is_ok_and(|m| m.is_dir()));
                let idx = if dir { 1 } else { 2 };

                if dir {
                    name.push("/");
                }

                dirs_files[idx].push(DisplayablePathBuf::from(name));
            }
        }
    }

    for entries in &mut dirs_files[1..] {
        entries.sort_by(|a, b| {
            let a = a.as_bytes();
            let b = b.as_bytes();

            let a_is_dir = a.last() == Some(&b'/');
            let b_is_dir = b.last() == Some(&b'/');

            match b_is_dir.cmp(&a_is_dir) {
                Ordering::Equal => icu::compare_strings(a, b),
                other => other,
            }
        });
    }

    state.folder_browser_entries = Some(dirs_files);
}

pub fn toggle_folder_browser(state: &mut State) {
    state.folder_browser_visible = !state.folder_browser_visible;
    if state.folder_browser_visible && state.folder_browser_entries.is_none() {
        refresh_folder_browser(state);
    }
}
