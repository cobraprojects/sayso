//! The words and the actions for each update status. Settings › General, the
//! Hub sidebar, and the Popover use them, so the three places agree.

use crate::hub::{Route, SettingsPage};
use crate::model::AppModel;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sayso_i18n::t;
use sayso_ui::ActivePaper;
use sayso_ui::assets::Icon;
use sayso_ui::components::*;
use sayso_ui::paper::PaperStyled;
use sayso_ui::text;
use sayso_update::{Failure, Manual, Package, Status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Accent,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Check,
    Install,
    Cancel,
    Restart,
    DownloadPage,
    ShowApp,
    /// Open Settings › General, where the status has its full text.
    OpenSettings,
}

/// The text of a button. An enum, not a string, so that a status stays `Copy`
/// and the text follows the interface language when it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    /// No button text.
    None,
    CheckForUpdates,
    UpdateNow,
    ShowInFinder,
    HowToUpdate,
    Download,
    Cancel,
    RestartToUpdate,
    OpenDownloadPage,
    TryAgain,
    SeeWhy,
}

impl Label {
    pub fn text(self) -> String {
        match self {
            Label::None => String::new(),
            Label::CheckForUpdates => t!("update.action.check"),
            Label::UpdateNow => t!("update.action.update_now"),
            Label::ShowInFinder => t!("update.action.show_in_finder"),
            Label::HowToUpdate => t!("update.action.how_to_update"),
            Label::Download => t!("update.action.download"),
            Label::Cancel => t!("update.action.cancel"),
            Label::RestartToUpdate => t!("update.action.restart"),
            Label::OpenDownloadPage => t!("update.action.open_download_page"),
            Label::TryAgain => t!("update.action.try_again"),
            Label::SeeWhy => t!("update.action.see_why"),
        }
    }
}

impl From<Label> for SharedString {
    fn from(label: Label) -> Self {
        label.text().into()
    }
}

/// How one status reads.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    /// The row in Settings › General.
    pub title: String,
    pub detail: String,
    pub tone: Tone,
    /// The part of the download that arrived, 0 to 1.
    pub progress: Option<f32>,
    pub notes_url: Option<String>,
    /// The button in Settings.
    pub action: Option<(Label, Action)>,
    /// The line in the sidebar and the Popover, with its action there. None
    /// when the user has nothing to do or to watch.
    pub short: Option<(String, Label, Action)>,
}

pub fn view(status: &Status) -> View {
    let version = sayso_update::VERSION;
    let plain = |title: String, detail: String| View { title, detail, tone: Tone::Plain, progress: None, notes_url: None, action: None, short: None };
    match status {
        Status::Idle { last_check } => View {
            action: Some((Label::CheckForUpdates, Action::Check)),
            ..plain(t!("update.up_to_date"), last_check_detail(*last_check))
        },
        Status::Checking => plain(t!("update.checking"), t!("update.version", version = version)),
        Status::Available { version: new, notes_url, manual } => {
            let title = t!("update.available", version = new);
            let (detail, action, short) = match manual {
                None => (t!("update.you_have", version = version), (Label::UpdateNow, Action::Install), (Label::UpdateNow, Action::Install)),
                Some(Manual::WrongFolder) => (
                    t!("update.wrong_folder"),
                    (Label::ShowInFinder, Action::ShowApp),
                    (Label::HowToUpdate, Action::OpenSettings),
                ),
                Some(Manual::NoPermission) => (
                    t!("update.no_permission", version = version),
                    (Label::Download, Action::DownloadPage),
                    (Label::HowToUpdate, Action::OpenSettings),
                ),
                Some(Manual::NoExchange) => (
                    t!("update.no_exchange", version = version),
                    (Label::Download, Action::DownloadPage),
                    (Label::HowToUpdate, Action::OpenSettings),
                ),
                Some(Manual::Unsupported) => (t!("update.unsupported", version = version), (Label::Download, Action::DownloadPage), (Label::Download, Action::DownloadPage)),
                Some(Manual::Packaged(package)) => return packaged(*package, title, notes_url),
            };
            View { tone: Tone::Accent, notes_url: Some(notes_url.clone()), action: Some(action), short: Some((title.clone(), short.0, short.1)), ..plain(title, detail) }
        }
        Status::Downloading { version: new, received, total } => View {
            progress: Some(if *total == 0 { 0. } else { *received as f32 / *total as f32 }),
            action: Some((Label::Cancel, Action::Cancel)),
            short: Some((t!("update.downloading_short", version = new), Label::None, Action::OpenSettings)),
            ..plain(
                t!("update.downloading", version = new),
                t!("update.downloaded", received = megabytes(*received), total = megabytes(*total)),
            )
        },
        Status::Preparing { version: new } => View {
            short: Some((t!("update.preparing_short", version = new), Label::None, Action::OpenSettings)),
            ..plain(t!("update.preparing", version = new), t!("update.preparing_detail"))
        },
        Status::Ready { version: new } => View {
            tone: Tone::Accent,
            action: Some((Label::RestartToUpdate, Action::Restart)),
            short: Some((t!("update.ready_short", version = new), Label::RestartToUpdate, Action::Restart)),
            ..plain(t!("update.ready", version = new), t!("update.ready_detail"))
        },
        Status::Unreadable => View {
            tone: Tone::Accent,
            action: Some((Label::OpenDownloadPage, Action::DownloadPage)),
            short: Some((t!("update.newer_available"), Label::Download, Action::DownloadPage)),
            ..plain(t!("update.newer_available"), t!("update.unreadable_detail", version = version))
        },
        Status::Failed { failure } => View {
            tone: Tone::Danger,
            action: Some((Label::TryAgain, Action::Check)),
            short: Some((t!("update.failed"), Label::SeeWhy, Action::OpenSettings)),
            ..plain(t!("update.failed"), failure_text(failure))
        },
    }
}

/// Why an update did not succeed, in the interface language. A detail from
/// the system stays as it is.
fn failure_text(failure: &Failure) -> String {
    match failure {
        Failure::Unknown => t!("update.failure.unknown"),
        // A record from an older Sayso has only the English text.
        Failure::Recorded(text) | Failure::Verify(text) => text.clone(),
        Failure::Busy => t!("update.failure.busy"),
        Failure::CannotUpdateItself => t!("update.failure.cannot_update_itself"),
        Failure::NothingStaged => t!("update.failure.nothing_staged"),
        Failure::NotInstalled => t!("update.failure.not_installed"),
        Failure::BadSignature => t!("update.failure.bad_signature"),
        Failure::Moved => t!("update.failure.moved"),
        Failure::NotNewer => t!("update.failure.not_newer"),
        Failure::StagingGone => t!("update.failure.staging_gone"),
        Failure::WriteRecord(error) => t!("update.failure.write_record", error = error),
        Failure::Exchange(error) => t!("update.failure.exchange", error = error),
        Failure::Relaunch(error) => t!("update.failure.relaunch", error = error),
        Failure::RelaunchAndRestore { error, back } => t!("update.failure.relaunch_and_restore", error = error, back = back),
        Failure::Unreachable(error) => t!("update.failure.unreachable", error = error),
        Failure::CheckFailed(error) => t!("update.failure.check_failed", error = error),
        Failure::NotAccepted(error) => t!("update.failure.not_accepted", error = error),
        Failure::Replayed(version) => t!("update.failure.replayed", version = version),
        Failure::Mismatch => t!("update.failure.mismatch"),
        Failure::Disk(error) => t!("update.failure.disk", error = error),
        Failure::Download(error) => t!("update.failure.download", error = error),
        Failure::Prepare(error) => t!("update.failure.prepare", error = error),
    }
}

/// A newer version for an install that a package holds. The download page
/// has the new AppImage and the new Flatpak file. An AUR helper and Nix get
/// the new version from their own source, which can have it later than the
/// release, so those two get no button.
fn packaged(package: Package, title: String, notes_url: &str) -> View {
    let version = sayso_update::VERSION;
    let download = Some((Label::Download, Action::DownloadPage));
    let (detail, action) = match package {
        Package::AppImage => (t!("update.packaged_appimage", version = version), download),
        // The scope must be the scope of this install: a file that goes to
        // the system does not replace an install of the user.
        Package::Flatpak => (t!("update.packaged_flatpak", version = version), download),
        Package::Aur => (t!("update.packaged_aur", version = version), None),
        // Sayso cannot tell a system configuration from `nix run` or a profile.
        Package::Nix => (t!("update.packaged_nix", version = version), None),
    };
    let short = action.unwrap_or((Label::HowToUpdate, Action::OpenSettings));
    View { title: title.clone(), detail, tone: Tone::Accent, progress: None, notes_url: Some(notes_url.to_string()), action, short: Some((title, short.0, short.1)) }
}

/// Do what a button for `action` says.
pub fn perform(action: Action, model: &Entity<AppModel>, cx: &mut App) {
    match action {
        Action::Check => model.read(cx).check_for_updates(),
        Action::Install => model.read(cx).install_update(),
        Action::Cancel => model.read(cx).cancel_update(),
        Action::Restart => model.update(cx, |m, cx| m.restart_to_update(cx)),
        Action::DownloadPage => crate::shell::open(sayso_update::DOWNLOAD_PAGE),
        Action::ShowApp => {
            if let Ok(exe) = std::env::current_exe() {
                // The bundle, when Sayso runs from one.
                let app = exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).unwrap_or(&exe);
                crate::shell::reveal(app);
            }
        }
        Action::OpenSettings => model.update(cx, |m, cx| m.open_hub(Route::Settings(SettingsPage::General), cx)),
    }
}

fn megabytes(bytes: u64) -> String {
    t!("update.megabytes", number = (bytes as f64 / 1_000_000.).round() as u64)
}

/// "Version 0.7.1. Last check today at 9:41.", in local time.
fn last_check_detail(last_check: Option<u64>) -> String {
    use chrono::{Local, TimeZone};
    let version = sayso_update::VERSION;
    let Some(unix_seconds) = last_check else {
        return t!("update.never_checked", version = version);
    };
    let Some(at) = Local.timestamp_opt(unix_seconds as i64, 0).single() else {
        return t!("update.last_check_unknown", version = version);
    };
    let time = at.format("%-H:%M").to_string();
    if at.date_naive() == Local::now().date_naive() {
        t!("update.last_check_today", version = version, time = time)
    } else {
        t!("update.last_check_date", version = version, date = sayso_i18n::dates::day_month_short(&at), time = time)
    }
}

/// The page of a release on GitHub.
pub fn release_notes(version: &sayso_update::manifest::Version) -> String {
    format!("{}tag/v{version}", sayso_update::manifest::URL_PREFIX)
}

/// The slip in the Hub sidebar, below the engine status.
/// The slip on the rail: one square button. The text is in the tooltip.
pub fn rail_slip(model: &Entity<AppModel>, cx: &App) -> Option<AnyElement> {
    let c = cx.paper().colors;
    let m = model.read(cx);
    let v = view(&m.update);
    let (headline, label, action) = v.short?;
    let can_restart = m.can_restart_to_update();
    let model = model.clone();
    let slip = div()
        .id("update-slip")
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(36.))
        .rounded(px(9.))
        .tooltip(text_tooltip(t!("update.tooltip", headline = headline, label = label.text())));
    Some(if action == Action::Restart {
        slip.ink_button(&c)
            .when(can_restart, |d| d.cursor_pointer().on_click(move |_, _, cx| perform(Action::Restart, &model, cx)))
            .when(!can_restart, |d| d.opacity(0.6))
            .child(icon(Icon::Download, 16., c.on_ink))
            .into_any_element()
    } else {
        let fg = if v.tone == Tone::Danger { c.danger } else { c.ink };
        slip.raised_small(&c)
            .cursor_pointer()
            .on_click(move |_, _, cx| perform(Action::OpenSettings, &model, cx))
            .child(icon(Icon::Download, 16., fg))
            .into_any_element()
    })
}

pub fn sidebar_slip(model: &Entity<AppModel>, cx: &App) -> Option<AnyElement> {
    let c = cx.paper().colors;
    let m = model.read(cx);
    let v = view(&m.update);
    let (headline, label, action) = v.short?;
    let can_restart = m.can_restart_to_update();
    let model = model.clone();
    let slip = div().id("update-slip").flex().w_full().rounded(px(10.)).py(px(9.)).pl(px(10.)).pr(px(8.));
    // Ready: the slip is the button.
    if action == Action::Restart {
        return Some(
            slip.items_center()
                .gap(px(10.))
                .pl(px(12.))
                .ink_button(&c)
                .when(can_restart, |d| d.cursor_pointer().on_click(move |_, _, cx| perform(Action::Restart, &model, cx)))
                .when(!can_restart, |d| d.opacity(0.6))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(1.))
                        .child(text::ui(label.text(), 13., FontWeight::SEMIBOLD, c.on_ink))
                        .child(text::ui(headline, 12., FontWeight::NORMAL, c.on_ink.opacity(0.7))),
                )
                .into_any_element(),
        );
    }
    let slip = slip.raised_small(&c).cursor_pointer().on_click(move |_, _, cx| perform(Action::OpenSettings, &model, cx));
    if let Some(progress) = v.progress {
        return Some(
            slip.flex_col()
                .gap(px(7.))
                .pr(px(10.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(text::ui(headline, 13., FontWeight::SEMIBOLD, c.ink).flex_1().truncate())
                        .child(text::mono(format!("{}%", (progress * 100.) as u32), 12., c.graphite).font_weight(FontWeight::NORMAL)),
                )
                .child(Progress::new(progress).height(4.))
                .into_any_element(),
        );
    }
    let (title, detail) = match &m.update {
        Status::Available { version, .. } => (t!("update.slip_title"), t!("update.slip_version", version = version)),
        Status::Unreadable => (t!("update.slip_title"), t!("update.slip_newer")),
        Status::Failed { .. } => (headline, label.text()),
        _ => (headline, String::new()),
    };
    let dot = match v.tone {
        Tone::Danger => Some(c.danger),
        Tone::Accent => Some(c.accent),
        Tone::Plain => None,
    };
    Some(
        slip.items_center()
            .gap(px(10.))
            .when_some(dot, |d, color| d.child(status_dot(color, 8.)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(1.))
                    .child(text::ui(title, 13., FontWeight::SEMIBOLD, c.ink).truncate())
                    .when(!detail.is_empty(), |d| d.child(text::ui(detail, 12., FontWeight::NORMAL, c.graphite))),
            )
            .child(icon(Icon::ChevronRight, 12., c.pencil))
            .into_any_element(),
    )
}

/// The line in the Popover, above the footer. `hide` closes the Popover.
pub fn popover_line(model: &Entity<AppModel>, hide: fn(&mut App), cx: &App) -> Option<AnyElement> {
    let c = cx.paper().colors;
    let m = model.read(cx);
    let line = || div().flex().flex_1().items_center().gap(px(10.)).h(px(38.)).pl(px(12.)).pr(px(4.)).rounded(px(9.));
    let wrap = |inner: Div| div().flex().px(px(4.)).pt(px(8.)).pb(px(6.)).child(inner).into_any_element();
    let button = |id: &'static str, label: String, color: Hsla| {
        div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .h(px(30.))
            .px(px(10.))
            .rounded(px(6.))
            .cursor_pointer()
            .hover(move |s| s.bg(color.opacity(0.08)))
            .child(text::ui(label, 13., FontWeight::SEMIBOLD, color))
    };
    let Some((headline, label, action)) = view(&m.update).short else {
        // Nothing to do. Say one time that an update happened.
        let version = m.update_notice.clone()?;
        let notes = release_notes(&version);
        let model = model.clone();
        return Some(wrap(
            line()
                .raised_small(&c)
                .pr(px(6.))
                .child(icon(Icon::Check, 14., c.graphite))
                .child(text::ui(t!("update.now_version", version = version), 13., FontWeight::MEDIUM, c.ink).flex_1().truncate())
                .child(button("update-notes", t!("update.whats_new"), c.accent).on_click({
                    let model = model.clone();
                    move |_, _, cx| {
                        // The notes in the app, when this version has some.
                        if crate::whats_new::current().is_some() {
                            hide(cx);
                            crate::whats_new::open(&model, cx);
                        } else {
                            crate::shell::open(&notes);
                        }
                    }
                }))
                .child(
                    div()
                        .id("update-notice-dismiss")
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(26.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .hover(|s| s.bg(c.deboss.opacity(0.6)))
                        .on_click(move |_, _, cx| model.update(cx, |m, cx| m.dismiss_update_notice(cx)))
                        .child(icon(Icon::Close, 10., c.pencil)),
                ),
        ));
    };
    let v = view(&m.update);
    if let Some(progress) = v.progress {
        return Some(wrap(
            line()
                .debossed(&c)
                .pr(px(12.))
                .child(text::ui(headline, 13., FontWeight::MEDIUM, c.ink).flex_1().truncate())
                .child(div().flex_none().w(px(96.)).child(Progress::new(progress).height(4.)))
                .child(text::mono(format!("{}%", (progress * 100.) as u32), 12., c.graphite).font_weight(FontWeight::NORMAL)),
        ));
    }
    if label == Label::None {
        return Some(wrap(line().debossed(&c).pr(px(12.)).child(text::ui(headline, 13., FontWeight::MEDIUM, c.ink).flex_1().truncate())));
    }
    let (wash, ink, strong) = match v.tone {
        Tone::Danger => (c.danger_wash, c.danger, c.danger),
        _ => (c.accent_wash, c.ink, c.accent),
    };
    let off = action == Action::Restart && !m.can_restart_to_update();
    let model = model.clone();
    Some(wrap(
        line()
            .bg(wash)
            .border_1()
            .border_color(strong.opacity(0.14))
            .child(div().flex_none().size(px(7.)).rounded_full().bg(strong))
            .child(text::ui(headline, 13., FontWeight::MEDIUM, ink).flex_1().truncate())
            .child(button("update-action", label.text(), strong).when(off, |d| d.opacity(0.5)).on_click(move |_, _, cx| {
                if off {
                    return;
                }
                // The download stays in view. Every other action leaves the Popover.
                if action != Action::Install {
                    hide(cx);
                }
                perform(action, &model, cx);
            })),
    ))
}

#[cfg(test)]
mod tests {
    // Not `super::*`: that brings in the `test` attribute of GPUI.
    use super::{Action, Label, view};
    use sayso_update::{Manual, Package, Status};

    fn available(manual: Option<Manual>) -> Status {
        Status::Available { version: "9.9.9".parse().unwrap(), notes_url: "https://example.com/notes".into(), manual }
    }

    #[test]
    fn the_user_sees_a_line_only_when_there_is_something_to_do_or_watch() {
        assert!(view(&Status::Idle { last_check: None }).short.is_none());
        assert!(view(&Status::Checking).short.is_none());
        for status in [
            available(None),
            Status::Downloading { version: "9.9.9".parse().unwrap(), received: 1, total: 2 },
            Status::Preparing { version: "9.9.9".parse().unwrap() },
            Status::Ready { version: "9.9.9".parse().unwrap() },
            Status::Unreadable,
            Status::Failed { failure: sayso_update::Failure::Busy },
        ] {
            assert!(view(&status).short.is_some(), "{status:?}");
        }
    }

    #[test]
    fn an_install_that_can_update_itself_offers_update_now() {
        let v = view(&available(None));
        assert_eq!(v.title, "Sayso 9.9.9 is available");
        assert_eq!(v.detail, format!("You have {}.", sayso_update::VERSION));
        assert_eq!(v.action, Some((Label::UpdateNow, Action::Install)));
        assert_eq!(Label::UpdateNow.text(), "Update now");
        assert_eq!(v.notes_url.as_deref(), Some("https://example.com/notes"));
    }

    #[test]
    fn an_install_that_cannot_update_itself_never_offers_update_now() {
        let packaged = [Package::AppImage, Package::Flatpak, Package::Aur, Package::Nix].map(Manual::Packaged);
        for manual in [Manual::Unsupported, Manual::WrongFolder, Manual::NoPermission, Manual::NoExchange].into_iter().chain(packaged) {
            let v = view(&available(Some(manual)));
            assert!(!matches!(v.action, Some((_, Action::Install))), "{manual:?}");
            assert!(!matches!(v.short, Some((_, _, Action::Install))), "{manual:?}");
        }
    }

    #[test]
    fn a_package_sends_the_user_to_the_place_of_its_new_version() {
        // The download page has the new file of these two.
        for package in [Package::AppImage, Package::Flatpak] {
            let v = view(&available(Some(Manual::Packaged(package))));
            assert_eq!(v.action, Some((Label::Download, Action::DownloadPage)), "{package:?}");
            assert!(matches!(v.short, Some((_, Label::Download, Action::DownloadPage))), "{package:?}");
        }
        // A Flatpak file without the scope of the install goes to the wrong place.
        assert!(view(&available(Some(Manual::Packaged(Package::Flatpak)))).detail.contains("flatpak install --user"));
        // An AUR helper and Nix get the new version. The download page is the wrong place.
        for (package, command) in [(Package::Aur, "yay -S sayso-bin"), (Package::Nix, "--refresh")] {
            let v = view(&available(Some(Manual::Packaged(package))));
            assert_eq!(v.title, "Sayso 9.9.9 is available");
            assert!(v.detail.contains(command), "{}", v.detail);
            assert_eq!(v.action, None, "{package:?}");
            assert!(matches!(v.short, Some((_, Label::HowToUpdate, Action::OpenSettings))), "{package:?}");
            assert_eq!(v.notes_url.as_deref(), Some("https://example.com/notes"));
        }
    }

    #[test]
    fn the_download_shows_its_part_and_its_size() {
        let v = view(&Status::Downloading { version: "9.9.9".parse().unwrap(), received: 18_000_000, total: 41_000_000 });
        assert_eq!(v.detail, "18 MB of 41 MB");
        assert!((v.progress.unwrap() - 18. / 41.).abs() < 0.001);
        assert_eq!(v.action, Some((Label::Cancel, Action::Cancel)));
    }
}
