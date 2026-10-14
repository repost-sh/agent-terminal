import type { ErrorBody } from "./protocol";

const tabPath = (session: string, tab: string) => `/api/session/${encodeURIComponent(session)}/${encodeURIComponent(tab)}`;

export const socketUrl = (path: string) => `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}${path}`;

export const sessionsStreamUrl = () => socketUrl("/api/sessions/stream");

export const tabStreamUrl = (session: string, tab: string) => socketUrl(`${tabPath(session, tab)}/stream`);

const refusal = (text: string, response: Response) => {
  try {
    return (JSON.parse(text) as { error: ErrorBody }).error.message;
  } catch {
    return text || `${response.status} ${response.statusText}`;
  }
};

const post = async <Data>(path: string) => {
  const response = await fetch(path, { method: "POST", headers: { "content-type": "application/json" } });
  const text = await response.text();
  if (!response.ok) throw new Error(refusal(text, response));
  return (JSON.parse(text) as { data: Data }).data;
};

export const reason = (error: unknown) => (error instanceof Error ? error.message : String(error));

export const closeAll = () => post("/api/sessions/close");

export const restartTab = (session: string, tab: string) => post(`${tabPath(session, tab)}/restart`);

export const closeTab = (session: string, tab: string) => post(`${tabPath(session, tab)}/close`);

export const startRecording = async (session: string, tab: string) => (await post<{ path: string }>(`${tabPath(session, tab)}/record/start`)).path;

export const stopRecording = async (session: string, tab: string) => (await post<{ path: string }>(`${tabPath(session, tab)}/record/stop`)).path;
