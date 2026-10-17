//! The type scale from the design file.
//!
//! Each helper gives its text an accessibility node: a heading for
//! [`display`] and [`title`], a label for the others. The id comes from the
//! call site and the text (see [`crate::a11y::call_site_id`]), so a helper in a
//! list needs a row with its own id above it.

use crate::fonts::{DISPLAY, Face, LocaleFonts as _, MONO, SERIF, UI};
use crate::a11y::A11yText;
use crate::theme::Colors;
use gpui_kit::*;

/// A display heading in Fraunces with a letterpress highlight below it.
#[track_caller]
pub fn display(text: impl Into<SharedString>, size: f32, c: &Colors) -> Div {
    letterpress(text, size, (size * 1.08).round(), 1, c)
}

/// A Fraunces heading at `level` with the letterpress highlight: a second
/// copy of the text, 1 px lower, in the highlight color. GPUI has no text
/// shadow. Only the ink copy has an accessibility node, so a screen reader
/// reads the heading once.
#[track_caller]
pub fn letterpress(text: impl Into<SharedString>, size: f32, line_height: f32, level: usize, c: &Colors) -> Div {
    let text: SharedString = text.into();
    let node = A11yText::new(text.clone()).heading(level);
    pressed(text, node, size, line_height, c)
}

/// A number in the [`display`] face: "1.9 GB". It reads as text, not as a heading.
#[track_caller]
pub fn display_figure(text: impl Into<SharedString>, size: f32, c: &Colors) -> Div {
    let text: SharedString = text.into();
    let node = A11yText::new(text.clone());
    pressed(text, node, size, (size * 1.08).round(), c)
}

fn pressed(text: SharedString, node: A11yText, size: f32, line_height: f32, c: &Colors) -> Div {
    let base = |color: Hsla| {
        div()
            .font_family(DISPLAY).locale_fonts(Face::Serif)
            .text_size(px(size))
            .line_height(px(line_height))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(color)
    };
    div()
        .relative()
        .child(base(c.highlight(0.85)).absolute().top(px(1.)).left_0().child(text))
        .child(base(c.ink).child(node))
}

/// Small caps label: "TODAY", "HOTKEYS".
#[track_caller]
pub fn caps(text: impl Into<SharedString>, c: &Colors) -> Div {
    caps_text(A11yText::new(text.into().to_uppercase()), c)
}

/// [`caps`] as a section heading at `level`.
#[track_caller]
pub fn caps_heading(text: impl Into<SharedString>, level: usize, c: &Colors) -> Div {
    caps_text(A11yText::new(text.into().to_uppercase()).heading(level), c)
}

fn caps_text(text: A11yText, c: &Colors) -> Div {
    div().font_family(UI).text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(c.graphite).child(text)
}

/// UI text in Instrument Sans.
#[track_caller]
pub fn ui(text: impl Into<SharedString>, size: f32, weight: FontWeight, color: Hsla) -> Div {
    div().font_family(UI).text_size(px(size)).font_weight(weight).text_color(color).child(A11yText::new(text))
}

/// Body text: 15 px, 22 px line height, graphite.
#[track_caller]
pub fn body(text: impl Into<SharedString>, c: &Colors) -> Div {
    ui(text, 15., FontWeight::NORMAL, c.graphite).line_height(px(22.))
}

/// Reading serif for transcripts.
#[track_caller]
pub fn serif(text: impl Into<SharedString>, size: f32, color: Hsla) -> Div {
    div().font_family(SERIF).locale_fonts(Face::Serif).text_size(px(size)).line_height(px((size * 1.42).round())).text_color(color).child(A11yText::new(text))
}

#[track_caller]
pub fn mono(text: impl Into<SharedString>, size: f32, color: Hsla) -> Div {
    div().font_family(MONO).text_size(px(size)).font_weight(FontWeight::MEDIUM).text_color(color).child(A11yText::new(text))
}

/// A display serif title smaller than [`display`], without the highlight.
#[track_caller]
pub fn title(text: impl Into<SharedString>, size: f32, c: &Colors) -> Div {
    div()
        .font_family(DISPLAY).locale_fonts(Face::Serif)
        .text_size(px(size))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c.ink)
        .child(A11yText::new(text).heading(2))
}


/// A number or value in the [`title`] face: "93.7%", "58 ms". It reads as
/// text, not as a heading.
#[track_caller]
pub fn figure(text: impl Into<SharedString>, size: f32, c: &Colors) -> Div {
    div()
        .font_family(DISPLAY).locale_fonts(Face::Serif)
        .text_size(px(size))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c.ink)
        .child(A11yText::new(text))
}
