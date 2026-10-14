//! [`MediaControl`] through MPRIS on the session bus.
//!
//! Each player owns a bus name `org.mpris.MediaPlayer2.<name>`. A browser has
//! one for all of its tabs. The Flatpak needs `--talk-name=org.mpris.MediaPlayer2.*`.

use sayso_platform::{MediaControl, PausedPlayer};
use std::sync::OnceLock;
use zbus::blocking::Connection;
use zbus::zvariant::OwnedValue;

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
/// playerctld shows the last active player again under its own name.
/// Sayso talks to that player directly.
const PLAYERCTLD: &str = "org.mpris.MediaPlayer2.playerctld";

#[derive(Default)]
pub struct LinuxMedia {
    bus: OnceLock<Option<Connection>>,
}

impl LinuxMedia {
    fn bus(&self) -> Option<&Connection> {
        self.bus
            .get_or_init(|| Connection::session().map_err(|e| log::warn!("media: no session bus: {e}")).ok())
            .as_ref()
    }
}

impl MediaControl for LinuxMedia {
    fn pause_playing(&self) -> Vec<PausedPlayer> {
        let Some(bus) = self.bus() else { return Vec::new() };
        let mut paused = Vec::new();
        for name in players(bus) {
            if status(bus, &name).as_deref() != Some("Playing") || !property(bus, &name, "CanPause").unwrap_or(false) {
                continue;
            }
            match call(bus, &name, "Pause") {
                Ok(()) => paused.push(PausedPlayer(name)),
                Err(e) => log::warn!("media: {name} did not pause: {e}"),
            }
        }
        paused
    }

    fn resume(&self, players: &[PausedPlayer]) {
        let Some(bus) = self.bus() else { return };
        for PausedPlayer(name) in players {
            // A player that the user stopped, or that plays again, stays as it is.
            if status(bus, name).as_deref() != Some("Paused") {
                continue;
            }
            if let Err(e) = call(bus, name, "Play") {
                log::warn!("media: {name} did not play: {e}");
            }
        }
    }
}

/// The bus names of the players.
fn players(bus: &Connection) -> Vec<String> {
    let names: Vec<String> = bus
        .call_method(Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", Some("org.freedesktop.DBus"), "ListNames", &())
        .and_then(|reply| reply.body().deserialize::<Vec<String>>())
        .map_err(|e| log::warn!("media: cannot list the bus names: {e}"))
        .unwrap_or_default();
    player_names(names)
}

fn player_names(names: Vec<String>) -> Vec<String> {
    names.into_iter().filter(|n| n.starts_with(PREFIX) && n != PLAYERCTLD).collect()
}

fn status(bus: &Connection, name: &str) -> Option<String> {
    property(bus, name, "PlaybackStatus")
}

/// Read a property of the player interface. None when the player does not have it.
fn property<T: TryFrom<OwnedValue>>(bus: &Connection, name: &str, property: &str) -> Option<T> {
    let reply = bus
        .call_method(Some(name), PATH, Some("org.freedesktop.DBus.Properties"), "Get", &(PLAYER, property))
        .ok()?;
    let value: OwnedValue = reply.body().deserialize().ok()?;
    T::try_from(value).ok()
}

fn call(bus: &Connection, name: &str, method: &str) -> zbus::Result<()> {
    bus.call_method(Some(name), PATH, Some(PLAYER), method, &()).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_players_count_and_playerctld_is_skipped() {
        let names = [
            ":1.42",
            "org.freedesktop.Notifications",
            "org.mpris.MediaPlayer2.spotify",
            "org.mpris.MediaPlayer2.firefox.instance_1_23",
            "org.mpris.MediaPlayer2.playerctld",
        ];
        assert_eq!(
            player_names(names.iter().map(|n| n.to_string()).collect()),
            ["org.mpris.MediaPlayer2.spotify", "org.mpris.MediaPlayer2.firefox.instance_1_23"]
        );
    }
}
