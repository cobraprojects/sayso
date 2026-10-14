//! Why a check or an update did not succeed.
//!
//! `Display` gives the English text for the log and the update record. The
//! app shows the text of the interface language. A detail in a variant is
//! the text of the system or of a lower step, and stays as it is.

use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum Failure {
    /// A failed record without a cause.
    #[error("The update failed.")]
    Unknown,
    /// A record from a version of Sayso before [`Failure`]: only its English text.
    #[error("{0}")]
    Recorded(String),
    /// Another Sayso runs from this install.
    #[error("Another Sayso runs from this app. Quit it, then try again.")]
    Busy,
    #[error("This install cannot update itself.")]
    CannotUpdateItself,
    #[error("No update is ready to install.")]
    NothingStaged,
    /// The exchange did not happen, or it was put back.
    #[error("Sayso could not install the update. You still have the version from before.")]
    NotInstalled,
    #[error("The update record does not have a signature of Sayso.")]
    BadSignature,
    #[error("Sayso moved to another folder after the update was prepared.")]
    Moved,
    #[error("The prepared update is not newer than this version.")]
    NotNewer,
    #[error("The prepared update is gone.")]
    StagingGone,
    /// The staged app did not pass the check of the system.
    #[error("{0}")]
    Verify(String),
    #[error("Sayso could not write the update record: {0}.")]
    WriteRecord(String),
    #[error("Sayso could not replace the app: {0}")]
    Exchange(String),
    #[error("Sayso could not start the new version: {0}. You still have the version from before.")]
    Relaunch(String),
    #[error("Sayso could not start the new version: {error}. It could not put the old version back: {back}. Install Sayso again from the download page.")]
    RelaunchAndRestore { error: String, back: String },
    #[error("Sayso could not reach the update server. Check your connection, then try again. ({0})")]
    Unreachable(String),
    #[error("Sayso could not get the update information: {0}. Try again later.")]
    CheckFailed(String),
    #[error("Sayso did not accept the update information: {0}.")]
    NotAccepted(String),
    /// The server offered a version older than one that Sayso saw before.
    #[error("The update server offered version {0}, which is older than a version that Sayso saw before. Try again later.")]
    Replayed(Version),
    #[error("The file that Sayso downloaded is not the file of the release. Try again later.")]
    Mismatch,
    #[error("Sayso could not write the file: {0}. Make sure that the disk has free space, then try again.")]
    Disk(String),
    #[error("Sayso could not download the update: {0}. Check your connection, then try again.")]
    Download(String),
    #[error("Sayso could not prepare the update: {0}.")]
    Prepare(String),
}
