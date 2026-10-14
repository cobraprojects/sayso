//! [`MediaControl`] through the media sessions of Windows (the media keys
//! and the volume flyout use them too). A browser has one session for each
//! tab that plays.

use parking_lot::Mutex;
use sayso_platform::{MediaControl, PausedPlayer};
use std::sync::atomic::{AtomicU64, Ordering};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};

/// Sessions have no id, and two tabs of one browser have the same app id.
/// So Sayso keeps the sessions that it paused, under a name of its own.
#[derive(Default)]
pub struct WinMedia {
    paused: Mutex<Vec<(String, Session)>>,
    next: AtomicU64,
}

impl MediaControl for WinMedia {
    fn pause_playing(&self) -> Vec<PausedPlayer> {
        let sessions = match sessions() {
            Ok(s) => s,
            Err(e) => {
                log::warn!("media: cannot read the media sessions: {e}");
                return Vec::new();
            }
        };
        let mut paused = Vec::new();
        for session in sessions {
            if !plays(&session) {
                continue;
            }
            if !session.TryPauseAsync().and_then(|op| op.join()).unwrap_or(false) {
                log::warn!("media: {} did not pause", app_id(&session));
                continue;
            }
            let name = format!("{} {}", app_id(&session), self.next.fetch_add(1, Ordering::Relaxed));
            self.paused.lock().push((name.clone(), session));
            paused.push(PausedPlayer(name));
        }
        paused
    }

    fn resume(&self, players: &[PausedPlayer]) {
        let sessions: Vec<Session> = {
            let mut held = self.paused.lock();
            let (resume, keep): (Vec<_>, Vec<_>) =
                std::mem::take(&mut *held).into_iter().partition(|(name, _)| players.iter().any(|p| p.0 == *name));
            *held = keep;
            resume.into_iter().map(|(_, session)| session).collect()
        };
        for session in sessions {
            // A session that the user stopped, or that plays again, stays as it is.
            // A closed session gives an error here.
            if status(&session) != Some(Status::Paused) {
                continue;
            }
            if !session.TryPlayAsync().and_then(|op| op.join()).unwrap_or(false) {
                log::warn!("media: {} did not play", app_id(&session));
            }
        }
    }
}

fn sessions() -> windows::core::Result<Vec<Session>> {
    let manager = Manager::RequestAsync()?.join()?;
    Ok(manager.GetSessions()?.into_iter().collect())
}

fn status(session: &Session) -> Option<Status> {
    session.GetPlaybackInfo().and_then(|info| info.PlaybackStatus()).ok()
}

fn plays(session: &Session) -> bool {
    let can_pause = session.GetPlaybackInfo().and_then(|info| info.Controls()).and_then(|c| c.IsPauseEnabled()).unwrap_or(false);
    status(session) == Some(Status::Playing) && can_pause
}

fn app_id(session: &Session) -> String {
    session.SourceAppUserModelId().map(|id| id.to_string()).unwrap_or_default()
}
