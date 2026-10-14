//! [`MediaControl`] through the MediaRemote adapter (`native/macos/MediaRemoteAdapter`).
//!
//! Since macOS 15.4, the private MediaRemote framework answers only to Apple
//! processes. `/usr/bin/perl` is one, so Sayso runs the script of the adapter
//! with perl, and the script loads the framework of the adapter.
//!
//! macOS has one now-playing app, and the commands go to it. So this pauses
//! one player at most. A run outside the app bundle has no adapter and
//! pauses nothing.

use sayso_platform::{MediaControl, PausedPlayer};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The commands of the adapter (`MRACommand` in `MediaRemoteAdapter.h`).
const PLAY: &str = "0";
const PAUSE: &str = "1";

/// The adapter stops a read after 2 seconds. This limit is for a perl that hangs.
const RUN_LIMIT: Duration = Duration::from_secs(4);

/// The app can report "playing" for a moment after the pause. The resume
/// reads the state again for this long before it decides that the user
/// played the media again.
const RESUME_TRIES: u32 = 5;
const RESUME_WAIT: Duration = Duration::from_millis(200);

pub struct MacMedia {
    adapter: Option<Adapter>,
}

impl MacMedia {
    pub fn new() -> Self {
        let adapter = std::env::current_exe().ok().and_then(|exe| Adapter::in_bundle(&exe));
        if adapter.is_none() {
            log::info!("media: no MediaRemote adapter beside the app, so Sayso cannot pause media");
        }
        Self { adapter }
    }
}

impl Default for MacMedia {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaControl for MacMedia {
    fn pause_playing(&self) -> Vec<PausedPlayer> {
        let Some(adapter) = &self.adapter else { return Vec::new() };
        match adapter.now_playing() {
            Some(now) if now.playing => {
                if adapter.send(PAUSE) {
                    vec![now.player]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    fn resume(&self, players: &[PausedPlayer]) {
        let Some(adapter) = &self.adapter else { return };
        for attempt in 0..RESUME_TRIES {
            if attempt > 0 {
                std::thread::sleep(RESUME_WAIT);
            }
            match adapter.now_playing() {
                Some(now) if players.contains(&now.player) => {
                    if !now.playing {
                        adapter.send(PLAY);
                        return;
                    }
                }
                // Another app plays now, or none: play nothing.
                _ => return,
            }
        }
    }
}

struct Adapter {
    script: PathBuf,
    framework: PathBuf,
}

impl Adapter {
    /// The adapter in the bundle of `exe` (`Sayso.app/Contents/MacOS/Sayso`).
    fn in_bundle(exe: &Path) -> Option<Self> {
        let contents = exe.parent()?.parent()?;
        let script = contents.join("Resources/mediaremote-adapter.pl");
        let framework = contents.join("Frameworks/MediaRemoteAdapter.framework");
        (script.is_file() && framework.is_dir()).then_some(Self { script, framework })
    }

    fn now_playing(&self) -> Option<NowPlaying> {
        let out = self.run(&["get", "--no-artwork", "--allow-missing-title"])?;
        parse_now_playing(&out)
    }

    fn send(&self, command: &str) -> bool {
        self.run(&["send", command]).is_some()
    }

    /// Run one adapter function. The output, or None when it failed.
    fn run(&self, args: &[&str]) -> Option<String> {
        let mut child = Command::new("/usr/bin/perl")
            .arg(&self.script)
            .arg(&self.framework)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| log::warn!("media: cannot start perl: {e}"))
            .ok()?;
        let deadline = Instant::now() + RUN_LIMIT;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    log::warn!("media: the adapter did not answer `{}` in time", args[0]);
                    return None;
                }
                Err(e) => {
                    log::warn!("media: {e}");
                    return None;
                }
            }
        };
        // The output is a few hundred bytes, so the pipes cannot fill before the exit.
        let mut out = String::new();
        let mut err = String::new();
        if let Some(mut s) = child.stdout.take() {
            let _ = s.read_to_string(&mut out);
        }
        if let Some(mut s) = child.stderr.take() {
            let _ = s.read_to_string(&mut err);
        }
        if !status.success() {
            log::warn!("media: the adapter failed `{}`: {}", args[0], err.trim());
            return None;
        }
        Some(out)
    }
}

#[derive(Debug, PartialEq)]
struct NowPlaying {
    player: PausedPlayer,
    playing: bool,
}

/// The output of `get`: a JSON object, or `null` when no app plays media.
fn parse_now_playing(out: &str) -> Option<NowPlaying> {
    let value: serde_json::Value = serde_json::from_str(out.trim()).ok()?;
    let pid = value.get("processIdentifier").and_then(|v| v.as_i64());
    let bundle = value
        .get("bundleIdentifier")
        .or_else(|| value.get("parentApplicationBundleIdentifier"))
        .and_then(|v| v.as_str());
    // The process names the player. The bundle id alone would also match
    // a new process of the same app.
    let player = match (bundle, pid) {
        (Some(b), Some(p)) => format!("{b} {p}"),
        (Some(b), None) => b.to_string(),
        (None, Some(p)) => p.to_string(),
        (None, None) => return None,
    };
    let playing = value.get("playing").and_then(|v| v.as_bool()).unwrap_or(false);
    Some(NowPlaying { player: PausedPlayer(player), playing })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_player_and_its_state() {
        let out = r#"{"title":"Song","bundleIdentifier":"com.spotify.client","playing":true,"processIdentifier":812}"#;
        assert_eq!(
            parse_now_playing(out),
            Some(NowPlaying { player: PausedPlayer("com.spotify.client 812".into()), playing: true })
        );
    }

    #[test]
    fn a_paused_player_is_not_playing() {
        let out = r#"{"bundleIdentifier":"com.apple.Music","playing":false,"processIdentifier":5}"#;
        assert!(!parse_now_playing(out).unwrap().playing);
    }

    #[test]
    fn null_means_no_player() {
        assert_eq!(parse_now_playing("null\n"), None);
        assert_eq!(parse_now_playing(""), None);
        assert_eq!(parse_now_playing(r#"{"playing":true}"#), None);
    }

    #[test]
    fn the_adapter_is_found_only_in_a_bundle() {
        let dir = std::env::temp_dir().join(format!("sayso-media-{}", std::process::id()));
        let contents = dir.join("Sayso.app/Contents");
        std::fs::create_dir_all(contents.join("MacOS")).unwrap();
        let exe = contents.join("MacOS/Sayso");
        assert!(Adapter::in_bundle(&exe).is_none());
        std::fs::create_dir_all(contents.join("Frameworks/MediaRemoteAdapter.framework")).unwrap();
        std::fs::create_dir_all(contents.join("Resources")).unwrap();
        std::fs::write(contents.join("Resources/mediaremote-adapter.pl"), "").unwrap();
        assert!(Adapter::in_bundle(&exe).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
