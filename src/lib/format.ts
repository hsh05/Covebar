const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

const pad = (n: number) => String(n).padStart(2, "0");

/** "14:05" or "2:05", plus seconds and an AM/PM suffix separately. */
export function timeParts(d: Date, h24: boolean) {
  let h = d.getHours();
  let suffix = "";
  if (!h24) {
    suffix = h < 12 ? "AM" : "PM";
    h = h % 12 || 12;
  }
  return {
    hm: `${h24 ? pad(h) : h}:${pad(d.getMinutes())}`,
    s: pad(d.getSeconds()),
    suffix,
  };
}

/** "Tue, 17 Nov" */
export function shortDate(d: Date) {
  return `${WEEKDAYS[d.getDay()]}, ${d.getDate()} ${MONTHS[d.getMonth()]}`;
}

/** "Tuesday, 17 November 2026" */
export function longDate(d: Date) {
  return d.toLocaleDateString("en-GB", { weekday: "long", day: "numeric", month: "long", year: "numeric" });
}

/** 3725000 -> "1:02:05", 65000 -> "1:05". */
export function duration(ms: number, opts: { tenths?: boolean; hundredths?: boolean } = {}) {
  const total = Math.max(0, ms);
  const s = Math.floor(total / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  let out = h ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`;
  if (opts.hundredths) out += `.${pad(Math.floor((total % 1000) / 10))}`;
  else if (opts.tenths) out += `.${Math.floor((total % 1000) / 100)}`;
  return out;
}

/** Countdown text that rounds up, so "0:01" shows until the very end. */
export function countdown(ms: number) {
  return duration(Math.ceil(Math.max(0, ms) / 1000) * 1000);
}

/** 1536 -> "1.5 KB". */
export function bytes(n: number) {
  const units = ["bytes", "KB", "MB", "GB", "TB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return i === 0 ? `${v} ${units[0]}` : `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

/** 90 -> "1:30", seconds as m:ss for media positions. */
export function mmss(sec: number) {
  return duration(Math.floor(Math.max(0, sec)) * 1000);
}

/** Preset label: 60 -> "1 min", 3600 -> "1 h", 90 -> "1:30". */
export function presetLabel(sec: number) {
  if (sec % 3600 === 0) return `${sec / 3600} h`;
  if (sec % 60 === 0) return `${sec / 60} min`;
  return duration(sec * 1000);
}

/** Time until/since, roughly: 8100000 -> "2 h 15 min", 360000000 -> "4 d 4 h". */
export function span(ms: number) {
  const min = Math.max(0, Math.round(ms / 60000));
  if (min < 1) return "less than a minute";
  if (min < 60) return `${min} min`;
  const h = Math.floor(min / 60);
  if (h < 24) return min % 60 ? `${h} h ${min % 60} min` : `${h} h`;
  const d = Math.floor(h / 24);
  return h % 24 ? `${d} d ${h % 24} h` : `${d} d`;
}

/** "just now", "5 min ago", "3 h ago", "2 d ago". */
export function ago(ms: number, now = Date.now()) {
  const d = now - ms;
  if (d < 60000) return "just now";
  const s = span(d).split(" ").slice(0, 2).join(" ");
  return `${s} ago`;
}
