// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

#![feature(allocator_api, let_chains, linked_list_cursors, string_from_utf8_lossy_owned)]

mod documents;
mod draw_ai_dock;
mod draw_editor;
mod draw_filepicker;
mod draw_folder_browser;
mod draw_menubar;
mod draw_statusbar;
mod draw_tabbar;
mod localization;
mod state;

use std::borrow::Cow;
#[cfg(feature = "debug-latency")]
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{env, process};

use draw_ai_dock::*;
use draw_editor::*;
use draw_filepicker::*;
use draw_folder_browser::*;
use draw_menubar::*;
use draw_statusbar::*;
use draw_tabbar::*;
use edit::arena::{self, Arena, ArenaString, scratch_arena};
use edit::framebuffer::{self, IndexedColor};
use edit::helpers::{CoordType, KIBI, MEBI, MetricFormatter, Rect, Size};
use edit::input::{self, kbmod, vk};
use edit::oklab::oklab_blend;
use edit::tui::*;
use edit::vt::{self, Token};
use edit::{apperr, arena_format, base64, path, sys, unicode};
use localization::*;
use state::*;

#[cfg(target_pointer_width = "32")]
const SCRATCH_ARENA_CAPACITY: usize = 128 * MEBI;
#[cfg(target_pointer_width = "64")]
const SCRATCH_ARENA_CAPACITY: usize = 512 * MEBI;

fn main() -> process::ExitCode {
    if cfg!(debug_assertions) {
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            drop(RestoreModes);
            drop(sys::Deinit);
            hook(info);
        }));
    }

    match run() {
        Ok(()) => process::ExitCode::SUCCESS,
        Err(err) => {
            sys::write_stdout(&format!("{}\r\n", FormatApperr::from(err)));
            process::ExitCode::FAILURE
        }
    }
}

fn run() -> apperr::Result<()> {
    // Init `sys` first, as everything else may depend on its functionality (IO, function pointers, etc.).
    let _sys_deinit = sys::init()?;
    // Next init `arena`, so that `scratch_arena` works. `loc` depends on it.
    arena::init(SCRATCH_ARENA_CAPACITY)?;
    // Init the `loc` module, so that error messages are localized.
    localization::init();

    let mut state = State::new()?;
    if handle_args(&mut state)? {
        return Ok(());
    }

    // sys::init() will switch the terminal to raw mode which prevents the user from pressing Ctrl+C.
    // Since the `read_file` call may hang for some reason, we must only call this afterwards.
    // `set_modes()` will enable mouse mode which is equally annoying to switch out for users
    // and so we do it afterwards, for similar reasons.
    sys::switch_modes()?;

    let mut vt_parser = vt::Parser::new();
    let mut input_parser = input::Parser::new();
    let mut tui = Tui::new()?;

    let _restore = setup_terminal(&mut tui, &mut state, &mut vt_parser);

    state.menubar_color_bg = oklab_blend(
        tui.indexed(IndexedColor::Background),
        tui.indexed_alpha(IndexedColor::BrightBlue, 1, 2),
    );
    state.menubar_color_fg = tui.contrasted(state.menubar_color_bg);
    let floater_bg = oklab_blend(
        tui.indexed_alpha(IndexedColor::Background, 2, 3),
        tui.indexed_alpha(IndexedColor::Foreground, 1, 3),
    );
    let floater_fg = tui.contrasted(floater_bg);
    tui.setup_modifier_translations(ModifierTranslations {
        ctrl: loc(LocId::Ctrl),
        alt: loc(LocId::Alt),
        shift: loc(LocId::Shift),
    });
    tui.set_floater_default_bg(floater_bg);
    tui.set_floater_default_fg(floater_fg);
    tui.set_modal_default_bg(floater_bg);
    tui.set_modal_default_fg(floater_fg);

    sys::inject_window_size_into_stdin();

    #[cfg(feature = "debug-latency")]
    let mut last_latency_width = 0;

    loop {
        #[cfg(feature = "debug-latency")]
        let time_beg;
        #[cfg(feature = "debug-latency")]
        let mut passes;

        // Process a batch of input.
        {
            let scratch = scratch_arena(None);
            let read_timeout = vt_parser.read_timeout().min(tui.read_timeout());
            let Some(input) = sys::read_stdin(&scratch, read_timeout) else {
                break;
            };

            #[cfg(feature = "debug-latency")]
            {
                time_beg = std::time::Instant::now();
                passes = 0usize;
            }

            let vt_iter = vt_parser.parse(&input);
            let mut input_iter = input_parser.parse(vt_iter);

            while {
                let input = input_iter.next();
                let more = input.is_some();
                let mut ctx = tui.create_context(input);

                draw(&mut ctx, &mut state);

                #[cfg(feature = "debug-latency")]
                {
                    passes += 1;
                }

                more
            } {}
        }

        // Continue rendering until the layout has settled.
        // This can take >1 frame, if the input focus is tossed between different controls.
        while tui.needs_settling() {
            let mut ctx = tui.create_context(None);

            draw(&mut ctx, &mut state);

            #[cfg(feature = "debug-latency")]
            {
                passes += 1;
            }
        }

        if state.exit {
            break;
        }

        // Render the UI and write it to the terminal.
        {
            let scratch = scratch_arena(None);
            let mut output = tui.render(&scratch);

            {
                let filename = state.documents.active().map_or("", |d| &d.filename);
                if filename != state.osc_title_filename {
                    write_terminal_title(&mut output, filename);
                    state.osc_title_filename = filename.to_string();
                }
            }

            if state.osc_clipboard_sync {
                write_osc_clipboard(&mut tui, &mut state, &mut output);
            }

            #[cfg(feature = "debug-latency")]
            {
                // Print the number of passes and latency in the top right corner.
                let time_end = std::time::Instant::now();
                let status = time_end - time_beg;

                let scratch_alt = scratch_arena(Some(&scratch));
                let status = arena_format!(
                    &scratch_alt,
                    "{}P {}B {:.3}μs",
                    passes,
                    output.len(),
                    status.as_nanos() as f64 / 1000.0
                );

                // "μs" is 3 bytes and 2 columns.
                let cols = status.len() as edit::helpers::CoordType - 3 + 2;

                // Since the status may shrink and grow, we may have to overwrite the previous one with whitespace.
                let padding = (last_latency_width - cols).max(0);

                // If the `output` is already very large,
                // Rust may double the size during the write below.
                // Let's avoid that by reserving the needed size in advance.
                output.reserve_exact(128);

                // To avoid moving the cursor, push and pop it onto the VT cursor stack.
                _ = write!(
                    output,
                    "\x1b7\x1b[0;41;97m\x1b[1;{0}H{1:2$}{3}\x1b8",
                    tui.size().width - cols - padding + 1,
                    "",
                    padding as usize,
                    status
                );

                last_latency_width = cols;
            }

            sys::write_stdout(&output);
        }
    }

    Ok(())
}

// Returns true if the application should exit early.
fn handle_args(state: &mut State) -> apperr::Result<bool> {
    let scratch = scratch_arena(None);
    let mut paths: Vec<PathBuf, &Arena> = Vec::new_in(&*scratch);
    let mut cwd = env::current_dir()?;

    // The best CLI argument parser in the world.
    for arg in env::args_os().skip(1) {
        if arg == "-h" || arg == "--help" || (cfg!(windows) && arg == "/?") {
            print_help();
            return Ok(true);
        } else if arg == "-v" || arg == "--version" {
            print_version();
            return Ok(true);
        } else if arg == "-" {
            paths.clear();
            break;
        }
        let p = cwd.join(Path::new(&arg));
        let p = path::normalize(&p);
        if !p.is_dir() {
            paths.push(p);
        }
    }

    for p in &paths {
        state.documents.add_file_path(p)?;
    }
    if let Some(parent) = paths.first().and_then(|p| p.parent()) {
        cwd = parent.to_path_buf();
    }

    if let Some(mut file) = sys::open_stdin_if_redirected() {
        let doc = state.documents.add_untitled()?;
        let mut tb = doc.buffer.borrow_mut();
        tb.read_file(&mut file, None)?;
        tb.mark_as_dirty();
    } else if paths.is_empty() {
        // No files were passed, and stdin is not redirected.
        state.documents.add_untitled()?;
    }

    state.file_picker_pending_dir = DisplayablePathBuf::from_path(cwd);
    Ok(false)
}

fn print_help() {
    sys::write_stdout(concat!(
        "Usage: edit [OPTIONS] [FILE[:LINE[:COLUMN]]]\r\n",
        "Options:\r\n",
        "    -h, --help       Print this help message\r\n",
        "    -v, --version    Print the version number\r\n",
        "\r\n",
        "Arguments:\r\n",
        "    FILE[:LINE[:COLUMN]]    The file to open, optionally with line and column (e.g., foo.txt:123:45)\r\n",
    ));
}

fn print_version() {
    sys::write_stdout(concat!("edit version ", env!("CARGO_PKG_VERSION"), "\r\n"));
}

fn draw(ctx: &mut Context, state: &mut State) {
    draw_menubar(ctx, state);
    draw_tabbar(ctx, state);
    
    if state.folder_browser_visible {
        // Calculate editor and folder widths for split view
        let screen_width = ctx.size().width;
        let editor_width = (screen_width as f32 * 0.7) as CoordType;  // 70% for editor
        let folder_browser_width = screen_width - editor_width;        // Remaining 30% for folder browser
        
        // Update state with calculated widths
        state.folder_browser_width = folder_browser_width;
        
        // Editor and Folder Browser side by side
        ctx.table_begin("content_layout");
        ctx.table_set_columns(&[editor_width, folder_browser_width]);
        ctx.table_next_row();
        draw_editor(ctx, state);
        draw_folder_browser(ctx, state);
        ctx.table_end();
    } else {
        // Just editor taking full width
        draw_editor(ctx, state);
    }
    
    draw_statusbar(ctx, state);
    draw_ai_dock(ctx, state); // Draw AI dock above status bar

    if state.wants_close {
        draw_handle_wants_close(ctx, state);
    }
    if state.wants_exit {
        draw_handle_wants_exit(ctx, state);
    }
    if state.wants_goto {
        draw_goto_menu(ctx, state);
    }
    if state.wants_file_picker != StateFilePicker::None {
        draw_file_picker(ctx, state);
    }
    if state.wants_save {
        draw_handle_save(ctx, state);
    }
    if state.wants_encoding_change != StateEncodingChange::None {
        draw_dialog_encoding_change(ctx, state);
    }
    if state.wants_go_to_file {
        draw_go_to_file(ctx, state);
    }
    if state.wants_about {
        draw_dialog_about(ctx, state);
    }
    if ctx.clipboard_ref().wants_host_sync() {
        draw_handle_clipboard_change(ctx, state);
    }
    if state.error_log_count != 0 {
        draw_error_log(ctx, state);
    }

    if let Some(key) = ctx.keyboard_input() {
        // Shortcuts that are not handled as part of the textarea, etc.

        if key == kbmod::CTRL | vk::N {
            draw_add_untitled_document(ctx, state);
        } else if key == kbmod::CTRL | vk::O {
            state.wants_file_picker = StateFilePicker::Open;
        } else if key == kbmod::CTRL | vk::S {
            state.wants_save = true;
        } else if key == kbmod::CTRL_SHIFT | vk::S {
            state.wants_file_picker = StateFilePicker::SaveAs;
        } else if key == kbmod::CTRL | vk::W {
            state.wants_close = true;
        } else if key == kbmod::ALT | vk::W && state.documents.len() > 1 {
            // Close current tab with Alt+W (only if multiple tabs open)
            let active_index = state.documents.active_index();
            if let Some(doc) = state.documents.active() {
                if doc.buffer.borrow().is_dirty() {
                    // If dirty, show close confirmation
                    state.wants_close = true;
                } else {
                    // Close without confirmation
                    state.documents.remove_at_index(active_index);
                }
            }
        } else if key == kbmod::CTRL | vk::P {
            state.wants_go_to_file = true;
        } else if key == kbmod::CTRL | vk::Q {
            state.wants_exit = true;
        } else if key == kbmod::CTRL | vk::G {
            state.wants_goto = true;
        } else if key == kbmod::CTRL | vk::F && state.wants_search.kind != StateSearchKind::Disabled
        {
            state.wants_search.kind = StateSearchKind::Search;
            state.wants_search.focus = true;
        } else if key == kbmod::CTRL | vk::R && state.wants_search.kind != StateSearchKind::Disabled
        {
            state.wants_search.kind = StateSearchKind::Replace;
            state.wants_search.focus = true;
        } else if key == vk::F3 {
            search_execute(ctx, state, SearchAction::Search);
        } else if key == kbmod::CTRL | vk::TAB && state.documents.len() > 1 {
            // Switch to next tab
            let active_index = state.documents.active_index();
            let next_index = (active_index + 1) % state.documents.len();
            state.documents.set_active_index(next_index);
        } else if key == kbmod::CTRL_SHIFT | vk::TAB && state.documents.len() > 1 {
            // Switch to previous tab
            let active_index = state.documents.active_index();
            let prev_index = if active_index == 0 { 
                state.documents.len() - 1 
            } else { 
                active_index - 1 
            };
            state.documents.set_active_index(prev_index);
        } else if key == kbmod::CTRL | vk::N1 && state.documents.len() > 0 {
            state.documents.set_active_index(0);
        } else if key == kbmod::CTRL | vk::N2 && state.documents.len() > 1 {
            state.documents.set_active_index(1);
        } else if key == kbmod::CTRL | vk::N3 && state.documents.len() > 2 {
            state.documents.set_active_index(2);
        } else if key == kbmod::CTRL | vk::N4 && state.documents.len() > 3 {
            state.documents.set_active_index(3);
        } else if key == kbmod::CTRL | vk::N5 && state.documents.len() > 4 {
            state.documents.set_active_index(4);
        } else if key == kbmod::CTRL | vk::N6 && state.documents.len() > 5 {
            state.documents.set_active_index(5);
        } else if key == kbmod::CTRL | vk::N7 && state.documents.len() > 6 {
            state.documents.set_active_index(6);
        } else if key == kbmod::CTRL | vk::N8 && state.documents.len() > 7 {
            state.documents.set_active_index(7);
        } else if key == kbmod::CTRL | vk::N9 && state.documents.len() > 8 {
            state.documents.set_active_index(8);
        } else if key == kbmod::CTRL_ALT | vk::B {
            // Toggle AI dock
            state.ai_dock_visible = !state.ai_dock_visible;
            if state.ai_dock_visible {
                state.ai_dock_focused = true;
            } else {
                state.ai_dock_focused = false;
            }
        } else if key == vk::F4 {
            // Toggle folder browser
            toggle_folder_browser(state);
        } else {
            return;
        }

        // All of the above shortcuts happen to require a rerender.
        ctx.needs_rerender();
        ctx.set_input_consumed();
    }
}

fn draw_handle_wants_exit(_ctx: &mut Context, state: &mut State) {
    while let Some(doc) = state.documents.active() {
        if doc.buffer.borrow().is_dirty() {
            state.wants_close = true;
            return;
        }
        state.documents.remove_active();
    }

    if state.documents.len() == 0 {
        state.exit = true;
    }
}

#[cold]
fn write_terminal_title(output: &mut ArenaString, filename: &str) {
    output.push_str("\x1b]0;");

    if !filename.is_empty() {
        output.push_str(&sanitize_control_chars(filename));
        output.push_str(" - ");
    }

    output.push_str("edit\x1b\\");
}

const LARGE_CLIPBOARD_THRESHOLD: usize = 128 * KIBI;

fn draw_handle_clipboard_change(ctx: &mut Context, state: &mut State) {
    let data_len = ctx.clipboard_ref().read().len();

    if state.osc_clipboard_always_send || data_len < LARGE_CLIPBOARD_THRESHOLD {
        ctx.clipboard_mut().mark_as_synchronized();
        state.osc_clipboard_sync = true;
        return;
    }

    let over_limit = data_len >= SCRATCH_ARENA_CAPACITY / 4;
    let mut done = None;

    ctx.modal_begin("warning", loc(LocId::WarningDialogTitle));
    {
        ctx.block_begin("description");
        ctx.attr_padding(Rect::three(1, 2, 1));

        if over_limit {
            ctx.label("line1", loc(LocId::LargeClipboardWarningLine1));
            ctx.attr_position(Position::Center);
            ctx.label("line2", loc(LocId::SuperLargeClipboardWarning));
            ctx.attr_position(Position::Center);
        } else {
            let label2 = {
                let template = loc(LocId::LargeClipboardWarningLine2);
                let size = arena_format!(ctx.arena(), "{}", MetricFormatter(data_len));

                let mut label =
                    ArenaString::with_capacity_in(template.len() + size.len(), ctx.arena());
                label.push_str(template);
                label.replace_once_in_place("{size}", &size);
                label
            };

            ctx.label("line1", loc(LocId::LargeClipboardWarningLine1));
            ctx.attr_position(Position::Center);
            ctx.label("line2", &label2);
            ctx.attr_position(Position::Center);
            ctx.label("line3", loc(LocId::LargeClipboardWarningLine3));
            ctx.attr_position(Position::Center);
        }
        ctx.block_end();

        ctx.table_begin("choices");
        ctx.inherit_focus();
        ctx.attr_padding(Rect::three(0, 2, 1));
        ctx.attr_position(Position::Center);
        ctx.table_set_cell_gap(Size { width: 2, height: 0 });
        {
            ctx.table_next_row();
            ctx.inherit_focus();

            if over_limit {
                if ctx.button("ok", loc(LocId::Ok), ButtonStyle::default()) {
                    done = Some(true);
                }
                ctx.inherit_focus();
            } else {
                if ctx.button("always", loc(LocId::Always), ButtonStyle::default()) {
                    state.osc_clipboard_always_send = true;
                    done = Some(true);
                }

                if ctx.button("yes", loc(LocId::Yes), ButtonStyle::default()) {
                    done = Some(true);
                }
                if data_len < 10 * LARGE_CLIPBOARD_THRESHOLD {
                    ctx.inherit_focus();
                }

                if ctx.button("no", loc(LocId::No), ButtonStyle::default()) {
                    done = Some(false);
                }
                if data_len >= 10 * LARGE_CLIPBOARD_THRESHOLD {
                    ctx.inherit_focus();
                }
            }
        }
        ctx.table_end();
    }
    if ctx.modal_end() {
        done = Some(false);
    }

    if let Some(sync) = done {
        state.osc_clipboard_sync = sync;
        ctx.clipboard_mut().mark_as_synchronized();
        ctx.needs_rerender();
    }
}

#[cold]
fn write_osc_clipboard(tui: &mut Tui, state: &mut State, output: &mut ArenaString) {
    let clipboard = tui.clipboard_mut();
    let data = clipboard.read();

    if !data.is_empty() {
        // Rust doubles the size of a string when it needs to grow it.
        // If `data` is *really* large, this may then double
        // the size of the `output` from e.g. 100MB to 200MB. Not good.
        // We can avoid that by reserving the needed size in advance.
        output.reserve_exact(base64::encode_len(data.len()) + 16);
        output.push_str("\x1b]52;c;");
        base64::encode(output, data);
        output.push_str("\x1b\\");
    }

    state.osc_clipboard_sync = false;
}

struct RestoreModes;

impl Drop for RestoreModes {
    fn drop(&mut self) {
        // Same as in the beginning but in the reverse order.
        // It also includes DECSCUSR 0 to reset the cursor style and DECTCEM to show the cursor.
        // We specifically don't reset mode 1036, because most applications expect it to be set nowadays.
        sys::write_stdout("\x1b[0 q\x1b[?25h\x1b]0;\x07\x1b[?1002;1006;2004l\x1b[?1049l");
    }
}

fn setup_terminal(tui: &mut Tui, state: &mut State, vt_parser: &mut vt::Parser) -> RestoreModes {
    sys::write_stdout(concat!(
        // 1049: Alternative Screen Buffer
        //   I put the ASB switch in the beginning, just in case the terminal performs
        //   some additional state tracking beyond the modes we enable/disable.
        // 1002: Cell Motion Mouse Tracking
        // 1006: SGR Mouse Mode
        // 2004: Bracketed Paste Mode
        // 1036: Xterm: "meta sends escape" (Alt keypresses should be encoded with ESC + char)
        "\x1b[?1049h\x1b[?1002;1006;2004h\x1b[?1036h",
        // OSC 4 color table requests for indices 0 through 15 (base colors).
        "\x1b]4;0;?;1;?;2;?;3;?;4;?;5;?;6;?;7;?\x07",
        "\x1b]4;8;?;9;?;10;?;11;?;12;?;13;?;14;?;15;?\x07",
        // OSC 10 and 11 queries for the current foreground and background colors.
        "\x1b]10;?\x07\x1b]11;?\x07",
        // Test whether ambiguous width characters are two columns wide.
        // We use "…", because it's the most common ambiguous width character we use,
        // and the old Windows conhost doesn't actually use wcwidth, it measures the
        // actual display width of the character and assigns it columns accordingly.
        // We detect it by writing the character and asking for the cursor position.
        "\r…\x1b[6n",
        // CSI c reports the terminal capabilities.
        // It also helps us to detect the end of the responses, because not all
        // terminals support the OSC queries, but all of them support CSI c.
        "\x1b[c",
    ));

    let mut done = false;
    let mut osc_buffer = String::new();
    let mut indexed_colors = framebuffer::DEFAULT_THEME;
    let mut color_responses = 0;
    let mut ambiguous_width = 1;

    while !done {
        let scratch = scratch_arena(None);

        // We explicitly set a high read timeout, because we're not
        // waiting for user keyboard input. If we encounter a lone ESC,
        // it's unlikely to be from a ESC keypress, but rather from a VT sequence.
        let Some(input) = sys::read_stdin(&scratch, Duration::from_secs(3)) else {
            break;
        };

        let mut vt_stream = vt_parser.parse(&input);
        while let Some(token) = vt_stream.next() {
            match token {
                Token::Csi(csi) => match csi.final_byte {
                    'c' => done = true,
                    // CPR (Cursor Position Report) response.
                    'R' => ambiguous_width = csi.params[1] as CoordType - 1,
                    _ => {}
                },
                Token::Osc { mut data, partial } => {
                    if partial {
                        osc_buffer.push_str(data);
                        continue;
                    }
                    if !osc_buffer.is_empty() {
                        osc_buffer.push_str(data);
                        data = &osc_buffer;
                    }

                    let mut splits = data.split_terminator(';');

                    let color = match splits.next().unwrap_or("") {
                        // The response is `4;<color>;rgb:<r>/<g>/<b>`.
                        "4" => match splits.next().unwrap_or("").parse::<usize>() {
                            Ok(val) if val < 16 => &mut indexed_colors[val],
                            _ => continue,
                        },
                        // The response is `10;rgb:<r>/<g>/<b>`.
                        "10" => &mut indexed_colors[IndexedColor::Foreground as usize],
                        // The response is `11;rgb:<r>/<g>/<b>`.
                        "11" => &mut indexed_colors[IndexedColor::Background as usize],
                        _ => continue,
                    };

                    let color_param = splits.next().unwrap_or("");
                    if !color_param.starts_with("rgb:") {
                        continue;
                    }

                    let mut iter = color_param[4..].split_terminator('/');
                    let rgb_parts = [(); 3].map(|_| iter.next().unwrap_or("0"));
                    let mut rgb = 0;

                    for part in rgb_parts {
                        if part.len() == 2 || part.len() == 4 {
                            let Ok(mut val) = usize::from_str_radix(part, 16) else {
                                continue;
                            };
                            if part.len() == 4 {
                                // Round from 16 bits to 8 bits.
                                val = (val * 0xff + 0x7fff) / 0xffff;
                            }
                            rgb = (rgb >> 8) | ((val as u32) << 16);
                        }
                    }

                    *color = rgb | 0xff000000;
                    color_responses += 1;
                    osc_buffer.clear();
                }
                _ => {}
            }
        }
    }

    if ambiguous_width == 2 {
        unicode::setup_ambiguous_width(2);
        state.documents.reflow_all();
    }

    if color_responses == indexed_colors.len() {
        tui.setup_indexed_colors(indexed_colors);
    }

    RestoreModes
}

/// Strips all C0 control characters from the string and replaces them with "_".
///
/// Jury is still out on whether this should also strip C1 control characters.
/// That requires parsing UTF8 codepoints, which is annoying.
fn sanitize_control_chars(text: &str) -> Cow<'_, str> {
    if let Some(off) = text.bytes().position(|b| (..0x20).contains(&b)) {
        let mut sanitized = text.to_string();
        // SAFETY: We only search for ASCII and replace it with ASCII.
        let vec = unsafe { sanitized.as_bytes_mut() };

        for i in &mut vec[off..] {
            *i = if (..0x20).contains(i) { b'_' } else { *i }
        }

        Cow::Owned(sanitized)
    } else {
        Cow::Borrowed(text)
    }
}
