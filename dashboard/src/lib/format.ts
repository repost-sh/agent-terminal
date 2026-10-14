const p2 = (n: number) => String(n).padStart(2, "0");

export const formatDuration = (ms: number) => {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)} s`;
  return `${Math.floor(ms / 60000)} min ${p2(Math.floor((ms % 60000) / 1000))} s`;
};

export const clockTime = (ms: number) => {
  const at = new Date(ms);
  return `${p2(at.getHours())}:${p2(at.getMinutes())}:${p2(at.getSeconds())}`;
};

const initial = (part: string) => (part.startsWith(".") ? part.slice(0, 2) : [...part][0]);

export const shortPath = (path: string, home: string | null) => {
  const tilde = home && (path === home || path.startsWith(`${home}/`)) ? `~${path.slice(home.length)}` : path;
  const parts = tilde.split("/");
  return parts.map((part, i) => (i === 0 || i === parts.length - 1 || !part ? part : initial(part))).join("/");
};

export const shellWord = (word: string) => (/^[\w@%+=:,./-]+$/.test(word) ? word : `'${word.replaceAll("'", `'\\''`)}'`);
