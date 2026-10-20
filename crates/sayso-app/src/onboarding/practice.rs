//! Step 6: try a dictation into a real text field in this window.
//!
//! Our window is the focused app, so the toggle hotkey records and the paste
//! lands in this field. The result line comes from the newest history entry.

use super::{OnboardingView, heading, live_ui};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::*;
use sayso_i18n::t;
use sayso_core::history::{EnhanceOutcome, HistoryEntry, InsertOutcome};
use sayso_core::stt::ModelStatus;
use sayso_ui::fonts::{Face, LocaleFonts as _};
use sayso_ui::a11y::{A11yExt as _, A11yText, Role};
use sayso_ui::assets::Icon;
use sayso_ui::components::*;
use sayso_ui::{ActivePaper, Colors, text};

pub const SENTENCE: &str = crate::shell::os_text!(
    "Sayso writes what I say, in any app on my Mac, and nothing leaves this computer.",
    "Sayso writes what I say, in any app on my PC, and nothing leaves this computer.",
    "Sayso writes what I say, in any app I use, and nothing leaves this computer.",
);

pub(super) struct Practice {
    pub(super) field: Entity<TextareaState>,
    /// Entries before this time belong to earlier attempts.
    since: chrono::DateTime<chrono::Utc>,
}

impl Practice {
    pub(super) fn new(window: &mut Window, cx: &mut App) -> Self {
        let field = cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 5).placeholder(t!("onboarding.practice.placeholder")));
        Self { field, since: chrono::Utc::now() }
    }

    pub(super) fn enter(&mut self, window: &mut Window, cx: &mut App) {
        self.since = chrono::Utc::now();
        self.field.update(cx, |s, cx| s.focus(window, cx));
    }
}

/// Words of the target sentence that the result has, in order (longest common subsequence).
fn words_correct(result: &str) -> (usize, usize) {
    let norm = |s: &str| -> Vec<String> {
        s.split_whitespace()
            .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase())
            .filter(|w| !w.is_empty())
            .collect()
    };
    let a = norm(SENTENCE);
    let b = norm(result);
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            dp[i][j] = if a[i - 1] == b[j - 1] { dp[i - 1][j - 1] + 1 } else { dp[i - 1][j].max(dp[i][j - 1]) };
        }
    }
    (dp[a.len()][b.len()], a.len())
}

/// Time from the stop to the insert: the final pass plus the style, if any.
fn delay_ms(e: &HistoryEntry) -> u64 {
    let ai = match &e.enhance {
        EnhanceOutcome::Applied { elapsed_ms, .. } => *elapsed_ms,
        _ => 0,
    };
    e.transcribe_ms + ai
}

fn readiness(label: String, color: Hsla, c: &Colors) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(div().size(px(7.)).rounded_full().bg(color))
        .child(text::ui(label, 13., FontWeight::NORMAL, c.ink))
}

impl OnboardingView {
    pub(super) fn practice_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.paper().colors;
        let m = self.model.read(cx);
        let keys = m.config.hotkeys.toggle.map(|h| h.keycaps().join(" "));
        let lead = match &keys {
            Some(k) => t!("onboarding.practice.lead_toggle", keys = k),
            None => t!("onboarding.practice.lead_push"),
        };
        let latest = m.recent.first().filter(|e| e.created_at >= self.practice.since).cloned();
        let info = m.active_model();
        let status = m.status_of(&info.id);
        let preview = m.preview_model();
        let preview_status = preview.as_ref().map(|p| m.status_of(p));
        let ready = m.ready_to_dictate();
        let model_on_disk = m.status_of(&m.config.dictation.model).is_on_disk();
        let mic_ok = m.permission(sayso_platform::Permission::Microphone) == sayso_platform::PermissionState::Granted;
        let state_text = match m.state() {
            sayso_core::dictation::State::Recording { .. } => Some(t!("onboarding.practice.listening")),
            sayso_core::dictation::State::Processing { .. } => Some(t!("onboarding.practice.writing")),
            _ => None,
        };

        // Readiness dots.
        let (final_label, final_color) = match &status {
            s if s.is_on_disk() => (t!("onboarding.practice.final_ready"), c.success),
            ModelStatus::Downloading { fraction, .. } => (t!("onboarding.practice.final_downloading", percent = format!("{:.0}", fraction * 100.0)), c.accent),
            ModelStatus::Failed { .. } => (t!("onboarding.practice.final_stopped"), c.danger),
            _ => (t!("onboarding.practice.final_not_downloaded"), c.pencil),
        };
        let (preview_label, preview_color) = match (&preview, &preview_status) {
            (None, _) if !m.config.dictation.live_preview => (t!("onboarding.practice.preview_off"), c.pencil),
            (None, _) => (t!("onboarding.practice.preview_none"), c.pencil),
            (Some(_), Some(ModelStatus::Ready)) => (t!("onboarding.practice.preview_ready"), c.success),
            (Some(_), Some(ModelStatus::Downloading { fraction, .. })) => (t!("onboarding.practice.preview_downloading", percent = format!("{:.0}", fraction * 100.0)), c.accent),
            (Some(_), _) => (t!("onboarding.practice.preview_loading"), c.accent),
        };
        let (opt_label, opt_color) = match &status {
            ModelStatus::Ready => (t!(crate::shell::os_text!("onboarding.practice.opt_ready_mac", "onboarding.practice.opt_ready_windows", "onboarding.practice.opt_ready_other")), c.success),
            ModelStatus::Optimizing => (t!(crate::shell::os_text!("onboarding.practice.opt_optimizing_mac", "onboarding.practice.opt_optimizing_windows", "onboarding.practice.opt_optimizing_other")), c.accent),
            ModelStatus::Downloaded if m.model_idle => (t!("onboarding.practice.opt_idle"), c.success),
            ModelStatus::Downloaded => (t!("onboarding.practice.opt_loading"), c.accent),
            _ => (t!("onboarding.practice.opt_after_download"), c.pencil),
        };

        // The result line is one live node: a screen reader reads the state
        // ("Listening…", "Inserted 1.2 s after you stopped") when it changes.
        let result: AnyElement = match &latest {
            Some(e) if !matches!(e.insert, InsertOutcome::Failed { .. }) => {
                let (ok, total) = words_correct(&e.final_text);
                let inserted = t!("onboarding.practice.inserted_after", seconds = sayso_i18n::numbers::decimal(delay_ms(e) as f64 / 1000.0, 1));
                let correct = t!("onboarding.practice.words_correct", correct = ok, total = total);
                div()
                    .id("practice-result")
                    .role(Role::Status)
                    .a11y_live(format!("{inserted} {correct}"), false)
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(22.))
                            .rounded_full()
                            .bg(c.accent)
                            .shadow(vec![BoxShadow::new(px(0.), px(-1.5), gpui_kit::black().opacity(0.3)).blur_radius(px(2.)).inset()])
                            .child(icon(Icon::Check, 11., c.on_ink)),
                    )
                    .child(super::plain(inserted, 13., FontWeight::SEMIBOLD, c.ink))
                    .child(super::plain(correct, 13., FontWeight::NORMAL, c.graphite))
                    .into_any_element()
            }
            Some(e) => {
                let reason = match &e.insert {
                    InsertOutcome::Failed { reason } => reason.clone(),
                    _ => String::new(),
                };
                live_ui(
                    "practice-result",
                    if crate::shell::HAS_INPUT_PERMISSIONS {
                        t!("onboarding.practice.paste_failed_ax", reason = reason)
                    } else {
                        t!("onboarding.practice.paste_failed", reason = reason)
                    },
                    13.,
                    FontWeight::NORMAL,
                    c.danger,
                    true,
                )
                .into_any_element()
            }
            None => live_ui(
                "practice-result",
                state_text.unwrap_or_else(|| t!("onboarding.practice.words_appear")),
                13.,
                FontWeight::NORMAL,
                c.graphite,
                false,
            )
            .into_any_element(),
        };

        let field = div()
            .id("practice-field")
            .flex()
            .flex_col()
            .gap(px(12.))
            .min_h(px(120.))
            .py(px(18.))
            .px(px(20.))
            .rounded(px(12.))
            .bg(c.sheet_raised)
            .shadow(vec![
                BoxShadow::new(px(0.), px(1.), c.shadow(0.12)).blur_radius(px(3.)).inset(),
                BoxShadow::new(px(0.), px(0.), c.accent).spread_radius(px(2.)),
                BoxShadow::new(px(0.), px(1.), c.highlight(1.0)),
            ])
            .cursor_text()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                this.practice.field.update(cx, |s, cx| s.focus(window, cx));
            }))
            .child(
                div()
                    .font_family(sayso_ui::fonts::DISPLAY).locale_fonts(Face::Serif)
                    .text_size(px(20.))
                    .line_height(px(28.))
                    .text_color(c.ink)
                    .child(Textarea::new(&self.practice.field).aria_label(t!("onboarding.practice.field_label")).appearance(false).font_family(sayso_ui::fonts::DISPLAY).text_size(px(20.)).line_height(px(28.)).p_0()),
            )
            .child(result);

        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(22.))
            .pt(px(28.))
            .px(px(self.side()))
            .pb(px(24.))
            .child(heading(&t!("onboarding.practice.title"), &lead, &c))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .px(px(4.))
                    .child(text::caps(t!("onboarding.practice.read_this"), &c))
                    .child(
                        div()
                            .font_family(sayso_ui::fonts::DISPLAY).locale_fonts(Face::Serif)
                            .italic()
                            .text_size(px(22.))
                            .line_height(px(30.))
                            .text_color(c.graphite)
                            .child(A11yText::new(format!("“{SENTENCE}”"))),
                    ),
            )
            .child(field)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(20.))
                    .px(px(4.))
                    .child(readiness(final_label, final_color, &c))
                    .child(readiness(preview_label, preview_color, &c))
                    .child(readiness(opt_label, opt_color, &c)),
            );
        if !ready {
            let why = if !model_on_disk {
                t!("onboarding.practice.not_ready_model")
            } else if !mic_ok {
                t!("onboarding.practice.not_ready_mic")
            } else {
                t!("onboarding.practice.not_ready_engine")
            };
            col = col.child(crate::hub::settings::kit::notice(BannerKind::Info, why, cx));
        }
        col.into_any_element()
    }
}
