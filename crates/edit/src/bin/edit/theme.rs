// Copyright (c) Pavel Sich.
// Licensed under the MIT License.

//! The colors of the editor's chrome, derived from the terminal's palette.
//!
//! Everything is a blend of the terminal's background and foreground colors,
//! plus a single accent color, so that it works with any light or dark theme.

use edit::framebuffer::IndexedColor;
use edit::oklab::StraightRgba;
use edit::tui::Tui;

#[derive(Clone, Copy, Default)]
pub struct Theme {
    /// The editor's background. The terminal's own background color.
    pub surface: StraightRgba,
    /// Side panels like the file browser and the assistant.
    pub panel: StraightRgba,
    /// The menubar, the statusbar and inactive tabs.
    pub chrome: StraightRgba,
    /// Menus, dialogs, popups and input cards.
    pub elevated: StraightRgba,
    /// Borders of inactive windows and other hairlines.
    pub border: StraightRgba,
    pub text: StraightRgba,
    /// Secondary text like hints, line numbers and statusbar items.
    pub text_dim: StraightRgba,
    /// The one accent: focus, selection and the active window.
    pub accent: StraightRgba,
    /// A faint wash of the accent, for tinted bars and hover states.
    pub accent_soft: StraightRgba,
    /// Text on top of [`Theme::accent`].
    pub on_accent: StraightRgba,
}

impl Theme {
    pub fn new(tui: &Tui) -> Self {
        let bg = tui.indexed(IndexedColor::Background);
        let fg = tui.indexed(IndexedColor::Foreground);
        let accent = tui.indexed(IndexedColor::BrightBlue);

        Self {
            surface: bg,
            panel: mix(bg, fg, 1, 24),
            chrome: mix(bg, fg, 1, 12),
            elevated: mix(bg, fg, 1, 7),
            border: mix(bg, fg, 1, 4),
            text: fg,
            text_dim: mix(bg, fg, 3, 5),
            accent,
            accent_soft: mix(bg, accent, 1, 4),
            on_accent: tui.contrasted(accent),
        }
    }

    /// Applies the theme to the widgets drawn by the TUI itself.
    pub fn apply(&self, tui: &mut Tui) {
        tui.set_floater_default_bg(self.elevated);
        tui.set_floater_default_fg(self.text);
        tui.set_modal_default_bg(self.elevated);
        tui.set_modal_default_fg(self.text);
        tui.set_highlight_bg(self.accent);
        tui.set_scrollbar_colors(self.chrome, self.border);
    }
}

/// Mixes `numerator / denominator` of `top` into `base` in sRGB space.
///
/// Unlike [`StraightRgba::oklab_blend`], small amounts stay visible on a pure black
/// background, which is what the subtle surface tints need.
fn mix(base: StraightRgba, top: StraightRgba, numerator: u32, denominator: u32) -> StraightRgba {
    let channel = |b: u32, t: u32| (b * (denominator - numerator) + t * numerator) / denominator;
    StraightRgba::from_rgba(
        channel(base.red(), top.red()) << 24
            | channel(base.green(), top.green()) << 16
            | channel(base.blue(), top.blue()) << 8
            | 0xff,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mix() {
        let black = StraightRgba::from_rgba(0x000000ff);
        let white = StraightRgba::from_rgba(0xc0c0c0ff);
        assert_eq!(mix(black, white, 1, 4).to_rgba(), 0x303030ff);
        assert_eq!(mix(black, white, 0, 4).to_rgba(), 0x000000ff);
        assert_eq!(mix(black, white, 4, 4).to_rgba(), 0xc0c0c0ff);
    }
}
