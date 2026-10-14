//! The models page. See the Paper board "Hub — Models".
//!
//! The active model as a hero card with real numbers, then the local catalog
//! in groups with a speed meter, the size, and one action per row, then the
//! cloud providers with their models.

use super::kit::{self, caps, mono, ui};
use crate::model::AppModel;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sayso_core::config::is_local_url;
use sayso_core::models::{EngineKind, Family, ModelInfo, format_size};
use sayso_core::speech::{SpeechProvider, SpeechProviderKind};
use sayso_core::stt::ModelStatus;
use sayso_i18n::t;
use sayso_ui::assets::Icon;
use sayso_ui::components::*;
use sayso_ui::paper::PaperStyled;
use sayso_ui::{ActivePaper, Colors, text};
use std::collections::HashMap;

/// "632 MB", or "macOS" for Apple Speech, whose files macOS downloads and stores.
pub fn size_label(info: &ModelInfo) -> String {
    if info.size_bytes == 0 { "macOS".into() } else { format_size(info.size_bytes) }
}

/// The groups of the local catalog, in display order.
pub const LOCAL_GROUPS: [Family; 3] = [Family::Parakeet, Family::Whisper, Family::Other];

/// The title of a group, in the interface language.
pub fn group_title(family: Family) -> String {
    match family {
        Family::Parakeet => "Parakeet".into(),
        Family::Whisper => "Whisper".into(),
        Family::Other | Family::Cloud => t!("models.group_more"),
    }
}

/// The speed meter: five bars, `speed` of them in ink.
pub fn speed_meter(speed: u8, c: &Colors) -> Div {
    let mut meter = div().flex().flex_none().items_center().gap(px(3.));
    for i in 0..5u8 {
        meter = meter.child(div().w(px(14.)).h(px(6.)).rounded(px(2.)).bg(if i < speed { c.ink } else { c.deboss_shade }));
    }
    meter
}

/// The result of a provider check, shown on the provider's line.
#[derive(Clone)]
enum Test {
    Running,
    Passed(u64),
    Failed(String),
}

/// The "Add a cloud provider" form.
struct AddProvider {
    kind: SpeechProviderKind,
    /// Custom endpoint only.
    name: Entity<InputState>,
    url: Entity<InputState>,
    model_id: Entity<InputState>,
    key: Entity<InputState>,
    error: Option<AddError>,
}

/// Why the "Add a cloud provider" form cannot save. It keeps the reason, not
/// the sentence, so it changes with the language.
enum AddError {
    Name,
    Url,
    ModelId,
    ServerKey,
    Key(String),
}

impl AddError {
    fn text(&self) -> String {
        match self {
            AddError::Name => t!("models.add.error_name"),
            AddError::Url => t!("models.add.error_url"),
            AddError::ModelId => t!("models.add.error_model_id"),
            AddError::ServerKey => t!("models.add.error_server_key"),
            AddError::Key(provider) => t!("models.add.error_key", provider = provider),
        }
    }
}

/// The placeholders of the form. The inputs keep them, so `ModelsPage` sets
/// them again when the language changes.
fn key_placeholder() -> String {
    t!(crate::shell::os_text!("models.add.key_placeholder_mac", "models.add.key_placeholder_windows", "models.add.key_placeholder_other"))
}

pub struct ModelsPage {
    model: Entity<AppModel>,
    confirm_delete: Option<String>,
    add: Option<AddProvider>,
    tests: HashMap<String, Test>,
    confirm_remove: Option<String>,
    /// The page has no room for all columns (set at each render).
    narrow: bool,
    /// The locale of the text in the placeholders of the add form.
    locale: String,
    _observe: Subscription,
    _subs: Vec<Subscription>,
}

impl ModelsPage {
    pub fn new(model: Entity<AppModel>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&model, |_, _, cx| cx.notify());
        Self { model, confirm_delete: None, add: None, tests: HashMap::new(), confirm_remove: None, narrow: false, locale: sayso_i18n::locale(), _observe: observe, _subs: Vec::new() }
    }

    fn hero(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.paper().colors;
        let m = self.model.read(cx);
        let info = m.active_model();
        let status = m.status_of(&info.id);
        let engine = m.services.engine.is_some();
        let measured = m.measured_speed(&info.id);

        let remote = info.is_remote();
        let (dot, label) = match &status {
            ModelStatus::Ready => (c.success, t!("models.hero.in_use")),
            ModelStatus::NotDownloaded if remote => (c.danger, t!("models.hero.provider_not_set_up")),
            // The idle time unloaded the model. The next dictation loads it.
            ModelStatus::Downloaded if m.model_idle => (c.success, t!("models.hero.in_use_idle")),
            ModelStatus::Downloaded => (c.accent, t!("models.hero.loading")),
            ModelStatus::Optimizing => (c.accent, crate::shell::optimizing()),
            ModelStatus::Downloading { fraction, .. } => (c.accent, t!("models.downloading_percent", percent = format!("{:.0}", fraction * 100.))),
            ModelStatus::NotDownloaded => (c.danger, t!("models.hero.not_downloaded")),
            ModelStatus::Failed { .. } => (c.danger, t!("models.hero.model_error")),
        };

        let stat = |value: String, note: String| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(text::title(value, 28., &c).line_height(px(34.)))
                .child(ui(note, 12., 16., FontWeight::NORMAL, c.graphite))
        };
        let mut stats = div().flex().flex_none().items_end().gap(px(28.)).pb(px(4.));
        match measured {
            Some((ms, dur)) => stats = stats.child(stat(t!("models.hero.ms", ms = ms), t!("models.hero.for_speech", duration = kit::duration_words(dur)))),
            None => stats = stats.child(stat(format!("{}/5", info.speed), t!("models.hero.speed"))),
        }
        if let Some(wer) = info.wer_percent {
            stats = stats.child(stat(format!("{wer}%"), t!("models.hero.word_error_rate")));
        }
        let parts = match info.engine {
            EngineKind::ParakeetUnified { .. } => 3,
            _ => 1,
        };
        stats = stats.child(if remote {
            stat(t!("models.hero.cloud"), t!("models.hero.audio_goes_to", vendor = info.vendor))
        } else {
            stat(size_label(&info), if info.size_bytes == 0 {
                t!("models.hero.stores_files")
            } else if status.is_on_disk() {
                t!("models.hero.parts_on_disk", count = parts)
            } else {
                t!("models.hero.to_download")
            })
        });

        let id = info.id.clone();
        let action: Option<AnyElement> = match &status {
            ModelStatus::NotDownloaded if remote => Some(
                ui(t!("models.hero.add_provider_again"), 13., 18., FontWeight::NORMAL, c.danger)
                    .into_any_element(),
            ),
            ModelStatus::NotDownloaded => Some(
                Button::new("hero-download", if info.size_bytes == 0 { t!("models.download") } else { t!("models.download_size", size = format_size(info.size_bytes)) })
                    .primary()
                    .icon(Icon::Download)
                    .disabled(!engine)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let id = id.clone();
                        this.model.update(cx, |m, cx| m.download_model(id, cx));
                    }))
                    .into_any_element(),
            ),
            ModelStatus::Failed { message } => Some(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(ui(format!("{message}."), 13., 18., FontWeight::NORMAL, c.danger))
                    .child(Button::new("hero-retry", t!("models.try_again")).small().on_click(cx.listener(move |this, _, _, cx| {
                        let id = id.clone();
                        this.model.update(cx, |m, cx| m.download_model(id, cx));
                    })))
                    .into_any_element(),
            ),
            ModelStatus::Downloading { fraction, bytes_done, bytes_total } => Some(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .w(px(360.))
                    .child(Progress::new(*fraction))
                    .when(*bytes_total > 0, |d| {
                        d.child(mono(t!("models.hero.bytes_of", done = format_size(*bytes_done), total = format_size(*bytes_total)), 11., c.graphite))
                    })
                    .into_any_element(),
            ),
            ModelStatus::Optimizing => Some(
                ui(
                    t!(crate::shell::os_text!("models.hero.optimizing_detail_mac", "models.hero.optimizing_detail_windows", "models.hero.optimizing_detail_other")),
                    13.,
                    18.,
                    FontWeight::NORMAL,
                    c.graphite,
                )
                    .into_any_element(),
            ),
            _ => None,
        };

        div()
            .flex()
            .flex_none()
            .gap(px(36.))
            // A narrow page: the numbers go under the name.
            .when(self.narrow, |d| d.flex_col().gap(px(20.)))
            .py(px(24.))
            .px(px(28.))
            .rounded(px(16.))
            .bg(c.sheet_raised)
            .shadow(vec![
                BoxShadow::new(px(0.), px(1.), c.highlight(1.0)).inset(),
                BoxShadow::new(px(0.), px(1.), c.shadow(0.14)).blur_radius(px(2.)),
                BoxShadow::new(px(0.), px(12.), c.shadow(0.10)).blur_radius(px(28.)),
            ])
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(10.))
                    .child(div().flex().items_center().gap(px(8.)).child(status_dot(dot, 8.)).child(caps(label, 12., c.graphite)))
                    .child(text::title(info.name.clone(), 32., &c).line_height(px(35.)))
                    .child(ui(format!("{} · {}", info.vendor, info.description), 14., 20., FontWeight::NORMAL, c.graphite))
                    .when_some(action, |d, a| d.child(div().flex().pt(px(6.)).child(a))),
            )
            .child(stats)
    }

    fn row(&self, info: &ModelInfo, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.paper().colors;
        let m = self.model.read(cx);
        let status = m.status_of(&info.id);
        let engine = m.services.engine.is_some();
        let is_preview = m.preview_model().as_ref() == Some(&info.id);
        let remote = info.is_remote();
        // A custom endpoint on this Mac is not cloud.
        let place = match sayso_core::speech::split_model_id(&info.id).and_then(|(p, _)| m.config.speech.provider(p)) {
            Some(p) if !p.is_cloud() => t!("models.row.server"),
            _ => t!("models.row.cloud"),
        };
        let engine_name = info.engine.runtime();
        let langs = info.language_label();
        let desc = info.description.trim_end_matches('.').to_string();
        let speed = speed_meter(info.speed, &c).w(px(120.));
        let id = info.id.clone();
        let key = info.id.as_str().to_string();
        let confirm = self.confirm_delete.as_deref() == Some(key.as_str());

        let delete = {
            let id = id.clone();
            let key = key.clone();
            if confirm {
                Button::new(SharedString::from(format!("del-yes-{key}")), t!("models.row.delete")).small().danger().on_click(cx.listener(
                    move |this, _, _, cx| {
                        let id = id.clone();
                        this.confirm_delete = None;
                        this.model.update(cx, |m, cx| m.delete_model(id, cx));
                    },
                ))
            } else {
                Button::new(SharedString::from(format!("del-{key}")), t!("models.row.delete")).small().ghost().on_click(cx.listener(move |this, _, _, cx| {
                    this.confirm_delete = Some(key.clone());
                    cx.notify();
                }))
            }
        };

        let action: AnyElement = match &status {
            ModelStatus::Downloading { fraction, .. } => div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .w_full()
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .child(ui(t!("models.row.downloading"), 12., 16., FontWeight::SEMIBOLD, c.ink))
                        .child(mono(format!("{:.0}%", fraction * 100.), 11., c.graphite)),
                )
                .child(Progress::new(*fraction))
                .into_any_element(),
            ModelStatus::NotDownloaded => Button::new(SharedString::from(format!("dl-{key}")), t!("models.download"))
                .disabled(!engine)
                .on_click(cx.listener(move |this, _, _, cx| {
                    let id = id.clone();
                    this.model.update(cx, |m, cx| m.download_model(id, cx));
                }))
                .into_any_element(),
            ModelStatus::Failed { message } => div()
                .flex()
                .flex_col()
                .items_end()
                .gap(px(4.))
                .child(Button::new(SharedString::from(format!("retry-{key}")), t!("models.try_again")).small().on_click(cx.listener(
                    move |this, _, _, cx| {
                        let id = id.clone();
                        this.model.update(cx, |m, cx| m.download_model(id, cx));
                    },
                )))
                .child(ui(message.clone(), 11., 14., FontWeight::NORMAL, c.danger).truncate().max_w(px(150.)))
                .into_any_element(),
            ModelStatus::Optimizing => ui(t!("models.row.optimizing"), 12., 16., FontWeight::SEMIBOLD, c.graphite).into_any_element(),
            _ if remote => Button::new(SharedString::from(format!("use-{key}")), t!("models.row.use"))
                .small()
                .on_click(cx.listener(move |this, _, _, cx| {
                    let id = id.clone();
                    this.model.update(cx, |m, cx| m.set_active_model(id, cx));
                }))
                .into_any_element(),
            _ if !info.final_pass => div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(ui(if is_preview { t!("models.row.live_preview") } else { t!("models.row.preview_only") }, 12., 16., FontWeight::MEDIUM, c.graphite))
                .child(delete)
                .into_any_element(),
            _ => div()
                .flex()
                .items_center()
                .gap(px(4.))
                .when(!confirm, |d| {
                    d.child(Button::new(SharedString::from(format!("use-{key}")), t!("models.row.use")).small().on_click(cx.listener(move |this, _, _, cx| {
                        let id = id.clone();
                        this.model.update(cx, |m, cx| m.set_active_model(id, cx));
                    })))
                })
                // macOS owns the Apple Speech files, so there is nothing to delete.
                .when(info.engine != EngineKind::AppleSpeech, |d| d.child(delete))
                .into_any_element(),
        };

        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(16.))
            .h(px(64.))
            .border_t_1()
            .border_color(c.rule)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(text::title(info.name.clone(), 17., &c).line_height(px(22.)))
                    .child(
                        ui(
                            if remote { format!("{langs} · {desc}") } else { format!("{} · {engine_name} · {langs} · {desc}", info.vendor) },
                            13.,
                            16.,
                            FontWeight::NORMAL,
                            c.graphite,
                        )
                        .truncate(),
                    ),
            )
            // A narrow page has no speed column.
            .when(!self.narrow, |d| d.child(speed))
            .child(mono(if remote { place } else { size_label(info) }, 12., c.graphite).w(px(80.)).flex_none())
            .child(div().flex().flex_none().justify_end().w(px(150.)).child(action))
    }

    /// One group of the local catalog: a caption row, then the models.
    fn group(&self, title: String, models: &[ModelInfo], cx: &mut Context<Self>) -> Div {
        let c = cx.paper().colors;
        let mut table = div().flex().flex_col().child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(16.))
                .h(px(36.))
                .child(caps(title, 12., c.graphite).flex_1())
                .when(!self.narrow, |d| d.child(caps(t!("models.group.speed"), 12., c.graphite).w(px(120.)).flex_none()))
                .child(caps(t!("models.group.size"), 12., c.graphite).w(px(80.)).flex_none())
                .child(div().w(px(150.)).flex_none()),
        );
        for info in models {
            table = table.child(self.row(info, cx));
        }
        table.child(div().h(px(1.)).bg(c.rule))
    }

    // -----------------------------------------------------------------------
    // Cloud providers
    // -----------------------------------------------------------------------

    fn test(&mut self, id: String, cx: &mut Context<Self>) {
        self.tests.insert(id.clone(), Test::Running);
        let page = cx.entity().downgrade();
        self.model.update(cx, |m, cx| {
            let key = id.clone();
            m.test_speech_provider(&id, cx, move |_, result, cx| {
                let _ = page.update(cx, |this, cx| {
                    this.tests.insert(key, match result {
                        Ok(ms) => Test::Passed(ms),
                        Err(e) => Test::Failed(e),
                    });
                    cx.notify();
                });
            });
        });
        cx.notify();
    }

    /// A provider's line: name, where the audio goes, the last check, and its actions.
    fn provider_head(&self, p: &SpeechProvider, cx: &mut Context<Self>) -> Div {
        let c = cx.paper().colors;
        let cloud = p.is_cloud();
        let host = p.base_url().map(|u| u.split("://").nth(1).unwrap_or(&u).to_string()).unwrap_or_default();
        let (line, line_color) = match self.tests.get(&p.id) {
            Some(Test::Running) => (t!("models.provider.checking"), c.graphite),
            Some(Test::Passed(ms)) => (t!("models.provider.connected", ms = ms), c.success),
            Some(Test::Failed(e)) => (e.clone(), c.danger),
            None if cloud => (t!("models.provider.sends_audio", host = host), c.graphite),
            None => (t!("models.provider.own_server", host = host), c.graphite),
        };
        let confirm = self.confirm_remove.as_deref() == Some(p.id.as_str());
        let (tid, rid, cid) = (p.id.clone(), p.id.clone(), p.id.clone());
        let running = matches!(self.tests.get(&p.id), Some(Test::Running));
        let actions = if confirm {
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(4.))
                .child(ui(t!("models.provider.confirm_remove"), 12., 16., FontWeight::NORMAL, c.graphite))
                .child(Button::new(SharedString::from(format!("sp-rm-yes-{}", p.id)), t!("models.provider.remove")).small().danger().on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.confirm_remove = None;
                        this.tests.remove(&rid);
                        let id = rid.clone();
                        this.model.update(cx, |m, cx| m.remove_speech_provider(&id, cx));
                    },
                )))
                .child(Button::new(SharedString::from(format!("sp-rm-no-{}", p.id)), t!("models.cancel")).small().ghost().on_click(cx.listener(
                    |this, _, _, cx| {
                        this.confirm_remove = None;
                        cx.notify();
                    },
                )))
        } else {
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(4.))
                .child(Button::new(SharedString::from(format!("sp-test-{}", p.id)), t!("models.provider.test")).small().disabled(running).on_click(cx.listener(
                    move |this, _, _, cx| this.test(tid.clone(), cx),
                )))
                .child(Button::new(SharedString::from(format!("sp-rm-{}", p.id)), t!("models.provider.remove")).small().ghost().on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.confirm_remove = Some(cid.clone());
                        cx.notify();
                    },
                )))
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(12.))
            .pt(px(18.))
            .pb(px(10.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(3.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(text::title(p.name.clone(), 20., &c).line_height(px(24.)))
                            .child(kit::place_badge(cloud, cx)),
                    )
                    .child(ui(line, 13., 16., FontWeight::NORMAL, line_color).truncate()),
            )
            .child(actions)
    }

    fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.add.is_some() {
            self.add = None;
            cx.notify();
            return;
        }
        let name = cx.new(|cx| InputState::new(window, cx).placeholder(t!("models.add.name_placeholder")));
        let url = cx.new(|cx| InputState::new(window, cx).placeholder("http://localhost:8000/v1"));
        let model_id = cx.new(|cx| InputState::new(window, cx).placeholder(t!("models.add.model_placeholder")));
        let key = cx.new(|cx| InputState::new(window, cx).placeholder(key_placeholder()).masked(true));
        self._subs.push(cx.subscribe_in(&url, window, |_, _, _: &InputEvent, _, cx| cx.notify()));
        key.update(cx, |s, cx| s.focus(window, cx));
        self.add = Some(AddProvider { kind: SpeechProviderKind::OpenAi, name, url, model_id, key, error: None });
        cx.notify();
    }

    fn save_add(&mut self, cx: &mut Context<Self>) {
        let Some(add) = self.add.as_mut() else { return };
        let key = add.key.read(cx).value().trim().to_string();
        let custom = add.kind == SpeechProviderKind::OpenAiCompatible;
        let (name, base_url, models) = if custom {
            let name = add.name.read(cx).value().trim().to_string();
            let url = add.url.read(cx).value().trim().trim_end_matches('/').to_string();
            let model = add.model_id.read(cx).value().trim().to_string();
            add.error = if name.is_empty() {
                Some(AddError::Name)
            } else if !(url.starts_with("http://") || url.starts_with("https://")) {
                Some(AddError::Url)
            } else if model.is_empty() {
                Some(AddError::ModelId)
            } else if key.is_empty() && !is_local_url(&url) {
                Some(AddError::ServerKey)
            } else {
                None
            };
            (name, Some(url), vec![model])
        } else {
            add.error = key.is_empty().then(|| AddError::Key(add.kind.label().to_string()));
            (add.kind.label().to_string(), None, Vec::new())
        };
        if add.error.is_some() {
            cx.notify();
            return;
        }
        let kind = add.kind;
        let id = self.model.read(cx).new_speech_provider_id(&name);
        let key = (!key.is_empty()).then_some(key);
        let provider = SpeechProvider { api_key_account: key.as_ref().map(|_| format!("speech.{id}")), id: id.clone(), name, kind, base_url, models };
        self.add = None;
        self.model.update(cx, |m, cx| m.save_speech_provider(provider, key, cx));
        self.test(id, cx);
    }

    fn add_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = cx.paper().colors;
        let add = self.add.as_ref()?;
        let mut kinds = div().flex().flex_wrap().gap(px(6.));
        for (i, kind) in SpeechProviderKind::ALL.into_iter().enumerate() {
            kinds = kinds.child(Chip::new(("speech-kind", i), kind.label(), add.kind == kind).on_click(cx.listener(move |this, _, _, cx| {
                if let Some(a) = this.add.as_mut() {
                    a.kind = kind;
                    a.error = None;
                }
                cx.notify();
            })));
        }
        let custom = add.kind == SpeechProviderKind::OpenAiCompatible;
        let url = add.url.read(cx).value().to_string();
        let cloud = !custom || (!url.trim().is_empty() && !is_local_url(&url));
        let mut body = div().flex().flex_col().gap(px(16.)).child(kit::field(&t!("models.add.provider"), kinds, None, cx));
        if custom {
            body = body
                .child(
                    div()
                        .flex()
                        .gap(px(12.))
                        .child(kit::field(&t!("models.add.name"), kit::input_well(&add.name, cx), None, cx).w(px(220.)))
                        .child(
                            kit::field(
                                &t!("models.add.base_url"),
                                kit::input_well(&add.url, cx),
                                Some(&t!("models.add.base_url_hint")),
                                cx,
                            )
                            .flex_1(),
                        ),
                )
                .child(kit::field(&t!("models.add.model"), kit::input_well(&add.model_id, cx), None, cx))
                .child(kit::field(&t!("models.add.api_key"), kit::input_well(&add.key, cx), Some(&t!("models.add.api_key_hint", computer = crate::shell::computer())), cx));
        } else {
            let names: Vec<&str> = add.kind.known_models().iter().map(|m| m.name).collect();
            let key_link = add.kind.key_url().map(|url| {
                kit::link("speech-key-link", t!("models.add.get_key", provider = add.kind.label()), 12., cx)
                    .on_click(move |_, _, _| AppModel::open_url(url))
                    .into_any_element()
            });
            body = body
                .child(kit::field(&t!("models.add.api_key"), kit::input_well(&add.key, cx), None, cx).when_some(key_link, |d, l| d.child(div().flex().child(l))))
                .child(ui(t!("models.add.adds", names = names.join(", ")), 13., 18., FontWeight::NORMAL, c.graphite));
        }
        if cloud {
            body = body.child(Banner::new(
                BannerKind::Info,
                t!("models.add.cloud_notice", computer = crate::shell::computer()),
            ));
        }
        let panel = div()
            .flex()
            .flex_col()
            .gap(px(18.))
            .mt(px(14.))
            .p(px(22.))
            .rounded(px(14.))
            .raised(&c)
            .child(text::title(t!("models.add.title"), 22., &c))
            .child(body)
            .when_some(add.error.as_ref(), |d, e| d.child(Banner::new(BannerKind::Warning, e.text())))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(Button::new("save-speech-provider", t!("models.add.save")).primary().on_click(cx.listener(|this, _, _, cx| this.save_add(cx))))
                    .child(Button::new("cancel-speech-provider", t!("models.cancel")).ghost().on_click(cx.listener(|this, _, _, cx| {
                        this.add = None;
                        cx.notify();
                    }))),
            );
        Some(panel.into_any_element())
    }

    /// The cloud section: every provider with its models, then the add form.
    fn cloud(&mut self, active: &sayso_core::models::ModelId, cx: &mut Context<Self>) -> Div {
        let c = cx.paper().colors;
        let providers = self.model.read(cx).config.speech.providers.clone();
        let note = ui(t!("models.cloud.note", computer = crate::shell::computer()), 13., 16., FontWeight::NORMAL, c.graphite).into_any_element();
        let mut col = div().flex().flex_col().child(kit::section_head(&t!("models.cloud.title"), Some(note), cx));
        if providers.is_empty() && self.add.is_none() {
            col = col.child(
                ui(
                    t!("models.cloud.empty"),
                    14.,
                    20.,
                    FontWeight::NORMAL,
                    c.graphite,
                )
                .max_w(px(620.))
                .pt(px(14.)),
            );
        }
        for p in &providers {
            col = col.child(self.provider_head(p, cx));
            let models: Vec<ModelInfo> = p.catalog().into_iter().filter(|m| &m.id != active).collect();
            for info in &models {
                col = col.child(self.row(info, cx));
            }
            col = col.child(div().h(px(1.)).bg(c.rule));
        }
        match self.add_panel(cx) {
            Some(panel) => col.child(panel),
            None => col.child(
                div().flex().pt(px(16.)).child(
                    kit::dashed("add-speech-provider", cx)
                        .h(px(36.))
                        .px(px(14.))
                        .rounded(px(9.))
                        .child(icon(Icon::Plus, 12., c.graphite))
                        .child(ui(t!("models.add.title"), 13., 16., FontWeight::MEDIUM, c.graphite))
                        .on_click(cx.listener(|this, _, window, cx| this.open_add(window, cx))),
                ),
            ),
        }
    }
}

impl Render for ModelsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c: Colors = cx.paper().colors;
        // The inputs of the add form keep their placeholders, so they change with the language.
        if self.locale != sayso_i18n::locale() {
            self.locale = sayso_i18n::locale();
            if let Some(add) = &self.add {
                add.name.update(cx, |s, cx| s.set_placeholder(t!("models.add.name_placeholder"), window, cx));
                add.model_id.update(cx, |s, cx| s.set_placeholder(t!("models.add.model_placeholder"), window, cx));
                add.key.update(cx, |s, cx| s.set_placeholder(key_placeholder(), window, cx));
            }
        }
        let pad = crate::layout::page_pad(window, &self.model.read(cx).config);
        self.narrow = crate::layout::page_narrow(window, &self.model.read(cx).config);
        let (catalog, active, used, dir, engine_down) = {
            let m = self.model.read(cx);
            (m.catalog(), m.active_model().id, m.models_disk_bytes(), m.models_dir_display(), m.services.engine.is_none())
        };
        let model = self.model.clone();
        let used_label = if used == 0 { t!("models.page.nothing_downloaded") } else { t!("models.page.used", size = format_size(used)) };
        let disk = div()
            .id("models-dir")
            .cursor_pointer()
            .pb(px(4.))
            .on_click(move |_, _, cx| AppModel::open_path(&model.read(cx).paths.models_dir()))
            .child(ui(format!("{used_label} · {dir}"), 13., 16., FontWeight::NORMAL, c.graphite))
            .into_any_element();
        let head = kit::page_head(
            &t!("models.page.title"),
            Some(kit::intro(
                t!(crate::shell::os_text!("models.page.intro_mac", "models.page.intro_windows", "models.page.intro_other")),
                600.,
                cx,
            )),
            Some(disk),
            cx,
        );
        let local = |family: Family| -> Vec<ModelInfo> {
            catalog.iter().filter(|i| i.id != active && !i.is_remote() && i.engine.family() == family).cloned().collect()
        };
        let mut tables = div().flex().flex_col().gap(px(20.));
        for family in LOCAL_GROUPS {
            let models = local(family);
            if !models.is_empty() {
                tables = tables.child(self.group(group_title(family), &models, cx));
            }
        }
        let cloud = self.cloud(&active, cx);
        let hero = self.hero(cx);
        div().id("models").absolute().inset_0().overflow_y_scroll().text_color(c.ink).child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .gap(px(28.))
                .py(px(pad.min(44.)))
                .px(px(pad))
                .child(head)
                .when(engine_down, |d| {
                    d.child(Banner::new(
                        BannerKind::Warning,
                        t!("models.page.engine_down"),
                    ))
                })
                .child(hero)
                .child(div().pt(px(8.)).child(tables))
                .child(div().pt(px(8.)).child(cloud)),
        )
    }
}
