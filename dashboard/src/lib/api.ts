import type { CommandOutput, Compared, ErrorBody } from "./protocol";
import { settle } from "./settle";
import type { TabAddress } from "./tabs";

const tabPath = (session: string, tab: string) => `/api/session/${encodeURIComponent(session)}/${encodeURIComponent(tab)}`;

export const socketUrl = (path: string) => `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}${path}`;

export const sessionsStreamUrl = () => socketUrl("/api/sessions/stream");

export const tabStreamUrl = (session: string, tab: string) => socketUrl(`${tabPath(session, tab)}/stream`);

export const commandsStreamUrl = (session: string, tab: string) => socketUrl(`${tabPath(session, tab)}/commands/stream`);

export const controlUrl = (session: string, tab: string) => socketUrl(`${tabPath(session, tab)}/control`);

export const screenshotUrl = (session: string, path: string) =>
  `/api/session/${encodeURIComponent(session)}/screenshots/${encodeURIComponent(path)}`;

export const screenshotsFeedPath = (session: string) => `/api/session/${encodeURIComponent(session)}/screenshots/stream`;

export const tabFeedPath = (address: TabAddress, feed: "activity" | "output" | "info") => `${tabPath(address.session, address.tab)}/${feed}`;

const refusal = async (text: string, response: Response) => {
  const parsed = await settle(() => (JSON.parse(text) as { error: ErrorBody }).error.message);

  return parsed.ok ? parsed.value : text || `${String(response.status)} ${response.statusText}`;
};

const post = async <Data>(path: string, body?: unknown) => {
  const response = await fetch(path, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
  const text = await response.text();

  if (!response.ok) {
    throw new Error(await refusal(text, response));
  }

  return (JSON.parse(text) as { data: Data }).data;
};

export const reason = (error: unknown) => (error instanceof Error ? error.message : String(error));

export interface Opening {
  session: string;
  new_session: boolean;
  tab: string;
  command: string;
  cwd: string;
  cols: number;
  rows: number;
  shell_integration: boolean;
}

export const openTab = (opening: Opening) => post<{ session: string; tab: string }>("/api/sessions", opening);

export const closeAll = () => post("/api/sessions/close");

export const restartTab = (session: string, tab: string) => post(`${tabPath(session, tab)}/restart`);

export const closeTab = (session: string, tab: string) => post(`${tabPath(session, tab)}/close`);

export const takeScreenshot = (session: string, tab: string) =>
  post<{ path: string }>(`${tabPath(session, tab)}/screenshot`).then(
    (taken) => taken.path,
    (error: unknown) => {
      console.error(`Could not take a screenshot of ${session} ${tab}: ${reason(error)}`);

      return null;
    },
  );

export const commandOutput = async (address: TabAddress, id: number) => {
  const response = await fetch(`${tabPath(address.session, address.tab)}/commands/${String(id)}`);
  const text = await response.text();

  if (!response.ok) {
    throw new Error(await refusal(text, response));
  }

  return (JSON.parse(text) as CommandOutput).output;
};

export interface ComparedPaths { older: string; newer: string }

export const compareScreenshots = async (session: string, paths: ComparedPaths) => {
  const response = await fetch(`/api/session/${encodeURIComponent(session)}/screenshots/compare`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ paths: [paths.older, paths.newer] }),
  });
  const text = await response.text();

  if (!response.ok) {
    throw new Error(await refusal(text, response));
  }

  return JSON.parse(text) as Compared;
};
