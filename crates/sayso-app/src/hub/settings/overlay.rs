//! Settings › Overlay: size, the idle pill, full screen, the display, live preview, pill position.

use super::kit::{self, group, row};
use crate::model::AppModel;
use gpui_kit::*;
use sayso_core::config::OverlaySize;
use sayso_i18n::t;
use sayso_ui::components::*;

pub struct OverlaySettings {
    model: Entity<AppModel>,
    reset_note: Option<Result<(), String>>,
}

impl OverlaySettings {
    pub fn new(model: Entity<AppModel>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { model, reset_note: None }
    }
}

impl Render for OverlaySettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let o = self.model.read(cx).config.overlay.clone();
        let (m1, m2, m3, m4, m5) = (self.model.clone(), self.model.clone(), self.model.clone(), self.model.clone(), self.model.clone());
        let size_index = OverlaySize::ALL.iter().position(|s| *s == o.size).unwrap_or(2);
        let size = Segmented::new("overlay-size", [t!("settings.overlay.size_small"), t!("settings.overlay.size_medium"), t!("settings.overlay.size_large")], size_index).on_select(move |i, _, cx| {
            let size = OverlaySize::ALL[i];
            m4.update(cx, |m, cx| m.edit_config(cx, |c| c.overlay.size = size));
        });
        let pill = Switch::new("idle-pill", o.idle_pill).on_toggle(move |on, _, cx| {
            m1.update(cx, |m, cx| m.edit_config(cx, |c| c.overlay.idle_pill = on));
        });
        let fullscreen = Switch::new("hide-fullscreen", o.hide_in_fullscreen).on_toggle(move |on, _, cx| {
            m2.update(cx, |m, cx| m.edit_config(cx, |c| c.overlay.hide_in_fullscreen = on));
        });
        let follow = Switch::new("follow-display", o.follow_display).on_toggle(move |on, _, cx| {
            m5.update(cx, |m, cx| m.edit_config(cx, |c| c.overlay.follow_display = on));
        });
        let follow_help = if crate::shell::can_place_windows() {
            t!("settings.overlay.follow_detail_placed")
        } else {
            t!("settings.overlay.follow_detail")
        };
        let preview = Switch::new("show-preview", o.show_preview).on_toggle(move |on, _, cx| {
            m3.update(cx, |m, cx| m.edit_config(cx, |c| c.overlay.show_preview = on));
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
            Some(Ok(())) => {
                g = g.child(kit::banner(BannerKind::Info, t!("settings.overlay.reset_done"), cx))
            }
            Some(Err(e)) => {
                g = g.child(kit::banner(BannerKind::Warning, t!("settings.overlay.reset_failed", error = e), cx))
            }
            None => {}
        }

        let rec = group(&t!("settings.overlay.while_dictating"), cx).child(row(
            &t!("settings.overlay.live_preview"),
            &t!("settings.overlay.live_preview_detail"),
            preview,
            cx,
        ));

        kit::page("overlay-page", &t!("settings.overlay.title"), &t!("settings.overlay.subtitle"), kit::body().child(g).child(rec), cx)
    }
}
