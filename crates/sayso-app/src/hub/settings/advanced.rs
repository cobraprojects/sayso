//! Settings › Advanced: engine path, model memory, log level, folders.

use super::kit::{self, group, row};
use crate::model::AppModel;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::*;
use sayso_i18n::t;
use sayso_ui::ActivePaper;
use sayso_ui::components::*;
use sayso_ui::paper::PaperStyled;

const LEVELS: [&str; 5] = ["error", "warn", "info", "debug", "trace"];
/// `dictation.keep_model_minutes`. None is "Always".
const KEEP_MINUTES: [Option<u32>; 4] = [None, Some(5), Some(15), Some(60)];

pub struct AdvancedSettings {
    model: Entity<AppModel>,
    engine: Entity<InputState>,
    engine_saved: bool,
    _engine_sub: Subscription,
}

impl AdvancedSettings {
    pub fn new(model: Entity<AppModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let current = model.read(cx).config.advanced.engine_path.clone().unwrap_or_default();
        let placeholder = sayso_core::paths::Paths::display(
            &sayso_engine_client::EngineClient::default_engine_path(),
            &sayso_core::paths::PathEnv::home(&sayso_core::paths::SystemEnv),
        );
        let engine = crate::widgets::input_state(&placeholder, &current, window, cx);
        let sub = cx.subscribe_in(&engine, window, |this, state, ev: &InputEvent, _, cx| {
            if !matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                return;
            }
            let value = state.read(cx).value().trim().to_string();
            let v = (!value.is_empty()).then_some(value);
            let before = this.model.read(cx).config.advanced.engine_path.clone();
            if before != v {
                this.model.update(cx, |m, cx| m.edit_config(cx, |c| c.advanced.engine_path = v));
                this.engine_saved = true;
                cx.notify();
            }
        });
        Self { model, engine, engine_saved: false, _engine_sub: sub }
    }
}

impl Render for AdvancedSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.paper().colors;
        let m = self.model.read(cx);
        let adv = m.config.advanced.clone();
        let paths = m.paths.clone();
        let engine_missing = adv.engine_path.as_ref().is_some_and(|p| !std::path::Path::new(p).exists());

        let level = adv.log_level.as_deref().unwrap_or("info");
        let level_index = LEVELS.iter().position(|l| *l == level).unwrap_or(2);
        let m1 = self.model.clone();
        let levels = Segmented::new("log-level", ["Error", "Warn", "Info", "Debug", "Trace"], level_index)
            .label(t!("settings.advanced.log_level"))
            .on_select(move |i, _, cx| {
            let v = if i == 2 { None } else { Some(LEVELS[i].to_string()) };
            m1.update(cx, |m, cx| m.edit_config(cx, |c| c.advanced.log_level = v));
        });

        let keep_index = KEEP_MINUTES.iter().position(|k| *k == m.config.dictation.keep_model_minutes).unwrap_or(0);
        let m2 = self.model.clone();
        let keep = Segmented::new("keep-model", [
            t!("settings.advanced.keep_always"),
            t!("settings.advanced.keep_minutes", count = 5),
            t!("settings.advanced.keep_minutes", count = 15),
            t!("settings.advanced.keep_hour"),
        ], keep_index)
        .label(t!("settings.advanced.keep"))
        .on_select(move |i, _, cx| {
            m2.update(cx, |m, cx| m.edit_config(cx, |c| c.dictation.keep_model_minutes = KEEP_MINUTES[i]));
        });
        // One model for the final pass and the live preview: the preview waits for the load too.
        // The Linux engine keeps the live preview of such a model in memory.
        let keep_desc = if !cfg!(target_os = "linux") && m.preview_model().as_ref() == Some(&m.config.dictation.model) {
            t!("settings.advanced.keep_detail_preview")
        } else {
            t!("settings.advanced.keep_detail")
        };

        let m3 = self.model.clone();
        let preload = Switch::new("preload-model", m.config.dictation.preload_model)
            .label(t!("settings.advanced.preload"))
            .on_toggle(move |on, _, cx| m3.update(cx, |m, cx| m.edit_config(cx, |c| c.dictation.preload_model = on)));

        let field = div()
            .flex()
            .items_center()
            .w(px(420.))
            .h(px(36.))
            .px(px(12.))
            .rounded(px(9.))
            .debossed(&c)
            .font_family(sayso_ui::fonts::MONO)
            .text_size(px(12.))
            .child(Input::new(&self.engine).aria_label(t!("settings.advanced.engine_path")).appearance(false).w_full());

        let mut engine = group(&t!("settings.advanced.engine"), cx).child(row(
            &t!("settings.advanced.engine_path"),
            &t!("settings.advanced.engine_path_detail"),
            field,
            cx,
        ));
        if engine_missing {
            engine = engine.child(kit::banner(BannerKind::Warning, t!("settings.advanced.engine_missing"), cx));
        } else if self.engine_saved {
            engine = engine.child(kit::banner(BannerKind::Info, t!("settings.advanced.engine_saved"), cx));
        }
        engine = engine.child(row(
            &t!("settings.advanced.preload"),
            &t!("settings.advanced.preload_detail"),
            preload,
            cx,
        ));
        engine = engine.child(kit::row_s(t!("settings.advanced.keep"), keep_desc, keep, cx));
        engine = engine.child(row(&t!("settings.advanced.log_level"), &t!("settings.advanced.log_level_detail"), levels, cx));

        let log_dir = paths.log_dir();
        let folders = group(&t!("settings.advanced.folders"), cx)
            .child(kit::path_row("reveal-config", &t!("settings.advanced.folder_config"), &paths.config_dir, cx))
            .child(kit::path_row("reveal-data", &t!("settings.advanced.folder_data"), &paths.data_dir, cx))
            .child(kit::path_row("reveal-cache", &t!("settings.advanced.folder_cache"), &paths.cache_dir, cx))
            .child(row(
                &t!("settings.advanced.logs"),
                &t!("settings.advanced.logs_detail"),
                Button::new("open-logs", t!("settings.advanced.open_logs")).small().on_click(move |_, _, _| {
                    let _ = std::fs::create_dir_all(&log_dir);
                    AppModel::open_path(&log_dir);
                }),
                cx,
            ));

        kit::page(
            "advanced-page",
            &t!("settings.advanced.title"),
            &t!("settings.advanced.subtitle"),
            kit::body().child(engine).child(folders),
            cx,
        )
    }
}
