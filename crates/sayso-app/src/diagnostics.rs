//! The system part of the diagnostics of a dictation.

use sayso_core::history::SystemInfo;
use std::sync::OnceLock;

/// The system that Sayso runs on. The first call can read a file or start a
/// program (`sw_vers` on macOS), so call it from a background thread.
pub fn system() -> SystemInfo {
    static INFO: OnceLock<SystemInfo> = OnceLock::new();
    INFO.get_or_init(|| {
        let mut info = SystemInfo {
            os: std::env::consts::OS.into(),
            os_version: os_version(),
            arch: std::env::consts::ARCH.into(),
            ..Default::default()
        };
        add_session(&mut info);
        info
    })
    .clone()
}

#[cfg(target_os = "macos")]
fn os_version() -> Option<String> {
    let output = std::process::Command::new("/usr/bin/sw_vers").arg("-productVersion").output().ok()?;
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string()).filter(|version| !version.is_empty())
}

/// The name of the distribution. In a Flatpak, `/etc/os-release` is the file
/// of the runtime, and the file of the host is in `/run/host`.
#[cfg(target_os = "linux")]
fn os_version() -> Option<String> {
    ["/run/host/os-release", "/etc/os-release"]
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| pretty_name(&text))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn os_version() -> Option<String> {
    None
}

/// The `PRETTY_NAME` of an `os-release` file.
#[cfg(any(target_os = "linux", test))]
fn pretty_name(os_release: &str) -> Option<String> {
    let value = os_release.lines().find_map(|line| line.strip_prefix("PRETTY_NAME="))?;
    Some(value.trim().trim_matches('"').to_string()).filter(|name| !name.is_empty())
}

#[cfg(target_os = "linux")]
fn add_session(info: &mut SystemInfo) {
    let session = sayso_platform_linux::session::current();
    info.session = Some(format!("{:?}", session.kind).to_lowercase());
    info.ui = Some(format!("{:?}", session.ui).to_lowercase());
    info.desktop = Some(format!("{:?}", session.desktop).to_lowercase());
    info.package = sayso_update::Package::detect().map(|package| format!("{package:?}").to_lowercase());
}

#[cfg(not(target_os = "linux"))]
fn add_session(_info: &mut SystemInfo) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_name_of_the_distribution() {
        let file = "NAME=\"Ubuntu\"\nPRETTY_NAME=\"Ubuntu 26.04 LTS\"\nID=ubuntu\n";
        assert_eq!(pretty_name(file).as_deref(), Some("Ubuntu 26.04 LTS"));
        assert_eq!(pretty_name("PRETTY_NAME=Arch Linux\n").as_deref(), Some("Arch Linux"));
        assert_eq!(pretty_name("ID=nixos\n"), None);
    }

    #[test]
    fn the_system_has_a_name_and_an_architecture() {
        let info = system();
        assert_eq!((info.os.as_str(), info.arch.as_str()), (std::env::consts::OS, std::env::consts::ARCH));
    }
}
