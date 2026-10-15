const RECONNECT_AFTER_MS = (failures: number) => Math.min(500 * 2 ** failures, 5000);

type Message = { type: string };

export type Follower<M extends Message> = {
  message: (message: Exclude<M, { type: "closed" }>, first: boolean, reply: (message: object) => void) => void;
  opened?: () => void;
  reconnecting?: () => void;
  closed?: () => void;
};

export const followSocket = <M extends Message>(url: string, follower: Follower<M>) => {
  let socket: WebSocket | null = null;
  let timer: number | undefined;
  let failures = 0;
  let finished = false;

  const connect = () => {
    const opening = new WebSocket(url);
    socket = opening;
    let first = true;
    const reply = (message: object) => {
      if (socket === opening && opening.readyState === WebSocket.OPEN) opening.send(JSON.stringify(message));
    };
    opening.onopen = () => {
      if (socket !== opening) return;
      failures = 0;
      follower.opened?.();
    };
    opening.onmessage = (event: MessageEvent<string>) => {
      if (socket !== opening) return;
      const message = JSON.parse(event.data) as M;
      if (message.type === "closed") {
        finished = true;
        socket = null;
        follower.closed?.();
        return;
      }
      follower.message(message as Exclude<M, { type: "closed" }>, first, reply);
      first = false;
    };
    opening.onclose = () => {
      if (finished || socket !== opening) return;
      follower.reconnecting?.();
      timer = window.setTimeout(connect, RECONNECT_AFTER_MS(failures));
      failures += 1;
    };
  };

  connect();
  return () => {
    finished = true;
    window.clearTimeout(timer);
    const open = socket;
    socket = null;
    open?.close();
  };
};
