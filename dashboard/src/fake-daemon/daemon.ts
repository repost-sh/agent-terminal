import { act, waitFor } from "@testing-library/react";
import { expect, vi } from "vitest";

import type { SessionInfo } from "../lib/protocol";
import { accept, deliver, drop, FakeSocket, forgetSockets, openAt, waiting, everySocket } from "./sockets";

export interface Request { method: string; path: string; body: unknown }

export interface Answer { status: number; body: unknown; sessions?: SessionInfo[] }

type Responder = (request: Request) => Answer;

export const SESSIONS_STREAM = "/api/sessions/stream";

const bodyOf = (init: RequestInit | undefined): unknown => (typeof init?.body === "string" ? JSON.parse(init.body) : null);

const pathOf = (input: RequestInfo | URL) => new URL(input instanceof Request ? input.url : String(input), location.href).pathname;

export class FakeDaemon {
  sessions: SessionInfo[];
  readonly requests: Request[] = [];
  readonly responders = new Map<string, Responder>();

  constructor(sessions: SessionInfo[]) {
    this.sessions = sessions;
  }

  answer(route: string, responder: Responder) {
    this.responders.set(route, responder);
  }

  answerTo(request: Request): Answer {
    const responder = this.responders.get(`${request.method} ${request.path}`);

    return responder ? responder(request) : { status: 404, body: { error: { message: `no ${request.method} ${request.path}` } } };
  }

  announce(sessions: SessionInfo[]) {
    this.sessions = sessions;
    for (const socket of openAt(SESSIONS_STREAM)) {
      deliver(socket, { sessions });
    }
  }

  async fetch(input: RequestInfo | URL, init?: RequestInit) {
    const request = { method: init?.method ?? "GET", path: pathOf(input), body: bodyOf(init) };

    this.requests.push(request);
    const answer = this.answerTo(request);

    if (answer.sessions) {
      this.announce(answer.sessions);
    }

    return Promise.resolve(new Response(typeof answer.body === "string" ? answer.body : JSON.stringify(answer.body), { status: answer.status }));
  }

  greet() {
    for (const socket of waiting()) {
      accept(socket);
      if (socket.path === SESSIONS_STREAM) {
        deliver(socket, { sessions: this.sessions });
      }
    }
  }

  async serve() {
    await act(async () => {
      this.greet();
      await Promise.resolve();
    });
  }

  async connected(path: string) {
    await waitFor(() => {
      this.greet();
      expect(openAt(path)).not.toHaveLength(0);
    });
  }

  async publish(sessions: SessionInfo[]) {
    this.sessions = sessions;
    await this.push(SESSIONS_STREAM, { sessions });
  }

  async push(path: string, message: object) {
    await act(async () => {
      for (const socket of openAt(path)) {
        deliver(socket, message);
      }

      await Promise.resolve();
    });
  }

  async refuse() {
    await act(async () => {
      for (const socket of waiting()) {
        drop(socket);
      }

      await Promise.resolve();
    });
  }

  async hangUp(path: string) {
    await act(async () => {
      for (const socket of openAt(path)) {
        drop(socket);
      }

      await Promise.resolve();
    });
  }

  sent(path: string) {
    return everySocket()
      .filter((socket) => socket.path === path)
      .flatMap((socket) => socket.sent);
  }
}

export const startDaemon = (sessions: SessionInfo[]) => {
  forgetSockets();
  const daemon = new FakeDaemon(sessions);

  vi.stubGlobal("WebSocket", FakeSocket);
  vi.stubGlobal("fetch", (input: RequestInfo | URL, init?: RequestInit) => daemon.fetch(input, init));

  return daemon;
};
