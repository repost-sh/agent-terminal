import type { CommandOutput, ErrorBody } from "./protocol";

const tabPath = (session: string, tab: string) => `/api/session/${encodeURIComponent(session)}/${encodeURIComponent(tab)}`;

export const socketUrl = (path: string) => `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}${path}`;

export const sessionsStreamUrl = () => socketUrl("/api/sessions/stream");

export const tabStreamUrl = (session: string, tab: string) => socketUrl(`${tabPath(session, tab)}/stream`);

export const commandsStreamUrl = (session: string, tab: string) => socketUrl(`${tabPath(session, tab)}/commands/stream`);

export const controlUrl = (session: string, tab: string) => socketUrl(`${tabPath(session, tab)}/control`);

export const screenshotUrl = (session: string, path: string) =>
  `/api/session/${encodeURIComponent(session)}/screenshots/${encodeURIComponent(path)}`;

export const tabFeedPath = (session: string, tab: string, feed: "activity" | "output" | "info") => `${tabPath(session, tab)}/${feed}`;

const refusal = (text: string, response: Response) => {
  try {
    return (JSON.parse(text) as { error: ErrorBody }).error.message;
  } catch {
    return text || `${response.status} ${response.statusText}`;
  }
};

const post = async (path: string) => {
  const response = await fetch(path, { method: "POST", headers: { "content-type": "application/json" } });
  if (!response.ok) throw new Error(refusal(await response.text(), response));
};

export const restartTab = (session: string, tab: string) => post(`${tabPath(session, tab)}/restart`);

export const closeTab = (session: string, tab: string) => post(`${tabPath(session, tab)}/close`);

export const commandOutput = async (session: string, tab: string, id: number) => {
  const response = await fetch(`${tabPath(session, tab)}/commands/${id}`);
  const text = await response.text();
  if (!response.ok) throw new Error(refusal(text, response));
  return (JSON.parse(text) as CommandOutput).output;
};
