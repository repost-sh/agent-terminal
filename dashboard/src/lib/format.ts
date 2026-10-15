const p2 = (n: number) => String(n).padStart(2, "0");

export const formatDuration = (ms: number) => {
  if (ms < 1000) {
    return `${String(Math.round(ms))} ms`;
  }

  if (ms < 60000) {
    return `${(ms / 1000).toFixed(1)} s`;
  }

  return `${String(Math.floor(ms / 60000))} min ${p2(Math.floor((ms % 60000) / 1000))} s`;
};

export const clockTime = (ms: number) => {
  const at = new Date(ms);

  return `${p2(at.getHours())}:${p2(at.getMinutes())}:${p2(at.getSeconds())}`;
};

export const clockMs = (iso: string) => {
  const at = new Date(iso);

  return `${clockTime(at.getTime())}.${String(at.getMilliseconds()).padStart(3, "0")}`;
};

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

export const dateTime = (iso: string) => {
  const at = new Date(iso);

  return `${String(MONTHS[at.getMonth()])} ${String(at.getDate())}, ${String(at.getFullYear())} ${clockTime(at.getTime())}`;
};

const initial = (part: string) => (part.startsWith(".") ? part.slice(0, 2) : Array.from(part)[0]);

export const shortPath = (path: string, home: string | null) => {
  const tilde = home && (path === home || path.startsWith(`${home}/`)) ? `~${path.slice(home.length)}` : path;
  const parts = tilde.split("/");

  return parts.map((part, i) => (i === 0 || i === parts.length - 1 || !part ? part : initial(part))).join("/");
};

export const shellWord = (word: string) => (/^[\w@%+=:,./-]+$/.test(word) ? word : `'${word.replaceAll("'", `'\\''`)}'`);
