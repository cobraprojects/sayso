//! Settings › Permissions: live state for each permission.

use super::kit::{self, group};
use crate::model::AppModel;
use gpui_kit::*;
use sayso_i18n::t;
use sayso_platform::{Permission, PermissionState};
use sayso_ui::components::*;

pub struct PermissionSettings {
    model: Entity<AppModel>,
}

impl PermissionSettings {
    pub fn new(model: Entity<AppModel>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self { model }
    }
}

impl Render for PermissionSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let m = self.model.read(cx);
        let mut rows = vec![
            (Permission::Microphone, crate::shell::permission_name(Permission::Microphone), t!("settings.permissions.microphone_detail")),
            (Permission::Accessibility, crate::shell::permission_name(Permission::Accessibility), t!("settings.permissions.accessibility_detail")),
            (
                Permission::InputMonitoring,
                crate::shell::permission_name(Permission::InputMonitoring),
                t!("settings.permissions.input_monitoring_detail"),
            ),
        ];
        if !crate::shell::HAS_INPUT_PERMISSIONS {
            rows.retain(|(p, _, _)| *p == Permission::Microphone);
        }
        let states: Vec<PermissionState> = rows.iter().map(|(p, _, _)| m.permission(*p)).collect();
        let mut g = group(&t!("settings.permissions.title"), cx);
        if !crate::dev::running_from_bundle() {
            g = g.child(kit::banner(BannerKind::Warning, crate::dev::UNBUNDLED_NOTE, cx));
        }
        for (i, ((perm, title, desc), state)) in rows.into_iter().zip(states).enumerate() {
            let perm2 = perm;
            let mut controls = div().flex().flex_none().items_center().gap(px(12.)).child(kit::permission_badge(state, cx).w(px(84.)));
            if state == PermissionState::NotDetermined && perm != Permission::Accessibility {
                controls = controls.child(Button::new(("perm-request", i), t!("settings.permissions.request")).small().on_click(cx.listener(move |this, _, _, cx| {
                    this.model.update(cx, |m, _| m.request_permission(perm2));
                })));
            }
            controls = controls.child(Button::new(("perm-open", i), crate::shell::open_permission_settings()).small().on_click(cx.listener(
                move |this, _, _, cx| {
                    this.model.update(cx, |m, _| m.open_permission_settings(perm));
                },
            )));
            g = g.child(kit::row(&title, &desc, controls, cx));
        }
        g = g.child(kit::banner(
            BannerKind::Info,
            t!(crate::shell::os_text!("settings.permissions.note_mac", "settings.permissions.note_windows", "settings.permissions.note_other")),
            cx,
        ));
        let subtitle = t!(crate::shell::os_text!("settings.permissions.subtitle_mac", "settings.permissions.subtitle_windows", "settings.permissions.subtitle_other"));
        kit::page("permissions-page", &t!("settings.permissions.title"), &subtitle, kit::body().child(g), cx)
    }
}
