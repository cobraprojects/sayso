//! Pause the media of other apps while Sayso records (`audio.pause_media`).
//!
//! The calls to the system can wait on other processes, so one worker thread
//! makes them, in the order of the requests. A resume that comes before its
//! pause is complete runs after it, so a short dictation cannot leave a
//! player paused.

use crossbeam_channel::Sender;
use sayso_platform::{MediaControl, PausedPlayer};
use std::sync::Arc;

enum Request {
    Pause,
    Resume,
}

pub struct MediaPause {
    tx: Sender<Request>,
}

impl MediaPause {
    pub fn new(media: Arc<dyn MediaControl>) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded::<Request>();
        let spawned = std::thread::Builder::new().name("sayso-media".into()).spawn(move || {
            let mut held = Held::default();
            for request in rx {
                held.handle(request, &*media);
            }
        });
        if let Err(err) = spawned {
            log::error!("cannot start the media thread: {err}");
        }
        Self { tx }
    }

    /// Pause each player that plays now.
    pub fn pause(&self) {
        let _ = self.tx.send(Request::Pause);
    }

    /// Play again the players that Sayso paused. With none, nothing happens.
    pub fn resume(&self) {
        let _ = self.tx.send(Request::Resume);
    }
}

/// The players that Sayso paused and did not play again yet.
#[derive(Default)]
struct Held(Vec<PausedPlayer>);

impl Held {
    fn handle(&mut self, request: Request, media: &dyn MediaControl) {
        match request {
            Request::Pause => {
                for player in media.pause_playing() {
                    if !self.0.contains(&player) {
                        self.0.push(player);
                    }
                }
            }
            Request::Resume => {
                let players = std::mem::take(&mut self.0);
                if !players.is_empty() {
                    media.resume(&players);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;

    /// Players by name, with true for a player that plays.
    #[derive(Default)]
    struct FakeMedia {
        players: Mutex<Vec<(String, bool)>>,
        resumed: Mutex<Vec<Vec<PausedPlayer>>>,
    }

    impl FakeMedia {
        fn with(players: &[(&str, bool)]) -> Self {
            Self { players: Mutex::new(players.iter().map(|(n, p)| (n.to_string(), *p)).collect()), ..Default::default() }
        }
        fn playing(&self) -> Vec<String> {
            self.players.lock().iter().filter(|(_, p)| *p).map(|(n, _)| n.clone()).collect()
        }
    }

    impl MediaControl for FakeMedia {
        fn pause_playing(&self) -> Vec<PausedPlayer> {
            let mut paused = Vec::new();
            for (name, playing) in self.players.lock().iter_mut() {
                if *playing {
                    *playing = false;
                    paused.push(PausedPlayer(name.clone()));
                }
            }
            paused
        }
        fn resume(&self, players: &[PausedPlayer]) {
            for (name, playing) in self.players.lock().iter_mut() {
                if players.iter().any(|p| p.0 == *name) {
                    *playing = true;
                }
            }
            self.resumed.lock().push(players.to_vec());
        }
    }

    #[test]
    fn resume_plays_only_the_players_that_the_pause_stopped() {
        let media = FakeMedia::with(&[("music", true), ("video", false)]);
        let mut held = Held::default();
        held.handle(Request::Pause, &media);
        assert!(media.playing().is_empty());
        held.handle(Request::Resume, &media);
        assert_eq!(media.playing(), ["music"]);
    }

    #[test]
    fn a_resume_with_nothing_paused_calls_nothing() {
        let media = FakeMedia::with(&[("video", false)]);
        let mut held = Held::default();
        held.handle(Request::Pause, &media);
        held.handle(Request::Resume, &media);
        held.handle(Request::Resume, &media);
        assert!(media.resumed.lock().is_empty());
        assert!(media.playing().is_empty());
    }

    #[test]
    fn a_second_pause_keeps_the_players_of_the_first() {
        let media = FakeMedia::with(&[("music", true), ("podcast", false)]);
        let mut held = Held::default();
        held.handle(Request::Pause, &media);
        // The user starts the podcast during the recording.
        media.players.lock()[1].1 = true;
        held.handle(Request::Pause, &media);
        held.handle(Request::Resume, &media);
        assert_eq!(media.playing(), ["music", "podcast"]);
        assert_eq!(media.resumed.lock().len(), 1);
    }

    #[test]
    fn the_worker_runs_a_quick_resume_after_its_pause() {
        let media = Arc::new(FakeMedia::with(&[("music", true)]));
        let pause = MediaPause::new(media.clone());
        pause.pause();
        pause.resume();
        drop(pause);
        // The worker ends when the sender is gone. Wait for it to drain.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while media.resumed.lock().is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(media.playing(), ["music"]);
    }
}
