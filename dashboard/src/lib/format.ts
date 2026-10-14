const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

const p2 = (n: number) => String(n).padStart(2, "0");

const clock = (date: Date) => `${p2(date.getHours())}:${p2(date.getMinutes())}:${p2(date.getSeconds())}`;

export const clockMs = (iso: string) => {
  const date = new Date(iso);
  return `${clock(date)}.${String(date.getMilliseconds()).padStart(3, "0")}`;
};

export const dateTime = (iso: string) => {
  const date = new Date(iso);
  return `${MONTHS[date.getMonth()]} ${date.getDate()}, ${date.getFullYear()} ${clock(date)}`;
};

export const duration = (ms: number) => {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)} s`;
  return `${Math.floor(ms / 60000)} min ${p2(Math.round((ms % 60000) / 1000))} s`;
};
