//! The collector's state and the operations the UI can ask for. No I/O here:
//! `twitch::engine` talks to Twitch and feeds results in; `demo` fakes it.

use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::model::{
    ActivityEvent, Channel, EventKind, Problem, SignIn, Snapshot, Status, UpdateInfo, UpdateState,
};

pub(crate) const SLOT_COUNT: usize = 2;
/// Twitch pays watch points every five minutes of watching.
pub(crate) const PAYOUT_SECONDS: f64 = 300.0;
/// The watch payout before any multiplier.
pub(crate) const PAYOUT_POINTS: u64 = 10;
/// A bonus is five watch payouts.
const BONUS_FACTOR: u64 = 5;
/// The smallest watch-streak reward.
const STREAK_MIN_POINTS: u64 = 300;
pub(crate) const RETRY_SECONDS: u32 = 40;

pub(crate) const MINUTE: u64 = 60_000;
pub(crate) const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

/// A streak can be earned on a new stream if the channel was offline at least this long.
const STREAK_OFFLINE_GAP: u64 = 30 * MINUTE;
/// Past this much watching, the streak has either been paid or is not coming.
const STREAK_WATCH_SECONDS: u64 = 7 * 60;
/// A channel counts as being watched if its last report is this recent.
const PING_FRESH: u64 = MINUTE;
/// Points that arrive this soon after a report are credited to watching.
const WATCHED_RECENTLY: u64 = 6 * MINUTE;
/// The Activity screen's longest range.
const KEEP_EVENTS: u64 = 30 * DAY;
/// Watch points come every five minutes. Three cycles watched with nothing
/// arriving means something is wrong, not late.
const OVERDUE_SECONDS: f64 = 3.0 * PAYOUT_SECONDS;
/// This many watch reports refused in a row (about a minute) is worth saying.
const REPORT_FAILURES_SHOWN: u32 = 3;
/// Twitch's API can still report a stream for a while after announcing its
/// end. For this long, that broadcast is treated as over.
const ENDED_GRACE: u64 = 2 * MINUTE;

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn valid_name(name: &str) -> bool {
    (4..=25).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// What the backend tracks about a channel that the UI never sees.
#[derive(Clone, Debug, Default)]
struct Track {
    /// The numeric channel id Twitch uses.
    twitch_id: Option<String>,
    /// Id of the current broadcast while live.
    broadcast_id: Option<String>,
    /// A bonus waiting to be claimed.
    claim_id: Option<String>,
    /// The bonus we last sent a claim for, so it is never sent twice.
    claim_sent: Option<String>,
    /// When Twitch last pushed news about this channel. An answer to a
    /// question asked before then is out of date.
    pushed_at: Option<u64>,
    /// The broadcast Twitch announced the end of, and when.
    ended: Option<(String, u64)>,
    /// Where watch time for the current broadcast is reported.
    spade_url: Option<String>,
    /// False until the first balance arrives, so it is not mistaken for a gain.
    balance_known: bool,
    /// We claimed a bonus and have not yet seen it in the balance.
    claimed_bonus: bool,
    offline_since: Option<u64>,
    /// When watch time was last reported successfully.
    last_ping: Option<u64>,
    /// Seconds watched since the last payout we saw, or since watching began.
    payout_elapsed: f64,
    /// Watch reports refused in a row.
    report_failures: u32,
}

/// What the watch loop needs to report watch time for one channel.
#[derive(Clone, Debug, PartialEq)]
pub struct WatchTarget {
    pub login: String,
    pub channel_id: String,
    pub broadcast_id: String,
    /// `None` until it has been looked up for this broadcast.
    pub spade_url: Option<String>,
}

pub struct Collector {
    pub(crate) status: Status,
    pub(crate) paused: bool,
    pub(crate) channels: Vec<Channel>,
    pub(crate) events: Vec<ActivityEvent>,
    pub(crate) account: Option<String>,
    pub(crate) sign_in: Option<SignIn>,
    pub(crate) retry_in: u32,
    pub open_at_login: bool,
    pub start_paused: bool,
    /// The welcome tour has been seen or skipped.
    pub welcomed: bool,
    pub update: UpdateInfo,
    pub auto_update: bool,
    tracks: HashMap<String, Track>,
    removed: Option<(Channel, usize)>,
    next_event: u64,
    /// Lines for the log, collected here because this type does no I/O.
    notes: Vec<String>,
}

impl Collector {
    /// Nothing known yet: no channels, not connected. Shows as loading until
    /// the engine has checked the stored token.
    pub fn new(start_paused: bool, open_at_login: bool) -> Self {
        Self {
            status: Status::Loading,
            paused: start_paused,
            channels: Vec::new(),
            events: Vec::new(),
            account: None,
            sign_in: None,
            retry_in: RETRY_SECONDS,
            open_at_login,
            start_paused,
            welcomed: false,
            update: UpdateInfo::in_state(UpdateState::Off),
            auto_update: true,
            tracks: HashMap::new(),
            removed: None,
            next_event: 0,
            notes: Vec::new(),
        }
    }

    /// Unique across runs, so saved entries never collide with new ones.
    pub(crate) fn event_id(&mut self) -> String {
        self.next_event += 1;
        format!("e{}-{}", now_ms(), self.next_event)
    }

    /// Live channels, streak-due first, then list order.
    pub(crate) fn slots(&self) -> Vec<Option<String>> {
        let live = || self.channels.iter().filter(|c| c.live);
        let mut picked = live()
            .filter(|c| c.streak_due)
            .chain(live().filter(|c| !c.streak_due))
            .map(|c| c.id.clone());
        (0..SLOT_COUNT).map(|_| picked.next()).collect()
    }

    fn collecting(&self) -> bool {
        self.status == Status::Ok && !self.paused
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            status: self.status,
            paused: self.paused,
            channels: self.channels.clone(),
            slots: self.slots(),
            account: self.account.clone(),
            sign_in: self.sign_in.clone(),
            retry_in: self.retry_in,
            open_at_login: self.open_at_login,
            start_paused: self.start_paused,
            welcomed: self.welcomed,
            version: env!("CARGO_PKG_VERSION").to_string(),
            update: self.update.clone(),
            auto_update: self.auto_update,
        }
    }

    pub fn activity(&self) -> Vec<ActivityEvent> {
        self.events.clone()
    }

    /// Replace the activity log with saved entries, dropping ones too old to show.
    pub fn load_events(&mut self, mut events: Vec<ActivityEvent>, now: u64) {
        events.retain(|e| now.saturating_sub(e.at) < KEEP_EVENTS);
        events.sort_by(|a, b| b.at.cmp(&a.at));
        self.events = events;
    }

    /// (login, display name) for each channel in priority order, for saving.
    pub fn saved_channels(&self) -> Vec<(String, String)> {
        self.channels.iter().map(|c| (c.id.clone(), c.name.clone())).collect()
    }

    /// Apply a saved channel list: keep that order, add logins not seen before.
    pub fn restore_channels(&mut self, saved: &[(String, String)]) {
        let mut known = std::mem::take(&mut self.channels);
        for (login, name) in saved {
            match known.iter().position(|c| c.id == *login) {
                Some(i) => self.channels.push(known.remove(i)),
                None => self.channels.push(blank_channel(login, name)),
            }
        }
    }

    /// What Twitch said about one channel. Returns the activity entries this
    /// created or updated. Unknown logins are ignored: the channel may have
    /// been removed while the request was in flight.
    pub fn apply_poll(&mut self, login: &str, poll: Poll, now: u64) -> Vec<ActivityEvent> {
        self.apply_poll_asked(login, poll, now, now)
    }

    /// As `apply_poll`, for an answer to a question asked at `asked_at`. If
    /// Twitch has pushed news about the channel since then, the answer is out
    /// of date and is dropped: applying it would undo the news, and the next
    /// answer would then look like a second gain.
    pub fn apply_poll_asked(&mut self, login: &str, poll: Poll, asked_at: u64, now: u64) -> Vec<ActivityEvent> {
        let Some(c) = self.channels.iter_mut().find(|c| c.id == login) else {
            return Vec::new();
        };
        let track = self.tracks.entry(login.to_string()).or_default();
        if track.pushed_at.is_some_and(|at| at > asked_at) {
            return Vec::new();
        }
        c.checked = true;
        let Some(mut found) = poll else {
            // The channel no longer exists on Twitch (renamed or closed).
            set_offline(c, track, now);
            return Vec::new();
        };

        c.name = found.display_name;
        // An answer without a picture does not take away one already known.
        if found.avatar.is_some() {
            c.avatar = found.avatar;
        }
        c.payout_points = payout_points(found.multiplier);
        track.twitch_id = Some(found.twitch_id);
        track.claim_id = found.claim_id;

        // The API can lag behind the announcement that a stream has ended.
        if let (Some(id), Some((ended, at))) = (&found.broadcast_id, &track.ended) {
            if id == ended && now.saturating_sub(*at) < ENDED_GRACE {
                found.broadcast_id = None;
            }
        }

        match found.broadcast_id {
            None => set_offline(c, track, now),
            Some(broadcast) => {
                if track.broadcast_id.as_deref() != Some(broadcast.as_str()) {
                    // A new stream: watch time starts over, and a streak may be on offer.
                    c.watch_seconds = 0;
                    c.payout = 0.0;
                    c.streak_due = track
                        .offline_since
                        .is_none_or(|since| now.saturating_sub(since) > STREAK_OFFLINE_GAP);
                    track.payout_elapsed = 0.0;
                    track.spade_url = None;
                    track.last_ping = None;
                    track.report_failures = 0;
                    c.problem = None;
                }
                c.live = true;
                track.broadcast_id = Some(broadcast);
                track.offline_since = None;
            }
        }

        let mut gains = Vec::new();
        if let Some(balance) = found.balance {
            if track.balance_known && balance > c.points {
                let watched = track.last_ping.is_some_and(|at| now.saturating_sub(at) < WATCHED_RECENTLY);
                gains = attribute(balance - c.points, track.claimed_bonus, c.streak_due, watched, c.payout_points);
            }
            for (kind, _) in &gains {
                match kind {
                    EventKind::Watch => {
                        track.payout_elapsed = 0.0;
                        c.payout = 0.0;
                        if c.problem == Some(Problem::NoPoints) {
                            c.problem = None;
                        }
                    }
                    EventKind::Streak => c.streak_due = false,
                    EventKind::Bonus => {}
                }
            }
            track.claimed_bonus = false;
            track.balance_known = true;
            c.points = balance;
            c.points_known = true;
        }

        let (name, unit) = (c.name.clone(), c.payout_points);
        gains
            .into_iter()
            .map(|(kind, points)| match kind {
                EventKind::Watch => self.record_watch(&name, points, watch_minutes(points, unit), now),
                kind => self.record(&name, kind, points, now),
            })
            .collect()
    }

    // ---- News pushed by Twitch ------------------------------------------------

    /// The profile picture of a channel just added from the followed list,
    /// so it shows before the channel's first check.
    pub fn set_avatar(&mut self, login: &str, avatar: Option<String>) {
        if let (Some(c), Some(avatar)) = (self.channels.iter_mut().find(|c| c.id == login), avatar) {
            c.avatar = Some(avatar);
        }
    }

    /// The first listed channel Twitch has not been asked about yet: one just
    /// added, or any channel right after the app starts.
    pub fn next_unchecked(&self) -> Option<String> {
        self.channels.iter().find(|c| !c.checked).map(|c| c.id.clone())
    }

    /// Stop treating a channel as waiting for its first answer, without one.
    /// Used when the question keeps failing, so the others still get their turn.
    pub fn give_up_first_check(&mut self, login: &str) {
        if let Some(c) = self.channels.iter_mut().find(|c| c.id == login) {
            c.checked = true;
        }
    }

    /// The listed channel with this Twitch id, if any.
    pub fn login_for(&self, twitch_id: &str) -> Option<String> {
        self.channels
            .iter()
            .find(|c| self.tracks.get(&c.id).is_some_and(|t| t.twitch_id.as_deref() == Some(twitch_id)))
            .map(|c| c.id.clone())
    }

    /// Twitch ids of the listed channels, in priority order, once known.
    pub fn channel_ids(&self) -> Vec<String> {
        self.channels
            .iter()
            .filter_map(|c| self.tracks.get(&c.id)?.twitch_id.clone())
            .collect()
    }

    /// Twitch told us something about a channel as it happened. Unlike a
    /// poll, this carries the reason points were earned, so activity is
    /// labelled exactly. Returns the activity entries created or updated.
    pub fn apply_push(&mut self, login: &str, pushed: Pushed, now: u64) -> Vec<ActivityEvent> {
        let Some(c) = self.channels.iter_mut().find(|c| c.id == login) else {
            return Vec::new();
        };
        let track = self.tracks.entry(login.to_string()).or_default();
        track.pushed_at = Some(now);

        let (points, reason) = match pushed {
            Pushed::Spent { balance } => {
                c.points = balance;
                c.points_known = true;
                track.balance_known = true;
                return Vec::new();
            }
            Pushed::BonusAvailable { claim_id } => {
                track.claim_id = Some(claim_id);
                return Vec::new();
            }
            Pushed::StreamUp => {
                track.ended = None;
                return Vec::new();
            }
            Pushed::StreamDown => {
                if let Some(id) = track.broadcast_id.clone() {
                    track.ended = Some((id, now));
                }
                set_offline(c, track, now);
                return Vec::new();
            }
            Pushed::Earned { balance, points, reason } => {
                // A balance we already hold means a poll counted this gain
                // first, or Twitch sent the message twice.
                if track.balance_known && balance <= c.points {
                    return Vec::new();
                }
                c.points = balance;
                c.points_known = true;
                track.balance_known = true;
                (points, reason)
            }
        };

        match reason {
            Reason::Watch => {
                track.payout_elapsed = 0.0;
                c.payout = 0.0;
                if c.problem == Some(Problem::NoPoints) {
                    c.problem = None;
                }
            }
            Reason::Claim => {
                track.claim_id = None;
                track.claimed_bonus = false;
            }
            Reason::Streak => c.streak_due = false,
            Reason::Other => {}
        }
        // Points earned while the app was not watching are not its doing.
        let watched = track.last_ping.is_some_and(|at| now.saturating_sub(at) < WATCHED_RECENTLY);
        if !watched {
            return Vec::new();
        }
        let (name, unit) = (c.name.clone(), c.payout_points);
        match reason {
            Reason::Watch => vec![self.record_watch(&name, points, watch_minutes(points, unit), now)],
            Reason::Claim => vec![self.record(&name, EventKind::Bonus, points, now)],
            Reason::Streak => vec![self.record(&name, EventKind::Streak, points, now)],
            Reason::Other => Vec::new(),
        }
    }

    fn record(&mut self, channel: &str, kind: EventKind, points: u64, now: u64) -> ActivityEvent {
        let event = ActivityEvent {
            id: self.event_id(),
            at: now,
            channel: channel.to_string(),
            kind,
            points,
            amount: None,
        };
        self.events.insert(0, event.clone());
        self.events.retain(|e| now.saturating_sub(e.at) < KEEP_EVENTS);
        event
    }

    /// Fold a payout into the channel's current watch entry, or start one.
    pub(crate) fn record_watch(&mut self, channel: &str, points: u64, minutes: u32, now: u64) -> ActivityEvent {
        let latest = self.events.iter().position(|e| e.channel == channel);
        if let Some(i) = latest {
            let e = &mut self.events[i];
            if e.kind == EventKind::Watch && now.saturating_sub(e.at) < 6 * HOUR {
                e.points += points;
                e.amount = Some(e.amount.unwrap_or(0) + minutes);
                e.at = now;
                return e.clone();
            }
        }
        let mut event = self.record(channel, EventKind::Watch, points, now);
        event.amount = Some(minutes);
        self.events[0].amount = Some(minutes);
        event
    }

    // ---- Watching -----------------------------------------------------------

    /// The channels to report watch time for right now: the ones in the
    /// slots, if collecting and Twitch has told us enough about them.
    pub fn watch_targets(&self) -> Vec<WatchTarget> {
        if !self.collecting() {
            return Vec::new();
        }
        self.slots()
            .into_iter()
            .flatten()
            .filter_map(|login| {
                let track = self.tracks.get(&login)?;
                Some(WatchTarget {
                    channel_id: track.twitch_id.clone()?,
                    broadcast_id: track.broadcast_id.clone()?,
                    spade_url: track.spade_url.clone(),
                    login,
                })
            })
            .collect()
    }

    pub fn set_spade_url(&mut self, login: &str, url: Option<String>) {
        if let Some(track) = self.tracks.get_mut(login) {
            track.spade_url = url;
        }
    }

    /// Watch time for this channel was reported successfully.
    pub fn watched(&mut self, login: &str, now: u64) {
        if let Some(track) = self.tracks.get_mut(login) {
            track.last_ping = Some(now);
            track.report_failures = 0;
        }
        if let Some(c) = self.channels.iter_mut().find(|c| c.id == login) {
            if c.problem == Some(Problem::Reports) {
                c.problem = None;
                self.notes.push(format!("watch {login}: reports are getting through again"));
            }
        }
    }

    /// Twitch refused a watch report for this channel. After a few in a row
    /// the channel is marked, so the UI can say so steadily instead of the
    /// whole app flipping in and out of an error state.
    pub fn report_failed(&mut self, login: &str) {
        let Some(track) = self.tracks.get_mut(login) else { return };
        track.report_failures += 1;
        if track.report_failures < REPORT_FAILURES_SHOWN {
            return;
        }
        if let Some(c) = self.channels.iter_mut().find(|c| c.id == login && c.live) {
            if c.problem != Some(Problem::Reports) {
                c.problem = Some(Problem::Reports);
                self.notes.push(format!("watch {login}: Twitch is not accepting watch reports"));
            }
        }
    }

    /// Lines for the log since this was last called.
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    /// One second passes. Advances watch time and payout progress for the
    /// channels being watched. Returns whether anything changed.
    pub fn tick(&mut self, now: u64) -> bool {
        if !self.collecting() {
            return false;
        }
        let mut changed = false;
        for c in self.channels.iter_mut().filter(|c| c.live) {
            let Some(track) = self.tracks.get_mut(&c.id) else { continue };
            if !track.last_ping.is_some_and(|at| now.saturating_sub(at) < PING_FRESH) {
                continue;
            }
            c.watch_seconds += 1;
            track.payout_elapsed += 1.0;
            // Holds at full until the payout shows up in the balance.
            c.payout = (track.payout_elapsed / PAYOUT_SECONDS).min(1.0);
            if c.streak_due && c.watch_seconds >= STREAK_WATCH_SECONDS {
                c.streak_due = false;
            }
            // Reports are going through, yet three payouts' worth of watching
            // has brought nothing: Twitch may have changed what earns points.
            if track.payout_elapsed >= OVERDUE_SECONDS && c.problem.is_none() {
                c.problem = Some(Problem::NoPoints);
                self.notes.push(format!(
                    "watch {}: watched {} minutes with no points arriving",
                    c.id,
                    (OVERDUE_SECONDS / 60.0) as u32
                ));
            }
            changed = true;
        }
        changed
    }

    /// (channel id, claim id) if a bonus is waiting on a channel being watched.
    pub fn claim_target(&self, login: &str) -> Option<(String, String)> {
        if !self.collecting() || !self.slots().iter().flatten().any(|id| id == login) {
            return None;
        }
        let track = self.tracks.get(login)?;
        let claim_id = track.claim_id.clone()?;
        if track.claim_sent.as_deref() == Some(claim_id.as_str()) {
            return None;
        }
        Some((track.twitch_id.clone()?, claim_id))
    }

    /// As `claim_target`, and notes that the claim is being sent, so a poll
    /// and a push that report the same bonus do not both claim it.
    pub fn begin_claim(&mut self, login: &str) -> Option<(String, String)> {
        let target = self.claim_target(login)?;
        self.tracks.get_mut(login)?.claim_sent = Some(target.1.clone());
        Some(target)
    }

    /// The claim did not go through; it may be tried again.
    pub fn claim_failed(&mut self, login: &str) {
        if let Some(track) = self.tracks.get_mut(login) {
            track.claim_sent = None;
        }
    }

    /// The bonus was claimed; the next balance rise includes it.
    pub fn bonus_claimed(&mut self, login: &str) {
        if let Some(track) = self.tracks.get_mut(login) {
            track.claim_id = None;
            track.claimed_bonus = true;
        }
    }

    // ---- What the UI asks for -----------------------------------------------

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Whether `raw` could be added: the cleaned-up name, or why not.
    pub fn check_new_channel(&self, raw: &str) -> Result<String, String> {
        let name = raw.trim().trim_start_matches('@');
        if !valid_name(name) {
            return Err("Channel names are 4 to 25 letters, numbers or underscores.".into());
        }
        let login = name.to_lowercase();
        if let Some(existing) = self.channels.iter().find(|c| c.id == login) {
            return Err(format!("{} is already in your list.", existing.name));
        }
        Ok(name.to_string())
    }

    /// Add a channel without asking Twitch about it (demo mode).
    pub fn add_channel(&mut self, raw: &str) -> Result<(), String> {
        let name = self.check_new_channel(raw)?;
        self.channels.push(blank_channel(&name.to_lowercase(), &name));
        self.removed = None;
        self.settle_all();
        Ok(())
    }

    /// Demo mode has no Twitch to ask, so every channel counts as answered.
    pub fn settle_all(&mut self) {
        for c in &mut self.channels {
            c.checked = true;
            c.points_known = true;
        }
    }

    /// Add a channel Twitch has confirmed, with what it said about it.
    pub fn add_resolved(&mut self, login: &str, found: Found, now: u64) -> Result<(), String> {
        self.check_new_channel(login)?;
        self.channels.push(blank_channel(login, &found.display_name));
        self.removed = None;
        self.tracks.remove(login);
        self.apply_poll(login, Some(found), now);
        Ok(())
    }

    /// Add channels Twitch has already named (the user's follows) to the end
    /// of the list, skipping any already in it. Polling fills in the rest.
    /// Returns how many were added.
    pub fn add_known(&mut self, channels: &[(String, String)]) -> usize {
        let before = self.channels.len();
        for (login, name) in channels {
            if !login.is_empty() && !self.channels.iter().any(|c| c.id == *login) {
                self.tracks.remove(login);
                self.channels.push(blank_channel(login, name));
            }
        }
        let added = self.channels.len() - before;
        if added > 0 {
            self.removed = None;
        }
        added
    }

    pub fn remove_channel(&mut self, id: &str) {
        if let Some(index) = self.channels.iter().position(|c| c.id == id) {
            self.removed = Some((self.channels.remove(index), index));
        }
    }

    pub fn undo_remove(&mut self) {
        if let Some((channel, index)) = self.removed.take() {
            self.channels.insert(index.min(self.channels.len()), channel);
        }
    }

    pub fn move_channel(&mut self, id: &str, index: usize) {
        if let Some(from) = self.channels.iter().position(|c| c.id == id) {
            let channel = self.channels.remove(from);
            self.channels.insert(index.min(self.channels.len()), channel);
        }
    }

    pub fn set_status(&mut self, status: Status) {
        self.status = status;
    }

    /// A sign-in code is waiting for the user, or `None` to withdraw it.
    pub fn set_sign_in(&mut self, sign_in: Option<SignIn>) {
        self.sign_in = sign_in;
    }

    pub fn signed_in(&mut self, login: &str) {
        self.status = Status::Ok;
        self.account = Some(login.to_string());
        self.sign_in = None;
    }

    pub fn signed_out(&mut self) {
        self.status = Status::LoggedOut;
        self.account = None;
        self.sign_in = None;
    }
}

/// What Twitch reported for a channel that exists.
#[derive(Clone, Debug, Default)]
pub struct Found {
    /// The name with the channel's own capitalisation.
    pub display_name: String,
    pub twitch_id: String,
    /// Present while the channel is live.
    pub broadcast_id: Option<String>,
    /// `None` if the points lookup had nothing for this channel.
    pub balance: Option<u64>,
    pub claim_id: Option<String>,
    /// Sum of the account's multipliers on this channel (0.2 = +20%).
    pub multiplier: f64,
    /// Address of the channel's profile picture.
    pub avatar: Option<String>,
}

/// `None` when Twitch has no such channel.
pub type Poll = Option<Found>;

/// Why Twitch says points were earned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reason {
    /// The five-minute watch payout.
    Watch,
    /// A claimed bonus.
    Claim,
    /// A watch streak.
    Streak,
    /// Anything the app does not collect (raids, predictions, gifts).
    Other,
}

/// News about one channel that Twitch pushed as it happened.
#[derive(Clone, Debug, PartialEq)]
pub enum Pushed {
    /// Points were earned. `balance` is the total afterwards.
    Earned { balance: u64, points: u64, reason: Reason },
    /// Points were spent. `balance` is the total afterwards.
    Spent { balance: u64 },
    BonusAvailable { claim_id: String },
    StreamUp,
    StreamDown,
}

/// Watch time a payout of `points` stands for, where one payout is `unit`.
fn watch_minutes(points: u64, unit: u64) -> u32 {
    let payouts = ((points as f64 / unit.max(1) as f64).round() as u32).max(1);
    payouts * (PAYOUT_SECONDS / 60.0) as u32
}

/// The watch payout on a channel, given the account's multipliers there.
pub(crate) fn payout_points(multiplier: f64) -> u64 {
    (PAYOUT_POINTS as f64 * (1.0 + multiplier.max(0.0))).round() as u64
}

/// Split a balance rise into what earned it. Without Twitch telling us the
/// reason, this is inference: the total is exact, the labels are a best fit.
///
/// - A bonus we claimed accounts for five watch payouts.
/// - While a streak is due on a channel being watched, a rise of 300 or more
///   is the streak (and absorbs any watch payout that landed with it).
/// - Anything left on a channel being watched is watch time.
/// - Points on a channel we were not watching came from somewhere else and
///   are not recorded.
pub(crate) fn attribute(
    delta: u64,
    claimed_bonus: bool,
    streak_due: bool,
    watched: bool,
    watch_unit: u64,
) -> Vec<(EventKind, u64)> {
    let mut gains = Vec::new();
    let mut rest = delta;
    let bonus = watch_unit * BONUS_FACTOR;
    if claimed_bonus && rest >= bonus {
        gains.push((EventKind::Bonus, bonus));
        rest -= bonus;
    }
    if streak_due && watched && rest >= STREAK_MIN_POINTS {
        gains.push((EventKind::Streak, rest));
        rest = 0;
    }
    if watched && rest > 0 {
        gains.push((EventKind::Watch, rest));
    }
    gains
}

fn set_offline(c: &mut Channel, track: &mut Track, now: u64) {
    if c.live {
        track.offline_since = Some(now);
    }
    c.live = false;
    c.watch_seconds = 0;
    c.payout = 0.0;
    c.streak_due = false;
    track.broadcast_id = None;
    track.spade_url = None;
    track.last_ping = None;
    track.payout_elapsed = 0.0;
    track.report_failures = 0;
    c.problem = None;
}

pub(crate) fn blank_channel(login: &str, name: &str) -> Channel {
    Channel {
        id: login.to_string(),
        name: name.to_string(),
        live: false,
        points: 0,
        watch_seconds: 0,
        payout: 0.0,
        payout_points: PAYOUT_POINTS,
        streak_due: false,
        checked: false,
        points_known: false,
        problem: None,
        avatar: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: u64 = 1_800_000_000_000;

    fn collector(channels: &[(&str, bool, bool)]) -> Collector {
        let mut c = Collector::new(false, false);
        c.set_status(Status::Ok);
        for (name, live, streak_due) in channels {
            let mut ch = blank_channel(&name.to_lowercase(), name);
            ch.live = *live;
            ch.streak_due = *streak_due;
            c.channels.push(ch);
        }
        c
    }

    fn names(c: &Collector) -> Vec<String> {
        c.channels.iter().map(|c| c.name.clone()).collect()
    }

    fn found(name: &str, broadcast: Option<&str>, balance: u64) -> Found {
        Found {
            display_name: name.into(),
            twitch_id: "42".into(),
            broadcast_id: broadcast.map(str::to_string),
            balance: Some(balance),
            claim_id: None,
            multiplier: 0.0,
            avatar: None,
        }
    }

    /// A collector watching one live channel, with a first balance of 1000.
    fn watching() -> Collector {
        let mut c = collector(&[("lirik", false, false)]);
        c.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 1000)), T0);
        c.channels[0].streak_due = false;
        c.watched("lirik", T0);
        c
    }

    // ---- Polling

    #[test]
    fn a_poll_sets_name_live_state_and_balance() {
        let mut c = collector(&[("lirik", false, false)]);
        let events = c.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 9870)), T0);
        let ch = &c.channels[0];
        assert_eq!((ch.name.as_str(), ch.live, ch.points), ("LIRIK", true, 9870));
        assert_eq!(c.slots(), vec![Some("lirik".into()), None]);
        assert!(events.is_empty(), "the first balance is not a gain");
    }

    #[test]
    fn a_channel_is_unchecked_until_twitch_answers_about_it() {
        let mut c = collector(&[]);
        c.add_known(&[("alpha".into(), "Alpha".into()), ("bravo".into(), "Bravo".into())]);
        assert_eq!(c.next_unchecked(), Some("alpha".into()));
        assert!(!c.channels[0].checked && !c.channels[0].points_known);

        c.apply_poll("alpha", Some(found("Alpha", None, 0)), T0);
        assert!(c.channels[0].checked);
        assert!(c.channels[0].points_known, "a balance of zero is still a known balance");
        assert_eq!(c.next_unchecked(), Some("bravo".into()));

        // Twitch has no points for this channel: checked, but the balance stays unknown.
        let mut none = found("Bravo", None, 0);
        none.balance = None;
        c.apply_poll("bravo", Some(none), T0);
        assert!(c.channels[1].checked && !c.channels[1].points_known);
        assert_eq!(c.next_unchecked(), None);
    }

    #[test]
    fn a_channel_whose_first_check_keeps_failing_gives_way_to_the_rest() {
        let mut c = collector(&[]);
        c.add_known(&[("alpha".into(), "Alpha".into()), ("bravo".into(), "Bravo".into())]);
        c.give_up_first_check("alpha");
        assert_eq!(c.next_unchecked(), Some("bravo".into()));
        assert!(!c.channels[0].points_known);
    }

    #[test]
    fn going_offline_clears_the_session() {
        let mut c = watching();
        c.channels[0].watch_seconds = 600;
        c.apply_poll("lirik", Some(found("LIRIK", None, 1000)), T0 + MINUTE);
        let ch = &c.channels[0];
        assert!(!ch.live && !ch.streak_due);
        assert_eq!(ch.watch_seconds, 0);
        assert!(c.watch_targets().is_empty());
    }

    #[test]
    fn a_new_broadcast_restarts_watch_time_but_the_same_one_keeps_it() {
        let mut c = watching();
        c.channels[0].watch_seconds = 600;
        c.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 1000)), T0 + MINUTE);
        assert_eq!(c.channels[0].watch_seconds, 600);
        c.apply_poll("lirik", Some(found("LIRIK", Some("b2"), 1000)), T0 + 2 * MINUTE);
        assert_eq!(c.channels[0].watch_seconds, 0);
    }

    #[test]
    fn a_channel_twitch_no_longer_has_goes_offline_and_unknown_logins_are_ignored() {
        let mut c = watching();
        c.apply_poll("lirik", None, T0);
        assert!(!c.channels[0].live);
        c.apply_poll("nobody", Some(found("Nobody", Some("b1"), 1)), T0);
        assert_eq!(c.channels.len(), 1);
    }

    #[test]
    fn a_profile_picture_arrives_with_a_check_and_is_not_lost_by_one_without() {
        let mut c = collector(&[("lirik", false, false)]);
        let mut with = found("LIRIK", None, 0);
        with.avatar = Some("https://static-cdn.jtvnw.net/a.png".into());
        c.apply_poll("lirik", Some(with), T0);
        assert_eq!(c.channels[0].avatar.as_deref(), Some("https://static-cdn.jtvnw.net/a.png"));
        c.apply_poll("lirik", Some(found("LIRIK", None, 0)), T0 + MINUTE);
        assert!(c.channels[0].avatar.is_some());
    }

    #[test]
    fn a_multiplier_raises_the_payout() {
        assert_eq!(payout_points(0.0), 10);
        assert_eq!(payout_points(0.2), 12);
        assert_eq!(payout_points(1.0), 20);
        let mut c = collector(&[("lirik", false, false)]);
        let mut f = found("LIRIK", Some("b1"), 0);
        f.multiplier = 0.2;
        c.apply_poll("lirik", Some(f), T0);
        assert_eq!(c.channels[0].payout_points, 12);
    }

    // ---- Streaks

    #[test]
    fn a_streak_is_due_on_a_new_stream_after_a_long_break_but_not_a_short_one() {
        let mut c = collector(&[("lirik", false, false)]);
        c.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 0)), T0);
        assert!(c.channels[0].streak_due, "first sighting of a live channel");

        c.apply_poll("lirik", Some(found("LIRIK", None, 0)), T0 + HOUR);
        c.apply_poll("lirik", Some(found("LIRIK", Some("b2"), 0)), T0 + HOUR + 10 * MINUTE);
        assert!(!c.channels[0].streak_due, "back after ten minutes");

        c.apply_poll("lirik", Some(found("LIRIK", None, 0)), T0 + 2 * HOUR);
        c.apply_poll("lirik", Some(found("LIRIK", Some("b3"), 0)), T0 + 3 * HOUR);
        assert!(c.channels[0].streak_due, "back after an hour");
    }

    #[test]
    fn a_streak_stops_being_due_after_seven_minutes_of_watching() {
        let mut c = watching();
        c.channels[0].streak_due = true;
        for second in 0..STREAK_WATCH_SECONDS {
            c.watched("lirik", T0 + second * 1000);
            c.tick(T0 + second * 1000);
        }
        assert!(!c.channels[0].streak_due);
    }

    // ---- Watching

    #[test]
    fn watch_targets_are_the_slots_twitch_has_told_us_about() {
        let mut c = collector(&[("alpha", false, false), ("bravo", true, false)]);
        c.apply_poll("alpha", Some(found("Alpha", Some("b1"), 0)), T0);
        // bravo is marked live but has never been polled: no ids to report with.
        let targets = c.watch_targets();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0], WatchTarget {
            login: "alpha".into(),
            channel_id: "42".into(),
            broadcast_id: "b1".into(),
            spade_url: None,
        });
        c.set_spade_url("alpha", Some("https://spade.example/x".into()));
        assert_eq!(c.watch_targets()[0].spade_url.as_deref(), Some("https://spade.example/x"));
    }

    #[test]
    fn nothing_is_watched_while_paused_or_not_connected() {
        let mut c = watching();
        c.set_paused(true);
        assert!(c.watch_targets().is_empty());
        assert!(!c.tick(T0 + 1000));
        c.set_paused(false);
        c.set_status(Status::Offline);
        assert!(c.watch_targets().is_empty());
        c.set_status(Status::Ok);
        assert_eq!(c.watch_targets().len(), 1);
    }

    #[test]
    fn ticking_advances_only_channels_with_a_recent_report() {
        let mut c = watching();
        assert!(c.tick(T0 + 1000));
        assert_eq!(c.channels[0].watch_seconds, 1);
        assert!((c.channels[0].payout - 1.0 / 300.0).abs() < 1e-9);
        // No report for over a minute: the clock stops.
        assert!(!c.tick(T0 + 2 * MINUTE));
        assert_eq!(c.channels[0].watch_seconds, 1);
    }

    #[test]
    fn the_payout_bar_holds_at_full_then_resets_when_the_points_arrive() {
        let mut c = watching();
        for second in 1..=320 {
            c.watched("lirik", T0 + second * 1000);
            c.tick(T0 + second * 1000);
        }
        assert_eq!(c.channels[0].payout, 1.0);
        let events = c.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 1010)), T0 + 321_000);
        assert_eq!(c.channels[0].payout, 0.0);
        assert_eq!((events[0].kind, events[0].points, events[0].amount), (EventKind::Watch, 10, Some(5)));
    }

    // ---- When watching is not working

    #[test]
    fn three_cycles_watched_with_no_points_is_flagged_and_clears_when_points_come() {
        let mut c = watching();
        for second in 1..=899 {
            c.watched("lirik", T0 + second * 1000);
            c.tick(T0 + second * 1000);
        }
        assert_eq!(c.channels[0].problem, None, "not yet fifteen minutes");
        c.watched("lirik", T0 + 900_000);
        c.tick(T0 + 900_000);
        assert_eq!(c.channels[0].problem, Some(Problem::NoPoints));
        assert_eq!(c.take_notes().len(), 1, "said once, not every second");
        c.tick(T0 + 901_000);
        assert!(c.take_notes().is_empty());

        c.apply_push("lirik", earned(1010, 10, Reason::Watch), T0 + 902_000);
        assert_eq!(c.channels[0].problem, None);
    }

    #[test]
    fn refused_watch_reports_are_flagged_after_three_and_clear_on_the_next_good_one() {
        let mut c = watching();
        c.report_failed("lirik");
        c.report_failed("lirik");
        assert_eq!(c.channels[0].problem, None, "a stream that just ended fails a couple");
        c.report_failed("lirik");
        assert_eq!(c.channels[0].problem, Some(Problem::Reports));
        c.report_failed("lirik");
        assert_eq!(c.take_notes().len(), 1);

        c.watched("lirik", T0 + MINUTE);
        assert_eq!(c.channels[0].problem, None);
        // The count starts again: two more failures are not yet worth saying.
        c.report_failed("lirik");
        c.report_failed("lirik");
        assert_eq!(c.channels[0].problem, None);
    }

    #[test]
    fn a_problem_does_not_outlive_the_stream() {
        let mut c = watching();
        for _ in 0..3 {
            c.report_failed("lirik");
        }
        c.apply_poll("lirik", Some(found("LIRIK", None, 1000)), T0 + MINUTE);
        assert_eq!(c.channels[0].problem, None);
    }

    // ---- Bonuses

    #[test]
    fn a_bonus_is_claimed_only_on_a_channel_being_watched() {
        let mut c = collector(&[("alpha", false, false), ("bravo", false, false), ("charlie", false, false)]);
        for login in ["alpha", "bravo", "charlie"] {
            let mut f = found(login, Some("b1"), 0);
            f.claim_id = Some("claim-1".into());
            c.apply_poll(login, Some(f), T0);
            c.channels.iter_mut().for_each(|ch| ch.streak_due = false);
        }
        assert_eq!(c.claim_target("alpha"), Some(("42".into(), "claim-1".into())));
        assert_eq!(c.claim_target("charlie"), None, "live but not in a slot");
        c.bonus_claimed("alpha");
        assert_eq!(c.claim_target("alpha"), None, "not claimed twice");
        c.set_paused(true);
        assert_eq!(c.claim_target("bravo"), None, "paused");
    }

    #[test]
    fn a_claim_is_handed_out_once_and_again_only_if_it_failed() {
        let mut c = watching();
        c.apply_push("lirik", Pushed::BonusAvailable { claim_id: "claim-1".into() }, T0);
        assert_eq!(c.begin_claim("lirik"), Some(("42".into(), "claim-1".into())));
        assert_eq!(c.begin_claim("lirik"), None, "already being sent");

        c.claim_failed("lirik");
        assert_eq!(c.begin_claim("lirik"), Some(("42".into(), "claim-1".into())));
        c.bonus_claimed("lirik");

        // A poll answered late still lists the bonus that was just claimed.
        let mut late = found("LIRIK", Some("b1"), 1000);
        late.claim_id = Some("claim-1".into());
        c.apply_poll("lirik", Some(late), T0 + MINUTE);
        assert_eq!(c.begin_claim("lirik"), None, "not claimed twice");

        let mut next = found("LIRIK", Some("b1"), 1000);
        next.claim_id = Some("claim-2".into());
        c.apply_poll("lirik", Some(next), T0 + 2 * MINUTE);
        assert_eq!(c.begin_claim("lirik"), Some(("42".into(), "claim-2".into())));
    }

    // ---- News pushed by Twitch

    fn earned(balance: u64, points: u64, reason: Reason) -> Pushed {
        Pushed::Earned { balance, points, reason }
    }

    #[test]
    fn channels_are_found_by_their_twitch_id_once_it_is_known() {
        let mut c = collector(&[("alpha", false, false), ("bravo", false, false)]);
        assert_eq!(c.login_for("42"), None);
        assert!(c.channel_ids().is_empty());
        let mut f = found("Bravo", None, 0);
        f.twitch_id = "77".into();
        c.apply_poll("bravo", Some(f), T0);
        assert_eq!(c.login_for("77"), Some("bravo".into()));
        assert_eq!(c.channel_ids(), vec!["77".to_string()]);
        c.remove_channel("bravo");
        assert_eq!(c.login_for("77"), None, "no longer listed");
    }

    #[test]
    fn a_pushed_payout_is_recorded_at_once_and_not_again_by_the_next_poll() {
        let mut c = watching();
        for second in 1..=300 {
            c.watched("lirik", T0 + second * 1000);
            c.tick(T0 + second * 1000);
        }
        assert_eq!(c.channels[0].payout, 1.0);

        let events = c.apply_push("lirik", earned(1010, 10, Reason::Watch), T0 + 301_000);
        assert_eq!((events[0].kind, events[0].points, events[0].amount), (EventKind::Watch, 10, Some(5)));
        assert_eq!(c.channels[0].points, 1010);
        assert_eq!(c.channels[0].payout, 0.0, "the bar resets without waiting for a poll");

        let events = c.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 1010)), T0 + 330_000);
        assert!(events.is_empty());
        assert_eq!(c.events.len(), 1);
    }

    #[test]
    fn pushed_reasons_label_what_inference_would_get_wrong() {
        let mut c = watching();
        // A streak under 300 would be read as watch time from the balance alone.
        c.channels[0].streak_due = true;
        let events = c.apply_push("lirik", earned(1250, 250, Reason::Streak), T0 + MINUTE);
        assert_eq!((events[0].kind, events[0].points), (EventKind::Streak, 250));
        assert!(!c.channels[0].streak_due);

        // A bonus the app did not claim itself would be read as watch time too.
        let events = c.apply_push("lirik", earned(1300, 50, Reason::Claim), T0 + 2 * MINUTE);
        assert_eq!((events[0].kind, events[0].points), (EventKind::Bonus, 50));

        // A watch payout landing with them stays a watch payout.
        let events = c.apply_push("lirik", earned(1310, 10, Reason::Watch), T0 + 2 * MINUTE);
        assert_eq!((events[0].kind, events[0].points), (EventKind::Watch, 10));
        assert_eq!(c.events.len(), 3);
    }

    #[test]
    fn a_push_for_a_gain_already_counted_is_dropped() {
        let mut c = watching();
        // The poll saw the rise first and inferred it.
        let events = c.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 1010)), T0 + MINUTE);
        assert_eq!(events.len(), 1);
        let events = c.apply_push("lirik", earned(1010, 10, Reason::Watch), T0 + MINUTE + 500);
        assert!(events.is_empty());

        // Twitch repeating itself.
        c.watched("lirik", T0 + 6 * MINUTE);
        assert_eq!(c.apply_push("lirik", earned(1020, 10, Reason::Watch), T0 + 6 * MINUTE).len(), 1);
        assert!(c.apply_push("lirik", earned(1020, 10, Reason::Watch), T0 + 6 * MINUTE).is_empty());
        assert_eq!(c.events.len(), 1);
        assert_eq!(c.events[0].points, 20);
    }

    #[test]
    fn an_answer_asked_for_before_a_push_is_out_of_date() {
        let mut c = watching();
        let asked = T0 + MINUTE;
        c.apply_push("lirik", earned(1010, 10, Reason::Watch), asked + 200);
        // The answer was on its way while the push arrived; it has the old balance.
        let events = c.apply_poll_asked("lirik", Some(found("LIRIK", Some("b1"), 1000)), asked, asked + 400);
        assert!(events.is_empty());
        assert_eq!(c.channels[0].points, 1010, "the old balance does not come back");

        let events = c.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 1010)), T0 + 2 * MINUTE);
        assert!(events.is_empty(), "so the gain is not counted a second time");
        assert_eq!(c.events.len(), 1);
    }

    #[test]
    fn pushed_points_the_app_did_not_earn_update_the_balance_only() {
        let mut c = watching();
        let events = c.apply_push("lirik", earned(1250, 250, Reason::Other), T0 + MINUTE);
        assert!(events.is_empty(), "a raid, a prediction, a gift");
        assert_eq!(c.channels[0].points, 1250);

        // Watch points from the user's own browser on a channel the app is not watching.
        let mut idle = collector(&[("lirik", false, false)]);
        idle.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 1000)), T0);
        let events = idle.apply_push("lirik", earned(1010, 10, Reason::Watch), T0 + MINUTE);
        assert!(events.is_empty());
        assert_eq!(idle.channels[0].points, 1010);

        let events = c.apply_push("lirik", Pushed::Spent { balance: 250 }, T0 + 2 * MINUTE);
        assert!(events.is_empty());
        assert_eq!(c.channels[0].points, 250);
        assert!(c.apply_push("nobody", earned(10, 10, Reason::Watch), T0).is_empty());
    }

    #[test]
    fn a_stream_ending_frees_the_slot_at_once_even_if_the_api_lags() {
        let mut c = watching();
        c.apply_push("lirik", Pushed::StreamDown, T0 + MINUTE);
        assert!(!c.channels[0].live);
        assert_eq!(c.slots(), vec![None, None]);

        // The API still lists the broadcast that just ended.
        c.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 1000)), T0 + MINUTE + 30_000);
        assert!(!c.channels[0].live);

        // A different broadcast is a new stream and is believed straight away.
        c.apply_poll("lirik", Some(found("LIRIK", Some("b2"), 1000)), T0 + MINUTE + 40_000);
        assert!(c.channels[0].live);
        assert!(!c.channels[0].streak_due, "back within half an hour");
    }

    #[test]
    fn an_ended_broadcast_is_believed_again_after_two_minutes_or_a_stream_up() {
        let mut c = watching();
        c.apply_push("lirik", Pushed::StreamDown, T0 + MINUTE);
        c.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 1000)), T0 + 4 * MINUTE);
        assert!(c.channels[0].live, "the API has had long enough to catch up");

        c.apply_push("lirik", Pushed::StreamDown, T0 + 5 * MINUTE);
        c.apply_push("lirik", Pushed::StreamUp, T0 + 5 * MINUTE + 10_000);
        assert!(!c.channels[0].live, "going live waits for the API to confirm it");
        c.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 1000)), T0 + 5 * MINUTE + 30_000);
        assert!(c.channels[0].live);
    }

    // ---- Attribution

    #[test]
    fn a_rise_is_split_into_bonus_streak_and_watch() {
        use EventKind::*;
        assert_eq!(attribute(10, false, false, true, 10), vec![(Watch, 10)]);
        assert_eq!(attribute(60, true, false, true, 10), vec![(Bonus, 50), (Watch, 10)]);
        assert_eq!(attribute(50, true, false, true, 10), vec![(Bonus, 50)]);
        assert_eq!(attribute(450, false, true, true, 10), vec![(Streak, 450)]);
        assert_eq!(attribute(360, true, true, true, 10), vec![(Bonus, 50), (Streak, 310)]);
        // With a 1.2x multiplier a bonus is 60.
        assert_eq!(attribute(72, true, false, true, 12), vec![(Bonus, 60), (Watch, 12)]);
        // A small rise while a streak is due is still just watching.
        assert_eq!(attribute(10, false, true, true, 10), vec![(Watch, 10)]);
    }

    #[test]
    fn points_from_elsewhere_are_not_recorded() {
        assert!(attribute(500, false, false, false, 10).is_empty());
        let mut c = collector(&[("lirik", false, false)]);
        c.apply_poll("lirik", Some(found("LIRIK", None, 100)), T0);
        let events = c.apply_poll("lirik", Some(found("LIRIK", None, 600)), T0 + MINUTE);
        assert!(events.is_empty());
        assert_eq!(c.channels[0].points, 600, "the balance still updates");
    }

    #[test]
    fn a_claimed_bonus_and_a_streak_become_activity() {
        let mut c = watching();
        c.bonus_claimed("lirik");
        let events = c.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 1050)), T0 + MINUTE);
        assert_eq!((events[0].kind, events[0].points), (EventKind::Bonus, 50));

        c.channels[0].streak_due = true;
        let events = c.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 1500)), T0 + 2 * MINUTE);
        assert_eq!((events[0].kind, events[0].points), (EventKind::Streak, 450));
        assert!(!c.channels[0].streak_due);
        assert_eq!(c.events.len(), 2);
    }

    #[test]
    fn spending_points_is_not_a_gain() {
        let mut c = watching();
        let events = c.apply_poll("lirik", Some(found("LIRIK", Some("b1"), 200)), T0 + MINUTE);
        assert!(events.is_empty());
        assert_eq!(c.channels[0].points, 200);
    }

    // ---- Activity log

    #[test]
    fn payouts_fold_into_one_watch_entry_per_channel() {
        let mut c = collector(&[]);
        let first = c.record_watch("alpha", 10, 5, T0);
        let second = c.record_watch("alpha", 10, 5, T0 + 5 * MINUTE);
        assert_eq!(first.id, second.id);
        assert_eq!((second.points, second.amount), (20, Some(10)));
        assert_eq!(c.events.len(), 1);

        let other = c.record_watch("bravo", 10, 5, T0);
        assert_ne!(other.id, first.id);
        assert_eq!(c.events.len(), 2);
    }

    #[test]
    fn a_bonus_in_between_starts_a_new_watch_entry() {
        let mut c = collector(&[]);
        let first = c.record_watch("alpha", 10, 5, T0);
        c.record("alpha", EventKind::Bonus, 50, T0 + MINUTE);
        let next = c.record_watch("alpha", 10, 5, T0 + 2 * MINUTE);
        assert_ne!(next.id, first.id);
        assert_eq!(c.events[0].amount, Some(5));
    }

    #[test]
    fn loading_drops_entries_older_than_thirty_days_and_sorts_newest_first() {
        let mut c = collector(&[]);
        let entry = |id: &str, at| ActivityEvent {
            id: id.into(),
            at,
            channel: "alpha".into(),
            kind: EventKind::Bonus,
            points: 50,
            amount: None,
        };
        c.load_events(vec![entry("old", T0 - 31 * DAY), entry("a", T0 - DAY), entry("b", T0 - HOUR)], T0);
        let ids: Vec<_> = c.events.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "a"]);
    }

    // ---- Slots and the list

    #[test]
    fn slots_take_the_first_two_live_channels_in_list_order() {
        let c = collector(&[("alpha", true, false), ("bravo", false, false), ("charlie", true, false), ("delta", true, false)]);
        assert_eq!(c.slots(), vec![Some("alpha".into()), Some("charlie".into())]);
    }

    #[test]
    fn a_streak_due_channel_jumps_ahead() {
        let c = collector(&[("alpha", true, false), ("bravo", true, false), ("charlie", true, true)]);
        assert_eq!(c.slots(), vec![Some("charlie".into()), Some("alpha".into())]);
    }

    #[test]
    fn an_offline_streak_channel_does_not_take_a_slot() {
        let c = collector(&[("alpha", true, false), ("bravo", false, true)]);
        assert_eq!(c.slots(), vec![Some("alpha".into()), None]);
    }

    #[test]
    fn free_slots_are_none() {
        assert_eq!(collector(&[]).slots(), vec![None, None]);
    }

    #[test]
    fn a_resolved_add_uses_the_login_as_id_and_the_display_name_as_name() {
        let mut c = collector(&[]);
        c.add_resolved("fps_shaka", found("釈迦", Some("b1"), 50), T0).unwrap();
        assert_eq!(c.saved_channels(), vec![("fps_shaka".to_string(), "釈迦".to_string())]);
        assert!(c.channels[0].live);
        assert!(c.add_resolved("fps_shaka", found("釈迦", None, 0), T0).is_err());
    }

    #[test]
    fn followed_channels_are_added_at_the_end_once_each() {
        let mut c = collector(&[("Lirik", false, false)]);
        let follows = [
            ("lirik".to_string(), "LIRIK".to_string()),
            ("fps_shaka".to_string(), "釈迦".to_string()),
            ("xqc".to_string(), "xQc".to_string()),
            ("xqc".to_string(), "xQc".to_string()),
        ];
        assert_eq!(c.add_known(&follows), 2);
        assert_eq!(names(&c), vec!["Lirik", "釈迦", "xQc"]);
        assert_eq!(c.channels[2].id, "xqc", "a three-letter login is fine: Twitch named it");
        assert_eq!(c.add_known(&follows), 0);
    }

    #[test]
    fn restoring_keeps_saved_order_and_names() {
        let mut c = collector(&[]);
        c.restore_channels(&[("lirik".into(), "LIRIK".into()), ("itshafu".into(), "itsHafu".into())]);
        assert_eq!(names(&c), vec!["LIRIK", "itsHafu"]);
        assert_eq!(c.channels[1].id, "itshafu");
    }

    #[test]
    fn add_validates_and_rejects_duplicates() {
        let mut c = collector(&[("Lirik", false, false)]);
        assert!(c.add_channel("a b").is_err());
        assert_eq!(c.add_channel("lirik").unwrap_err(), "Lirik is already in your list.");
        assert!(c.add_channel("@Sodapoppin").is_ok());
        assert_eq!(names(&c), vec!["Lirik", "Sodapoppin"]);
    }

    #[test]
    fn remove_then_undo_restores_the_position() {
        let mut c = collector(&[("alpha", false, false), ("bravo", false, false), ("charlie", false, false)]);
        c.remove_channel("bravo");
        assert_eq!(names(&c), vec!["alpha", "charlie"]);
        c.undo_remove();
        assert_eq!(names(&c), vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn signing_out_withdraws_a_pending_code() {
        let mut c = collector(&[]);
        c.set_sign_in(Some(SignIn {
            user_code: "ABCD".into(),
            verification_uri: "https://www.twitch.tv/activate".into(),
            expires_at: 1,
        }));
        c.signed_out();
        assert_eq!(c.status, Status::LoggedOut);
        assert!(c.snapshot().sign_in.is_none());
    }
}
