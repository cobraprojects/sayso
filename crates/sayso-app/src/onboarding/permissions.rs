//! Step 4: microphone and accessibility on one screen, with live detection
//! and the mic test inline.

use super::{OnboardingView, card, heading, live_ui, ring};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sayso_i18n::t;
use sayso_core::stt::ModelStatus;
use sayso_platform::{Permission, PermissionState};
use sayso_ui::a11y::LiveStatus;
use sayso_ui::components::*;
use sayso_ui::paper::PaperStyled;
use sayso_ui::{ActivePaper, Colors, text};

impl OnboardingView {
    pub(super) fn permissions_step(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let c = cx.paper().colors;
        let m = self.model.read(cx);
        let mic = m.permission(Permission::Microphone);
        let ax = m.permission(Permission::Accessibility);
        let info = m.active_model();
        let status = m.status_of(&info.id);
        let devices = m.input_devices();
        let device_id = m.resolve_input_device(m.config.audio.input_device.as_deref());
        let device_name = match device_id {
            Some(id) => devices.iter().find(|d| d.id == id).map(|d| d.name.clone()),
            None => devices.iter().find(|d| d.is_default).map(|d| d.name.clone()),
        }
        .unwrap_or_else(|| t!("onboarding.permissions.default_microphone"));
        // The ring marks the card that needs you now.
        let focus_mic = mic != PermissionState::Granted;
        let focus_ax = !focus_mic && ax != PermissionState::Granted;

        // Microphone card.
        let mut mic_card = card(&c)
            .when(focus_mic, |d| d.shadow(ring(&c)))
            .child(card_head(&crate::shell::permission_name(Permission::Microphone), mic, &c))
            .child(text::ui(
                match mic {
                    PermissionState::Granted => t!("onboarding.permissions.mic_granted"),
                    PermissionState::Denied if cfg!(windows) => t!("onboarding.permissions.mic_denied_windows"),
                    PermissionState::Denied => t!("onboarding.permissions.mic_denied"),
                    PermissionState::NotDetermined => t!(crate::shell::os_text!("onboarding.permissions.mic_undetermined_mac", "onboarding.permissions.mic_undetermined_other")),
                },
                13.,
                FontWeight::NORMAL,
                c.graphite,
            )
            .line_height(px(19.)));
        mic_card = match mic {
            PermissionState::Granted => {
                let well = div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .py(px(12.))
                    .px(px(14.))
                    .rounded(px(12.))
                    .debossed(&c);
                if let Some(e) = &self.meter_error {
                    mic_card.child(well.child(live_ui("mic-error", t!("onboarding.permissions.mic_failed", error = e), 12., FontWeight::NORMAL, c.danger, true)))
                } else {
                    let reduce = cx.paper().reduce_motion;
                    if self.meter.is_some() && !reduce {
                        window.request_animation_frame();
                    }
                    let t = if reduce { 0.0 } else { self.started.elapsed().as_secs_f32() };
                    mic_card.child(
                        well.child(ink_waveform(self.live.smoothed(28), t, c.ink).w(px(200.)).h(px(34.)).flex_none()).child(
                            text::ui(device_name, 12., FontWeight::NORMAL, c.graphite).flex_1().min_w_0().text_right().truncate(),
                        ),
                    )
                }
            }
            PermissionState::NotDetermined => mic_card.child(
                div().flex().pt(px(6.)).child(Button::new("allow-mic", t!("onboarding.permissions.allow_microphone")).primary().on_click(cx.listener(|this, _, _, cx| {
                    this.model.update(cx, |m, _| m.request_permission(Permission::Microphone));
                }))),
            ),
            PermissionState::Denied => mic_card.child(
                div().flex().pt(px(6.)).child(Button::new("open-mic", crate::shell::open_permission_settings()).primary().on_click(cx.listener(|this, _, _, cx| {
                    this.model.update(cx, |m, _| m.open_permission_settings(Permission::Microphone));
                }))),
            ),
        };

        // Accessibility card.
        let mut ax_card = card(&c)
            .when(focus_ax, |d| d.shadow(ring(&c)))
            // macOS has no "denied" for Accessibility; until it is on, Sayso waits.
            .child(card_head(&crate::shell::permission_name(Permission::Accessibility), if ax == PermissionState::Granted { ax } else { PermissionState::NotDetermined }, &c))
            .child(text::ui(
                if ax == PermissionState::Granted {
                    t!("onboarding.permissions.ax_granted")
                } else {
                    t!(crate::shell::os_text!("onboarding.permissions.ax_needed_mac", "onboarding.permissions.ax_needed_other"))
                },
                13.,
                FontWeight::NORMAL,
                c.graphite,
            )
            .line_height(px(19.)));
        if ax != PermissionState::Granted {
            ax_card = ax_card.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .pt(px(6.))
                    .child(Button::new("open-ax", t!(crate::shell::os_text!("onboarding.permissions.grant_ax_mac", "onboarding.permissions.grant_ax_other"))).primary().on_click(cx.listener(|this, _, _, cx| {
                        // `request` shows the macOS prompt, which adds Sayso to the list and opens Settings.
                        this.model.update(cx, |m, _| m.request_permission(Permission::Accessibility));
                    })))
                    .child(Button::new("why-ax", t!("onboarding.permissions.why_ax")).ghost().expanded(self.show_why).on_click(cx.listener(|this, _, _, cx| {
                        this.show_why = !this.show_why;
                        cx.notify();
                    }))),
            );
            if self.show_why {
                ax_card = ax_card.child(
                    text::ui(
                        t!(crate::shell::os_text!("onboarding.permissions.why_ax_text_mac", "onboarding.permissions.why_ax_text_other")),
                        12.,
                        FontWeight::NORMAL,
                        c.graphite,
                    )
                    .line_height(px(17.)),
                );
            }
        }

        // The background download.
        let (fraction, label) = match &status {
            ModelStatus::Downloading { fraction, .. } => (*fraction, format!("{:.0}%", fraction * 100.0)),
            ModelStatus::Ready => (1.0, t!("onboarding.permissions.model_ready")),
            ModelStatus::Optimizing => (1.0, t!("onboarding.permissions.model_optimizing")),
            ModelStatus::Downloaded => (1.0, t!("onboarding.permissions.model_downloaded")),
            ModelStatus::NotDownloaded => (0.0, t!("onboarding.permissions.model_not_started")),
            ModelStatus::Failed { .. } => (0.0, t!("onboarding.permissions.model_stopped")),
        };
        let download = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(4.))
            .child(text::ui(info.name.clone(), 13., FontWeight::NORMAL, c.graphite).flex_none())
            .child(
                div()
                    .id("permissions-download")
                    .role(Role::ProgressIndicator)
                    .aria_label(t!("onboarding.permissions.download_label", name = info.name))
                    .aria_min_numeric_value(0.)
                    .aria_max_numeric_value(100.)
                    .aria_numeric_value((fraction.clamp(0.0, 1.0) * 100.).round() as f64)
                    .flex()
                    .flex_1()
                    .h(px(4.))
                    .rounded_full()
                    .bg(c.deboss)
                    .shadow(vec![BoxShadow::new(px(0.), px(1.), c.shadow(0.25)).blur_radius(px(1.)).inset()])
                    .child(div().h_full().w(relative(fraction.clamp(0.0, 1.0))).rounded_full().bg(c.ink_fill)),
            )
            .child(text::mono(label, 12., c.graphite).font_weight(FontWeight::NORMAL).flex_none());

        div()
            .flex()
            .flex_col()
            .gap(px(22.))
            .pt(px(28.))
            .px(px(self.side()))
            .pb(px(24.))
            .child(if crate::shell::HAS_INPUT_PERMISSIONS {
                heading(&t!("onboarding.permissions.title"), &t!("onboarding.permissions.lead"), &c)
            } else {
                heading(&t!("onboarding.permissions.title_mic_only"), &t!("onboarding.permissions.lead_mic_only"), &c)
            })
            .when(!crate::dev::running_from_bundle(), |d| {
                d.child(crate::hub::settings::kit::notice(BannerKind::Warning, crate::dev::UNBUNDLED_NOTE, cx))
            })
            .child(self.columns().child(mic_card).when(crate::shell::HAS_INPUT_PERMISSIONS, |d| d.child(ax_card)))
            .child(download)
            .when(crate::shell::HAS_INPUT_PERMISSIONS, |d| {
                d.child(crate::hub::settings::kit::notice(
                    BannerKind::Info,
                    t!(crate::shell::os_text!("onboarding.permissions.im_note_mac", "onboarding.permissions.im_note_other")), cx))
            })
            .into_any_element()
    }
}

fn card_head(title: &str, state: PermissionState, c: &Colors) -> Div {
    // The state is one live node: a screen reader says "Allowed" when the
    // user grants the permission.
    let word = match state {
        PermissionState::Granted => t!("onboarding.permissions.allowed"),
        PermissionState::Denied => t!("onboarding.permissions.blocked"),
        PermissionState::NotDetermined => t!("onboarding.permissions.waiting"),
    };
    let badge = match state {
        PermissionState::Granted => super::check_line_w(word.clone(), FontWeight::SEMIBOLD, c),
        PermissionState::Denied => div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(crate::hub::settings::kit::halo_dot(8., c.danger, 3., 0.16))
            .child(text::ui(word.clone(), 13., FontWeight::SEMIBOLD, c.danger)),
        PermissionState::NotDetermined => div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(crate::hub::settings::kit::halo_dot(8., c.accent, 3., 0.16))
            .child(text::ui(word.clone(), 13., FontWeight::SEMIBOLD, c.accent)),
    };
    div()
        .flex()
        .items_center()
        .justify_between()
        .child(text::title(title.to_string(), 20., c).line_height(px(24.)))
        .child(
            LiveStatus::new(SharedString::from(format!("permission-{title}")), format!("{title}: {word}")).child(badge),
        )
}
