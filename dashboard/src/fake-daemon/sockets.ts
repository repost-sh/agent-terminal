type Handler<E> = ((event: E) => void) | null;

const made: FakeSocket[] = [];

export class FakeSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;

  readyState: number = FakeSocket.CONNECTING;
  onopen: Handler<Event> = null;
  onmessage: Handler<MessageEvent<string>> = null;
  onclose: Handler<CloseEvent> = null;
  readonly path: string;
  readonly sent: unknown[] = [];

  constructor(url: string) {
    this.path = new URL(url).pathname;
    made.push(this);
  }

  send(data: string) {
    this.sent.push(JSON.parse(data));
  }

  close() {
    if (this.readyState !== FakeSocket.CLOSED) {
      this.readyState = FakeSocket.CLOSED;
      this.onclose?.(new CloseEvent("close"));
    }
  }
}

export const waiting = () => made.filter((socket) => socket.readyState === FakeSocket.CONNECTING);

export const openAt = (path: string) => made.filter((socket) => socket.path === path && socket.readyState === FakeSocket.OPEN);

export const everySocket = () => [...made];

export const accept = (socket: FakeSocket) => {
  socket.readyState = FakeSocket.OPEN;
  socket.onopen?.(new Event("open"));
};

export const deliver = (socket: FakeSocket, message: object) => {
  socket.onmessage?.(new MessageEvent("message", { data: JSON.stringify(message) }));
};

export const drop = (socket: FakeSocket) => {
  socket.readyState = FakeSocket.CLOSED;
  socket.onclose?.(new CloseEvent("close"));
};

export const forgetSockets = () => {
  made.length = 0;
};
