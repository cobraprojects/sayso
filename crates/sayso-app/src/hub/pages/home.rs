//! The home page. See the Paper board "Hub — Home".

use super::ext::StyleRoute;
use super::kit::{self, caps, fraunces, mono, ui};
use crate::hub::{Route, SettingsPage};
use crate::model::AppModel;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sayso_core::stats::format_count;
use sayso_i18n::t;
use sayso_core::stt::ModelStatus;
use sayso_platform::Permission;
use sayso_ui::components::*;
use sayso_ui::paper::PaperStyled;
use sayso_ui::{ActivePaper, Colors, text};

/// A fix action for a row in the "All set" list.
type Fix = Box<dyn Fn(&mut AppModel, &mut Context<AppModel>)>;

pub struct HomePage {
    model: Entity<AppModel>,
    _observe: Subscription,
}

impl HomePage {
    pub fn new(model: Entity<AppModel>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&model, |_, _, cx| cx.notify());
        Self { model, _observe: observe }
    }

    fn header(&self, narrow: bool, cx: &App) -> Div {
        let c = cx.paper().colors;
        let m = self.model.read(cx);
        let now = chrono::Local::now();
        let hour = chrono::Timelike::hour(&now);
        // The name from Settings › General, or the Mac account's first name.
        let name = m.config.general.name().map(str::to_string).or_else(kit::first_name);
        let greeting = match (name, hour) {
            (Some(name), 0..=11) => t!("home.greeting.morning_name", name = name),
            (Some(name), 12..=17) => t!("home.greeting.afternoon_name", name = name),
            (Some(name), _) => t!("home.greeting.evening_name", name = name),
            (None, 0..=11) => t!("home.greeting.morning"),
            (None, 12..=17) => t!("home.greeting.afternoon"),
            (None, _) => t!("home.greeting.evening"),
        };
        let hint = match &m.config.hotkeys.toggle {
            Some(hk) => div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ui(t!("home.hint.press"), 14., 18., FontWeight::NORMAL, c.graphite))
                .child(Keycaps::hotkey(hk))
                .child(ui(t!("home.hint.anywhere"), 14., 18., FontWeight::NORMAL, c.graphite)),
            None => {
                let model = self.model.clone();
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(ui(t!("home.hint.no_hotkey"), 14., 18., FontWeight::NORMAL, c.graphite))
                    .child(kit::link("set-hotkey", t!("home.hint.set_hotkey"), 14., cx).on_click(move |_, _, cx| {
                        model.update(cx, |m, cx| m.navigate(Route::Settings(SettingsPage::Dictation), cx))
                    }))
            }
        };
        let title = div()
            .flex()
            .flex_col()
            .min_w_0()
            .gap(px(10.))
            .child(caps(sayso_i18n::dates::weekday_day_month(&now), 13., c.graphite))
            .child(text::display(greeting, if narrow { 36. } else { 44. }, &c));
        // A narrow page has no room for the hint at the side of the greeting.
        if narrow {
            return div().flex().flex_col().flex_none().gap(px(14.)).child(title).child(hint);
        }
        div().flex().flex_none().items_end().justify_between().gap(px(24.)).child(title.flex_1()).child(hint.flex_none().pb(px(6.)))
    }

    fn stats(&self, narrow: bool, cx: &App) -> Div {
        let c = cx.paper().colors;
        let s = &self.model.read(cx).stats;
        let (big, small) = if narrow { (34., 28.) } else { (40., 32.) };
        let stat = |label: String, value: Option<String>, first: bool| {
            let v = match value {
                Some(v) => text::display(v, big, &c),
                None => text::display(t!("home.stats.none_yet"), small, &c).opacity(0.45),
            };
            div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(px(6.))
                .when(!first, |d| d.pl(px(24.)).border_l_1().border_color(c.rule))
                .child(ui(label, 13., 16., FontWeight::MEDIUM, c.graphite))
                .child(v)
        };
        let max = s.week_by_day.iter().copied().max().unwrap_or(0).max(1) as f32;
        let mut bars = div().flex().flex_none().items_end().gap(px(6.)).h(px(44.));
        for (i, words) in s.week_by_day.iter().enumerate() {
            let (h, color) = if i > s.today_index || *words == 0 {
                (6., c.rule)
            } else {
                let h = (6. + 34. * (*words as f32 / max)).round();
                (h, if i == s.today_index { c.ink } else { c.deboss_shade })
            };
            bars = bars.child(div().flex_none().w(px(8.)).h(px(h)).rounded(px(3.)).bg(color));
        }
        let mut week = div()
            .flex()
            .items_end()
            .justify_between()
            .pl(px(24.))
            .border_l_1()
            .border_color(c.rule)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(ui(t!("home.stats.this_week"), 13., 16., FontWeight::MEDIUM, c.graphite))
                    .child(text::display(format_count(s.words_this_week), big, &c)),
            )
            .child(bars);
        week.style().flex_grow = Some(1.3);
        week.style().flex_shrink = Some(1.);
        week.style().flex_basis = Some(relative(0.).into());
        let strip = div().flex().flex_none().py(px(24.)).border_t_1().border_b_1().border_color(c.rule);
        let words = stat(t!("home.stats.words_today"), Some(format_count(s.words_today)), true);
        let saved = stat(t!("home.stats.time_saved"), Some(t!("home.stats.minutes", minutes = s.minutes_saved_today)), false);
        let pace = s.pace_wpm.map(|p| t!("home.stats.wpm", wpm = p));
        // A narrow page: two rows of two.
        if narrow {
            week.style().flex_grow = Some(1.);
            return strip
                .flex_col()
                .gap(px(20.))
                .py(px(20.))
                .child(div().flex().child(words).child(saved))
                .child(div().flex().child(stat(t!("home.stats.pace"), pace, true)).child(week));
        }
        strip.child(words).child(saved).child(stat(t!("home.stats.pace"), pace, false)).child(week)
    }

    /// "Turn on history" and a link to the setting, for the two "History is off" states.
    fn history_off_actions(&self, cx: &App) -> (Button, Stateful<Div>) {
        let (on, open) = (self.model.clone(), self.model.clone());
        let turn_on = Button::new("turn-on-history", t!("home.history_off.turn_on"))
            .small()
            .on_click(move |_, _, cx| on.update(cx, |m, cx| m.edit_config(cx, |c| c.history.enabled = true)));
        let settings = kit::link("open-history-settings", t!("home.history_off.open_settings"), 14., cx)
            .on_click(move |_, _, cx| open.update(cx, |m, cx| m.navigate(Route::Settings(SettingsPage::HistoryPrivacy), cx)));
        (turn_on, settings)
    }

    /// Takes the place of the stats while history is off and entries from before remain.
    fn history_off_strip(&self, cx: &App) -> Div {
        let c = cx.paper().colors;
        let (turn_on, settings) = self.history_off_actions(cx);
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(32.))
            .py(px(24.))
            .border_t_1()
            .border_b_1()
            .border_color(c.rule)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(6.))
                    .child(text::title(t!("home.history_off.title"), 22., &c).line_height(px(28.)))
                    .child(ui(
                        t!("home.history_off.strip_detail"),
                        14.,
                        20.,
                        FontWeight::NORMAL,
                        c.graphite,
                    )),
            )
            .child(div().flex().flex_none().flex_wrap().items_center().gap(px(16.)).child(settings).child(turn_on))
    }

    /// Takes the place of the stats and the recent list while history is off and no entry remains.
    fn history_off_block(&self, cx: &App) -> Div {
        let c = cx.paper().colors;
        let (turn_on, settings) = self.history_off_actions(cx);
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(18.))
            .child(text::title(t!("home.history_off.title"), 22., &c).line_height(px(28.)))
            .child(
                text::body(
                    t!("home.history_off.block_detail"),
                    &c,
                )
                .max_w(px(460.)),
            )
            .child(div().flex().items_center().gap(px(16.)).child(turn_on).child(settings))
    }

    fn recent(&self, cx: &App) -> Div {
        let c = cx.paper().colors;
        let m = self.model.read(cx);
        if !m.config.history.enabled && m.recent.is_empty() {
            return self.history_off_block(cx);
        }
        let open = {
            let model = self.model.clone();
            kit::link("open-history", t!("home.recent.open_history"), 14., cx)
                .on_click(move |_, _, cx| model.update(cx, |m, cx| m.navigate(Route::History, cx)))
        };
        let mut col = div().flex().flex_col().flex_1().min_w_0().gap(px(6.)).child(
            div()
                .flex()
                .items_end()
                .justify_between()
                .pb(px(8.))
                .child(text::title(t!("home.recent.title"), 22., &c).line_height(px(28.)))
                .when(!m.recent.is_empty(), |d| d.child(open)),
        );
        if m.recent.is_empty() {
            let hk = m.config.hotkeys.toggle.as_ref().map(|h| h.keycaps().join(" ")).unwrap_or_else(|| t!("home.recent.your_hotkey"));
            return col.child(
                div().rounded(px(14.)).well(&c).child(kit::empty_state(
                    sayso_ui::assets::Icon::Mic,
                    &t!("home.recent.empty_title"),
                    &t!("home.recent.empty_body", hotkey = hk),
                    None,
                    cx,
                )),
            );
        }
        for e in m.recent.iter().take(6) {
            let style = m.styles.get(&e.style_id);
            let dot = style.map(|s| kit::ink_color(&s.ink, &c));
            let name = m.style_name(&e.style_id);
            let app = e.app.as_ref().map(|a| a.name.clone()).unwrap_or_else(|| t!("home.recent.unknown_app"));
            let app_icon = e.app.as_ref().and_then(|a| m.icons.get(&a.bundle_id));
            let model = self.model.clone();
            let id = e.id;
            col = col.child(
                div()
                    .id(("recent", e.id as u64))
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(14.))
                    .h(px(56.))
                    .px(px(4.))
                    .border_b_1()
                    .border_color(c.rule)
                    .cursor_pointer()
                    .hover(|s| s.bg(c.deboss.opacity(0.35)))
                    .on_click(move |_, _, cx| {
                        kit::select_history_entry(id, cx);
                        model.update(cx, |m, cx| m.navigate(Route::History, cx));
                    })
                    .child(mono(kit::clock_or_day(e.created_at), 12., c.graphite).w(px(44.)).flex_none())
                    .child(AppBadge::new(app).icon(app_icon).site(m.site_icon(e)))
                    .child(fraunces(e.final_text.replace('\n', " "), 15., 18., c.ink).flex_1().min_w_0().truncate())
                    .child(StyleTag::new(name, dot).width(84.))
                    .child(mono(kit::duration(e.duration_ms), 12., c.graphite).w(px(36.)).flex_none().text_right()),
            );
        }
        col
    }

    fn active_style(&self, cx: &App) -> Div {
        let c = cx.paper().colors;
        let m = self.model.read(cx);
        let style = m.active_style();
        let route = m.style_route(&style);
        let detail = match &route {
            StyleRoute::NoAi => t!("home.style.detail_no_ai", computer = crate::shell::computer()),
            StyleRoute::NeedsProvider => t!("home.style.detail_needs_provider"),
            StyleRoute::NeedsModel { provider } => t!("home.style.detail_needs_model", provider = provider),
            StyleRoute::Local { provider, model } => t!("home.style.detail_local", computer = crate::shell::computer(), provider = provider, model = model),
            StyleRoute::Cloud { provider, model } => t!("home.style.detail_cloud", provider = provider, model = model),
        };
        let description = if style.description.is_empty() { detail } else { format!("{} {}", style.description, detail) };
        let badge = match &route {
            StyleRoute::Cloud { .. } => Some(kit::place_badge(true, cx)),
            StyleRoute::Local { .. } | StyleRoute::NoAi => Some(kit::place_badge(false, cx)),
            StyleRoute::NeedsProvider | StyleRoute::NeedsModel { .. } => None,
        };
        let model = self.model.clone();
        let change = Button::new("change-style", t!("home.style.change"))
            .small()
            .on_click(move |_, _, cx| model.update(cx, |m, cx| m.navigate(Route::Styles, cx)));
        let cycle = m.config.hotkeys.cycle_style.as_ref().map(|hk| {
            div().flex().items_center().h(px(30.)).px(px(4.)).child(
                div()
                    .flex()
                    .items_center()
                    .h(px(22.))
                    .px(px(6.))
                    .rounded(px(5.))
                    .keycap(&c)
                    .child(mono(hk.compact_label(), 11., c.ink).line_height(px(14.))),
            )
        });
        let under = c.sheet.blend(c.deboss.opacity(0.5));
        div()
            .relative()
            .flex()
            .flex_col()
            .pt(px(10.))
            .child(div().absolute().top_0().left(px(14.)).right(px(14.)).h(px(40.)).rounded(px(12.)).bg(c.deboss).shadow(vec![
                BoxShadow::new(px(0.), px(1.), c.shadow(0.10)).blur_radius(px(2.)),
            ]))
            .child(div().absolute().top(px(5.)).left(px(7.)).right(px(7.)).h(px(40.)).rounded(px(12.)).bg(under).shadow(vec![
                BoxShadow::new(px(0.), px(1.), c.shadow(0.10)).blur_radius(px(2.)),
            ]))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .py(px(18.))
                    .px(px(20.))
                    .rounded(px(12.))
                    .bg(c.sheet_raised)
                    .shadow(vec![
                        BoxShadow::new(px(0.), px(1.), c.highlight(1.0)).inset(),
                        BoxShadow::new(px(0.), px(1.), c.shadow(0.14)).blur_radius(px(2.)),
                        BoxShadow::new(px(0.), px(10.), c.shadow(0.12)).blur_radius(px(26.)),
                    ])
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .h(px(22.))
                            .child(caps(t!("home.style.default"), 12., c.graphite))
                            .when_some(badge, |d, b| d.child(b)),
                    )
                    .child(text::display(style.name.clone(), 28., &c))
                    .child(ui(description, 14., 20., FontWeight::NORMAL, c.graphite))
                    .child(div().flex().gap(px(6.)).pt(px(4.)).child(change).when_some(cycle, |d, k| d.child(k))),
            )
    }

    fn checklist(&self, cx: &App) -> Div {
        let c = cx.paper().colors;
        let m = self.model.read(cx);
        let mic_ok = m.granted(Permission::Microphone);
        let ax_ok = m.granted(Permission::Accessibility);
        let active = m.active_model();
        let status = m.status_of(&active.id);
        let model_ok = m.model_ready();
        let all_ok = mic_ok && ax_ok && model_ok;

        let fix = |id: &'static str, label: String, f: Fix| {
            let model = self.model.clone();
            kit::link(id, label, 13., cx).on_click(move |_, _, cx| model.update(cx, |m, cx| f(m, cx)))
        };
        let row = |ok: bool, label: String, value: AnyElement| {
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(kit::check_dot(ok, cx))
                .child(ui(label, 14., 18., FontWeight::NORMAL, c.ink).flex_1())
                .child(value)
        };
        let plain = |s: String| ui(s, 13., 16., FontWeight::NORMAL, c.graphite).truncate().max_w(px(170.)).into_any_element();

        let mic = if mic_ok {
            plain(m.microphone_name().unwrap_or_else(|| t!("home.checklist.allowed")))
        } else {
            fix("fix-mic", t!("home.checklist.allow"), Box::new(|m, _| m.fix_permission(Permission::Microphone))).into_any_element()
        };
        let ax = if ax_ok {
            plain(t!("home.checklist.allowed"))
        } else {
            fix("fix-ax", t!("home.checklist.allow"), Box::new(|m, _| m.fix_permission(Permission::Accessibility))).into_any_element()
        };
        let model_value = match &status {
            ModelStatus::NotDownloaded if active.is_remote() => {
                fix("fix-model", t!("home.checklist.open_models"), Box::new(|m, cx| m.navigate(Route::Models, cx))).into_any_element()
            }
            ModelStatus::Ready | ModelStatus::Downloaded => plain(active.name.clone()),
            ModelStatus::Optimizing => plain(crate::shell::optimizing()),
            ModelStatus::Downloading { fraction, .. } => plain(t!("home.checklist.downloading", percent = format!("{:.0}", fraction * 100.))),
            ModelStatus::NotDownloaded => {
                fix("fix-model", t!("home.checklist.download"), Box::new(|m, cx| m.navigate(Route::Models, cx))).into_any_element()
            }
            ModelStatus::Failed { .. } => fix("fix-model", t!("home.checklist.open_models"), Box::new(|m, cx| m.navigate(Route::Models, cx))).into_any_element(),
        };
        let words = m.words.len();
        let rules = m.replacements.len();
        let dict = if words + rules == 0 {
            fix("fix-dict", t!("home.checklist.add_words"), Box::new(|m, cx| m.navigate(Route::Dictionary, cx))).into_any_element()
        } else {
            plain(t!("home.checklist.dictionary_summary", words = t!("home.checklist.words", count = words), rules = t!("home.checklist.rules", count = rules)))
        };
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .px(px(4.))
            .child(caps(if all_ok { t!("home.checklist.all_set") } else { t!("home.checklist.needs_attention") }, 12., c.graphite))
            .child(row(mic_ok, t!("home.checklist.microphone"), mic))
            .when(crate::shell::HAS_INPUT_PERMISSIONS, |d| d.child(row(ax_ok, crate::shell::permission_name(Permission::Accessibility), ax)))
            .child(row(model_ok, t!("home.checklist.model"), model_value))
            .child(row(true, t!("home.checklist.dictionary"), dict));
        // A cloud model does not need the engine for its final pass.
        if !active.is_remote() && (m.services.engine.is_none() || m.engine_down.is_some()) {
            list = list.child(row(false, t!("home.checklist.engine"), plain(t!("home.checklist.engine_stopped"))));
        }
        list
    }
}

impl Render for HomePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c: Colors = cx.paper().colors;
        let m = self.model.read(cx);
        let pad = crate::layout::page_pad(window, &self.model.read(cx).config);
        let narrow = crate::layout::page_narrow(window, &self.model.read(cx).config);
        // Stats come from history entries. While history is off, a notice takes their place.
        let top = if m.config.history.enabled {
            Some(self.stats(narrow, cx))
        } else {
            (!m.recent.is_empty()).then(|| self.history_off_strip(cx))
        };
        div()
            .id("home")
            .absolute()
            .inset_0()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .gap(px(if narrow { 28. } else { 40. }))
                    .py(px(pad.min(44.)))
                    .px(px(pad))
                    .text_color(c.ink)
                    .child(self.header(narrow, cx))
                    .children(top)
                    .child(if narrow {
                        // A narrow page: the side column goes under the list, as a row.
                        div().flex().flex_col().w_full().gap(px(32.)).child(self.recent(cx)).child(
                            div()
                                .flex()
                                .items_start()
                                .gap(px(28.))
                                .child(self.active_style(cx).flex_1().min_w_0())
                                .child(self.checklist(cx).flex_1().min_w_0()),
                        )
                    } else {
                        div().flex().w_full().gap(px(40.)).child(self.recent(cx)).child(
                            div().flex().flex_col().flex_none().w(px(300.)).gap(px(28.)).child(self.active_style(cx)).child(self.checklist(cx)),
                        )
                    }),
            )
    }
}
