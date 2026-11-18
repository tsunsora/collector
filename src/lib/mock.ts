// Mock data for building and reviewing the UI without the Rust backend.
import type { ActivityEvent, Channel } from './types';

const ch = (
  name: string,
  live: boolean,
  points: number,
  watchSeconds: number,
  payout: number,
  streakDue: boolean,
): Channel => ({
  id: name.toLowerCase(),
  name,
  live,
  points,
  watchSeconds,
  payout,
  payoutPoints: 10,
  streakDue,
  checked: true,
  pointsKnown: true,
  problem: null,
  avatar: null,
});

export const seedChannels = (): Channel[] => [
  ch('CohhCarnage', true, 48_210, 4_335, 0.34, false),
  ch('Northernlion', true, 126_480, 1_580, 0.72, false),
  ch('Lirik', true, 9_870, 0, 0, true),
  ch('itsHafu', false, 31_150, 0, 0, false),
  ch('DansGaming', true, 2_340, 0, 0, false),
  ch('AdmiralBahroo', false, 77_905, 0, 0, false),
];

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

/** Followed channels for the mock. Three are already in the seeded list. */
export function seedFollowed() {
  return [
    'CohhCarnage', 'Lirik', 'itsHafu', 'Sodapoppin', 'shroud', 'Quin69', 'BurkeBlack', 'GiantWaffle',
    'Lobosjr', 'EsfandTV', 'summit1g', 'Kitboga', 'ZeRo', 'Elajjaz',
  ].map((name) => ({ login: name.toLowerCase(), name, avatar: null }));
}

/** A long list, as after adding every followed channel. Few are live. */
export function longList(): Channel[] {
  const extra = Array.from({ length: 54 }, (_, i) => {
    const name = `Streamer_${String(i + 1).padStart(2, '0')}`;
    return ch(name, i % 9 === 4, 1200 + i * 137, 0, 0, false);
  });
  return [...seedChannels(), ...extra];
}

export function seedEvents(now = Date.now()): ActivityEvent[] {
  let n = 0;
  const ev = (
    ago: number,
    channel: string,
    kind: ActivityEvent['kind'],
    points: number,
    amount?: number,
  ): ActivityEvent => ({ id: `seed-${n++}`, at: now - ago, channel, kind, points, amount });

  const events: ActivityEvent[] = [
    ev(4 * MIN, 'CohhCarnage', 'watch', 140, 70),
    ev(9 * MIN, 'Northernlion', 'bonus', 50),
    ev(11 * MIN, 'Northernlion', 'watch', 50, 25),
    ev(24 * MIN, 'CohhCarnage', 'bonus', 50),
    ev(41 * MIN, 'CohhCarnage', 'bonus', 50),
    ev(70 * MIN, 'CohhCarnage', 'streak', 450, 4),
    ev(3 * HOUR, 'DansGaming', 'watch', 60, 30),
    ev(3 * HOUR + 20 * MIN, 'DansGaming', 'streak', 300, 1),
  ];

  // Older days, so the 7- and 30-day ranges have something to show.
  const names = ['CohhCarnage', 'Northernlion', 'Lirik', 'itsHafu', 'AdmiralBahroo'];
  for (let d = 1; d <= 24; d++) {
    const a = names[d % names.length];
    const b = names[(d + 2) % names.length];
    const minutes = 60 + ((d * 35) % 150);
    events.push(ev(d * DAY + 2 * HOUR, a, 'watch', (minutes / 5) * 10, minutes));
    events.push(ev(d * DAY + 2 * HOUR + 30 * MIN, a, 'bonus', 50));
    if (d % 2 === 0) events.push(ev(d * DAY + 3 * HOUR, a, 'streak', 300 + 50 * (d % 4), 1 + (d % 4)));
    if (d % 3 !== 0) events.push(ev(d * DAY + 5 * HOUR, b, 'watch', 120, 60));
  }
  return events;
}
