//! Cancel detection for the Esc key: double Esc inside a time window, or a
//! single Esc when the user asks for it.
//!
//! Pure state machine. The caller passes the time of each press, so the tests
//! need no sleeping.

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct EscDetector {
    single: bool,
    window: Duration,
    last: Option<Instant>,
}

impl EscDetector {
    pub fn new(single: bool, window: Duration) -> Self {
        Self { single, window, last: None }
    }

    pub fn configure(&mut self, single: bool, window: Duration) {
        self.single = single;
        self.window = window;
        self.last = None;
    }

    /// Forget a half-finished double press. Call when recording starts or stops.
    pub fn reset(&mut self) {
        self.last = None;
    }

    /// Feed one Esc key-down. Returns true when the press completes a cancel.
    /// Auto-repeat must be filtered out by the caller.
    pub fn press(&mut self, now: Instant) -> bool {
        if self.single {
            self.last = None;
            return true;
        }
        let double = self.last.is_some_and(|prev| now.saturating_duration_since(prev) <= self.window);
        self.last = if double { None } else { Some(now) };
        double
    }
}

/// Take Esc from the app while a dictation records, so the Esc that cancels
/// does not also close a dialog or clear a field. Pure state machine like
/// [`EscDetector`]: the tap feeds it each Esc event and takes the key when it
/// answers true.
#[derive(Debug, Clone, Default)]
pub struct EscTake {
    /// The setting is on.
    on: bool,
    recording: bool,
    /// Esc went down while recording and is not up yet. Its repeats and its
    /// release go to no app either, also after the recording stopped.
    held: bool,
}

impl EscTake {
    pub fn set_on(&mut self, on: bool) {
        self.on = on;
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    /// An Esc that went down while recording is not up yet.
    pub fn is_held(&self) -> bool {
        self.held
    }

    pub fn set_recording(&mut self, recording: bool) {
        self.recording = recording;
        if recording {
            // A release that the tap did not see must not take the next Esc.
            self.held = false;
        }
    }

    /// One Esc event. `plain` is true when no Command, Control, or Option is
    /// down: only a plain Esc cancels, so only a plain Esc is taken. `repeat`
    /// is an auto-repeat of a key that is still down. True: the app must not get it.
    pub fn key(&mut self, down: bool, repeat: bool, plain: bool) -> bool {
        if !down {
            return std::mem::take(&mut self.held);
        }
        if repeat {
            return self.held;
        }
        if self.on && self.recording && plain {
            self.held = true;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Duration = Duration::from_millis(400);

    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    #[test]
    fn two_presses_inside_the_window_cancel() {
        let t0 = Instant::now();
        let mut d = EscDetector::new(false, WINDOW);
        assert!(!d.press(t0));
        assert!(d.press(at(t0, 180)));
    }

    #[test]
    fn slow_second_press_starts_a_new_attempt() {
        let t0 = Instant::now();
        let mut d = EscDetector::new(false, WINDOW);
        assert!(!d.press(t0));
        assert!(!d.press(at(t0, 770)));
        // The slow press became the first press of a new pair.
        assert!(d.press(at(t0, 900)));
    }

    #[test]
    fn the_window_edge_counts() {
        let t0 = Instant::now();
        let mut d = EscDetector::new(false, WINDOW);
        assert!(!d.press(t0));
        assert!(d.press(at(t0, 400)));
    }

    #[test]
    fn a_cancel_consumes_both_presses() {
        let t0 = Instant::now();
        let mut d = EscDetector::new(false, WINDOW);
        assert!(!d.press(t0));
        assert!(d.press(at(t0, 100)));
        // A third press is the start of a new pair, not another cancel.
        assert!(!d.press(at(t0, 200)));
    }

    #[test]
    fn single_mode_cancels_on_every_press() {
        let t0 = Instant::now();
        let mut d = EscDetector::new(true, WINDOW);
        assert!(d.press(t0));
        assert!(d.press(at(t0, 5000)));
    }

    #[test]
    fn reset_forgets_the_first_press() {
        let t0 = Instant::now();
        let mut d = EscDetector::new(false, WINDOW);
        assert!(!d.press(t0));
        d.reset();
        assert!(!d.press(at(t0, 100)));
    }

    #[test]
    fn configure_changes_mode_and_window() {
        let t0 = Instant::now();
        let mut d = EscDetector::new(false, WINDOW);
        d.configure(false, Duration::from_millis(50));
        assert!(!d.press(t0));
        assert!(!d.press(at(t0, 100)));
        d.configure(true, WINDOW);
        assert!(d.press(at(t0, 200)));
    }

    fn taking(on: bool) -> EscTake {
        let mut t = EscTake::default();
        t.set_on(on);
        t.set_recording(true);
        t
    }

    #[test]
    fn esc_while_recording_reaches_no_app() {
        let mut t = taking(true);
        assert!(t.key(true, false, true));
        assert!(t.is_held());
        assert!(t.key(true, true, true), "auto-repeat");
        assert!(t.key(false, false, true), "the release");
        assert!(!t.is_held());
        assert!(t.key(true, false, true), "the second press of a double Esc");
    }

    #[test]
    fn with_the_setting_off_esc_goes_to_the_app() {
        let mut t = taking(false);
        assert!(!t.key(true, false, true));
        assert!(!t.key(false, false, true));
    }

    #[test]
    fn esc_outside_a_recording_goes_to_the_app() {
        let mut t = EscTake::default();
        t.set_on(true);
        assert!(!t.key(true, false, true));
        assert!(!t.key(false, false, true));
    }

    #[test]
    fn a_modified_esc_goes_to_the_app() {
        let mut t = taking(true);
        assert!(!t.key(true, false, false), "Command+Esc and the like");
        assert!(!t.key(false, false, false));
    }

    #[test]
    fn a_held_esc_stays_taken_after_the_cancel() {
        let mut t = taking(true);
        assert!(t.key(true, false, true));
        t.set_recording(false);
        assert!(t.key(true, true, true), "auto-repeat after the stop");
        assert!(t.key(false, false, true));
        assert!(!t.key(true, false, true), "the next press is the app's again");
    }

    #[test]
    fn an_esc_held_from_before_the_recording_goes_to_the_app() {
        let mut t = EscTake::default();
        t.set_on(true);
        assert!(!t.key(true, false, true));
        t.set_recording(true);
        assert!(!t.key(true, true, true), "its auto-repeat");
        assert!(!t.key(false, false, true), "its release");
    }

    #[test]
    fn a_new_recording_forgets_a_lost_release() {
        let mut t = taking(true);
        assert!(t.key(true, false, true));
        t.set_recording(false);
        t.set_recording(true);
        assert!(!t.is_held());
    }
}
