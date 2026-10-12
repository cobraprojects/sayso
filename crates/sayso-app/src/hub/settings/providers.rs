//! Settings › Providers. See the Paper page "Providers".
//!
//! One row for each AI provider. A row opens in place and shows the login,
//! the folder of the login, the model, and the whole text of a problem. The
//! sign-in of a CLI provider runs here: the tool does the sign-in, and Sayso
//! shows the page and takes the code.

use super::kit::{self, group};
use crate::hub::pages::ext::provider_detail;
use crate::hub::pages::kit::{self as pkit, ui};
use crate::model::{AppModel, ProviderLogin};
use crate::model_picker;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sayso_core::config::{Provider, ProviderKind, is_local_url};
use sayso_enhance::AuthStatus;
use sayso_ui::assets::Icon;
use sayso_ui::components::*;
use sayso_ui::paper::PaperStyled;
use sayso_ui::{ActivePaper, text};
use std::collections::HashMap;

#[derive(Clone)]
enum Test {
    Running,
    Passed(u64),
    Failed(String),
}

#[derive(Clone, Copy, PartialEq)]
enum AddKind {
    OpenAi,
    Claude,
    Codex,
}

struct AddProvider {
    kind: AddKind,
    name: Entity<InputState>,
    url: Entity<InputState>,
    key: Entity<InputState>,
    error: Option<String>,
    _sub: Subscription,
}

/// The row that is open, with its text fields.
struct Open {
    id: String,
    /// The folder of the login.
    folder: Entity<InputState>,
    /// The code from the sign-in page.
    code: Entity<InputState>,
    _subs: [Subscription; 2],
}

/// What is wrong with a provider: a sentence for the user, and the words of
/// the tool or the server.
struct Problem {
    message: String,
    details: Option<String>,
}

/// The status line of a row.
struct Status {
    dot: Hsla,
    line: String,
    color: Hsla,
    problem: Option<Problem>,
}

/// (name, base URL). The model is chosen from the provider's list after it is added.
const PRESETS: [(&str, &str); 4] = [
    ("OpenRouter", "https://openrouter.ai/api/v1"),
    ("OpenAI", "https://api.openai.com/v1"),
    ("Ollama", "http://localhost:11434/v1"),
    ("LM Studio", "http://localhost:1234/v1"),
];

pub struct ProviderSettings {
    model: Entity<AppModel>,
    open: Option<Open>,
    confirm_remove: Option<String>,
    add: Option<AddProvider>,
    tests: HashMap<String, Test>,
    _observe: Subscription,
}

fn is_cli(p: &Provider) -> bool {
    matches!(p.kind, ProviderKind::ClaudeCli { .. } | ProviderKind::CodexCli { .. })
}

/// The name of the tool in a sentence, and the folder it uses by default.
fn tool(p: &Provider) -> (&'static str, &'static str) {
    match p.kind {
        ProviderKind::CodexCli { .. } => ("Codex", "~/.codex"),
        _ => ("Claude Code", "~/.claude"),
    }
}

impl ProviderSettings {
    pub fn new(model: Entity<AppModel>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&model, |_, _, cx| cx.notify());
        let ids: Vec<String> = model.read(cx).config.ai.providers.iter().filter(|p| is_cli(p)).map(|p| p.id.clone()).collect();
        model.update(cx, |m, cx| {
            for id in &ids {
                m.check_provider_login(id, cx);
            }
        });
        Self { model, open: None, confirm_remove: None, add: None, tests: HashMap::new(), _observe: observe }
    }

    fn provider(&self, id: &str, cx: &App) -> Option<Provider> {
        self.model.read(cx).config.ai.providers.iter().find(|p| p.id == id).cloned()
    }

    /// True when the login of `p` is in the folder that Sayso keeps for it.
    fn sayso_login(&self, p: &Provider, cx: &App) -> bool {
        let dir = self.model.read(cx).sayso_login_dir(&p.id);
        p.config_dir().is_some_and(|d| std::path::Path::new(d) == dir)
    }

    // -----------------------------------------------------------------------
    // Rows
    // -----------------------------------------------------------------------

    fn toggle(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_remove = None;
        if self.open.as_ref().is_some_and(|o| o.id == id) {
            self.open = None;
        } else {
            self.open_row(id, window, cx);
        }
        cx.notify();
    }

    /// Open the row of a provider, and read its login again.
    fn open_row(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.provider(id, cx) else { return };
        let own = if self.sayso_login(&p, cx) { "" } else { p.config_dir().unwrap_or_default() };
        let folder = crate::widgets::input_state(tool(&p).1, own, window, cx);
        let code = crate::widgets::input_state("Paste the code from the browser", "", window, cx);
        let folder_sub = cx.subscribe_in(&folder, window, |this, state, ev: &InputEvent, _, cx| {
            if !matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                return;
            }
            let Some(id) = this.open.as_ref().map(|o| o.id.clone()) else { return };
            // The field is hidden for the login that Sayso keeps: do not replace that folder.
            if this.provider(&id, cx).is_none_or(|p| this.sayso_login(&p, cx)) {
                return;
            }
            let value = state.read(cx).value().trim().to_string();
            this.tests.remove(&id);
            this.model.update(cx, |m, cx| m.set_provider_config_dir(&id, Some(value), cx));
        });
        let code_sub = cx.subscribe_in(&code, window, |this, _, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.send_code(window, cx);
            }
        });
        self.open = Some(Open { id: id.to_string(), folder, code, _subs: [folder_sub, code_sub] });
        self.model.update(cx, |m, cx| m.check_provider_login(id, cx));
    }

    fn send_code(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &self.open else { return };
        let code = open.code.read(cx).value().trim().to_string();
        if code.is_empty() {
            return;
        }
        let id = open.id.clone();
        open.code.update(cx, |s, cx| s.set_value("", window, cx));
        self.model.update(cx, |m, cx| m.send_sign_in_code(&id, &code, cx));
    }

    /// Use the login of the terminal (false) or a login for Sayso only (true).
    fn set_sayso_login(&mut self, id: &str, sayso: bool, window: &mut Window, cx: &mut Context<Self>) {
        let dir = sayso.then(|| self.model.read(cx).sayso_login_dir(id).display().to_string());
        self.tests.remove(id);
        self.model.update(cx, |m, cx| m.set_provider_config_dir(id, dir, cx));
        // The folder field starts again from the new value.
        self.open_row(id, window, cx);
        cx.notify();
    }

    fn test(&mut self, id: String, cx: &mut Context<Self>) {
        self.tests.insert(id.clone(), Test::Running);
        let entity = cx.entity();
        self.model.update(cx, |m, cx| {
            m.test_provider(&id.clone(), cx, move |m, result, cx| {
                // A test that fails can mean that the login ended.
                m.check_provider_login(&id, cx);
                entity.update(cx, |this, cx| {
                    this.tests.insert(id, match result {
                        Ok(ms) => Test::Passed(ms),
                        Err(e) => Test::Failed(e),
                    });
                    cx.notify();
                });
            })
        });
        cx.notify();
    }

    fn status(&self, p: &Provider, login: Option<&ProviderLogin>, cx: &App) -> Status {
        let c = cx.paper().colors;
        let test = self.tests.get(&p.id);
        let (name, _) = tool(p);
        let bad = |line: &str, problem: Option<Problem>| Status { dot: c.danger, line: line.to_string(), color: c.danger, problem };
        if login.is_some_and(|l| l.sign_in.is_some()) {
            return Status { dot: c.accent, line: "Waiting for the browser".into(), color: c.graphite, problem: None };
        }
        if matches!(test, Some(Test::Running)) {
            return Status { dot: c.accent, line: "Testing".into(), color: c.graphite, problem: None };
        }
        if p.needs_model() {
            return bad("Choose a model to use this provider", None);
        }
        let mut signed_in = None;
        if is_cli(p) {
            match login.and_then(|l| l.status.as_ref()) {
                None => return Status { dot: c.pencil, line: "Checking the login".into(), color: c.graphite, problem: None },
                Some(AuthStatus::SignedIn { account }) => {
                    let mut parts = vec!["Signed in".to_string()];
                    parts.extend(account.clone());
                    parts.push("Cloud".into());
                    signed_in = Some(parts.join(" · "));
                }
                Some(AuthStatus::SignedOut { detail }) => {
                    let place = if self.sayso_login(p, cx) {
                        "Sign in to make a login for Sayso.".to_string()
                    } else {
                        "Sign in, or set the folder that holds your login.".to_string()
                    };
                    return bad(
                        "Not signed in",
                        Some(Problem { message: format!("{name} has no account in the folder that Sayso uses. {place}"), details: Some(detail.clone()) }),
                    );
                }
                Some(AuthStatus::NotInstalled) => {
                    return bad(
                        "Not installed",
                        Some(Problem {
                            message: format!("Sayso cannot find the {name} command. Install {name}, then open this row again."),
                            details: None,
                        }),
                    );
                }
                Some(AuthStatus::Unknown { detail }) => {
                    return bad(
                        "The login is not known",
                        Some(Problem { message: format!("Sayso could not read the login of {name}. Test the connection to see if it works."), details: Some(detail.clone()) }),
                    );
                }
            }
        }
        match test {
            Some(Test::Failed(raw)) => bad("The test failed", Some(Problem { message: test_error(raw), details: Some(raw.clone()) })),
            Some(Test::Passed(ms)) => Status { dot: c.success, line: format!("Answered in {ms} ms"), color: c.success, problem: None },
            _ => match signed_in {
                Some(line) => Status { dot: c.success, line, color: c.graphite, problem: None },
                None => Status {
                    dot: if p.is_cloud() { c.pencil } else { c.success },
                    line: provider_detail(p),
                    color: c.graphite,
                    problem: None,
                },
            },
        }
    }

    fn provider_row(&self, p: &Provider, default: bool, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.paper().colors;
        let login = self.model.read(cx).logins.get(&p.id).cloned();
        let status = self.status(p, login.as_ref(), cx);
        let open = self.open.as_ref().filter(|o| o.id == p.id);
        let signing_in = login.as_ref().is_some_and(|l| l.sign_in.is_some());
        let signed_out = matches!(login.as_ref().and_then(|l| l.status.as_ref()), Some(AuthStatus::SignedOut { .. }));

        let toggle_id = p.id.clone();
        let label = div()
            .id(SharedString::from(format!("provider-{}", p.id)))
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(px(14.))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, window, cx| this.toggle(&toggle_id, window, cx)))
            .child(div().flex_none().size(px(8.)).rounded_full().bg(status.dot))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(ui(p.name.clone(), 15., 18., FontWeight::SEMIBOLD, c.ink).truncate())
                            .when(default, |d| {
                                d.child(div().flex_none().px(px(6.)).py(px(2.)).rounded(px(5.)).bg(c.deboss).child(pkit::caps("Default", 11., c.graphite)))
                            }),
                    )
                    .child(ui(status.line.clone(), 13., 16., FontWeight::NORMAL, status.color).truncate()),
            );

        let mut head = div().flex().items_center().gap(px(14.)).child(label);
        if signing_in {
            let id = p.id.clone();
            head = head.child(
                Button::new(SharedString::from(format!("cancel-sign-in-{}", p.id)), "Cancel")
                    .on_click(cx.listener(move |this, _, _, cx| this.model.update(cx, |m, cx| m.cancel_sign_in(&id, cx)))),
            );
        } else if signed_out {
            let id = p.id.clone();
            head = head.child(Button::new(SharedString::from(format!("sign-in-{}", p.id)), "Sign in").primary().on_click(cx.listener(
                move |this, _, window, cx| {
                    if this.open.as_ref().is_none_or(|o| o.id != id) {
                        this.open_row(&id, window, cx);
                    }
                    this.model.update(cx, |m, cx| m.sign_in_provider(&id, cx));
                    cx.notify();
                },
            )));
        } else if open.is_none() {
            // The open row has the picker in its Model line.
            head = head.child(self.model_picker(p, cx));
        }
        let chevron_id = p.id.clone();
        head = head.child(
            pkit::icon_button(SharedString::from(format!("chevron-{}", p.id)), if open.is_some() { Icon::ChevronDown } else { Icon::ChevronRight }, 14., cx)
                .on_click(cx.listener(move |this, _, window, cx| this.toggle(&chevron_id, window, cx))),
        );

        let mut row = div().flex().flex_col().w_full().min_w_0().py(px(14.)).border_b_1().border_color(c.rule).child(head);
        if let Some(open) = open {
            row = row.child(self.detail(p, default, open, login.as_ref(), status.problem, cx));
        }
        row.into_any_element()
    }

    /// The open part of a row.
    fn detail(&self, p: &Provider, default: bool, open: &Open, login: Option<&ProviderLogin>, problem: Option<Problem>, cx: &mut Context<Self>) -> Div {
        let c = cx.paper().colors;
        let (name, _) = tool(p);
        let mut col = div().flex().flex_col().w_full().min_w_0().pl(px(22.)).pt(px(14.));

        if let Some(sign_in) = login.and_then(|l| l.sign_in.as_ref()) {
            col = col.child(self.sign_in_panel(p, open, sign_in.takes_code(), login.and_then(|l| l.url.clone()), cx));
        } else if let Some(problem) = problem {
            col = col.child(self.problem(&p.id, problem, cx));
        }
        if let Some(error) = login.and_then(|l| l.error.clone()) {
            col = col.child(kit::banner(BannerKind::Warning, error, cx));
        }

        if is_cli(p) {
            let sayso = self.sayso_login(p, cx);
            let entity = cx.entity();
            let id = p.id.clone();
            let choice = Segmented::new(SharedString::from(format!("login-{}", p.id)), [format!("My {name} login"), "A login for Sayso only".to_string()], usize::from(sayso))
                .on_select(move |i, window, cx| entity.update(cx, |this, cx| this.set_sayso_login(&id, i == 1, window, cx)));
            let about = if sayso {
                format!("Sayso keeps a login of its own. The {name} in your terminal does not use it.")
            } else {
                format!("Sayso uses the account that {name} uses in your terminal.")
            };
            col = col.child(kit::row_s("Login".into(), about, choice, cx));
            if !sayso {
                let field = div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .w(px(320.))
                    .h(px(36.))
                    .px(px(12.))
                    .rounded(px(9.))
                    .debossed(&c)
                    .font_family(sayso_ui::fonts::MONO)
                    .text_size(px(12.))
                    .child(gpui_kit::component::input::Input::new(&open.folder).appearance(false).w_full());
                col = col.child(kit::row(
                    "Config folder",
                    "Set this when your login is in another folder, for example a profile of an account switcher. Leave it empty for the default folder.",
                    field,
                    cx,
                ));
            }
        } else if let ProviderKind::OpenAiCompatible { base_url, .. } = &p.kind {
            let place = if is_local_url(base_url) { format!("A server on this {}.", crate::shell::COMPUTER) } else { "A cloud server. You use your own key.".to_string() };
            col = col.child(kit::row_s("Address".into(), place, text::mono(base_url.clone(), 12., c.graphite).font_weight(FontWeight::NORMAL), cx));
        }

        if p.kind != ProviderKind::AppleIntelligence {
            col = col.child(kit::row("Model", "A style can name another model.", self.model_picker(p, cx), cx));
        }

        let path = match &p.kind {
            ProviderKind::ClaudeCli { path, .. } | ProviderKind::CodexCli { path, .. } => path.clone(),
            _ => None,
        };
        let mut left = div().flex().flex_col().flex_1().min_w_0().gap(px(2.));
        if let Some(path) = path {
            left = left
                .child(ui("Program", 15., 18., FontWeight::SEMIBOLD, c.ink))
                .child(text::mono(path, 12., c.graphite).font_weight(FontWeight::NORMAL));
        }
        col.child(div().flex().items_center().w_full().gap(px(24.)).pt(px(14.)).child(left).child(self.actions(p, default, cx)))
    }

    /// Test, Make default, and Remove. Remove asks first, in place.
    fn actions(&self, p: &Provider, default: bool, cx: &mut Context<Self>) -> Div {
        let c = cx.paper().colors;
        let mut row = div().flex().flex_none().items_center().gap(px(8.));
        if self.confirm_remove.as_deref() == Some(p.id.as_str()) {
            let id = p.id.clone();
            return row
                .child(ui(format!("Remove {}?", p.name), 13., 16., FontWeight::MEDIUM, c.ink))
                .child(Button::new("remove-yes", "Remove").danger().on_click(cx.listener(move |this, _, _, cx| {
                    this.confirm_remove = None;
                    this.open = None;
                    this.tests.remove(&id);
                    this.model.update(cx, |m, cx| m.remove_provider(&id, cx));
                })))
                .child(Button::new("remove-no", "Cancel").ghost().on_click(cx.listener(|this, _, _, cx| {
                    this.confirm_remove = None;
                    cx.notify();
                })));
        }
        let running = matches!(self.tests.get(&p.id), Some(Test::Running));
        let id = p.id.clone();
        row = row.child(
            Button::new("test-provider", "Test connection")
                .disabled(running || p.needs_model())
                .on_click(cx.listener(move |this, _, _, cx| this.test(id.clone(), cx))),
        );
        if !default {
            let id = p.id.clone();
            row = row.child(Button::new("default-provider", "Make default").on_click(cx.listener(move |this, _, _, cx| {
                let id = id.clone();
                this.model.update(cx, |m, cx| m.edit_config(cx, |c| c.ai.default_provider = Some(id)));
            })));
        }
        let id = p.id.clone();
        row.child(Button::new("remove-provider", "Remove").danger().on_click(cx.listener(move |this, _, _, cx| {
            this.confirm_remove = Some(id.clone());
            cx.notify();
        })))
    }

    /// The whole text of a problem. The text wraps, and Copy takes the words
    /// of the tool.
    fn problem(&self, id: &str, problem: Problem, cx: &mut Context<Self>) -> Div {
        let c = cx.paper().colors;
        let text_color = if c.is_dark() { c.danger } else { hsla(0.02, 0.6, 0.23, 1.0) };
        let copy = problem.details.clone().map(|details| {
            let all = format!("{}\n{details}", problem.message);
            pkit::link(SharedString::from(format!("copy-problem-{id}")), "Copy details", 13., cx)
                .text_color(c.danger)
                .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(all.clone())))
        });
        div().w_full().min_w_0().pb(px(6.)).child(
            div()
                .flex()
                .items_start()
                .w_full()
                .min_w_0()
                .gap(px(12.))
                .px(px(14.))
                .py(px(12.))
                .rounded(px(10.))
                .bg(c.danger_wash)
                .border_1()
                .border_color(c.danger.opacity(0.18))
                .child(icon(Icon::Warning, 16., c.danger).mt(px(1.)))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(6.))
                        .child(ui(problem.message, 13., 18., FontWeight::NORMAL, text_color))
                        .when_some(problem.details, |d, details| {
                            d.child(div().w_full().min_w_0().font_family(sayso_ui::fonts::MONO).text_size(px(12.)).line_height(px(17.)).text_color(text_color.opacity(0.85)).child(details))
                        }),
                )
                .children(copy),
        )
    }

    /// The panel of a sign-in that runs: the link to the page, and the field
    /// for the code when the tool takes one.
    fn sign_in_panel(&self, p: &Provider, open: &Open, takes_code: bool, url: Option<String>, cx: &mut Context<Self>) -> Div {
        let c = cx.paper().colors;
        let (name, _) = tool(p);
        let about = if takes_code {
            format!("{name} opened its sign-in page. When the page shows a code, paste it here. Sayso does not see your password.")
        } else {
            format!("{name} opened its sign-in page. Sayso continues when you finish there. Sayso does not see your password.")
        };
        let link = url.filter(|u| u.starts_with("https://") || u.starts_with("http://")).map(|url| {
            pkit::link(SharedString::from(format!("sign-in-page-{}", p.id)), "Open the sign-in page again", 13., cx)
                .font_weight(FontWeight::SEMIBOLD)
                .on_click(move |_, _, _| AppModel::open_url(&url))
        });
        let mut panel = div()
            .flex()
            .flex_col()
            .w_full()
            .min_w_0()
            .gap(px(12.))
            .px(px(16.))
            .py(px(14.))
            .rounded(px(10.))
            .bg(c.accent_wash)
            .border_1()
            .border_color(c.accent.opacity(0.18))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(16.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(3.))
                            .child(ui("Finish the sign-in in your browser", 14., 18., FontWeight::SEMIBOLD, c.ink))
                            .child(ui(about, 13., 18., FontWeight::NORMAL, c.graphite)),
                    )
                    .children(link),
            );
        if takes_code {
            panel = panel.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(pkit::input_well(&open.code, cx).flex_1().min_w_0().font_family(sayso_ui::fonts::MONO))
                    .child(Button::new("send-code", "Send code").primary().on_click(cx.listener(|this, _, window, cx| this.send_code(window, cx)))),
            );
        }
        div().w_full().min_w_0().pb(px(6.)).child(panel)
    }

    /// The model of a provider. A click opens the provider's list.
    fn model_picker(&self, p: &Provider, cx: &mut Context<Self>) -> AnyElement {
        let m = self.model.read(cx);
        let label = match p.model() {
            Some(id) => m.model_label(&p.id, id),
            None if p.needs_model() => "Choose a model".into(),
            // macOS chooses the model. Its name comes with the list.
            None if p.kind == ProviderKind::AppleIntelligence => match m.model_lists.get(&p.id) {
                Some(crate::model::ModelList::Ready(list)) if !list.is_empty() => list[0].name.clone(),
                _ => "Default model".into(),
            },
            None => "Default model".into(),
        };
        let pid = p.id.clone();
        model_picker::trigger(&format!("provider:{}", p.id), label, !p.needs_model(), cx)
            .on_click(cx.listener(move |this, _, _, cx| this.open_models(&pid, cx)))
            .into_any_element()
    }

    /// Open the model list of a saved provider under its trigger.
    fn open_models(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(p) = self.provider(id, cx) else { return };
        self.model.update(cx, |m, cx| m.load_provider_models(id, false, cx));
        let page = cx.entity().downgrade();
        let model = self.model.clone();
        let (pick_id, refresh_id) = (p.id.clone(), p.id.clone());
        let request = model_picker::Request {
            key: format!("provider:{}", p.id),
            list_key: p.id.clone(),
            selected: p.model().map(str::to_string),
            extra: None,
            on_pick: Box::new(move |pick, cx| {
                let Some(choice) = pick else { return };
                let id = pick_id.clone();
                let _ = page.update(cx, |this, cx| {
                    this.tests.remove(&id);
                    this.model.update(cx, |m, cx| m.set_provider_model(&id, choice, cx));
                });
            }),
            on_refresh: Box::new(move |cx| model.update(cx, |m, cx| m.load_provider_models(&refresh_id, true, cx))),
        };
        model_picker::open(&self.model, request, cx);
    }

    /// A provider that Sayso found on this computer and that is not added yet.
    fn suggestion(&self, id: &'static str, title: &str, detail: String, provider: Provider, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.paper().colors;
        div()
            .flex()
            .items_center()
            .w_full()
            .gap(px(14.))
            .py(px(14.))
            .border_b_1()
            .border_color(c.rule)
            .child(div().flex_none().size(px(8.)).rounded_full().border_1().border_color(c.success))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(ui(title.to_string(), 15., 18., FontWeight::SEMIBOLD, c.ink))
                    .child(ui(detail, 13., 16., FontWeight::NORMAL, c.graphite)),
            )
            .child(Button::new(SharedString::from(format!("{id}-add")), "Add").on_click(cx.listener(move |this, _, window, cx| {
                let p = provider.clone();
                this.added(p, None, window, cx);
            })))
            .into_any_element()
    }

    /// The providers found on this computer that are not set up yet.
    fn suggestions(&self, providers: &[Provider], cx: &mut Context<Self>) -> Vec<AnyElement> {
        let detected = self.model.read(cx).detected.clone();
        let new_id = |name: &str| self.model.read(cx).new_provider_id(name);
        let mut found: Vec<(&'static str, &'static str, String, Provider)> = Vec::new();
        let has_ollama = providers.iter().any(|p| matches!(&p.kind, ProviderKind::OpenAiCompatible { base_url, .. } if base_url.contains(":11434")));
        if let (Some(models), false) = (&detected.ollama, has_ollama) {
            // With no model pulled, the picker opens after Add instead.
            let first = models.first().cloned().unwrap_or_default();
            let kind = ProviderKind::OpenAiCompatible { base_url: "http://localhost:11434/v1".into(), model: first, api_key_account: None, zero_data_retention: false };
            let detail = if models.is_empty() {
                format!("Running on this {} · no models pulled yet", crate::shell::COMPUTER)
            } else {
                format!("Running on this {} · {}", crate::shell::COMPUTER, pkit::plural(models.len(), "model", "models"))
            };
            found.push(("suggest-ollama", "Ollama", detail, Provider { id: new_id("ollama"), name: "Ollama".into(), kind }));
        }
        let has_claude = providers.iter().any(|p| matches!(p.kind, ProviderKind::ClaudeCli { .. }));
        if let (Some(path), false) = (&detected.claude_cli, has_claude) {
            let kind = ProviderKind::ClaudeCli { path: Some(path.display().to_string()), model: "haiku".into(), config_dir: None };
            let detail = format!("Found on this {} · Cloud", crate::shell::COMPUTER);
            found.push(("suggest-claude", "Claude CLI", detail, Provider { id: new_id("claude"), name: "Claude CLI".into(), kind }));
        }
        let has_codex = providers.iter().any(|p| matches!(p.kind, ProviderKind::CodexCli { .. }));
        if let (Some(path), false) = (&detected.codex_cli, has_codex) {
            let kind = ProviderKind::CodexCli { path: Some(path.display().to_string()), model: None, config_dir: None };
            let detail = format!("Found on this {} · Cloud · about 5 s", crate::shell::COMPUTER);
            found.push(("suggest-codex", "Codex CLI", detail, Provider { id: new_id("codex"), name: "Codex CLI".into(), kind }));
        }
        let has_apple = providers.iter().any(|p| matches!(p.kind, ProviderKind::AppleIntelligence));
        if let (Some(model), false) = (&detected.apple_intelligence, has_apple) {
            let p = Provider { id: new_id("apple-intelligence"), name: "Apple Intelligence".into(), kind: ProviderKind::AppleIntelligence };
            found.push(("suggest-apple", "Apple Intelligence", format!("Runs on this Mac · {model}"), p));
        }
        found.into_iter().map(|(id, title, detail, p)| self.suggestion(id, title, detail, p, cx)).collect()
    }

    // -----------------------------------------------------------------------
    // Add a provider
    // -----------------------------------------------------------------------

    fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.add.is_some() {
            self.add = None;
            cx.notify();
            return;
        }
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("For example, OpenRouter"));
        let url = cx.new(|cx| InputState::new(window, cx).placeholder("https://openrouter.ai/api/v1"));
        let key = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(crate::shell::os_text!("Stored in your Keychain", "Stored in Windows Credential Manager", "Stored in your system keyring"))
                .masked(true)
        });
        let sub = cx.subscribe_in(&url, window, |_, _, _: &InputEvent, _, cx| cx.notify());
        name.update(cx, |s, cx| s.focus(window, cx));
        self.add = Some(AddProvider { kind: AddKind::OpenAi, name, url, key, error: None, _sub: sub });
        cx.notify();
    }

    fn apply_preset(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(add) = self.add.as_mut() else { return };
        let (name, url) = PRESETS[i];
        add.name.update(cx, |s, cx| s.set_value(name, window, cx));
        add.url.update(cx, |s, cx| s.set_value(url, window, cx));
        add.error = None;
        cx.notify();
    }

    fn save_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(add) = self.add.as_mut() else { return };
        let name = add.name.read(cx).value().trim().to_string();
        let detected = self.model.read(cx).detected.clone();
        let (name, kind, key) = match add.kind {
            AddKind::OpenAi => {
                let url = add.url.read(cx).value().trim().trim_end_matches('/').to_string();
                if name.is_empty() {
                    add.error = Some("Enter a name for the provider.".into());
                } else if !(url.starts_with("http://") || url.starts_with("https://")) {
                    add.error = Some("Enter a base URL that starts with http:// or https://.".into());
                }
                if add.error.is_some() {
                    cx.notify();
                    return;
                }
                let key = add.key.read(cx).value().trim().to_string();
                (name, Some(url), (!key.is_empty()).then_some(key))
            }
            AddKind::Claude | AddKind::Codex => {
                let claude = add.kind == AddKind::Claude;
                let found = if claude { detected.claude_cli.is_some() } else { detected.codex_cli.is_some() };
                if !found {
                    add.error = Some(if claude {
                        "Sayso cannot find the Claude CLI. Install it, then try again.".into()
                    } else {
                        "Sayso cannot find the Codex CLI. Install it, then try again.".into()
                    });
                    cx.notify();
                    return;
                }
                (if claude { "Claude CLI".into() } else { "Codex CLI".into() }, None, None)
            }
        };
        let id = self.model.read(cx).new_provider_id(&name);
        let kind = match (add.kind, kind) {
            (AddKind::OpenAi, Some(base_url)) => ProviderKind::OpenAiCompatible {
                api_key_account: key.as_ref().map(|_| format!("provider.{id}")),
                zero_data_retention: base_url.contains("openrouter.ai"),
                base_url,
                model: String::new(),
            },
            // Haiku is the fast choice for cleanup; the picker offers the rest.
            (AddKind::Claude, _) => ProviderKind::ClaudeCli { path: detected.claude_cli.map(|p| p.display().to_string()), model: "haiku".into(), config_dir: None },
            _ => ProviderKind::CodexCli { path: detected.codex_cli.map(|p| p.display().to_string()), model: None, config_dir: None },
        };
        let provider = Provider { id, name, kind };
        self.add = None;
        self.added(provider, key, window, cx);
    }

    /// Save a new provider, open its row, and fetch its models. A provider
    /// that cannot run without a model opens its picker. A CLI provider shows
    /// its login. Other providers are tested right away.
    fn added(&mut self, provider: Provider, key: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let id = provider.id.clone();
        let needs_model = provider.needs_model();
        let cli = is_cli(&provider);
        self.model.update(cx, |m, cx| {
            m.save_provider(provider, key, false, cx);
            m.load_provider_models(&id, true, cx);
        });
        self.open_row(&id, window, cx);
        if needs_model {
            // The trigger records its place when it is drawn; open after that.
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(std::time::Duration::from_millis(120)).await;
                let _ = this.update(cx, |this, cx| this.open_models(&id, cx));
            })
            .detach();
        } else if !cli {
            self.test(id, cx);
        }
        cx.notify();
    }

    fn add_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = cx.paper().colors;
        let add = self.add.as_ref()?;
        let (claude, codex) = {
            let m = self.model.read(cx);
            (m.detected.claude_cli.clone(), m.detected.codex_cli.clone())
        };
        let selected = match add.kind {
            AddKind::OpenAi => 0,
            AddKind::Claude => 1,
            AddKind::Codex => 2,
        };
        let entity = cx.entity();
        let seg = Segmented::new("add-kind", ["OpenAI-compatible", "Claude CLI", "Codex CLI"], selected).on_select(move |i, _, cx| {
            entity.update(cx, |this, cx| {
                if let Some(a) = this.add.as_mut() {
                    a.kind = [AddKind::OpenAi, AddKind::Claude, AddKind::Codex][i];
                    a.error = None;
                }
                cx.notify();
            })
        });

        let mut body = div().flex().flex_col().gap(px(16.));
        match add.kind {
            AddKind::OpenAi => {
                let mut presets = div().flex().gap(px(6.));
                for (i, (name, _)) in PRESETS.iter().enumerate() {
                    presets = presets.child(Chip::new(("preset", i), *name, false).on_click(cx.listener(move |this, _, w, cx| this.apply_preset(i, w, cx))));
                }
                let url = add.url.read(cx).value().to_string();
                let cloud = !url.trim().is_empty() && !is_local_url(&url);
                body = body
                    .child(pkit::field("Start from", presets, None, cx))
                    .child(
                        div()
                            .flex()
                            .gap(px(12.))
                            .child(pkit::field("Name", pkit::input_well(&add.name, cx), None, cx).w(px(220.)))
                            .child(pkit::field("Base URL", pkit::input_well(&add.url, cx), None, cx).flex_1()),
                    )
                    .child(pkit::field(
                        "API key",
                        pkit::input_well(&add.key, cx),
                        Some("Not needed for local servers. You choose the model from the provider's list after you add it."),
                        cx,
                    ))
                    .when(cloud, |d| {
                        d.child(Banner::new(
                            BannerKind::Info,
                            "This is a cloud provider. When a style uses it, Sayso sends the style prompt, your dictionary words, and the transcript to it.",
                        ))
                    });
            }
            AddKind::Claude | AddKind::Codex => {
                let (path, name) = if add.kind == AddKind::Claude { (claude, "Claude CLI") } else { (codex, "Codex CLI") };
                let status = match path {
                    Some(p) => format!("Found at {}. Sayso runs it with no tools and no saved session.", p.display()),
                    None => format!("Sayso cannot find the {name}. Install it, then try again."),
                };
                body = body.child(ui(status, 13., 18., FontWeight::NORMAL, c.graphite)).child(Banner::new(
                    BannerKind::Info,
                    if add.kind == AddKind::Claude {
                        "Cloud. About 1.8 s for each dictation with Haiku. The CLI uses your own account. You can sign in and choose another model after you add it."
                    } else {
                        "Cloud. About 5 s for each dictation. The CLI uses your own account. You can sign in and choose a model after you add it."
                    },
                ));
            }
        }
        let panel = div()
            .flex()
            .flex_col()
            .gap(px(18.))
            .mt(px(14.))
            .p(px(22.))
            .rounded(px(14.))
            .raised(&c)
            .child(div().flex().items_center().justify_between().child(text::title("Add a provider", 22., &c)).child(seg))
            .child(body)
            .when_some(add.error.clone(), |d, e| d.child(Banner::new(BannerKind::Warning, e)))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(Button::new("save-provider", "Add provider").primary().on_click(cx.listener(|this, _, window, cx| this.save_add(window, cx))))
                    .child(Button::new("cancel-provider", "Cancel").ghost().on_click(cx.listener(|this, _, _, cx| {
                        this.add = None;
                        cx.notify();
                    }))),
            );
        Some(panel.into_any_element())
    }
}

/// A provider test error in plain words, with what to do next.
fn test_error(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("not signed in") {
        "The provider is not signed in. Sign in, or set the folder that holds your login.".into()
    } else if lower.contains("refused") || lower.contains("connect") || lower.contains("dns") {
        "Cannot connect. Start the server or check the URL, then test again.".into()
    } else if lower.contains("timed out") || lower.contains("timeout") {
        "No answer in time. Test again, or raise the timeout.".into()
    } else if lower.contains("401") || lower.contains("403") || lower.contains("unauthorized") || lower.contains("api key") {
        "The provider refused the API key. Check the key, then test again.".into()
    } else if lower.contains("404") || lower.contains("model") {
        "The provider did not accept the model. Choose another model, then test again.".into()
    } else {
        "The provider did not answer the test.".into()
    }
}

impl Render for ProviderSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.paper().colors;
        let (providers, default, enabled, searching) = {
            let m = self.model.read(cx);
            (m.config.ai.providers.clone(), m.config.ai.default_provider.clone(), m.config.ai.enabled, !m.detected.done)
        };
        // A provider that is gone has no row to keep open.
        if self.open.as_ref().is_some_and(|o| !providers.iter().any(|p| p.id == o.id)) {
            self.open = None;
        }
        let model = self.model.clone();
        let use_ai = kit::row(
            "Use AI for styles",
            "When this is off, Sayso inserts every dictation with the Raw style.",
            Switch::new("ai-on", enabled).on_toggle(move |on, _, cx| model.update(cx, |m, cx| m.set_ai_enabled(on, cx))),
            cx,
        );

        let mut list = group("Your providers", cx);
        if providers.is_empty() {
            let line = if searching {
                format!("Looking for AI providers on this {}.", crate::shell::COMPUTER)
            } else {
                "No provider is set up. Add one to use styles with AI.".to_string()
            };
            list = list.child(div().py(px(14.)).child(ui(line, 13., 16., FontWeight::NORMAL, c.graphite)));
        }
        for p in &providers {
            let is_default = default.as_deref() == Some(p.id.as_str());
            list = list.child(self.provider_row(p, is_default, cx));
        }
        list = list.children(self.suggestions(&providers, cx));
        if self.add.is_none() {
            list = list.child(
                pkit::dashed("add-provider", cx)
                    .justify_start()
                    .gap(px(12.))
                    .mt(px(14.))
                    .px(px(16.))
                    .py(px(14.))
                    .rounded(px(10.))
                    .on_click(cx.listener(|this, _, w, cx| this.open_add(w, cx)))
                    .child(icon(Icon::Plus, 14., c.graphite))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(ui("Add a provider", 15., 18., FontWeight::SEMIBOLD, c.ink))
                            .child(ui(
                                "OpenRouter, OpenAI, Ollama, LM Studio, or any server with the OpenAI API. You use your own key.",
                                13.,
                                16.,
                                FontWeight::NORMAL,
                                c.graphite,
                            )),
                    ),
            );
        }
        list = list.children(self.add_panel(cx));

        kit::page(
            "providers-page",
            "Providers",
            "The AI services that styles run on. A cloud provider receives the style prompt and your transcript. Nothing else.",
            kit::body().child(div().flex().flex_col().w_full().min_w_0().child(use_ai)).child(list),
            cx,
        )
    }
}
