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
