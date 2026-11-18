import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Backend } from './backend';
import type { ActivityEvent, Section, Snapshot } from './types';

// Command and event names match src-tauri/src/lib.rs.
export const tauriBackend: Backend = {
  getState: () => invoke('get_state'),
  getActivity: () => invoke('get_activity'),
  setPaused: (paused) => invoke('set_paused', { paused }),
  addChannel: (name) => invoke('add_channel', { name }),
  getFollowed: () => invoke('get_followed'),
  addFollowed: (logins) => invoke('add_followed', { logins }),
  removeChannel: (id) => invoke('remove_channel', { id }),
  undoRemove: () => invoke('undo_remove'),
  moveChannel: (id, index) => invoke('move_channel', { id, index }),
  signIn: () => invoke('sign_in'),
  cancelSignIn: () => invoke('cancel_sign_in'),
  openSignInPage: () => invoke('open_sign_in_page'),
  signOut: () => invoke('sign_out'),
  reconnect: () => invoke('reconnect'),
  setOpenAtLogin: (enabled) => invoke('set_open_at_login', { enabled }),
  setStartPaused: (enabled) => invoke('set_start_paused', { enabled }),
  checkUpdate: () => invoke('check_update'),
  downloadUpdate: () => invoke('download_update'),
  installUpdate: () => invoke('install_update'),
  setAutoUpdate: (enabled) => invoke('set_auto_update', { enabled }),
  setStreakFirst: (enabled) => invoke('set_streak_first', { enabled }),
  showLog: () => invoke('show_log'),
  quit: () => invoke('quit_now'),
  setWelcomed: (welcomed) => invoke('set_welcomed', { welcomed }),
  openMain: (section) => invoke('open_main', { section: section ?? null }),
  onState: (cb) => void listen<Snapshot>('state', (e) => cb(e.payload)),
  onActivity: (cb) => void listen<ActivityEvent>('activity', (e) => cb(e.payload)),
  onNavigate: (cb) => void listen<Section>('navigate', (e) => cb(e.payload)),
};
