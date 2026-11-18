// In-memory stand-in for the Rust backend, used when the UI runs in a plain
// browser. It behaves like the real thing: it owns the state, ticks once a
// second, and pushes snapshots. The preview bench drives it through the
// set* helpers at the bottom.
import type { Backend } from './backend';
import { seedChannels, seedEvents, seedFollowed } from './mock';
import type { ActivityEvent, Channel, Followed, Section, SignIn, Snapshot, Status, UpdateInfo } from './types';

const SLOT_COUNT = 2;
const PAYOUT_SECONDS = 300;
const PAYOUT_POINTS = 10;
const RETRY_SECONDS = 40;
const ACCOUNT = 'rin_afk';
const NAME_RE = /^[A-Za-z0-9_]{4,25}$/;
const VERSION = '0.5.0';
const NEXT_VERSION = '0.5.1';
const NOTES = 'Faster first balances, and a warning when points stop arriving.';
const update = (state: UpdateInfo['state'], more: Partial<UpdateInfo> = {}): UpdateInfo => ({
  state,
  version: null,
  notes: null,
  checkedAt: null,
  message: null,
  ...more,
});

class MockBackend implements Backend {
  #status: Status = 'ok';
  #paused = false;
  #channels = seedChannels();
  #events = seedEvents();
  #account: string | null = ACCOUNT;
  #signIn: SignIn | null = null;
  #retryIn = RETRY_SECONDS;
  #openAtLogin = true;
  #startPaused = false;
  // True, so the preview bench does not open on the tour. The bench has a button for it.
  #welcomed = true;
  #update = update('idle');
  #autoUpdate = true;
  #removed: { channel: Channel; index: number } | null = null;
  #followed: Followed[] | 'error' = seedFollowed();
  #timer: ReturnType<typeof setInterval> | undefined;

  #onState = new Set<(s: Snapshot) => void>();
  #onActivity = new Set<(e: ActivityEvent) => void>();
  #onNavigate = new Set<(s: Section) => void>();

  /** Live channels, streak-due first, then list order. */
  #slots(): (string | null)[] {
    const live = this.#channels.filter((c) => c.live);
    const picked = [...live.filter((c) => c.streakDue), ...live.filter((c) => !c.streakDue)];
    return Array.from({ length: SLOT_COUNT }, (_, i) => picked[i]?.id ?? null);
  }

  #snapshot(): Snapshot {
    return structuredClone({
      status: this.#status,
      paused: this.#paused,
      channels: this.#channels,
      slots: this.#slots(),
      account: this.#account,
      signIn: this.#signIn,
      retryIn: this.#retryIn,
      openAtLogin: this.#openAtLogin,
      startPaused: this.#startPaused,
      welcomed: this.#welcomed,
      version: VERSION,
      update: this.#update,
      autoUpdate: this.#autoUpdate,
    });
  }

  #push(): Promise<Snapshot> {
    const s = this.#snapshot();
    for (const cb of this.#onState) cb(s);
    return Promise.resolve(s);
  }

  #tick() {
    if (this.#status === 'rateLimited') {
      if (--this.#retryIn <= 0) this.setStatus('ok');
      else this.#push();
      return;
    }
    if (this.#status !== 'ok' || this.#paused) return;
    const watching = this.#slots();
    for (const c of this.#channels) {
      if (!watching.includes(c.id)) continue;
      c.watchSeconds += 1;
      c.payout += 1 / PAYOUT_SECONDS;
      if (c.payout >= 1) {
        c.payout = 0;
        c.points += PAYOUT_POINTS;
        this.#recordWatch(c.name);
      }
    }
    this.#push();
  }

  #recordWatch(channel: string) {
    let e = this.#events.find((x) => x.channel === channel);
    if (e && e.kind === 'watch' && Date.now() - e.at < 6 * 3_600_000) {
      e.points += PAYOUT_POINTS;
      e.amount = (e.amount ?? 0) + PAYOUT_SECONDS / 60;
      e.at = Date.now();
    } else {
      e = {
        id: crypto.randomUUID(),
        at: Date.now(),
        channel,
        kind: 'watch',
        points: PAYOUT_POINTS,
        amount: PAYOUT_SECONDS / 60,
      };
      this.#events.unshift(e);
    }
    for (const cb of this.#onActivity) cb(structuredClone(e));
  }

  getState() {
    this.#timer ??= setInterval(() => this.#tick(), 1000);
    return Promise.resolve(this.#snapshot());
  }

  getActivity() {
    return Promise.resolve(structuredClone(this.#events));
  }

  setPaused(paused: boolean) {
    this.#paused = paused;
    return this.#push();
  }

  /** Takes a moment, like the real lookup. Names starting "zzz" do not exist. */
  async addChannel(raw: string) {
    const name = raw.trim();
    if (!NAME_RE.test(name)) throw 'Channel names are 4 to 25 letters, numbers or underscores.';
    await new Promise((resolve) => setTimeout(resolve, 600));
    if (name.toLowerCase().startsWith('zzz')) throw `No Twitch channel called ${name}.`;
    const existing = this.#channels.find((c) => c.id === name.toLowerCase());
    if (existing) throw `${existing.name} is already in your list.`;
    this.#channels.push({
      id: name.toLowerCase(),
      name,
      live: false,
      points: 0,
      watchSeconds: 0,
      payout: 0,
      payoutPoints: PAYOUT_POINTS,
      streakDue: false,
      checked: true,
      pointsKnown: true,
      problem: null,
      avatar: null,
    });
    this.#removed = null;
    return this.#push();
  }

  /** Takes a moment, like the real fetch. */
  async getFollowed() {
    await new Promise((resolve) => setTimeout(resolve, 500));
    if (this.#followed === 'error') throw 'Can’t reach Twitch. Check your connection and try again.';
    return structuredClone(this.#followed);
  }

  addFollowed(logins: string[]) {
    const followed = this.#followed === 'error' ? [] : this.#followed;
    for (const f of followed) {
      if (!logins.includes(f.login) || this.#channels.some((c) => c.id === f.login)) continue;
      this.#channels.push({
        id: f.login,
        name: f.name,
        live: false,
        points: 0,
        watchSeconds: 0,
        payout: 0,
        payoutPoints: PAYOUT_POINTS,
        streakDue: false,
        checked: false,
        pointsKnown: false,
        problem: null,
        avatar: f.avatar,
      });
      this.#removed = null;
    }
    // Like the real thing: Twitch is asked about the new channels shortly after.
    setTimeout(() => {
      for (const c of this.#channels) {
        if (c.checked) continue;
        c.checked = true;
        c.pointsKnown = true;
        c.points = (c.id.length * 1370) % 9000;
      }
      this.#push();
    }, 900);
    return this.#push();
  }

  removeChannel(id: string) {
    const index = this.#channels.findIndex((c) => c.id === id);
    if (index >= 0) this.#removed = { channel: this.#channels.splice(index, 1)[0], index };
    return this.#push();
  }

  undoRemove() {
    if (this.#removed) {
      const { channel, index } = this.#removed;
      this.#channels.splice(Math.min(index, this.#channels.length), 0, channel);
      this.#removed = null;
    }
    return this.#push();
  }

  moveChannel(id: string, index: number) {
    const from = this.#channels.findIndex((c) => c.id === id);
    if (from >= 0) {
      const [channel] = this.#channels.splice(from, 1);
      this.#channels.splice(Math.max(0, Math.min(index, this.#channels.length)), 0, channel);
    }
    return this.#push();
  }

  #connect() {
    this.setStatus('loading');
    setTimeout(() => this.setStatus('ok'), 900);
    return Promise.resolve(this.#snapshot());
  }

  /** Issues a made-up code. "Open Twitch" stands in for the user approving it. */
  signIn() {
    this.setStatus('loggedOut');
    this.#signIn = {
      userCode: 'BDKQ7WXM',
      verificationUri: 'https://www.twitch.tv/activate',
      expiresAt: Date.now() + 30 * 60_000,
    };
    return this.#push();
  }

  cancelSignIn() {
    this.#signIn = null;
    return this.#push();
  }

  openSignInPage() {
    if (this.#signIn) setTimeout(() => this.#connect(), 1500);
    return Promise.resolve();
  }

  reconnect() {
    return this.#connect();
  }

  signOut() {
    this.setStatus('loggedOut');
    return Promise.resolve(this.#snapshot());
  }

  setOpenAtLogin(enabled: boolean) {
    this.#openAtLogin = enabled;
    return this.#push();
  }

  setStartPaused(enabled: boolean) {
    this.#startPaused = enabled;
    return this.#push();
  }

  /** Finds a newer version, and with "Update automatically" on, fetches it. */
  checkUpdate() {
    if (this.#update.state === 'off') return this.#push();
    this.#update = update('checking');
    const found = { version: NEXT_VERSION, notes: NOTES, checkedAt: Date.now() };
    setTimeout(() => {
      if (!this.#autoUpdate) return this.setUpdate(update('available', found));
      this.setUpdate(update('downloading', found));
      setTimeout(() => this.setUpdate(update('ready', found)), 900);
    }, 600);
    return this.#push();
  }

  downloadUpdate() {
    const found = { version: NEXT_VERSION, notes: NOTES, checkedAt: Date.now() };
    this.#update = update('downloading', found);
    setTimeout(() => this.setUpdate(update('ready', found)), 900);
    return this.#push();
  }

  /** The real thing restarts the app; here the update simply counts as done. */
  installUpdate() {
    this.setUpdate(update('idle', { checkedAt: Date.now() }));
    return Promise.resolve();
  }

  setAutoUpdate(enabled: boolean) {
    this.#autoUpdate = enabled;
    return this.#push();
  }

  showLog() {
    return Promise.resolve();
  }

  quit() {
    return Promise.resolve();
  }

  setWelcomed(welcomed: boolean) {
    this.#welcomed = welcomed;
    return this.#push();
  }

  openMain(section?: Section) {
    if (location.hash === '#popover') location.hash = '#main';
    if (section) for (const cb of this.#onNavigate) cb(section);
    return Promise.resolve();
  }

  onState(cb: (s: Snapshot) => void) {
    this.#onState.add(cb);
  }

  onActivity(cb: (e: ActivityEvent) => void) {
    this.#onActivity.add(cb);
  }

  onNavigate(cb: (s: Section) => void) {
    this.#onNavigate.add(cb);
  }

  // Preview bench controls. Not part of the Backend contract.

  setStatus(status: Status) {
    this.#status = status;
    this.#signIn = null;
    this.#retryIn = RETRY_SECONDS;
    this.#account = status === 'loggedOut' ? null : ACCOUNT;
    this.#push();
  }

  setUpdate(info: UpdateInfo) {
    this.#update = info;
    this.#push();
  }

  /** What the followed-channels fetch returns: a list, or a failure. */
  setFollowed(followed: Followed[] | 'error') {
    this.#followed = followed;
  }

  setChannels(channels: Channel[]) {
    this.#channels = structuredClone(channels);
    this.#removed = null;
    this.#push();
  }
}

export const mock = new MockBackend();
