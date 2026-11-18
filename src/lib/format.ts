const nf = new Intl.NumberFormat();
const timeFmt = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' });
const dayFmt = new Intl.DateTimeFormat(undefined, { weekday: 'short', day: 'numeric', month: 'short' });

export const num = (n: number) => nf.format(n);
export const signed = (n: number) => `+${nf.format(n)}`;

/** 4320 -> "1 h 12 min", 2520 -> "42 min" */
export function duration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const h = Math.floor(m / 60);
  return h > 0 ? `${h} h ${m % 60} min` : `${m} min`;
}

/** 200 -> "3:20" */
export function countdown(seconds: number): string {
  const s = Math.max(0, Math.ceil(seconds));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

export const clock = (at: number) => timeFmt.format(at);

export function startOfDay(at: number): number {
  const d = new Date(at);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

const DAY = 86_400_000;

export function dayLabel(at: number, now = Date.now()): string {
  const diff = Math.round((startOfDay(now) - startOfDay(at)) / DAY);
  if (diff === 0) return 'Today';
  if (diff === 1) return 'Yesterday';
  return dayFmt.format(at);
}
