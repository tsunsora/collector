// The contract between the UI and the collector. Inside Tauri it is served by
// Rust commands and events (src-tauri/src/lib.rs); in a plain browser by an
// in-memory mock, so the UI can be built and audited without the app shell.
import type { ActivityEvent, Followed, Section, Snapshot } from './types';
import { mock } from './mockBackend';
import { tauriBackend } from './tauriBackend';

export interface Backend {
  getState(): Promise<Snapshot>;
  getActivity(): Promise<ActivityEvent[]>;

  // Every mutation resolves with the resulting state. The backend also pushes
  // it to all windows, so the popover and the main window stay in step.
  setPaused(paused: boolean): Promise<Snapshot>;
  /** Rejects with a message to show under the field. */
  addChannel(name: string): Promise<Snapshot>;
  /**
   * The channels the signed-in user follows on Twitch, oldest follow first.
   * Rejects with a message to show in place of the list.
   */
  getFollowed(): Promise<Followed[]>;
  /** Adds followed channels, by login, to the end of the list. */
  addFollowed(logins: string[]): Promise<Snapshot>;
  removeChannel(id: string): Promise<Snapshot>;
  /** Puts back the most recently removed channel at its old position. */
  undoRemove(): Promise<Snapshot>;
  moveChannel(id: string, index: number): Promise<Snapshot>;
  /**
   * Starts sign-in. Twitch issues a code, which arrives in a later snapshot as
   * `signIn`; the user approves it in their browser and the state moves on.
   */
  signIn(): Promise<Snapshot>;
  cancelSignIn(): Promise<Snapshot>;
  /** Opens the page where the pending code is entered. */
  openSignInPage(): Promise<void>;
  signOut(): Promise<Snapshot>;
  reconnect(): Promise<Snapshot>;
  setOpenAtLogin(enabled: boolean): Promise<Snapshot>;
  setStartPaused(enabled: boolean): Promise<Snapshot>;
  /** Look for an update now. The outcome arrives in later snapshots, as `update`. */
  checkUpdate(): Promise<Snapshot>;
  /** Download the update that was announced. */
  downloadUpdate(): Promise<Snapshot>;
  /** Put the downloaded update in place and start the app again. Rejects with a message. */
  installUpdate(): Promise<void>;
  setAutoUpdate(enabled: boolean): Promise<Snapshot>;
  /** Show the log file in its folder. */
  showLog(): Promise<void>;
  /** Stop collecting and close the app. A downloaded update installs on the way out. */
  quit(): Promise<void>;
  /** Records that the welcome tour has been seen or skipped. */
  setWelcomed(welcomed: boolean): Promise<Snapshot>;

  /** Show the main window, on a section if one is given, otherwise where it was. */
  openMain(section?: Section): Promise<void>;

  onState(cb: (state: Snapshot) => void): void;
  /** A new activity entry, or an updated one with the same id. */
  onActivity(cb: (event: ActivityEvent) => void): void;
  onNavigate(cb: (section: Section) => void): void;
}

export const inTauri = '__TAURI_INTERNALS__' in window;

export const backend: Backend = inTauri ? tauriBackend : mock;
