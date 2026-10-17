#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
home="$root/cli/target/dev-home"
bin="$root/cli/target/debug/agent-terminal"
port="${AGENT_TERMINAL_DEV_PORT:-4859}"
vite="${AGENT_TERMINAL_DEV_VITE:-5173}"
wrapper="${AGENT_TERMINAL_DEV_WRAPPER:-$HOME/.local/bin/agent-terminal-dev}"

usage() {
  cat <<USAGE
scripts/dev.sh [up|restart|link|stop]

  up       build the CLI, link agent-terminal-dev, start the dev daemon's dashboard on
           http://localhost:$port and run Vite on http://localhost:$vite with live reload (default)
  restart  rebuild the CLI and restart the dev daemon after a Rust change; Vite keeps running
  link     write $wrapper, which runs this checkout's build with its own home in $home
  stop     stop the dev daemon and its dashboard

The dev daemon keeps its sessions under $home, apart from the agent-terminal you use for work.
USAGE
}

build() {
  (cd "$root/cli" && cargo build --quiet)
}

link() {
  mkdir -p "$(dirname "$wrapper")" "$home"
  cat >"$wrapper" <<WRAPPER
#!/usr/bin/env bash
export AGENT_TERMINAL_HOME="\${AGENT_TERMINAL_HOME:-$home}"
exec "$bin" "\$@"
WRAPPER
  chmod +x "$wrapper"
  echo "agent-terminal-dev -> $bin (home: $home)"
}

stop() {
  AGENT_TERMINAL_HOME="$home" "$bin" daemon stop >/dev/null 2>&1 || true
}

restart() {
  build
  link
  stop
  AGENT_TERMINAL_HOME="$home" "$bin" dashboard start --port "$port" --allow-origin "http://localhost:$vite"
}

up() {
  restart
  cd "$root/dashboard"
  [ -x node_modules/.bin/vite ] || npm ci
  AGENT_TERMINAL_DASHBOARD="http://localhost:$port" exec npm run dev -- --port "$vite"
}

case "${1:-up}" in
  up) up ;;
  restart) restart ;;
  link) link ;;
  stop) stop ;;
  *) usage; exit 2 ;;
esac
