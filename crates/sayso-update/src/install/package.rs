//! The package that holds this Sayso, when one does. Sayso cannot replace
//! such an install, and the download page is not always the place of the
//! new version: an AUR helper or Nix gets it.

#[cfg(any(target_os = "linux", test))]
use std::path::Path;

/// A package that holds this install. `docs/linux.md` says how each one
/// gets a new version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Package {
    /// One file from the download page. The user replaces the file.
    AppImage,
    /// A Flatpak bundle from the download page. A bundle has no update source.
    Flatpak,
    /// The AUR package `sayso-bin`. An AUR helper updates it.
    Aur,
    /// The Nix flake of the repository. Nix updates it.
    Nix,
}

/// The folder where the `.deb`, the `.rpm`, and the AUR package put the binaries.
#[cfg(target_os = "linux")]
const PACKAGE_DIR: &str = "/usr/lib/sayso";

/// A folder that only the AUR package makes (`packaging/linux/aur/PKGBUILD.in`).
/// The `.deb` and the `.rpm` use `/usr/share/licenses/sayso`.
#[cfg(target_os = "linux")]
const AUR_LICENSE_DIR: &str = "/usr/share/licenses/sayso-bin";

impl Package {
    /// The package of the Sayso that runs now. None for an install from the
    /// tarball, the `.deb`, or the `.rpm`: the download page has their new version.
    #[cfg(target_os = "linux")]
    pub fn detect() -> Option<Self> {
        let exe = std::env::current_exe().and_then(|p| p.canonicalize()).ok()?;
        let set = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
        Self::of(&exe, set("FLATPAK_ID").is_some(), set("APPDIR").as_deref().map(Path::new), Path::new(AUR_LICENSE_DIR).is_dir().then_some(Path::new(PACKAGE_DIR)))
    }

    /// `appdir` is the mounted AppImage of the process that started this
    /// one. It counts only when this executable is in it: an app from an
    /// AppImage gives the variable to each program that it starts. `aur_dir`
    /// is the folder of the binaries of the AUR package, when that package
    /// is on the system. `exe` is a canonical path, so the two folders are
    /// made canonical too.
    #[cfg(any(target_os = "linux", test))]
    fn of(exe: &Path, flatpak: bool, appdir: Option<&Path>, aur_dir: Option<&Path>) -> Option<Self> {
        if flatpak {
            return Some(Self::Flatpak);
        }
        if appdir.and_then(|dir| dir.canonicalize().ok()).is_some_and(|dir| exe.starts_with(dir)) {
            return Some(Self::AppImage);
        }
        if exe.starts_with("/nix/store") {
            return Some(Self::Nix);
        }
        let aur_dir = aur_dir.map(|dir| dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf()));
        (aur_dir.is_some() && exe.parent() == aur_dir.as_deref()).then_some(Self::Aur)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_place_of_the_executable_names_the_package() {
        let aur_dir = Some(Path::new("/usr/lib/sayso"));
        let packaged = Path::new("/usr/lib/sayso/sayso");
        let tarball = Path::new("/home/a/.local/lib/sayso/sayso");
        // The tarball, the .deb, and the .rpm: the download page has the new version.
        assert_eq!(Package::of(tarball, false, None, None), None);
        assert_eq!(Package::of(packaged, false, None, None), None);
        assert_eq!(Package::of(packaged, false, None, aur_dir), Some(Package::Aur));
        // The AUR package is on the system, but this Sayso is another install.
        assert_eq!(Package::of(tarball, false, None, aur_dir), None);
        assert_eq!(Package::of(Path::new("/nix/store/abc-sayso-0.7.1/lib/sayso/sayso"), false, None, None), Some(Package::Nix));
        assert_eq!(Package::of(Path::new("/app/bin/sayso"), true, None, None), Some(Package::Flatpak));
    }

    #[test]
    fn an_appimage_counts_only_when_sayso_runs_from_it() {
        let dir = tempfile::tempdir().unwrap();
        let mount = dir.path().canonicalize().unwrap();
        assert_eq!(Package::of(&mount.join("usr/bin/sayso"), false, Some(dir.path()), None), Some(Package::AppImage));
        // An app from an AppImage started a Sayso from a package.
        assert_eq!(Package::of(Path::new("/usr/lib/sayso/sayso"), false, Some(dir.path()), Some(Path::new("/usr/lib/sayso"))), Some(Package::Aur));
        // The mount is gone.
        assert_eq!(Package::of(Path::new("/usr/lib/sayso/sayso"), false, Some(&mount.join("none")), None), None);
    }

    #[cfg(unix)]
    #[test]
    fn the_aur_package_counts_when_its_folder_is_behind_a_symlink() {
        // `/usr/lib` is a symlink on some systems, and the executable is a canonical path.
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().canonicalize().unwrap().join("real");
        std::fs::create_dir_all(real.join("sayso")).unwrap();
        let link = dir.path().join("lib");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert_eq!(Package::of(&real.join("sayso/sayso"), false, None, Some(&link.join("sayso"))), Some(Package::Aur));
    }
}
