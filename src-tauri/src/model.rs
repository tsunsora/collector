//! Types shared with the UI. Field names serialize as camelCase to match
//! `src/lib/types.ts`; change both together.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Ok,
    Loading,
    Offline,
    LoggedOut,
    Error,
    RateLimited,
    /// Twitch's private API has moved on and this build no longer speaks it.
    Changed,
}

/// Something wrong with watching one channel, short of a failure of the whole app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Problem {
    /// Twitch is not accepting the watch reports.
    Reports,
    /// Reports are accepted, but no points have arrived for a long while.
    NoPoints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub live: bool,
    /// Channel-point balance on this channel.
    pub points: u64,
    /// Seconds watched in the current session.
    pub watch_seconds: u64,
    /// 0..1 progress toward the next watch payout. Holds at 1 until the
    /// payout shows up in the balance.
    pub payout: f64,
    /// Points one watch payout is worth here, with the account's multipliers.
    pub payout_points: u64,
    /// A watch streak is available on the current stream and not yet claimed.
    pub streak_due: bool,
    /// Twitch has answered about this channel since it was added or the app
    /// started. Until then `live` is not known.
    pub checked: bool,
    /// `points` is a balance Twitch reported. False until the first answer,
    /// and for a channel that has no channel points.
    pub points_known: bool,
    /// Set while watching this channel is not working as it should.
    pub problem: Option<Problem>,
    /// Address of the channel's profile picture, once Twitch has given it.
    pub avatar: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EventKind {
    Watch,
    Bonus,
    Streak,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub id: String,
    /// Milliseconds since the Unix epoch.
    pub at: u64,
    pub channel: String,
    pub kind: EventKind,
    pub points: u64,
    /// Watch: minutes watched. Streak: streak day.
    pub amount: Option<u32>,
}

/// A channel the signed-in user follows on Twitch, offered for adding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Followed {
    pub login: String,
    /// The name with the channel's own capitalisation.
    pub name: String,
    /// Address of the channel's profile picture.
    pub avatar: Option<String>,
}

/// Where the app is with updating itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateState {
    /// Updates are not set up in this build.
    Off,
    /// Nothing newer was found, or nothing has been looked for yet.
    Idle,
    Checking,
    /// A newer version exists and has not been downloaded.
    Available,
    Downloading,
    /// Downloaded and checked; installs when the app quits or on request.
    Ready,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub state: UpdateState,
    /// The newer version, once one has been found.
    pub version: Option<String>,
    /// What the release says about itself.
    pub notes: Option<String>,
    /// When the feed was last read. Milliseconds since the Unix epoch.
    pub checked_at: Option<u64>,
    /// Why it failed, in words for the user.
    pub message: Option<String>,
}

impl UpdateInfo {
    pub fn in_state(state: UpdateState) -> Self {
        Self { state, version: None, notes: None, checked_at: None, message: None }
    }
}

/// A device code waiting for the user to approve it at Twitch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignIn {
    /// What the user types at `verification_uri`.
    pub user_code: String,
    pub verification_uri: String,
    /// Milliseconds since the Unix epoch.
    pub expires_at: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub status: Status,
    pub paused: bool,
    /// In priority order.
    pub channels: Vec<Channel>,
    /// Channel id per watch slot, `None` for a free slot.
    pub slots: Vec<Option<String>>,
    pub account: Option<String>,
    /// Set while a sign-in code is waiting to be approved.
    pub sign_in: Option<SignIn>,
    /// Seconds until the next attempt while rate-limited.
    pub retry_in: u32,
    pub open_at_login: bool,
    pub start_paused: bool,
    /// The welcome tour has been seen or skipped. False on a fresh start.
    pub welcomed: bool,
    /// This app's version.
    pub version: String,
    pub update: UpdateInfo,
    /// Download updates as they appear, without being asked.
    pub auto_update: bool,
}
