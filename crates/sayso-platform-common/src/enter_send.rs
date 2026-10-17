//! Send with Enter: while a dictation records, Enter stops it, and Sayso
//! presses Enter after the insert.
//!
//! The key must not reach the app: the app would send its message before the
//! text is in. So the keyboard tap takes the key while it records, and the
//! repeats and the release of that key after. Apart from its own hotkeys, Sayso
//! takes only this key and Esc (see `esc::EscTake`). Pure state machine: the tap feeds it each
//! Return or keypad Enter event and does what it answers.

/// What the tap does with one Enter event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnterDecision {
    /// Take the key: the app does not get it.
    pub swallow: bool,
    /// Stop the dictation and send it.
    pub submit: bool,
}

#[derive(Debug, Clone, Default)]
pub struct EnterSend {
    /// The setting is on.
    on: bool,
    recording: bool,
    /// Enter went down while recording and is not up yet. Its repeats and its
    /// release go to no app either, also after the recording stopped.
    held: bool,
    /// This recording sent its submit already.
    sent: bool,
}

impl EnterSend {
    pub fn set_on(&mut self, on: bool) {
        self.on = on;
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    /// An Enter that went down while recording is not up yet.
    pub fn is_held(&self) -> bool {
        self.held
    }

    pub fn set_recording(&mut self, recording: bool) {
        self.recording = recording;
        if recording {
            // A release that the tap did not see must not take the next Enter.
            self.held = false;
            self.sent = false;
        }
    }

    /// One Return or keypad Enter event, with any modifiers: a push-to-talk
    /// key such as Right Option is often down at the same time. `repeat` is
    /// an auto-repeat of a key that is still down.
    pub fn key(&mut self, down: bool, repeat: bool) -> EnterDecision {
        if !down {
            return EnterDecision { swallow: std::mem::take(&mut self.held), submit: false };
        }
        if repeat {
            // The repeats of an Enter that Sayso took stay Sayso's, also after
            // the recording stopped. An Enter held from before goes on to the app.
            return EnterDecision { swallow: self.held, submit: false };
        }
        if self.on && self.recording {
            self.held = true;
            let submit = !std::mem::replace(&mut self.sent, true);
            return EnterDecision { swallow: true, submit };
        }
        EnterDecision::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASS: EnterDecision = EnterDecision { swallow: false, submit: false };
    const TAKE: EnterDecision = EnterDecision { swallow: true, submit: false };
    const SUBMIT: EnterDecision = EnterDecision { swallow: true, submit: true };

    fn recording(on: bool) -> EnterSend {
        let mut e = EnterSend::default();
        e.set_on(on);
        e.set_recording(true);
        e
    }

    #[test]
    fn enter_while_recording_submits_once_and_reaches_no_app() {
        let mut e = recording(true);
        assert_eq!(e.key(true, false), SUBMIT);
        assert!(e.is_held());
        assert_eq!(e.key(true, true), TAKE, "auto-repeat");
        assert_eq!(e.key(false, false), TAKE, "the release");
        assert!(!e.is_held());
        assert_eq!(e.key(true, false), TAKE, "a second press in the same recording");
    }

    #[test]
    fn with_the_setting_off_enter_goes_to_the_app() {
        let mut e = recording(false);
        assert_eq!(e.key(true, false), PASS);
        assert_eq!(e.key(false, false), PASS);
    }

    #[test]
    fn enter_outside_a_recording_goes_to_the_app() {
        let mut e = EnterSend::default();
        e.set_on(true);
        assert_eq!(e.key(true, false), PASS);
        assert_eq!(e.key(false, false), PASS);
    }

    #[test]
    fn a_held_enter_stays_taken_after_the_recording_stops() {
        let mut e = recording(true);
        assert_eq!(e.key(true, false), SUBMIT);
        e.set_recording(false);
        assert_eq!(e.key(true, true), TAKE, "auto-repeat after the stop");
        assert_eq!(e.key(false, false), TAKE);
        assert_eq!(e.key(true, false), PASS, "the next press is the app's again");
    }

    #[test]
    fn an_enter_held_from_before_the_recording_does_not_submit() {
        let mut e = EnterSend::default();
        e.set_on(true);
        assert_eq!(e.key(true, false), PASS);
        e.set_recording(true);
        assert_eq!(e.key(true, true), PASS, "its auto-repeat");
        assert_eq!(e.key(false, false), PASS, "its release");
        assert_eq!(e.key(true, false), SUBMIT, "a new press");
    }

    #[test]
    fn a_new_recording_can_submit_again_and_forgets_a_lost_release() {
        let mut e = recording(true);
        assert_eq!(e.key(true, false), SUBMIT);
        e.set_recording(false);
        e.set_recording(true);
        assert_eq!(e.key(true, false), SUBMIT);
    }
}
