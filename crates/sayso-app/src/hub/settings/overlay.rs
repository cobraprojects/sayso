//! Settings › Overlay: the style (pill or notch), then the settings of that
//! style: size, the idle pill, full screen, the display, live preview, pill position.

use super::kit::{self, group, row};
use crate::model::AppModel;
use gpui_kit::*;
use sayso_core::config::{OverlaySize, OverlayStyle};
use sayso_core::ink::Appearance;
use sayso_i18n::t;
use sayso_ui::a11y::{A11yExt as _, Pressable, Role};
use sayso_ui::components::*;
use sayso_ui::{ActivePaper, Colors, text};

/// A still waveform for the previews of the style cards.
const PREVIEW_LEVELS: [f32; 9] = [0.2, 0.5, 0.8, 0.45, 0.9, 0.6, 0.75, 0.4, 0.2];

pub struct OverlaySettings {
    model: Entity<AppModel>,
    reset_note: Option<Result<(), String>>,
}

impl OverlaySettings {
    pub fn new(model: Entity<AppModel>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { model, reset_note: None }
    }

    fn choose_style(&mut self, style: OverlayStyle, cx: &mut Context<Self>) {
        self.model.update(cx, |m, cx| m.edit_config(cx, |c| c.overlay.style = style));
    }

    /// The style picker: one radio card for each style. The arrow keys
    /// change the style, as in the ink wells.
    fn style_cards(&mut self, style: OverlayStyle, cx: &mut Context<Self>) -> Div {
        let c = cx.paper().colors;
        let dark = cx.paper().colors_for(Appearance::Dark);
        let count = OverlayStyle::ALL.len();
        let selected_index = OverlayStyle::ALL.iter().position(|s| *s == style).unwrap_or(0);
        let mut cards = div()
            .id("overlay-styles")
            .role(Role::RadioGroup)
            .aria_label(t!("settings.overlay.style"))
            .flex()
            .gap(px(20.))
            .pt(px(6.))
            .on_key_down(cx.listener(move |this, e: &KeyDownEvent, window, cx| {
                let next = match e.keystroke.key.as_str() {
                    "right" | "down" if selected_index + 1 < count => selected_index + 1,
                    "left" | "up" if selected_index > 0 => selected_index - 1,
                    _ => return,
                };
                cx.stop_propagation();
                this.choose_style(OverlayStyle::ALL[next], cx);
                if next > selected_index { window.focus_next(cx) } else { window.focus_prev(cx) }
            }));
        for (i, s) in OverlayStyle::ALL.into_iter().enumerate() {
            let selected = s == style;
            let (name, detail, preview) = match s {
                OverlayStyle::Pill => (t!("settings.overlay.pill"), t!("settings.overlay.style_pill_detail"), pill_preview(&c)),
                OverlayStyle::Notch => {
                    let detail = if cfg!(target_os = "macos") {
                        t!("settings.overlay.style_notch_detail_mac")
                    } else {
                        t!("settings.overlay.style_notch_detail_other")
                    };
                    (t!("settings.overlay.style_notch"), detail, notch_preview(&c, &dark))
                }
            };
            cards = cards.child(
                Pressable::new(("overlay-style", i))
                    .a11y_radio(name.clone(), selected, i, count)
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap(px(12.))
                    .w(px(264.))
                    .cursor_pointer()
                    .child(preview.shadow(card_shadow(selected, &c)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .child(
                                div()
                                    .font_family(sayso_ui::fonts::UI)
                                    .text_size(px(15.))
                                    .line_height(px(18.))
                                    .font_weight(if selected { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
                                    .text_color(if selected { c.ink } else { c.graphite })
                                    .child(name.to_string()),
                            )
                            .child(text::ui(detail, 13., FontWeight::NORMAL, c.graphite).line_height(px(18.))),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.choose_style(s, cx))),
            );
        }
        div()
            .flex()
            .flex_col()
            .w_full()
            .min_w_0()
            .gap(px(14.))
            .pt(px(8.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(crate::widgets::section(&t!("settings.overlay.style"), cx))
                    .child(text::ui(t!("settings.overlay.style_detail"), 15., FontWeight::NORMAL, c.graphite).line_height(px(22.)).max_w(px(560.))),
            )
            .child(cards)
    }
}

impl Render for OverlaySettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let o = self.model.read(cx).config.overlay.clone();
        let notch = o.style == OverlayStyle::Notch;
        let (m1, m2, m3, m4, m5, m6) = (self.model.clone(), self.model.clone(), self.model.clone(), self.model.clone(), self.model.clone(), self.model.clone());
        let follow = Switch::new("follow-display", o.follow_display).label(t!("settings.overlay.follow")).on_toggle(move |on, _, cx| {
            m5.update(cx, |m, cx| m.edit_config(cx, |c| c.overlay.follow_display = on));
        });
        let follow_help = match (notch, crate::shell::can_place_windows()) {
            (true, true) => t!("settings.overlay.follow_notch_detail_placed"),
            (true, false) => t!("settings.overlay.follow_notch_detail"),
            (false, true) => t!("settings.overlay.follow_detail_placed"),
            (false, false) => t!("settings.overlay.follow_detail"),
        };
        let preview = Switch::new("show-preview", o.show_preview).label(t!("settings.overlay.live_preview")).on_toggle(move |on, _, cx| {
            m3.update(cx, |m, cx| m.edit_config(cx, |c| c.overlay.show_preview = on));
        });

        let style = self.style_cards(o.style, cx);
        let g = if notch {
            let mut g = group(&t!("settings.overlay.style_notch"), cx);
            // Only a Mac has displays with a camera notch. On the other
            // systems every display gets a drawn notch.
            if cfg!(target_os = "macos") {
                let others = Switch::new("notch-other-displays", o.notch_on_other_displays).label(t!("settings.overlay.notch_other_displays")).on_toggle(move |on, _, cx| {
                    m6.update(cx, |m, cx| m.edit_config(cx, |c| c.overlay.notch_on_other_displays = on));
                });
                g = g.child(row(&t!("settings.overlay.notch_other_displays"), &t!("settings.overlay.notch_other_displays_detail"), others, cx));
            }
            g.child(row(&t!("settings.overlay.follow"), &follow_help, follow, cx))
        } else {
            let size_index = OverlaySize::ALL.iter().position(|s| *s == o.size).unwrap_or(2);
            let size = Segmented::new("overlay-size", [t!("settings.overlay.size_small"), t!("settings.overlay.size_medium"), t!("settings.overlay.size_large")], size_index)
                .label(t!("settings.overlay.size"))
                .on_select(move |i, _, cx| {
                    let size = OverlaySize::ALL[i];
                    m4.update(cx, |m, cx| m.edit_config(cx, |c| c.overlay.size = size));
                });
            let pill = Switch::new("idle-pill", o.idle_pill).label(t!("settings.overlay.idle_pill")).on_toggle(move |on, _, cx| {
                m1.update(cx, |m, cx| m.edit_config(cx, |c| c.overlay.idle_pill = on));
            });
            let fullscreen = Switch::new("hide-fullscreen", o.hide_in_fullscreen).label(t!("settings.overlay.hide_fullscreen")).on_toggle(move |on, _, cx| {
                m2.update(cx, |m, cx| m.edit_config(cx, |c| c.overlay.hide_in_fullscreen = on));
            });
            let reset = Button::new("reset-pill", t!("settings.overlay.reset_position")).small().on_click(cx.listener(|this, _, _, cx| {
                this.reset_note = Some(this.model.read(cx).reset_pill_position());
                cx.notify();
            }));
            let mut g = group(&t!("settings.overlay.pill"), cx)
                .child(row(&t!("settings.overlay.size"), &t!("settings.overlay.size_detail"), size, cx))
                .child(row(&t!("settings.overlay.idle_pill"), &t!("settings.overlay.idle_pill_detail"), pill, cx))
                .child(row(&t!("settings.overlay.hide_fullscreen"), &t!("settings.overlay.hide_fullscreen_detail"), fullscreen, cx))
                .child(row(&t!("settings.overlay.follow"), &follow_help, follow, cx))
                .child(row(&t!("settings.overlay.position"), &t!("settings.overlay.position_detail"), reset, cx));
            match &self.reset_note {
                Some(Ok(())) => g = g.child(kit::banner(BannerKind::Info, t!("settings.overlay.reset_done"), cx)),
                Some(Err(e)) => g = g.child(kit::banner(BannerKind::Warning, t!("settings.overlay.reset_failed", error = e), cx)),
                None => {}
            }
            g
        };

        let preview_help = if notch { t!("settings.overlay.live_preview_notch_detail") } else { t!("settings.overlay.live_preview_detail") };
        let rec = group(&t!("settings.overlay.while_dictating"), cx).child(row(&t!("settings.overlay.live_preview"), &preview_help, preview, cx));

        kit::page("overlay-page", &t!("settings.overlay.title"), &t!("settings.overlay.subtitle"), kit::body().child(style).child(g).child(rec), cx)
    }
}

/// The bevel of a preview, with an ink ring when its style is chosen.
fn card_shadow(selected: bool, c: &Colors) -> Vec<BoxShadow> {
    let mut s = vec![
        BoxShadow::new(px(0.), px(1.), c.shadow(0.22)).blur_radius(px(3.)).inset(),
        BoxShadow::new(px(0.), px(-1.), c.highlight(0.7)).inset(),
    ];
    if selected {
        s.push(BoxShadow::new(px(0.), px(0.), c.ink).spread_radius(px(2.)));
    }
    s
}

/// A small desktop: the screen of a preview, with a menu bar at the top.
fn desktop(menu_bar: f32, c: &Colors) -> Div {
    div()
        .relative()
        .flex_none()
        .w(px(264.))
        .h(px(132.))
        .rounded(px(12.))
        .overflow_hidden()
        .bg(c.deboss)
        .child(div().absolute().top_0().left_0().w_full().h(px(menu_bar)).bg(c.sheet.opacity(0.6)))
}

/// The pill at the bottom of the desktop.
fn pill_preview(c: &Colors) -> Div {
    desktop(12., c).child(
        div()
            .absolute()
            .bottom(px(16.))
            .left(px(80.))
            .w(px(104.))
            .h(px(26.))
            .flex()
            .items_center()
            .gap(px(8.))
            .pl(px(5.))
            .pr(px(10.))
            .rounded_full()
            .bg(c.sheet_raised)
            .shadow(vec![
                BoxShadow::new(px(0.), px(1.), c.highlight(1.)).inset(),
                BoxShadow::new(px(0.), px(1.), c.shadow(0.18)).blur_radius(px(2.)),
                BoxShadow::new(px(0.), px(6.), c.shadow(0.16)).blur_radius(px(14.)),
            ])
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(16.))
                    .rounded_full()
                    .bg(c.deboss)
                    .shadow(vec![BoxShadow::new(px(0.), px(1.), c.shadow(0.3)).blur_radius(px(2.)).inset()])
                    .child(div().size(px(6.)).rounded_full().bg(c.accent)),
            )
            .child(div().w(px(62.)).h(px(12.)).child(ink_waveform(PREVIEW_LEVELS.to_vec(), 0., c.ink).size_full())),
    )
}

/// The notch at the top of the desktop, with a slip of words under it.
fn notch_preview(c: &Colors, dark: &Colors) -> Div {
    let bar = |w: f32, color: Hsla, alpha: f32| div().flex_none().w(px(w)).h(px(3.)).rounded(px(2.)).bg(color.opacity(alpha));
    desktop(19., c)
        // The menu bar of a display with a notch is black above the notch.
        .child(div().absolute().top_0().left_0().w_full().h(px(5.)).bg(black()))
        .child(
            div()
                .absolute()
                .top(px(5.))
                .left_0()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_between()
                        .w(px(152.))
                        .h(px(14.))
                        .px(px(10.))
                        .rounded_b(px(6.))
                        .bg(black())
                        .child(div().size(px(5.)).rounded_full().bg(dark.accent))
                        .child(div().w(px(24.)).h(px(8.)).child(ink_waveform(PREVIEW_LEVELS.to_vec(), 0., dark.ink).size_full())),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_none()
                        .gap(px(5.))
                        .w(px(138.))
                        .pt(px(7.))
                        .pb(px(8.))
                        .px(px(10.))
                        .rounded_b(px(7.))
                        .bg(c.sheet)
                        .shadow(vec![
                            BoxShadow::new(px(0.), px(4.), c.shadow(0.3)).blur_radius(px(4.)).spread_radius(px(-4.)).inset(),
                            BoxShadow::new(px(0.), px(4.), c.shadow(0.14)).blur_radius(px(10.)),
                        ])
                        .child(bar(112., c.ink, 0.75))
                        .child(div().flex().gap(px(4.)).child(bar(58., c.ink, 0.75)).child(bar(26., c.pencil, 0.7))),
                ),
        )
}
