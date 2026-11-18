//! Drives the collector against the real Twitch. Checks the stored token on
//! start and runs the device-code sign-in; then, while signed in, four loops:
//!
//! - `poll_loop` asks about each listed channel in turn (live state, balance,
//!   waiting bonus) and claims bonuses on the channels being watched;
//! - `watch_loop` reports watch time for the channels in the slots;
//! - `tick_loop` advances the on-screen watch clock once a second;
//! - `hermes_loop` listens for what Twitch pushes as it happens: points
//!   earned and why, a bonus to claim, a stream starting or ending.
//!
//! Polling alone is a working collector. Hermes makes it exact and prompt,
//! and nothing depends on it: if it cannot connect, the app carries on.
//!
//! Each step pushes the new state to the windows through `crate::update`.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use tauri::{AppHandle, Manager};
use tokio_tungstenite::tungstenite::Message;

use super::{
    auth,
    constants::{topic_playback, topic_user_points},
    gql::Gql,
    hermes::{self, Incoming},
    random_id, watch, TwitchError,
};
use crate::{
    collector::{now_ms, Collector, Found, Poll, Pushed, RETRY_SECONDS},
    model::{Followed, SignIn, Snapshot, Status},
    publish_activity, update, update_if, AppState,
};

/// Each channel is asked about roughly once per round.
const ROUND: Duration = Duration::from_secs(60);
/// Never closer together than this, however short the list.
const MIN_GAP: Duration = Duration::from_secs(2);
/// A channel waiting for its first answer is followed quickly, so the list fills in.
const FIRST_CHECK_GAP: Duration = Duration::from_millis(300);
/// Only this many in a row. A long list of followed channels would otherwise
/// mean hundreds of requests in a burst when it is added or at start-up.
const FIRST_CHECK_QUICK: usize = 20;
const OFFLINE_RETRY: Duration = Duration::from_secs(15);
const ERROR_RETRY: Duration = Duration::from_secs(60);
/// What a field-level message says when Twitch's API has moved on.
const CHANGED_MESSAGE: &str = "Twitch has changed something this version doesn’t understand. Check for an update.";
/// This many odd answers in a row before the UI is told something is wrong.
const ERROR_THRESHOLD: u32 = 3;
const MAX_BACKOFF_SECONDS: u32 = 300;

/// Each watched channel's watch time is reported this often, as the
/// reference miner does.
const WATCH_INTERVAL: Duration = Duration::from_secs(20);
const WATCH_IDLE: Duration = Duration::from_secs(5);

/// After the Hermes connection ends, wait this long before opening another,
/// as the reference miner does.
const HERMES_RETRY: Duration = Duration::from_secs(30);
const HERMES_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// How long to wait for Twitch's first message, before it has said how often
/// it will send keepalives.
const HERMES_FIRST_SILENCE: Duration = Duration::from_secs(30);
/// Allowed on top of the keepalive interval before the connection counts as dead.
const HERMES_SILENCE_MARGIN: Duration = Duration::from_secs(5);
/// How often the subscriptions are compared with the channel list.
const HERMES_RECONCILE: Duration = Duration::from_secs(5);
/// Twitch allows this many subscriptions on one connection.
const HERMES_MAX_TOPICS: usize = 50;
/// Twitch announces a stream before its API reports it. Ask again after this.
const STREAM_UP_RECHECK: Duration = Duration::from_secs(20);

/// On unless `COLLECTOR_HERMES=0`, which leaves polling to do everything.
fn hermes_enabled() -> bool {
    std::env::var("COLLECTOR_HERMES").map_or(true, |v| v != "0")
}

struct Session {
    gql: Gql,
    /// The signed-in user's numeric id, sent with watch reports.
    user_id: String,
    /// Hermes asks for the token itself. Never logged.
    access_token: String,
}

pub struct Engine {
    http: Client,
    /// Bumped whenever a pending sign-in is cancelled or replaced, so the
    /// task polling for the old code knows to stop.
    sign_in_generation: AtomicU64,
    /// Bumped on every connect and sign-out, so the loops of an earlier
    /// session stop.
    session_generation: AtomicU64,
    session: Mutex<Option<Arc<Session>>>,
    /// The followed channels last fetched, login to name and picture, so the ones the
    /// user picks can be added without asking Twitch about each.
    followed: Mutex<HashMap<String, (String, Option<String>)>>,
    /// Wakes the poll loop early, when channels have just been added.
    poke: tokio::sync::Notify,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .expect("http client"),
            sign_in_generation: AtomicU64::new(0),
            session_generation: AtomicU64::new(0),
            session: Mutex::new(None),
            followed: Mutex::new(HashMap::new()),
            poke: tokio::sync::Notify::new(),
        }
    }

    fn session(&self) -> Option<Arc<Session>> {
        self.session.lock().unwrap().clone()
    }

    fn end_session(&self) -> u64 {
        *self.session.lock().unwrap() = None;
        self.session_generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn is_current(&self, generation: u64) -> bool {
        self.session_generation.load(Ordering::SeqCst) == generation
    }
}

/// The status the UI should show for a failure.
pub fn status_for(error: &TwitchError) -> Status {
    match error {
        TwitchError::Network(_) => Status::Offline,
        TwitchError::Unauthorized => Status::LoggedOut,
        TwitchError::RateLimited => Status::RateLimited,
        TwitchError::Unexpected(_) => Status::Error,
        TwitchError::Changed(_) => Status::Changed,
    }
}

fn fail(app: &AppHandle, context: &str, error: &TwitchError) {
    crate::log!("{context}: {error}");
    let status = status_for(error);
    update(app, false, |c| {
        if status == Status::LoggedOut {
            c.signed_out();
        } else {
            c.set_status(status);
        }
    });
}

/// A device id that stays the same across runs, as Twitch's own clients keep.
fn device_id(app: &AppHandle) -> String {
    let path = crate::data_dir(app).map(|dir| dir.join("device-id.txt"));
    if let Some(existing) = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
        let existing = existing.trim();
        if existing.len() == 32 && existing.chars().all(|c| c.is_ascii_alphanumeric()) {
            return existing.to_string();
        }
    }
    let id = random_id(32);
    if let Some(path) = path {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, &id);
    }
    id
}

/// Check the stored token with Twitch and settle on signed in or signed out.
/// When signed in, starts the polling, watching and clock loops.
pub async fn connect(app: AppHandle) {
    let engine = app.state::<Engine>();
    let generation = engine.end_session();
    update(&app, false, |c| c.set_status(Status::Loading));

    let Some(token) = auth::load_token() else {
        update(&app, false, Collector::signed_out);
        return;
    };
    match auth::validate(&engine.http, &token).await {
        Ok(Some(identity)) => {
            let gql = Gql::new(engine.http.clone(), token.access_token.clone(), device_id(&app));
            let session = Session { gql, user_id: identity.user_id, access_token: token.access_token.clone() };
            *engine.session.lock().unwrap() = Some(Arc::new(session));
            update(&app, false, |c| c.signed_in(&identity.login));
            tauri::async_runtime::spawn(poll_loop(app.clone(), generation));
            tauri::async_runtime::spawn(watch_loop(app.clone(), generation));
            tauri::async_runtime::spawn(tick_loop(app.clone(), generation));
            if hermes_enabled() {
                tauri::async_runtime::spawn(hermes_loop(app.clone(), generation));
            }
        }
        Ok(None) => {
            auth::delete_token();
            update(&app, false, Collector::signed_out);
        }
        Err(e) => fail(&app, "token check", &e),
    }
}

/// Ask Twitch for a code, show it, and wait for the user to approve it.
pub async fn sign_in(app: AppHandle) {
    let engine = app.state::<Engine>();
    let generation = engine.sign_in_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let http = engine.http.clone();
    let superseded = || app.state::<Engine>().sign_in_generation.load(Ordering::SeqCst) != generation;

    let code = match auth::request_device_code(&http).await {
        Ok(code) => code,
        Err(e) => return fail(&app, "sign-in", &e),
    };
    let expires_at = now_ms() + code.expires_in * 1000;
    update(&app, false, |c| {
        c.signed_out();
        c.set_sign_in(Some(SignIn {
            user_code: code.user_code.clone(),
            verification_uri: code.verification_uri.clone(),
            expires_at,
        }));
    });

    let token = loop {
        tokio::time::sleep(Duration::from_secs(code.interval.max(1))).await;
        if superseded() {
            return;
        }
        if now_ms() >= expires_at {
            update(&app, false, |c| c.set_sign_in(None));
            return;
        }
        match auth::poll_token(&http, &code.device_code).await {
            Ok(auth::Poll::Granted(token)) => break token,
            Ok(auth::Poll::Pending) => {}
            // A dropped connection while waiting is not worth abandoning the code for.
            Err(TwitchError::Network(_)) => {}
            Err(e) => return fail(&app, "sign-in", &e),
        }
    };
    if superseded() {
        return;
    }
    if let Err(e) = auth::save_token(&token) {
        crate::log!("could not store the Twitch token: {e}");
    }
    connect(app.clone()).await;
}

/// Withdraw a code the user no longer wants.
pub fn cancel_sign_in(app: &AppHandle) {
    app.state::<Engine>().sign_in_generation.fetch_add(1, Ordering::SeqCst);
    update(app, false, |c| c.set_sign_in(None));
}

pub fn sign_out(app: &AppHandle) {
    let engine = app.state::<Engine>();
    engine.sign_in_generation.fetch_add(1, Ordering::SeqCst);
    engine.end_session();
    auth::delete_token();
    update(app, false, Collector::signed_out);
}

/// Everything Twitch knows about one channel that the collector needs.
/// Read-only: looks, does not claim.
async fn poll_channel(gql: &Gql, login: &str) -> Result<Poll, TwitchError> {
    let Some(info) = gql.channel_info(login).await? else {
        return Ok(None);
    };
    let points = gql.points_context(login).await?;
    Ok(Some(Found {
        display_name: info.display_name,
        avatar: info.avatar,
        twitch_id: info.id,
        broadcast_id: info.stream.map(|s| s.broadcast_id),
        balance: points.as_ref().map(|p| p.balance),
        multiplier: points.as_ref().map_or(0.0, |p| p.multiplier),
        claim_id: points.and_then(|p| p.claim_id),
    }))
}

/// Ask Twitch whether the channel exists, then add it with what it reported.
pub async fn add_channel(app: &AppHandle, raw: &str) -> Result<Snapshot, String> {
    let name = app.state::<AppState>().collector.lock().unwrap().check_new_channel(raw)?;
    let Some(session) = app.state::<Engine>().session() else {
        return Err("Sign in to Twitch before adding channels.".into());
    };
    let login = name.to_lowercase();
    let found = match poll_channel(&session.gql, &login).await {
        Ok(Some(found)) => found,
        Ok(None) => return Err(format!("No Twitch channel called {name}.")),
        Err(e) => {
            crate::log!("add channel: {e}");
            return Err(add_error(&e).into());
        }
    };
    let (result, snapshot) = update(app, true, |c| c.add_resolved(&login, found, now_ms()));
    result.map(|()| snapshot)
}

/// Why a channel could not be looked up, in words for the Add field.
pub fn add_error(error: &TwitchError) -> &'static str {
    match error {
        TwitchError::Network(_) => "Can’t reach Twitch. Check your connection and try again.",
        TwitchError::Unauthorized => "Your Twitch sign-in has expired. Sign in again, then add the channel.",
        TwitchError::RateLimited => "Twitch is rate-limiting requests. Try again in a minute.",
        TwitchError::Unexpected(_) => "Twitch didn’t answer as expected. Try again in a moment.",
        TwitchError::Changed(_) => CHANGED_MESSAGE,
    }
}

/// The channels the signed-in user follows on Twitch, oldest follow first.
pub async fn followed(app: &AppHandle) -> Result<Vec<Followed>, String> {
    let engine = app.state::<Engine>();
    let Some(session) = engine.session() else {
        return Err("Sign in to Twitch to see the channels you follow.".into());
    };
    let follows = session.gql.followed().await.map_err(|e| {
        crate::log!("followed channels: {e}");
        followed_error(&e).to_string()
    })?;
    *engine.followed.lock().unwrap() =
        follows.iter().map(|f| (f.login.clone(), (f.display_name.clone(), f.avatar.clone()))).collect();
    Ok(follows
        .into_iter()
        .map(|f| Followed { login: f.login, name: f.display_name, avatar: f.avatar })
        .collect())
}

/// Add followed channels to the end of the list. Only channels from the last
/// fetch are accepted, so nothing is added that Twitch did not name.
pub fn add_followed(app: &AppHandle, logins: &[String]) -> Snapshot {
    let engine = app.state::<Engine>();
    let known: Vec<(String, String)> = {
        let followed = engine.followed.lock().unwrap();
        logins.iter().filter_map(|login| Some((login.clone(), followed.get(login)?.0.clone()))).collect()
    };
    let pictures: Vec<(String, Option<String>)> = {
        let followed = engine.followed.lock().unwrap();
        known.iter().map(|(login, _)| (login.clone(), followed.get(login).and_then(|f| f.1.clone()))).collect()
    };
    let snapshot = update(app, true, |c| {
        c.add_known(&known);
        for (login, avatar) in pictures {
            c.set_avatar(&login, avatar);
        }
    })
    .1;
    // The poll loop may be part-way through a long wait; the new channels should not wait with it.
    engine.poke.notify_one();
    snapshot
}

/// Why the followed channels could not be fetched, in words for the UI.
pub fn followed_error(error: &TwitchError) -> &'static str {
    match error {
        TwitchError::Network(_) => "Can’t reach Twitch. Check your connection and try again.",
        TwitchError::Unauthorized => "Your Twitch sign-in has expired. Sign in again, then try again.",
        TwitchError::RateLimited => "Twitch is rate-limiting requests. Try again in a minute.",
        TwitchError::Unexpected(_) => "Twitch didn’t answer as expected. Try again in a moment.",
        TwitchError::Changed(_) => CHANGED_MESSAGE,
    }
}

/// How long to wait between channels so each is asked about once per round.
pub fn gap_for(channels: usize) -> Duration {
    (ROUND / channels.max(1) as u32).max(MIN_GAP)
}

/// The wait after asking about a channel. A channel that is waiting for its
/// first answer (just added, or any channel right after start-up) is followed
/// quickly, so a list fills in within seconds; but only so many in a row, so
/// adding hundreds of followed channels is not a burst of requests. `quick_left`
/// is that allowance; it refills once nothing is waiting.
pub fn gap_after(first_check: bool, quick_left: &mut usize, channels: usize) -> Duration {
    if !first_check {
        *quick_left = FIRST_CHECK_QUICK;
        return gap_for(channels);
    }
    if *quick_left == 0 {
        return gap_for(channels);
    }
    *quick_left -= 1;
    FIRST_CHECK_GAP
}

/// How long to wait between watch reports so each channel gets one per interval.
pub fn watch_gap_for(channels: usize) -> Duration {
    WATCH_INTERVAL / channels.max(1) as u32
}

/// Claim a waiting bonus that `Collector::begin_claim` handed out.
async fn claim(app: &AppHandle, session: &Session, login: &str, (channel_id, claim_id): (String, String)) {
    match session.gql.claim_bonus(&channel_id, &claim_id).await {
        Ok(()) => {
            crate::log!("claimed a bonus on {login}");
            update(app, false, |c| c.bonus_claimed(login));
        }
        // Not worth a status change: the next poll offers the bonus again.
        Err(e) => {
            crate::log!("claim bonus on {login}: {e}");
            update_if(app, |c| {
                c.claim_failed(login);
                false
            });
        }
    }
}

/// Keep every listed channel's live state and balance current, and claim
/// bonuses on the channels being watched, until the session ends.
async fn poll_loop(app: AppHandle, generation: u64) {
    let engine = app.state::<Engine>();
    let stale = || !engine.is_current(generation);
    let mut odd_answers = 0u32;
    let mut backoff = RETRY_SECONDS;
    // Where the regular round has got to in the list.
    let mut turn = 0usize;
    let mut quick_left = FIRST_CHECK_QUICK;

    loop {
        let (logins, waiting) = {
            let state = app.state::<AppState>();
            let collector = state.collector.lock().unwrap();
            let logins: Vec<String> = collector.saved_channels().into_iter().map(|(login, _)| login).collect();
            (logins, collector.next_unchecked())
        };
        if logins.is_empty() {
            tokio::time::sleep(MIN_GAP).await;
            if stale() {
                return;
            }
            continue;
        }
        // A channel Twitch has not been asked about yet goes ahead of the
        // round: one just added should not sit at "0 points" for a minute.
        let first_check = waiting.is_some();
        let login = waiting.unwrap_or_else(|| {
            turn = (turn + 1) % logins.len();
            logins[turn].clone()
        });
        let gap = gap_after(first_check, &mut quick_left, logins.len());

        let Some(session) = engine.session() else { return };
        let asked_at = now_ms();
        let result = poll_channel(&session.gql, &login).await;
        if stale() {
            return;
        }
        match result {
            Ok(poll) => {
                odd_answers = 0;
                backoff = RETRY_SECONDS;
                let ((events, due), _) = update(&app, false, |c| {
                    let events = c.apply_poll_asked(&login, poll, asked_at, now_ms());
                    // A good answer ends any offline, error or rate-limited spell.
                    if c.status != Status::Ok {
                        c.set_status(Status::Ok);
                    }
                    (events, c.begin_claim(&login))
                });
                publish_activity(&app, &events);
                if let Some(due) = due {
                    claim(&app, &session, &login, due).await;
                }
            }
            Err(TwitchError::Unauthorized) => {
                engine.end_session();
                return fail(&app, "poll", &TwitchError::Unauthorized);
            }
            Err(TwitchError::RateLimited) => {
                for remaining in (1..=backoff).rev() {
                    update(&app, false, |c| {
                        c.set_status(Status::RateLimited);
                        c.retry_in = remaining;
                    });
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    if stale() {
                        return;
                    }
                }
                backoff = (backoff * 2).min(MAX_BACKOFF_SECONDS);
                continue;
            }
            Err(e @ TwitchError::Network(_)) => {
                fail(&app, "poll", &e);
                tokio::time::sleep(OFFLINE_RETRY).await;
                if stale() {
                    return;
                }
                continue;
            }
            Err(e @ (TwitchError::Unexpected(_) | TwitchError::Changed(_))) => {
                odd_answers += 1;
                crate::log!("poll {login}: {e}");
                // Do not ask about the same channel again at once; the round comes back to it.
                update(&app, false, |c| c.give_up_first_check(&login));
                if odd_answers >= ERROR_THRESHOLD {
                    fail(&app, "poll", &e);
                    tokio::time::sleep(ERROR_RETRY).await;
                    if stale() {
                        return;
                    }
                    continue;
                }
            }
        }
        // Wait out the gap, or less if channels have just been added.
        tokio::select! {
            _ = tokio::time::sleep(gap) => {}
            _ = engine.poke.notified() => {}
        }
        if stale() {
            return;
        }
    }
}

/// Report watch time for the channels in the slots. Does nothing while
/// paused, signed out or in an error state: `watch_targets` is empty then.
async fn watch_loop(app: AppHandle, generation: u64) {
    let engine = app.state::<Engine>();
    let stale = || !engine.is_current(generation);

    loop {
        let targets = app.state::<AppState>().collector.lock().unwrap().watch_targets();
        if targets.is_empty() {
            tokio::time::sleep(WATCH_IDLE).await;
            if stale() {
                return;
            }
            continue;
        }
        let gap = watch_gap_for(targets.len());

        for target in targets {
            let Some(session) = engine.session() else { return };
            let result = report(&app, &engine.http, &session, &target).await;
            if stale() {
                return;
            }
            match result {
                Ok(()) => {
                    update_if(&app, |c| {
                        c.watched(&target.login, now_ms());
                        false // the clock loop pushes the visible change
                    });
                }
                Err(TwitchError::Unauthorized) => {
                    engine.end_session();
                    return fail(&app, "watch", &TwitchError::Unauthorized);
                }
                // The poll loop notices a lost connection or a rate limit and
                // shows it; watching resumes when it clears.
                Err(TwitchError::Network(_)) | Err(TwitchError::RateLimited) => {}
                // A stream that has just ended fails a few reports before
                // polling notices. The collector marks the channel only once
                // it has failed several in a row, and says so on the slot;
                // the rest of the app carries on.
                Err(e @ (TwitchError::Unexpected(_) | TwitchError::Changed(_))) => {
                    crate::log!("watch {}: {e}", target.login);
                    update(&app, false, |c| {
                        // The address may have changed; look it up again next time.
                        c.set_spade_url(&target.login, None);
                        c.report_failed(&target.login);
                    });
                }
            }
            tokio::time::sleep(gap).await;
            if stale() {
                return;
            }
        }
    }
}

/// One watch report for one channel, looking up its spade address first if needed.
async fn report(
    app: &AppHandle,
    http: &Client,
    session: &Session,
    target: &crate::collector::WatchTarget,
) -> Result<(), TwitchError> {
    let spade_url = match &target.spade_url {
        Some(url) => url.clone(),
        None => {
            let url = session.gql.spade_url(&target.login).await?;
            update_if(app, |c| {
                c.set_spade_url(&target.login, Some(url.clone()));
                false
            });
            url
        }
    };
    let target = watch::Target {
        login: &target.login,
        channel_id: &target.channel_id,
        broadcast_id: &target.broadcast_id,
        spade_url: &spade_url,
    };
    watch::ping(http, &session.gql, &session.user_id, &target).await
}

// ---- Hermes -----------------------------------------------------------------

/// Keep a Hermes connection open for as long as the session lasts. Its
/// failures are logged and retried; they never change what the UI shows,
/// because polling still keeps everything current.
async fn hermes_loop(app: AppHandle, generation: u64) {
    let engine = app.state::<Engine>();
    loop {
        if !engine.is_current(generation) {
            return;
        }
        let Some(session) = engine.session() else { return };
        if let Err(why) = hermes_session(&app, generation, &session).await {
            crate::log!("hermes: {why}");
        }
        tokio::time::sleep(HERMES_RETRY).await;
    }
}

/// The topics to listen to: the user's own points, then each listed
/// channel's stream, in priority order, up to Twitch's limit.
pub fn wanted_topics(user_id: &str, channel_ids: &[String]) -> Vec<String> {
    std::iter::once(topic_user_points(user_id))
        .chain(channel_ids.iter().map(|id| topic_playback(id)))
        .take(HERMES_MAX_TOPICS)
        .collect()
}

/// One connection, from opening to its end. `Ok` means the session it
/// belonged to is over; `Err` says why the connection was lost.
async fn hermes_session(app: &AppHandle, generation: u64, session: &Arc<Session>) -> Result<(), String> {
    let engine = app.state::<Engine>();
    let mut socket = tokio::time::timeout(HERMES_CONNECT_TIMEOUT, hermes::connect())
        .await
        .map_err(|_| "connecting took too long".to_string())?
        .map_err(|e| e.to_string())?;

    let mut silence = HERMES_FIRST_SILENCE;
    let mut deadline = tokio::time::Instant::now() + silence;
    let mut authenticated = false;
    // Subscription id to topic. Twitch names the subscription, not the topic,
    // in each notification. Channels removed from the list stay subscribed
    // until the next connection; their news is ignored.
    let mut subscribed: HashMap<String, String> = HashMap::new();
    let mut last_notification: Option<String> = None;
    let mut reconcile = tokio::time::interval(HERMES_RECONCILE);

    let ended = loop {
        tokio::select! {
            message = socket.next() => {
                let text = match message {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Close(_))) | None => break Err("closed by Twitch".to_string()),
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => break Err(e.to_string()),
                };
                deadline = tokio::time::Instant::now() + silence;
                match hermes::parse(&text) {
                    Some(Incoming::Welcome { keepalive_sec }) => {
                        silence = Duration::from_secs(keepalive_sec) + HERMES_SILENCE_MARGIN;
                        deadline = tokio::time::Instant::now() + silence;
                        let hello = hermes::authenticate_message(&session.access_token);
                        if let Err(e) = socket.send(Message::text(hello)).await {
                            break Err(e.to_string());
                        }
                    }
                    Some(Incoming::Authenticated(Ok(()))) => {
                        crate::log!("hermes: connected");
                        authenticated = true;
                        // Subscribe on the next pass, which comes at once.
                        reconcile.reset_immediately();
                    }
                    Some(Incoming::Authenticated(Err(why))) => {
                        break Err(format!("sign-in refused: {why}"));
                    }
                    Some(Incoming::Subscribed { subscription_id, ok: false }) => {
                        // Left in the map so it is not asked for again on this connection.
                        let family = subscribed.get(&subscription_id).and_then(|t| t.split('.').next());
                        crate::log!("hermes: subscription refused ({})", family.unwrap_or("unknown"));
                    }
                    Some(Incoming::Notification { subscription_id, pubsub }) => {
                        // Twitch sometimes sends the same message twice in a row.
                        if last_notification.as_deref() == Some(pubsub.as_str()) {
                            continue;
                        }
                        if let Some(topic) = subscribed.get(&subscription_id) {
                            if let Some((channel_id, pushed)) = hermes::parse_pubsub(topic, &pubsub) {
                                on_push(app, generation, session, &channel_id, pushed);
                            }
                        }
                        last_notification = Some(pubsub);
                    }
                    // Twitch asks the client to move. Like the reference, open a
                    // fresh connection at the usual address rather than the one given.
                    Some(Incoming::Reconnect { .. }) => break Err("Twitch asked for a reconnect".to_string()),
                    Some(Incoming::Subscribed { .. } | Incoming::Keepalive | Incoming::Other(_)) | None => {}
                }
            }
            _ = reconcile.tick() => {
                if !engine.is_current(generation) {
                    break Ok(());
                }
                if !authenticated {
                    continue;
                }
                let channel_ids = app.state::<AppState>().collector.lock().unwrap().channel_ids();
                let mut failed = None;
                for topic in wanted_topics(&session.user_id, &channel_ids) {
                    if subscribed.len() >= HERMES_MAX_TOPICS || subscribed.values().any(|t| *t == topic) {
                        continue;
                    }
                    let (message, subscription_id) = hermes::subscribe_message(&topic);
                    if let Err(e) = socket.send(Message::text(message)).await {
                        failed = Some(e.to_string());
                        break;
                    }
                    subscribed.insert(subscription_id, topic);
                }
                if let Some(why) = failed {
                    break Err(why);
                }
            }
            _ = tokio::time::sleep_until(deadline) => break Err("no word from Twitch".to_string()),
        }
    };
    let _ = socket.close(None).await;
    ended
}

/// Feed news Twitch pushed about a channel into the collector, then do what
/// it calls for: claim a bonus, or look again at a channel that has gone live.
fn on_push(app: &AppHandle, generation: u64, session: &Arc<Session>, channel_id: &str, pushed: Pushed) {
    let login = app.state::<AppState>().collector.lock().unwrap().login_for(channel_id);
    let Some(login) = login else { return };
    // What arrived, for the console of a debug build. No ids, no balances.
    match &pushed {
        Pushed::Earned { points, reason, .. } => crate::log!("hermes: {login} earned {points} ({reason:?})"),
        Pushed::Spent { .. } => crate::log!("hermes: {login} points spent"),
        Pushed::BonusAvailable { .. } => crate::log!("hermes: {login} bonus available"),
        Pushed::StreamUp => crate::log!("hermes: {login} stream up"),
        Pushed::StreamDown => crate::log!("hermes: {login} stream down"),
    }
    let went_live = pushed == Pushed::StreamUp;

    let ((events, due), _) = update(app, false, |c| (c.apply_push(&login, pushed, now_ms()), c.begin_claim(&login)));
    publish_activity(app, &events);

    if let Some(due) = due {
        let (app, session, login) = (app.clone(), session.clone(), login.clone());
        tauri::async_runtime::spawn(async move { claim(&app, &session, &login, due).await });
    }
    if went_live {
        tauri::async_runtime::spawn(recheck(app.clone(), generation, session.clone(), login));
    }
}

/// Ask about one channel out of turn, shortly after Twitch announced its
/// stream. If the API is not caught up yet, the regular round picks it up.
async fn recheck(app: AppHandle, generation: u64, session: Arc<Session>, login: String) {
    tokio::time::sleep(STREAM_UP_RECHECK).await;
    let current = || app.state::<Engine>().is_current(generation);
    if !current() {
        return;
    }
    let asked_at = now_ms();
    // A failure here is left to the poll loop, which reports it properly.
    let Ok(poll) = poll_channel(&session.gql, &login).await else { return };
    if !current() {
        return;
    }
    let ((events, due), _) = update(&app, false, |c| {
        (c.apply_poll_asked(&login, poll, asked_at, now_ms()), c.begin_claim(&login))
    });
    publish_activity(&app, &events);
    if let Some(due) = due {
        claim(&app, &session, &login, due).await;
    }
}

/// Advance the watch clock and payout bar once a second while watching.
async fn tick_loop(app: AppHandle, generation: u64) {
    let engine = app.state::<Engine>();
    let mut every_second = tokio::time::interval(Duration::from_secs(1));
    loop {
        every_second.tick().await;
        if !engine.is_current(generation) {
            return;
        }
        let mut notes = Vec::new();
        update_if(&app, |c| {
            let changed = c.tick(now_ms());
            notes = c.take_notes();
            changed
        });
        for note in notes {
            crate::log!("{note}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_map_to_the_states_the_ui_draws() {
        assert_eq!(status_for(&TwitchError::Network("x".into())), Status::Offline);
        assert_eq!(status_for(&TwitchError::Unauthorized), Status::LoggedOut);
        assert_eq!(status_for(&TwitchError::RateLimited), Status::RateLimited);
        assert_eq!(status_for(&TwitchError::Unexpected("x".into())), Status::Error);
        assert_eq!(status_for(&TwitchError::Changed("x".into())), Status::Changed);
    }

    #[test]
    fn channels_share_one_round_but_never_crowd() {
        assert_eq!(gap_for(1), Duration::from_secs(60));
        assert_eq!(gap_for(6), Duration::from_secs(10));
        assert_eq!(gap_for(20), Duration::from_secs(3));
        // A very long list still leaves two seconds between channels.
        assert_eq!(gap_for(100), Duration::from_secs(2));
        assert_eq!(gap_for(0), Duration::from_secs(60));
    }

    #[test]
    fn hermes_listens_to_the_user_first_then_channels_up_to_the_limit() {
        let ids: Vec<String> = (1..=3).map(|n| n.to_string()).collect();
        assert_eq!(
            wanted_topics("999", &ids),
            vec![
                "community-points-user-v1.999",
                "video-playback-by-id.1",
                "video-playback-by-id.2",
                "video-playback-by-id.3",
            ]
        );
        let many: Vec<String> = (1..=80).map(|n| n.to_string()).collect();
        let topics = wanted_topics("999", &many);
        assert_eq!(topics.len(), 50);
        assert_eq!(topics[0], "community-points-user-v1.999");
        assert_eq!(topics[49], "video-playback-by-id.49");
    }

    #[test]
    fn first_checks_go_quickly_but_only_twenty_in_a_row() {
        let mut quick = FIRST_CHECK_QUICK;
        for _ in 0..20 {
            assert_eq!(gap_after(true, &mut quick, 300), FIRST_CHECK_GAP);
        }
        // The twenty-first waiting channel is asked about at the usual pace.
        assert_eq!(gap_after(true, &mut quick, 300), Duration::from_secs(2));
        // A regular turn means nothing is waiting, and the allowance refills.
        assert_eq!(gap_after(false, &mut quick, 6), Duration::from_secs(10));
        assert_eq!(gap_after(true, &mut quick, 6), FIRST_CHECK_GAP);
    }

    #[test]
    fn each_watched_channel_is_reported_every_twenty_seconds() {
        assert_eq!(watch_gap_for(1), Duration::from_secs(20));
        assert_eq!(watch_gap_for(2), Duration::from_secs(10));
    }
}
