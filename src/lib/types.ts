/** `changed`: Twitch's private API has moved on and this version no longer speaks it. */
export type Status = 'ok' | 'loading' | 'offline' | 'loggedOut' | 'error' | 'rateLimited' | 'changed';
/**
 * Something wrong with watching one channel. `reports`: Twitch is not
 * accepting the watch reports. `noPoints`: they are accepted, but no points
 * have arrived for a long while.
 */
export type Problem = 'reports' | 'noPoints';
export type UpdateState = 'off' | 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'failed';

/** Where the app is with updating itself. */
export interface UpdateInfo {
  /** `off`: updates are not set up in this build. */
  state: UpdateState;
  /** The newer version, once one has been found. */
  version: string | null;
  /** What the release says about itself. */
  notes: string | null;
  /** When the feed was last read. Milliseconds since the Unix epoch. */
  checkedAt: number | null;
  /** Why it failed, in words for the user. */
  message: string | null;
}
export type ThemeMode = 'system' | 'light' | 'dark';
export type Section = 'watching' | 'channels' | 'activity' | 'settings';
export type EventKind = 'watch' | 'bonus' | 'streak';

export interface Channel {
  id: string;
  name: string;
  live: boolean;
  /** Channel-point balance on this channel. */
  points: number;
  /** Seconds watched in the current session. */
  watchSeconds: number;
  /** 0..1 progress toward the next watch payout. Holds at 1 until it lands. */
  payout: number;
  /** Points one watch payout is worth here, with the account's multipliers. */
  payoutPoints: number;
  /** A watch streak is available on the current stream and not yet claimed. */
  streakDue: boolean;
  /**
   * Twitch has answered about this channel since it was added or the app
   * started. Until then `live` is not known.
   */
  checked: boolean;
  /**
   * `points` is a balance Twitch reported. False until the first answer, and
   * for a channel that has no channel points.
   */
  pointsKnown: boolean;
  /** Set while watching this channel is not working as it should. */
  problem: Problem | null;
  /** Address of the channel's profile picture, once Twitch has given it. */
  avatar: string | null;
}

export interface ActivityEvent {
  id: string;
  at: number;
  channel: string;
  kind: EventKind;
  points: number;
  /** watch: minutes watched. streak: streak day. */
  amount?: number;
}

/** A channel the signed-in user follows on Twitch, offered for adding. */
export interface Followed {
  login: string;
  /** The name with the channel's own capitalisation. */
  name: string;
  /** Address of the channel's profile picture. */
  avatar: string | null;
}

/** A sign-in code waiting for the user to approve it at Twitch. */
export interface SignIn {
  /** What the user types at `verificationUri`. */
  userCode: string;
  verificationUri: string;
  /** Milliseconds since the Unix epoch. */
  expiresAt: number;
}

/** Everything the backend pushes about the collector. Activity is separate. */
export interface Snapshot {
  status: Status;
  paused: boolean;
  /** In priority order. */
  channels: Channel[];
  /** Channel id per watch slot, null for a free slot. Decided by the backend. */
  slots: (string | null)[];
  account: string | null;
  /** Set while a sign-in code is waiting to be approved. */
  signIn: SignIn | null;
  /** Seconds until the next attempt while rate-limited. */
  retryIn: number;
  openAtLogin: boolean;
  startPaused: boolean;
  /** The welcome tour has been seen or skipped. False on a fresh start. */
  welcomed: boolean;
  /** This app's version. */
  version: string;
  update: UpdateInfo;
  /** Download updates as they appear, without being asked. */
  autoUpdate: boolean;
}
