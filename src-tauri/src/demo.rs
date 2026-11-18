//! Made-up data and a simulated collector, for exercising the shell and UI
//! without a Twitch account. Enabled with `COLLECTOR_DEMO=1`; never talks to
//! Twitch and never touches the stored token.

use crate::{
    collector::{blank_channel, now_ms, Collector, HOUR, MINUTE, PAYOUT_POINTS, PAYOUT_SECONDS, RETRY_SECONDS},
    model::{ActivityEvent, Channel, EventKind, Status},
};

pub const ACCOUNT: &str = "demo_account";

pub fn enabled() -> bool {
    std::env::var("COLLECTOR_DEMO").is_ok_and(|v| v == "1")
}

/// Whether only one copy may run. Always, except in demo mode, which never
/// talks to Twitch and so may run beside the real app. `COLLECTOR_DEMO_SINGLE=1`
/// puts the rule back for a demo copy, to test restarting under it.
pub fn single_instance() -> bool {
    !enabled() || std::env::var("COLLECTOR_DEMO_SINGLE").is_ok_and(|v| v == "1")
}

/// Made-up follows: (login, name). Some are already in the demo list.
pub fn followed() -> Vec<(String, String)> {
    [
        "CohhCarnage", "Lirik", "itsHafu", "Sodapoppin", "shroud", "Quin69", "BurkeBlack", "GiantWaffle",
        "Lobosjr", "EsfandTV", "summit1g", "Kitboga", "ZeRo", "Elajjaz",
    ]
    .iter()
    .map(|name| (name.to_lowercase(), name.to_string()))
    .collect()
}

impl Collector {
    pub fn demo(start_paused: bool, open_at_login: bool) -> Self {
        let ch = |name: &str, live, points, watch_seconds, payout, streak_due| Channel {
            live,
            points,
            watch_seconds,
            payout,
            streak_due,
            checked: true,
            points_known: true,
            ..blank_channel(&name.to_lowercase(), name)
        };
        let mut c = Self::new(start_paused, open_at_login);
        c.signed_in(ACCOUNT);
        c.channels = vec![
            ch("CohhCarnage", true, 48_210, 4_335, 0.34, false),
            ch("Northernlion", true, 126_480, 1_580, 0.72, false),
            ch("Lirik", true, 9_870, 0, 0.0, true),
            ch("itsHafu", false, 31_150, 0, 0.0, false),
            ch("DansGaming", true, 2_340, 0, 0.0, false),
            ch("AdmiralBahroo", false, 77_905, 0, 0.0, false),
        ];
        let now = now_ms();
        for (ago, channel, kind, points, amount) in [
            (4 * MINUTE, "CohhCarnage", EventKind::Watch, 140, Some(70)),
            (9 * MINUTE, "Northernlion", EventKind::Bonus, 50, None),
            (11 * MINUTE, "Northernlion", EventKind::Watch, 50, Some(25)),
            (24 * MINUTE, "CohhCarnage", EventKind::Bonus, 50, None),
            (70 * MINUTE, "CohhCarnage", EventKind::Streak, 450, Some(4)),
            (3 * HOUR, "DansGaming", EventKind::Watch, 60, Some(30)),
        ] {
            let id = c.event_id();
            c.events.push(ActivityEvent {
                id,
                at: now.saturating_sub(ago),
                channel: channel.to_string(),
                kind,
                points,
                amount,
            });
        }
        c
    }

    /// One second of simulated collecting. `None` when nothing changed;
    /// otherwise the activity entries that were added or updated.
    pub fn demo_tick(&mut self) -> Option<Vec<ActivityEvent>> {
        if self.status == Status::RateLimited {
            self.retry_in = self.retry_in.saturating_sub(1);
            if self.retry_in == 0 {
                self.status = Status::Ok;
                self.retry_in = RETRY_SECONDS;
            }
            return Some(Vec::new());
        }
        if self.status != Status::Ok || self.paused {
            return None;
        }
        let watching: Vec<String> = self.slots().into_iter().flatten().collect();
        let mut paid = Vec::new();
        for c in self.channels.iter_mut().filter(|c| watching.contains(&c.id)) {
            c.watch_seconds += 1;
            c.payout += 1.0 / PAYOUT_SECONDS;
            if c.payout >= 1.0 {
                c.payout = 0.0;
                c.points += PAYOUT_POINTS;
                paid.push(c.name.clone());
            }
        }
        let now = now_ms();
        Some(paid.iter().map(|name| self.record_watch(name, PAYOUT_POINTS, 5, now)).collect())
    }
}
