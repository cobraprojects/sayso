//! The styles page. See the Paper board "Hub — Styles".
//!
//! A grid of style cards (click to make one active, Edit to change it), the
//! style editor panel, and a line that names the default provider. The
//! providers are in Settings.

use super::ext::StyleRoute;
use super::kit::{self, ui};
use crate::model::AppModel;
use crate::model_picker::{self, Extra};
use gpui_kit::component::input::{InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sayso_core::ink::NamedInk;
use sayso_core::style::{Style, StyleOrigin};
use sayso_core::style_rules::{self, TypedTarget};
use sayso_platform::AppInfo;
use sayso_ui::assets::Icon;
use sayso_ui::components::*;
use sayso_ui::paper::PaperStyled;
use sayso_ui::{ActivePaper, Colors, text};

struct Editor {
    id: String,
    is_new: bool,
    origin: Option<StyleOrigin>,
    base: Style,
    name: Entity<InputState>,
    description: Entity<InputState>,
    prompt: Entity<TextareaState>,
    ink: String,
    provider: Option<String>,
    provider_menu: bool,
    /// The style's model. None uses the provider's model.
    model: Option<String>,
    timeout: Entity<InputState>,
    /// False for a standalone style.
    use_base: bool,
    error: Option<String>,
    confirm_delete: bool,
    /// The apps and sites that use this style (see `style_rules`).
    apps: Vec<String>,
    sites: Vec<String>,
    /// The text of the "add" field.
    target: Entity<InputState>,
    /// The "add" field shows all apps, also with no text.
    target_open: bool,
    _target_sub: Subscription,
}

/// A row of the list under the "add" field.
#[derive(Clone)]
enum Choice {
    /// An app on this computer.
    App(AppInfo),
    /// A typed app id that no app on this computer has.
    AppId(String),
    Site(String),
}

/// The most rows that the list under the "add" field draws.
const MAX_CHOICES: usize = 40;

/// The editor of the base prompt.
struct BaseEditor {
    prompt: Entity<TextareaState>,
    error: Option<String>,
}

pub struct StylesPage {
    model: Entity<AppModel>,
    hovered: Option<String>,
    editor: Option<Editor>,
    base_editor: Option<BaseEditor>,
    /// The apps on this computer, for the names and the picker of the style rules.
    apps: Vec<AppInfo>,
    _subs: Vec<Subscription>,
}

impl StylesPage {
    pub fn new(model: Entity<AppModel>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&model, |_, _, cx| cx.notify());
        // The list of apps reads many folders, so it loads off the main thread.
        let context = model.read(cx).services.platform.context.clone();
        cx.spawn(async move |this, cx| {
            let mut apps = cx.background_spawn(async move { context.installed_apps() }).await;
            apps.retain(|a| a.bundle_id != "dev.sayso.Sayso" && !a.bundle_id.is_empty() && !a.name.is_empty());
            let _ = this.update(cx, |this, cx| {
                this.apps = apps;
                cx.notify();
            });
        })
        .detach();
        Self {
            model,
            hovered: None,
            editor: None,
            base_editor: None,
            apps: Vec::new(),
            _subs: vec![observe],
        }
    }

    // -----------------------------------------------------------------------
    // Editor
    // -----------------------------------------------------------------------

    fn open_editor(&mut self, style: Option<Style>, window: &mut Window, cx: &mut Context<Self>) {
        let is_new = style.is_none();
        let base = style.unwrap_or_else(|| Style {
            id: String::new(),
            name: String::new(),
            ink: format!("#{:06X}", NamedInk::Indigo.color().0),
            description: String::new(),
            prompt: String::new(),
            provider: None,
            model: None,
            temperature: None,
            timeout_ms: None,
            standalone: false,
        });
        let origin = if is_new { None } else { self.model.read(cx).style_origin(&base.id) };
        let http_s = self.model.read(cx).config.ai.http_timeout_ms as f64 / 1000.0;
        let b = base.clone();
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("For example, Slack").default_value(b.name.clone()));
        let description = cx.new(|cx| InputState::new(window, cx).placeholder("One line about what the style does").default_value(b.description.clone()));
        let prompt = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Tell the AI how to rewrite the text. Leave empty for no AI.")
                .default_value(b.prompt.clone())
        });
        let timeout = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(format!("Default, {} s", trim_float(http_s)))
                .default_value(b.timeout_ms.map(|ms| trim_float(ms as f64 / 1000.0)).unwrap_or_default())
        });
        name.update(cx, |s, cx| s.focus(window, cx));
        let (apps, sites, sites_ok) = {
            let m = self.model.read(cx);
            let rule = m.config.ai.style_rules.rule(&base.id).cloned().unwrap_or_default();
            (rule.apps, rule.sites, m.services.platform.context.reads_page_url())
        };
        let target = cx.new(|cx| {
            InputState::new(window, cx).placeholder(match (sites_ok, cfg!(target_os = "macos")) {
                (true, true) => "Add an app, a site, or a bundle ID",
                (true, false) => "Add an app, a site, or an app ID",
                (false, true) => "Add an app or a bundle ID",
                (false, false) => "Add an app or an app ID",
            })
        });
        let _target_sub = cx.subscribe_in(&target, window, |this, _, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. })
                && let Some(first) = this.target_choices(cx).into_iter().next()
            {
                this.add_target(first, window, cx);
            }
            cx.notify();
        });
        let style_model = base.model.clone();
        let use_base = !base.standalone;
        self.editor = Some(Editor {
            id: base.id.clone(),
            is_new,
            origin,
            ink: base.ink.clone(),
            provider: base.provider.clone(),
            base,
            name,
            description,
            prompt,
            provider_menu: false,
            model: style_model,
            timeout,
            use_base,
            error: None,
            confirm_delete: false,
            apps,
            sites,
            target,
            target_open: false,
            _target_sub,
        });
        cx.notify();
    }

    /// The name of an app for the user: its name on this computer, else the last part of its id.
    fn app_name(&self, id: &str, cx: &App) -> String {
        match self.apps.iter().find(|a| a.bundle_id.eq_ignore_ascii_case(id)) {
            Some(app) => app.name.clone(),
            None => self.model.read(cx).app_name_for(id, &[]),
        }
    }

    /// The rows under the "add" field: the apps that fit the text, then what
    /// the text itself can be. With no text, all apps when the list is open.
    fn target_choices(&self, cx: &App) -> Vec<Choice> {
        let Some(ed) = self.editor.as_ref() else { return Vec::new() };
        let text = ed.target.read(cx).value().trim().to_string();
        let query = text.to_lowercase();
        if query.is_empty() && !ed.target_open {
            return Vec::new();
        }
        let added = |id: &str| ed.apps.iter().any(|a| a.eq_ignore_ascii_case(id));
        let mut apps: Vec<AppInfo> = self
            .apps
            .iter()
            .filter(|a| !added(&a.bundle_id))
            .filter(|a| a.name.to_lowercase().contains(&query) || a.bundle_id.to_lowercase().contains(&query))
            .cloned()
            .collect();
        // An app whose name fits comes before an app where only the id fits.
        apps.sort_by_key(|a| !a.name.to_lowercase().contains(&query));
        let mut choices: Vec<Choice> = apps.into_iter().map(Choice::App).collect();
        let sites_ok = self.model.read(cx).services.platform.context.reads_page_url();
        // The text as the user typed it: an app id and a path can have capital letters.
        for typed in style_rules::typed_targets(&text) {
            match typed {
                // A bundle id has a dot. A text with no dot is the start of a name.
                TypedTarget::App(id) if cfg!(target_os = "macos") && !id.contains('.') => {}
                TypedTarget::App(id) => {
                    if !added(&id) && !self.apps.iter().any(|a| a.bundle_id.eq_ignore_ascii_case(&id)) {
                        choices.push(Choice::AppId(id));
                    }
                }
                TypedTarget::Site(site) if sites_ok && !ed.sites.contains(&site) => choices.push(Choice::Site(site)),
                TypedTarget::Site(_) => {}
            }
        }
        choices
    }

    fn add_target(&mut self, choice: Choice, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ed) = self.editor.as_mut() else { return };
        match choice {
            Choice::App(app) => ed.apps.push(app.bundle_id),
            Choice::AppId(id) => ed.apps.push(id),
            Choice::Site(site) => ed.sites.push(site),
        }
        ed.target_open = false;
        ed.target.update(cx, |s, cx| s.set_value("", window, cx));
        cx.notify();
    }

    fn save_editor(&mut self, cx: &mut Context<Self>) {
        let Some(ed) = self.editor.as_mut() else { return };
        let name = ed.name.read(cx).value().trim().to_string();
        if name.is_empty() {
            ed.error = Some("Enter a name for the style.".into());
            cx.notify();
            return;
        }
        let timeout = ed.timeout.read(cx).value().trim().to_string();
        let timeout_ms = if timeout.is_empty() {
            None
        } else {
            match timeout.trim_end_matches('s').trim().parse::<f64>() {
                Ok(s) if s > 0.0 && s <= 120.0 => Some((s * 1000.0).round() as u64),
                _ => {
                    ed.error = Some("Enter the timeout in seconds, from 0.5 to 120. For example, 4.".into());
                    cx.notify();
                    return;
                }
            }
        };
        let id = if ed.is_new { self.model.read(cx).new_style_id(&name) } else { ed.id.clone() };
        let style = Style {
            id,
            name,
            ink: ed.ink.clone(),
            description: ed.description.read(cx).value().trim().to_string(),
            prompt: ed.prompt.read(cx).value().trim().to_string(),
            provider: ed.provider.clone(),
            model: ed.model.clone(),
            timeout_ms,
            standalone: !ed.use_base,
            ..ed.base.clone()
        };
        let (apps, sites) = (ed.apps.clone(), ed.sites.clone());
        let saved = self.model.update(cx, |m, cx| {
            m.save_style(&style, cx)?;
            // The rules are in the config file, not in the style file.
            let rule = m.config.ai.style_rules.rule(&style.id);
            if rule.map(|r| (&r.apps, &r.sites)) != Some((&apps, &sites)) && (rule.is_some() || !apps.is_empty() || !sites.is_empty()) {
                m.edit_config(cx, |c| c.ai.style_rules.set_targets(&style.id, apps, sites));
            }
            Ok::<_, String>(())
        });
        match saved {
            Ok(()) => self.editor = None,
            Err(e) => {
                if let Some(ed) = self.editor.as_mut() {
                    ed.error = Some(format!("Sayso could not save the style: {e}"));
                }
            }
        }
        cx.notify();
    }

    fn editor_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = cx.paper().colors;
        let ed = self.editor.as_ref()?;
        let m = self.model.read(cx);
        let providers: Vec<(String, String)> = m.config.ai.providers.iter().map(|p| (p.id.clone(), p.name.clone())).collect();
        let default_name = m.config.ai.provider(None).map(|p| p.name.clone());
        let styles_dir = m.styles_dir_display();
        // The provider the style runs on, for its model list.
        let target = m.config.ai.provider(ed.provider.as_deref()).cloned();
        let model_label = match (&ed.model, &target) {
            (Some(id), Some(p)) => m.model_label(&p.id, id),
            (Some(id), None) => id.clone(),
            (None, Some(p)) => match p.model() {
                Some(id) => format!("Provider's model ({})", m.model_label(&p.id, id)),
                None => "Provider's model".into(),
            },
            (None, None) => "Provider's model".into(),
        };

        // Ink swatches.
        let mut swatches = div().flex().items_center().gap(px(10.));
        for ink in NamedInk::ALL {
            let hex = format!("#{:06X}", ink.color().0);
            let selected = ed.ink.eq_ignore_ascii_case(&hex);
            let h2 = hex.clone();
            swatches = swatches.child(
                div()
                    .id(SharedString::from(format!("ink-{}", ink.name())))
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(30.))
                    .rounded_full()
                    .cursor_pointer()
                    .border_2()
                    .border_color(if selected { c.ink } else { transparent_black() })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(ed) = this.editor.as_mut() {
                            ed.ink = h2.clone();
                        }
                        cx.notify();
                    }))
                    .child(kit::seal_dot(Some(&hex), 22., cx)),
            );
        }
        let ink_name = NamedInk::ALL
            .iter()
            .find(|n| ed.ink.eq_ignore_ascii_case(&format!("#{:06X}", n.color().0)))
            .map(|n| n.name().to_string())
            .unwrap_or_else(|| format!("Custom {}", ed.ink));
        swatches = swatches.child(ui(ink_name, 13., 16., FontWeight::MEDIUM, c.graphite).pl(px(4.)));

        // Provider select.
        let current = match &ed.provider {
            Some(id) => providers.iter().find(|p| &p.0 == id).map(|p| p.1.clone()).unwrap_or_else(|| format!("{id} (not set up)")),
            None => match &default_name {
                Some(n) => format!("Default provider ({n})"),
                None => "Default provider".into(),
            },
        };
        let mut select = div().relative().child(
            div()
                .id("provider-select")
                .flex()
                .items_center()
                .justify_between()
                .h(px(36.))
                .px(px(12.))
                .rounded(px(9.))
                .raised_small(&c)
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(ed) = this.editor.as_mut() {
                        ed.provider_menu = !ed.provider_menu;
                    }
                    cx.notify();
                }))
                .child(ui(current, 14., 18., FontWeight::NORMAL, c.ink))
                .child(icon(Icon::ChevronDown, 12., c.graphite)),
        );
        if ed.provider_menu {
            let mut items: Vec<(SharedString, bool)> = vec![("Default provider".into(), ed.provider.is_none())];
            for (id, name) in &providers {
                items.push((name.clone().into(), ed.provider.as_deref() == Some(id)));
            }
            let ids: Vec<String> = providers.iter().map(|p| p.0.clone()).collect();
            let e1 = cx.entity();
            let e2 = cx.entity();
            select = select.child(div().absolute().top_full().left_0().child(kit::menu(
                "provider-menu",
                items,
                move |i, _, cx| {
                    let pick = if i == 0 { None } else { ids.get(i - 1).cloned() };
                    e1.update(cx, |this, cx| {
                        if let Some(ed) = this.editor.as_mut() {
                            // A model from one provider means nothing to another.
                            if ed.provider != pick {
                                ed.model = None;
                            }
                            ed.provider = pick;
                            ed.provider_menu = false;
                        }
                        cx.notify();
                    });
                },
                move |_, cx| {
                    e2.update(cx, |this, cx| {
                        if let Some(ed) = this.editor.as_mut() {
                            ed.provider_menu = false;
                        }
                        cx.notify();
                    })
                },
                cx,
            )));
        }
        let provider_field = if providers.is_empty() {
            ui("No providers yet. Add one in Settings to use AI.", 13., 18., FontWeight::NORMAL, c.graphite).into_any_element()
        } else {
            select.into_any_element()
        };

        let model_field = match target {
            None => ui("Add a provider to choose a model.", 13., 18., FontWeight::NORMAL, c.graphite).into_any_element(),
            Some(p) => {
                let pid = p.id.clone();
                div()
                    .id("style-model")
                    .relative()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(36.))
                    .px(px(12.))
                    .rounded(px(9.))
                    .raised_small(&c)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let selected = this.editor.as_ref().and_then(|e| e.model.clone());
                        let page = cx.entity().downgrade();
                        let model = this.model.clone();
                        let refresh_id = pid.clone();
                        this.model.update(cx, |m, cx| m.load_provider_models(&pid, false, cx));
                        let request = model_picker::Request {
                            key: "style-model".into(),
                            list_key: pid.clone(),
                            extra: Some(Extra { label: "Provider's model".into(), selected: selected.is_none() }),
                            selected,
                            on_pick: Box::new(move |pick, cx| {
                                let _ = page.update(cx, |this, cx| {
                                    if let Some(ed) = this.editor.as_mut() {
                                        ed.model = pick;
                                    }
                                    cx.notify();
                                });
                            }),
                            on_refresh: Box::new(move |cx| model.update(cx, |m, cx| m.load_provider_models(&refresh_id, true, cx))),
                        };
                        model_picker::open(&this.model, request, cx);
                    }))
                    .child(ui(model_label, 14., 18., FontWeight::NORMAL, c.ink).truncate())
                    .child(icon(Icon::ChevronDown, 12., c.graphite))
                    .child(model_picker::anchor("style-model"))
                    .into_any_element()
            }
        };

        let prompt = div()
            .p(px(4.))
            .rounded(px(10.))
            .debossed(&c)
            .text_size(px(14.))
            .child(Textarea::new(&ed.prompt).appearance(false).h(px(150.)).text_size(px(14.)));
        let targets = self.targets_group(ed, cx);

        let title = if ed.is_new { "New style".to_string() } else { format!("Edit {}", ed.base.name) };
        let file_note = if ed.is_new {
            format!("Saved as a file in {}.", styles_dir)
        } else {
            match ed.origin {
                Some(StyleOrigin::BuiltIn) => "A built-in style. Saving writes your copy to the styles folder.".into(),
                Some(StyleOrigin::Overridden) => "You changed this built-in style. Reset brings back the original.".into(),
                _ => format!("{}/{}.toml", styles_dir, ed.id),
            }
        };

        let mut buttons = div().flex().items_center().gap(px(8.)).child(
            Button::new("save-style", "Save").primary().on_click(cx.listener(|this, _, _, cx| this.save_editor(cx))),
        );
        buttons = buttons.child(Button::new("cancel-style", "Cancel").ghost().on_click(cx.listener(|this, _, _, cx| {
            this.editor = None;
            cx.notify();
        })));
        buttons = buttons.child(div().flex_1());
        if ed.origin == Some(StyleOrigin::Overridden) {
            let id = ed.id.clone();
            buttons = buttons.child(Button::new("reset-style", "Reset to built-in").icon(Icon::Undo).on_click(cx.listener(
                move |this, _, _, cx| {
                    let id = id.clone();
                    this.model.update(cx, |m, cx| m.reset_style(&id, cx));
                    this.editor = None;
                    cx.notify();
                },
            )));
        }
        if ed.origin == Some(StyleOrigin::User) {
            let id = ed.id.clone();
            if ed.confirm_delete {
                buttons = buttons.child(Button::new("delete-style-yes", "Delete style").danger().on_click(cx.listener(move |this, _, _, cx| {
                    let id = id.clone();
                    this.model.update(cx, |m, cx| m.delete_style(&id, cx));
                    this.editor = None;
                    cx.notify();
                })));
            } else {
                buttons = buttons.child(Button::new("delete-style", "Delete").danger().icon(Icon::Trash).on_click(cx.listener(|this, _, _, cx| {
                    if let Some(ed) = this.editor.as_mut() {
                        ed.confirm_delete = true;
                    }
                    cx.notify();
                })));
            }
        }

        let body = div()
            .flex()
            .flex_col()
            .gap(px(18.))
            .p(px(28.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .child(div().flex().flex_col().gap(px(6.)).child(text::title(title, 28., &c)).child(ui(
                        file_note,
                        13.,
                        18.,
                        FontWeight::NORMAL,
                        c.graphite,
                    )))
                    .child(kit::icon_button("close-editor", Icon::Close, 12., cx).on_click(cx.listener(|this, _, _, cx| {
                        this.editor = None;
                        cx.notify();
                    }))),
            )
            .child(kit::field("Name", kit::input_well(&ed.name, cx), None, cx))
            .child(kit::field("Description", kit::input_well(&ed.description, cx), None, cx))
            .child(kit::field("Ink", swatches, None, cx))
            .child(kit::field("Prompt", prompt, Some("Sayso adds your dictionary words and asks for JSON. You only write the style."), cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(ui("Use the base prompt", 13., 16., FontWeight::SEMIBOLD, c.ink))
                            .child(ui(
                                "The base prompt cleans up the text: filler words, corrections, numbers. Turn it off for a style that does something else, for example a translation.",
                                12.,
                                16.,
                                FontWeight::NORMAL,
                                c.graphite,
                            )),
                    )
                    .child(div().flex_none().child(Switch::new("use-base", ed.use_base).on_toggle({
                        let page = cx.entity();
                        move |on, _, cx| {
                            page.update(cx, |this, cx| {
                                if let Some(ed) = this.editor.as_mut() {
                                    ed.use_base = on;
                                }
                                cx.notify();
                            })
                        }
                    }))),
            )
            .child(kit::field("Provider", provider_field, None, cx))
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .child(kit::field("Model", model_field, None, cx).flex_1())
                    .child(kit::field("Timeout in seconds", kit::input_well(&ed.timeout, cx), None, cx).w(px(150.))),
            )
            .child(targets)
            .when_some(ed.error.clone(), |d, e| d.child(Banner::new(BannerKind::Warning, e)))
            .child(div().pt(px(6.)).child(buttons));

        Some(self.side_panel("style-editor", body, cx))
    }

    /// The line on a style card that names the apps and sites of the style.
    /// None when the style rules are off or the style has none.
    fn target_line(&self, style: &str, cx: &App) -> Option<String> {
        let rules = &self.model.read(cx).config.ai.style_rules;
        let rule = rules.rule(style).filter(|_| rules.enabled)?;
        // The apps on this computer first: the user knows their names.
        let (here, away): (Vec<&String>, Vec<&String>) =
            rule.apps.iter().partition(|id| self.apps.iter().any(|a| a.bundle_id.eq_ignore_ascii_case(id)));
        let names: Vec<String> = here.into_iter().chain(away).map(|id| self.app_name(id, cx)).chain(rule.sites.iter().cloned()).collect();
        Some(match names.as_slice() {
            [] => return None,
            [one] => one.clone(),
            [one, two] => format!("{one} and {two}"),
            [one, two, rest @ ..] => format!("{one}, {two}, and {} more", rest.len()),
        })
    }

    /// The "Use automatically" part of the style editor: the apps and sites
    /// of the style, and the field that adds one.
    fn targets_group(&self, ed: &Editor, cx: &Context<Self>) -> Div {
        let c = cx.paper().colors;
        let m = self.model.read(cx);
        let rules = &m.config.ai.style_rules;
        let sites_ok = m.services.platform.context.reads_page_url();
        // "Moves from Message": the app or site leaves that style on Save.
        let moves_from = |owner: Option<&str>| owner.filter(|id| *id != ed.id).map(|id| format!("Moves from {}", m.style_name(id)));
        let note = |s: String| ui(s, 12., 16., FontWeight::NORMAL, c.graphite).flex_none();

        let count = ed.apps.len() + ed.sites.len();
        let mut list = div().flex().flex_col().rounded(px(10.)).bg(c.sheet_raised).raised_small(&c);
        let row = |i: usize| div().flex().items_center().gap(px(10.)).h(px(40.)).px(px(10.)).when(i + 1 < count, |d| d.border_b_1().border_color(c.rule));
        for (i, id) in ed.apps.iter().enumerate() {
            let known = self.apps.iter().find(|a| a.bundle_id.eq_ignore_ascii_case(id));
            let name = self.app_name(id, cx);
            let remove = id.clone();
            list = list.child(
                row(i)
                    .child(AppBadge::new(name.clone()).icon(m.icons.get(id)).size(20.))
                    .when(known.is_some(), |d| d.child(ui(name, 14., 18., FontWeight::MEDIUM, c.ink).flex_none()))
                    .child(div().flex_1().min_w_0().child(text::mono(id.clone(), 11., if known.is_some() { c.graphite } else { c.ink }).truncate()))
                    // Before the list of apps loads, each app would get this note.
                    .when(known.is_none() && !self.apps.is_empty(), |d| d.child(note("Not on this computer".into())))
                    .when_some(moves_from(rules.style_of_app(id)), |d, s| d.child(note(s)))
                    .child(kit::icon_button(("remove-app", i), Icon::Close, 10., cx).on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(ed) = this.editor.as_mut() {
                            ed.apps.retain(|a| *a != remove);
                        }
                        cx.notify();
                    }))),
            );
        }
        for (i, site) in ed.sites.iter().enumerate() {
            let remove = site.clone();
            list = list.child(
                row(ed.apps.len() + i)
                    .child(div().flex().flex_none().items_center().justify_center().size(px(20.)).child(icon(Icon::Globe, 16., c.graphite)))
                    .child(div().flex_1().min_w_0().child(text::mono(site.clone(), 12., c.ink).truncate()))
                    .when_some(moves_from(rules.style_of_site(site)), |d, s| d.child(note(s)))
                    .child(kit::icon_button(("remove-site", i), Icon::Close, 10., cx).on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(ed) = this.editor.as_mut() {
                            ed.sites.retain(|s| *s != remove);
                        }
                        cx.notify();
                    }))),
            );
        }

        let open = kit::icon_button("target-open", Icon::ChevronDown, 12., cx).on_click(cx.listener(|this, _, _, cx| {
            if let Some(ed) = this.editor.as_mut() {
                ed.target_open = !ed.target_open;
            }
            cx.notify();
        }));
        let typed = !ed.target.read(cx).value().trim().is_empty();
        let choices = self.target_choices(cx);
        // An address is not the name of an app, so it gets no such line.
        let no_app = typed && !ed.target.read(cx).value().contains('/') && !choices.iter().any(|ch| matches!(ch, Choice::App(_)));
        let mut panel = div().id("target-choices").flex().flex_col().gap(px(2.)).p(px(6.)).max_h(px(236.)).overflow_y_scroll().rounded(px(10.)).bg(c.sheet_raised).raised_small(&c);
        if no_app {
            panel = panel.child(div().px(px(6.)).py(px(4.)).child(note("No app on this computer has this name or ID.".into())));
        }
        for (i, choice) in choices.into_iter().take(MAX_CHOICES).enumerate() {
            let line = div()
                .id(("target-choice", i))
                .flex()
                .flex_none()
                .items_center()
                .gap(px(10.))
                .h(px(36.))
                .px(px(8.))
                .rounded(px(8.))
                .cursor_pointer()
                // Return adds the first row.
                .when(i == 0 && typed, |d| d.bg(c.accent_wash))
                .hover(|s| s.bg(c.accent_wash));
            let slot = |ic: Icon| div().flex().flex_none().items_center().justify_center().size(px(20.)).child(icon(ic, 14., c.graphite));
            let label = |s: &str| ui(s.to_string(), 14., 18., FontWeight::MEDIUM, c.ink).flex_none();
            let id_text = |s: &str, color: Hsla| div().flex_1().min_w_0().child(text::mono(s.to_string(), 11., color).truncate());
            let line = match &choice {
                Choice::App(app) => line
                    .child(AppBadge::new(app.name.clone()).icon(m.icons.get(&app.bundle_id)).size(20.))
                    .child(label(&app.name))
                    .child(id_text(&app.bundle_id, c.graphite))
                    .when_some(moves_from(rules.style_of_app(&app.bundle_id)), |d, s| d.child(note(s))),
                Choice::AppId(id) => line
                    .child(slot(Icon::Plus))
                    .child(label(if cfg!(target_os = "macos") { "Add bundle ID" } else { "Add app ID" }))
                    .child(id_text(id, c.ink)),
                Choice::Site(site) => line
                    .child(slot(Icon::Globe))
                    .child(label("Add site"))
                    .child(id_text(site, c.ink))
                    .when_some(moves_from(rules.style_of_site(site)), |d, s| d.child(note(s))),
            };
            panel = panel.child(line.on_click(cx.listener(move |this, _, window, cx| this.add_target(choice.clone(), window, cx))));
        }

        let intro = if sites_ok {
            format!("Sayso uses {} when you dictate into one of these apps or sites.", if ed.base.name.is_empty() { "this style" } else { &ed.base.name })
        } else {
            format!("Sayso uses {} when you dictate into one of these apps.", if ed.base.name.is_empty() { "this style" } else { &ed.base.name })
        };
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .pt(px(18.))
            .border_t_1()
            .border_color(c.rule)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(text::title("Use automatically".to_string(), 18., &c).line_height(px(24.)))
                    .child(ui(intro, 13., 18., FontWeight::NORMAL, c.graphite)),
            )
            .when(!rules.enabled, |d| d.child(Banner::new(BannerKind::Info, "This is off. Turn it on in Settings › Dictation.")))
            .when(count > 0, |d| d.child(list))
            .child(kit::search_well(&ed.target, 36., Some(open.into_any_element()), cx))
            .when(typed || ed.target_open, |d| d.child(panel))
            .when(sites_ok, |d| {
                d.child(ui(
                    "A site also covers its subdomains. Add a path to limit it: facebook.com/messages.",
                    12.,
                    16.,
                    FontWeight::NORMAL,
                    c.graphite,
                ))
            })
    }

    /// A panel on the right of the page, over a scrim. A click on the scrim closes it.
    fn side_panel(&self, id: &'static str, body: Div, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.paper().colors;
        let panel = div()
            .id(id)
            .absolute()
            .top(px(12.))
            .right(px(12.))
            .bottom(px(12.))
            .w(px(500.))
            .rounded(px(14.))
            .floating(&c)
            .overflow_y_scroll()
            .occlude()
            .child(body);
        let scrim = div()
            .id("scrim")
            .absolute()
            .inset_0()
            .bg(c.ground.opacity(0.45))
            .occlude()
            .on_click(cx.listener(|this, _, _, cx| {
                this.editor = None;
                this.base_editor = None;
                cx.notify();
            }));
        div().absolute().inset_0().child(scrim).child(panel).into_any_element()
    }

    // -----------------------------------------------------------------------
    // Base prompt
    // -----------------------------------------------------------------------

    fn open_base_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.model.read(cx).styles.base_prompt().to_string();
        let prompt = cx.new(|cx| TextareaState::new(window, cx).placeholder("Rules that all styles share. Leave empty for no shared rules.").default_value(text));
        prompt.update(cx, |s, cx| s.focus(window, cx));
        self.base_editor = Some(BaseEditor { prompt, error: None });
        cx.notify();
    }

    fn save_base_editor(&mut self, cx: &mut Context<Self>) {
        let Some(ed) = self.base_editor.as_ref() else { return };
        let text = ed.prompt.read(cx).value().to_string();
        // The default text saved again is not a change.
        let result = if text.trim() == sayso_core::style::DEFAULT_BASE_PROMPT {
            self.model.update(cx, |m, cx| m.reset_base_prompt(cx));
            Ok(())
        } else {
            self.model.update(cx, |m, cx| m.save_base_prompt(&text, cx))
        };
        match result {
            Ok(()) => self.base_editor = None,
            Err(e) => {
                if let Some(ed) = self.base_editor.as_mut() {
                    ed.error = Some(format!("Sayso could not save the base prompt: {e}"));
                }
            }
        }
        cx.notify();
    }

    fn base_editor_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = cx.paper().colors;
        let ed = self.base_editor.as_ref()?;
        let m = self.model.read(cx);
        let changed = m.styles.user_base_prompt.is_some();
        let file_note = if changed {
            format!("You changed the base prompt. It is the file {}/{}.", m.styles_dir_display(), sayso_core::style::BASE_PROMPT_FILE)
        } else {
            "The default base prompt. Saving a change writes your copy to the styles folder.".to_string()
        };
        let prompt = div()
            .p(px(4.))
            .rounded(px(10.))
            .debossed(&c)
            .text_size(px(14.))
            .child(Textarea::new(&ed.prompt).appearance(false).h(px(380.)).text_size(px(14.)));
        let mut buttons = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(Button::new("save-base", "Save").primary().on_click(cx.listener(|this, _, _, cx| this.save_base_editor(cx))))
            .child(Button::new("cancel-base", "Cancel").ghost().on_click(cx.listener(|this, _, _, cx| {
                this.base_editor = None;
                cx.notify();
            })))
            .child(div().flex_1());
        if changed {
            buttons = buttons.child(Button::new("reset-base", "Reset to default").icon(Icon::Undo).on_click(cx.listener(|this, _, _, cx| {
                this.model.update(cx, |m, cx| m.reset_base_prompt(cx));
                this.base_editor = None;
                cx.notify();
            })));
        }
        let body = div()
            .flex()
            .flex_col()
            .gap(px(18.))
            .p(px(28.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(12.))
                    .child(div().flex().flex_col().flex_1().min_w_0().gap(px(6.)).child(text::title("Base prompt", 28., &c)).child(ui(
                        file_note,
                        13.,
                        18.,
                        FontWeight::NORMAL,
                        c.graphite,
                    )))
                    .child(kit::icon_button("close-base", Icon::Close, 12., cx).on_click(cx.listener(|this, _, _, cx| {
                        this.base_editor = None;
                        cx.notify();
                    }))),
            )
            .child(kit::field(
                "Rules for all styles",
                prompt,
                Some("Short rules and a few short examples work best with a small model. A style with \"Use the base prompt\" off does not get this text."),
                cx,
            ))
            .when_some(ed.error.clone(), |d, e| d.child(Banner::new(BannerKind::Warning, e)))
            .child(div().pt(px(6.)).child(buttons));
        Some(self.side_panel("base-editor", body, cx))
    }

    // -----------------------------------------------------------------------
    // Cards
    // -----------------------------------------------------------------------

    fn card(&self, style: &Style, active: bool, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.paper().colors;
        let route = self.model.read(cx).style_route(style);
        let line_color = match &route {
            StyleRoute::Cloud { .. } => c.accent,
            StyleRoute::Local { .. } => c.success,
            StyleRoute::NoAi => c.graphite,
            StyleRoute::NeedsProvider => c.pencil,
            StyleRoute::NeedsModel { .. } => c.danger,
        };
        let hovered = self.hovered.as_deref() == Some(style.id.as_str());
        let rules_on = self.model.read(cx).config.ai.style_rules.enabled;
        let targets = self.target_line(&style.id, cx);
        let id2 = style.id.clone();
        let id3 = style.id.clone();
        let st = style.clone();
        let edit = Button::new(SharedString::from(format!("edit-{}", style.id)), "Edit").small().ghost().on_click(cx.listener(
            move |this, _, w, cx| {
                cx.stop_propagation();
                this.open_editor(Some(st.clone()), w, cx);
            },
        ));
        let mut shadows = Vec::new();
        if active {
            shadows.push(BoxShadow::new(px(0.), px(0.), c.ink).spread_radius(px(2.)));
        }
        shadows.extend([
            BoxShadow::new(px(0.), px(1.), c.highlight(1.0)).inset(),
            BoxShadow::new(px(0.), px(1.), c.shadow(0.14)).blur_radius(px(2.)),
            BoxShadow::new(px(0.), px(8.), c.shadow(if active { 0.12 } else { 0.08 })).blur_radius(px(22.)),
        ]);
        div()
            .id(SharedString::from(format!("style-{}", style.id)))
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h(px(if rules_on { 200. } else { 178. }))
            .gap(px(10.))
            .py(px(18.))
            .px(px(20.))
            .rounded(px(14.))
            .bg(c.sheet_raised)
            .shadow(shadows)
            .cursor_pointer()
            .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                if *h {
                    this.hovered = Some(id2.clone());
                } else if this.hovered.as_deref() == Some(id2.as_str()) {
                    this.hovered = None;
                }
                cx.notify();
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                let id = id3.clone();
                this.model.update(cx, |m, cx| m.set_active_style(&id, cx));
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(22.))
                    .child(kit::seal_dot(style.uses_ai().then_some(style.ink.as_str()), 16., cx))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .when(hovered || active, |d| d.child(edit))
                            .when(active, |d| d.child(Badge::new("Default", BadgeTone::Ink))),
                    ),
            )
            .child(text::title(style.name.clone(), 24., &c).line_height(px(30.)))
            .child(ui(style.description.clone(), 14., 20., FontWeight::NORMAL, c.graphite).flex_1().line_clamp(2).text_ellipsis())
            .when_some(targets, |d, line| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(icon(Icon::ArrowRight, 11., c.graphite))
                        .child(ui(line, 12., 16., FontWeight::MEDIUM, c.ink).truncate()),
                )
            })
            .child(ui(route.line(), 12., 16., FontWeight::MEDIUM, line_color).truncate())
            .into_any_element()
    }

    /// The default provider in one line, with the way to the providers.
    fn provider_line(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.paper().colors;
        let m = self.model.read(cx);
        let default = m.config.ai.provider(None).filter(|_| m.config.ai.enabled);
        let (dot, line) = match default {
            Some(p) => {
                let model = match p.model() {
                    Some(id) => format!(" · {}", m.model_label(&p.id, id)),
                    None => String::new(),
                };
                (c.success, format!("Styles run on {}{model} unless a style names another provider.", p.name))
            }
            None if m.config.ai.providers.is_empty() => (c.pencil, "No AI provider is set up, so each style inserts the transcript.".to_string()),
            None => (c.pencil, "AI is off, so each style inserts the transcript.".to_string()),
        };
        let label = if m.config.ai.providers.is_empty() { "Set up a provider" } else { "Manage providers" };
        let model = self.model.clone();
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .pt(px(16.))
            .border_t_1()
            .border_color(c.rule)
            .child(div().flex_none().size(px(8.)).rounded_full().bg(dot))
            .child(ui(line, 13., 18., FontWeight::NORMAL, c.graphite).flex_1().min_w_0())
            .child(
                div()
                    .id("manage-providers")
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(4.))
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.75))
                    .on_click(move |_, _, cx| model.update(cx, |m, cx| m.navigate(crate::hub::Route::Settings(crate::hub::SettingsPage::Providers), cx)))
                    .child(ui(label, 13., 18., FontWeight::SEMIBOLD, c.accent))
                    .child(icon(Icon::ChevronRight, 12., c.accent)),
            )
            .into_any_element()
    }
}

fn trim_float(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

impl Render for StylesPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c: Colors = cx.paper().colors;
        let (styles, active, dir) = {
            let m = self.model.read(cx);
            (m.styles.styles().cloned().collect::<Vec<_>>(), m.active_style().id, m.styles_dir_display())
        };
        let actions = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(Button::new("edit-base", "Base prompt").large().icon(Icon::File).on_click(cx.listener(|this, _, w, cx| this.open_base_editor(w, cx))))
            .child(
                Button::new("new-style", "New style")
                    .primary()
                    .large()
                    .icon(Icon::Plus)
                    .on_click(cx.listener(|this, _, w, cx| this.open_editor(None, w, cx))),
            )
            .into_any_element();
        let head = kit::page_head(
            "Styles",
            Some(kit::intro(
                format!("A style rewrites what you said before it is inserted. Each style is a file in {dir}, so you can edit it by hand."),
                560.,
                cx,
            )),
            Some(actions),
            cx,
        );
        let pad = crate::layout::page_pad(window, &self.model.read(cx).config);
        // Three cards in a row, or two on a narrow page.
        let columns = if crate::layout::page_narrow(window, &self.model.read(cx).config) { 2 } else { 3 };
        let mut grid = div().flex().flex_col().gap(px(18.));
        for chunk in styles.chunks(columns) {
            let mut row = div().flex().gap(px(18.));
            for s in chunk {
                row = row.child(self.card(s, s.id == active, cx));
            }
            for _ in chunk.len()..columns {
                row = row.child(div().flex_1());
            }
            grid = grid.child(row);
        }
        let errors = self.model.read(cx).styles.errors.clone();
        let providers = self.provider_line(cx);
        let editor = self.editor_panel(cx).or_else(|| self.base_editor_panel(cx));
        div()
            .absolute()
            .inset_0()
            .text_color(c.ink)
            .child(
                div().id("styles").absolute().inset_0().overflow_y_scroll().child(
                    div()
                        .flex()
                        .flex_col()
                        .w_full()
                        .gap(px(28.))
                        .py(px(pad.min(44.)))
                        .px(px(pad))
                        .child(head)
                        .children(errors.into_iter().map(|e| Banner::new(BannerKind::Warning, format!("A style file has an error: {e}"))))
                        .child(grid)
                        .child(providers),
                ),
            )
            .children(editor)
    }
}
