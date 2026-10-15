const RECONNECT_AFTER_MS = (failures: number) => Math.min(500 * 2 ** failures, 5000);

interface Message { type: string }

export interface Delivery { first: boolean; reply: (message: object) => void }

export interface Follower<M extends Message> {
  message: (message: Exclude<M, { type: "closed" }>, delivery: Delivery) => void;
  opened?: () => void;
  reconnecting?: () => void;
  closed?: () => void;
}

interface Following<M extends Message> {
  url: string;
  follower: Follower<M>;
  socket: WebSocket | null;
  timer: number | undefined;
  failures: number;
  finished: boolean;
}

interface Connection<M extends Message> { following: Following<M>; opening: WebSocket }

const replier = <M extends Message>(connection: Connection<M>) => (message: object) => {
  if (connection.following.socket === connection.opening && connection.opening.readyState === WebSocket.OPEN) {
    connection.opening.send(JSON.stringify(message));
  }
};

const receiver = <M extends Message>(connection: Connection<M>) => {
  const following = connection.following;
  const reply = replier(connection);
  let first = true;

  return (event: MessageEvent<string>) => {
    if (following.socket !== connection.opening) {
      return;
    }

    const message = JSON.parse(event.data) as M;

    if (message.type === "closed") {
      following.finished = true;
      following.socket = null;
      following.follower.closed?.();

      return;
    }

    following.follower.message(message as Exclude<M, { type: "closed" }>, { first, reply });
    first = false;
  };
};

const connect = <M extends Message>(following: Following<M>) => {
  const opening = new WebSocket(following.url);

  following.socket = opening;
  opening.onopen = () => {
    if (following.socket !== opening) {
      return;
    }

    following.failures = 0;
    following.follower.opened?.();
  };

  opening.onmessage = receiver({ following, opening });
  opening.onclose = () => {
    if (following.finished || following.socket !== opening) {
      return;
    }

    following.follower.reconnecting?.();
    following.timer = window.setTimeout(() => {
      connect(following);
    }, RECONNECT_AFTER_MS(following.failures));
    following.failures += 1;
  };
};

export const followSocket = <M extends Message>(url: string, follower: Follower<M>) => {
  const following: Following<M> = { url, follower, socket: null, timer: undefined, failures: 0, finished: false };

  connect(following);

  return () => {
    following.finished = true;
    window.clearTimeout(following.timer);
    const open = following.socket;

    following.socket = null;
    open?.close();
  };
};
