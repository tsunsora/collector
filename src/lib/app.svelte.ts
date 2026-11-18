// UI state for one window. The backend owns the collector state and pushes
// snapshots; this class mirrors them and adds what is purely presentational
// (current section, theme, the undo strip, live-region text).
import { backend } from './backend';
import { startOfDay } from './format';
import { still } from './motion.svelte';
import type {
  ActivityEvent,
  Channel,
  Followed,
  Section,
  SignIn,
  Snapshot,
  Status,
  ThemeMode,
  UpdateInfo,
} from './types';

/** Twitch pays watch points every five minutes of watching. */
export const PAYOUT_SECONDS = 300;
const NAME_RE = /^[A-Za-z0-9_]{4,25}$/;

function readTheme(): ThemeMode {
  try {
    const t = localStorage.getItem('theme');
    if (t === 'light' || t === 'dark') return t;
  } catch {}
  return 'system';
}

/**
 * Turn colour fades off for the moment the theme changes. Two frames: one for
 * the new colours to land with fades off, one more before they come back on.
 */
function withoutFades(change: () => void) {
  const root = document.documentElement;
  const done = () => delete root.dataset.switching;
  root.dataset.switching = '';
  change();
  requestAnimationFrame(() => requestAnimationFrame(done));
  // A hidden window draws no frames; make sure the fades still come back.
  setTimeout(done, 400);
}

function applyTheme(mode: ThemeMode) {
  const root = document.documentElement;
  withoutFades(() => {
    if (mode === 'system') delete root.dataset.theme;
    else root.dataset.theme = mode;
  });
}

class App {
  // Mirrored from the backend. Loading until the first snapshot arrives.
  status = $state<Status>('loading');
  paused = $state(false);
  channels = $state.raw<Channel[]>([]);
  slotIds = $state.raw<(string | null)[]>([null, null]);
  account = $state<string | null>(null);
  /** The code to enter at Twitch while a sign-in is waiting for approval. */
  signInCode = $state.raw<SignIn | null>(null);
  retryIn = $state(0);
  openAtLogin = $state(false);
  startPaused = $state(false);
  /** The welcome tour has been seen or skipped. */
  welcomed = $state(true);
  version = $state('');
  update = $state.raw<UpdateInfo>({ state: 'off', version: null, notes: null, checkedAt: null, message: null });
  autoUpdate = $state(true);
  streakFirst = $state(true);
  events = $state.raw<ActivityEvent[]>([]);

  // Local to this window.
  section = $state<Section>('watching');
  theme = $state<ThemeMode>(readTheme());
  /** Name of the channel the undo strip is offering to restore. */
  removed = $state<{ id: string; name: string } | null>(null);
  /** Polite live-region text for reorder, add and remove. */
  announcement = $state('');
  /** The channels the user follows on Twitch. Null until first asked for. */
  followed = $state.raw<Followed[] | null>(null);
  followedLoading = $state(false);
  followedError = $state<string | null>(null);
  /** The welcome tour is showing over the window. */
  touring = $state(false);
  /** Which of the tour's stops is showing. */
  tourStop = $state(0);
  #tourOffered = false;

  /** Collecting right now: connected, signed in and not paused. */
  active = $derived(this.status === 'ok' && !this.paused);
  slots = $derived(this.slotIds.map((id) => this.channels.find((c) => c.id === id) ?? null));
  queue = $derived(this.channels.filter((c) => !this.slotIds.includes(c.id)));
  watchingCount = $derived(this.slotIds.filter(Boolean).length);

  /** States in which there is nothing to show but a notice. */
  blocked = $derived(
    this.status === 'loading' || this.status === 'loggedOut' || this.status === 'error' || this.status === 'changed',
  );
  canPause = $derived(!this.blocked && this.channels.length > 0 && this.status !== 'offline');

  /** One line saying what the collector is doing. Shown in both windows. */
  summary = $derived.by(() => {
    switch (this.status) {
      case 'loading':
        return 'Loading channels…';
      case 'loggedOut':
        return this.signInCode ? 'Waiting for you at Twitch' : 'Signed out of Twitch';
      case 'error':
        return 'Can’t reach Twitch';
      case 'changed':
        return 'Needs a newer version';
      case 'offline':
        return 'No connection';
      case 'rateLimited':
        return 'Waiting on Twitch';
    }
    const total = this.channels.length;
    if (total === 0) return 'No channels yet';
    if (this.paused) return 'Paused';
    // Until Twitch has answered about every channel, "none live" is not yet true.
    if (this.watchingCount === 0 && this.channels.some((c) => !c.checked)) return 'Checking channels…';
    if (this.watchingCount === 0) return 'No channels are live';
    return `Watching ${this.watchingCount} of ${total} ${total === 1 ? 'channel' : 'channels'}`;
  });

  earnedToday = $derived.by(() => {
    const from = startOfDay(Date.now());
    return this.events.reduce((sum, e) => (e.at >= from ? sum + e.points : sum), 0);
  });

  #apply = (s: Snapshot) => {
    this.status = s.status;
    this.paused = s.paused;
    this.channels = s.channels;
    this.slotIds = s.slots;
    this.account = s.account;
    this.signInCode = s.signIn;
    this.retryIn = s.retryIn;
    this.openAtLogin = s.openAtLogin;
    this.startPaused = s.startPaused;
    this.welcomed = s.welcomed;
    this.version = s.version;
    this.update = s.update;
    this.autoUpdate = s.autoUpdate;
    this.streakFirst = s.streakFirst;
    // A fresh start gets the tour once, as soon as the first state arrives.
    if (!s.welcomed && !this.#tourOffered) this.startTour();
    this.#tourOffered = true;
    if (this.removed && s.channels.some((c) => c.id === this.removed!.id)) this.removed = null;
  };

  /** Run a backend mutation and mirror its result. False if it failed. */
  async #run(call: Promise<Snapshot>): Promise<boolean> {
    try {
      this.#apply(await call);
      return true;
    } catch (e) {
      console.error(e);
      return false;
    }
  }

  async init() {
    backend.onState(this.#apply);
    backend.onActivity((e) => {
      this.events = [e, ...this.events.filter((x) => x.id !== e.id)];
    });
    backend.onNavigate((section) => (this.section = section));
    // Windows switching between light and dark is a theme change too.
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => withoutFades(() => {}));
    // A theme change in one window reaches the others through localStorage.
    window.addEventListener('storage', (e) => {
      if (e.key !== 'theme') return;
      this.theme = readTheme();
      applyTheme(this.theme);
    });
    try {
      const [state, events] = await Promise.all([backend.getState(), backend.getActivity()]);
      this.#apply(state);
      this.events = events;
    } catch (e) {
      console.error(e);
      this.status = 'error';
    }
  }

  togglePause() {
    return this.#run(backend.setPaused(!this.paused));
  }

  retry() {
    return this.#run(backend.reconnect());
  }

  signIn() {
    return this.#run(backend.signIn());
  }

  cancelSignIn() {
    return this.#run(backend.cancelSignIn());
  }

  openSignInPage() {
    void backend.openSignInPage().catch(console.error);
  }

  signOut() {
    return this.#run(backend.signOut());
  }

  checkUpdate() {
    return this.#run(backend.checkUpdate());
  }

  downloadUpdate() {
    return this.#run(backend.downloadUpdate());
  }

  /** Restarts the app into the new version. Says so if it could not. */
  async installUpdate() {
    try {
      await backend.installUpdate();
    } catch (e) {
      this.announcement = typeof e === 'string' ? e : 'The update couldn’t be installed.';
    }
  }

  setStreakFirst(enabled: boolean) {
    return this.#run(backend.setStreakFirst(enabled));
  }

  setAutoUpdate(enabled: boolean) {
    return this.#run(backend.setAutoUpdate(enabled));
  }

  showLog() {
    void backend.showLog().catch(console.error);
  }

  quit() {
    void backend.quit().catch(console.error);
  }

  setOpenAtLogin(enabled: boolean) {
    return this.#run(backend.setOpenAtLogin(enabled));
  }

  setStartPaused(enabled: boolean) {
    return this.#run(backend.setStartPaused(enabled));
  }

  setTheme(mode: ThemeMode) {
    this.theme = mode;
    // The whole window cross-fades as one picture, so no single element is
    // ever caught between its old colour and its new one.
    const doc = document as Document & { startViewTransition?: (change: () => void) => unknown };
    if (doc.startViewTransition && !still()) doc.startViewTransition(() => applyTheme(mode));
    else applyTheme(mode);
    try {
      if (mode === 'system') localStorage.removeItem('theme');
      else localStorage.setItem('theme', mode);
    } catch {}
  }

  /** Resolves with an error message, or null when the channel was added. */
  async addChannel(raw: string): Promise<string | null> {
    const name = raw.trim().replace(/^@/, '');
    if (!name) return 'Enter a channel name.';
    if (!NAME_RE.test(name)) return 'Channel names are 4 to 25 letters, numbers or underscores.';
    const existing = this.channels.find((c) => c.id === name.toLowerCase());
    if (existing) return `${existing.name} is already in your list.`;
    try {
      this.#apply(await backend.addChannel(name));
    } catch (e) {
      return typeof e === 'string' ? e : 'That channel couldn’t be added. Try again.';
    }
    this.removed = null;
    this.announcement = `${name} added at position ${this.channels.length}.`;
    return null;
  }

  /** Fetch the followed channels. Skipped if already loaded, unless `refresh`. */
  async loadFollowed(refresh = false) {
    if (this.followedLoading || (this.followed && !refresh)) return;
    this.followedLoading = true;
    this.followedError = null;
    try {
      this.followed = await backend.getFollowed();
    } catch (e) {
      this.followedError = typeof e === 'string' ? e : 'The channels you follow couldn’t be loaded. Try again.';
    }
    this.followedLoading = false;
  }

  /** Add followed channels to the end of the list. False if it failed. */
  async addFollowed(logins: string[]): Promise<boolean> {
    const before = this.channels.length;
    if (!(await this.#run(backend.addFollowed(logins)))) return false;
    const added = this.channels.length - before;
    this.removed = null;
    this.announcement =
      added === 1
        ? `${this.channels[before].name} added at position ${this.channels.length}.`
        : `${added} channels added to the end of your list.`;
    return true;
  }

  async removeChannel(id: string) {
    const channel = this.channels.find((c) => c.id === id);
    if (!channel || !(await this.#run(backend.removeChannel(id)))) return;
    this.removed = { id, name: channel.name };
    this.announcement = `${channel.name} removed.`;
  }

  async undoRemove() {
    const removed = this.removed;
    if (!removed || !(await this.#run(backend.undoRemove()))) return;
    this.removed = null;
    const position = this.channels.findIndex((c) => c.id === removed.id) + 1;
    this.announcement = `${removed.name} restored at position ${position}.`;
  }

  /** Move a channel to a position (0-based). False if nothing changed. */
  async moveChannelTo(id: string, index: number): Promise<boolean> {
    const from = this.channels.findIndex((c) => c.id === id);
    if (from < 0 || index === from || index < 0 || index >= this.channels.length) return false;
    const name = this.channels[from].name;
    if (!(await this.#run(backend.moveChannel(id, index)))) return false;
    this.announcement = `${name} moved to position ${index + 1} of ${this.channels.length}.`;
    return true;
  }

  /** Move a channel up (-1) or down (+1). */
  moveChannel(id: string, delta: -1 | 1) {
    return this.moveChannelTo(id, this.channels.findIndex((c) => c.id === id) + delta);
  }

  /** Show the main window, on a section if one is given. */
  openMain(section?: Section) {
    if (section) this.section = section;
    void backend.openMain(section);
  }

  startTour() {
    this.tourStop = 0;
    this.touring = true;
  }

  /** However the tour ends (finished, skipped, Escape), it is not offered again. */
  endTour() {
    this.touring = false;
    if (!this.welcomed) void this.#run(backend.setWelcomed(true));
  }
}

export const app = new App();
