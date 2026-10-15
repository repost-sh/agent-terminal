---
status: accepted
---

# MCP tools run the CLI

The spec (46H-19, Architecture) has the MCP server send the daemon the same requests the CLI sends. Instead, `agent-terminal mcp` generates one tool per command from the CLI's own command definitions, and a tool call runs `agent-terminal <command> <arguments> --json` as a child process and returns what it prints. We chose this because the CLI is the contract (`docs/commands.md`): argument parsing, defaults, validation, the session from `AGENT_TERMINAL_SESSION`, starting the daemon, waits, `--json` output and exit codes all live in it, so a tool call behaves exactly like the same command run by an agent, and there is no second mapping from arguments to requests to keep in step. A new command or option is a new tool or property with no MCP code, and a test fails when one has no property. The daemon still sees one kind of request: the MCP server reaches it through the CLI.

## Considered options

- **The MCP server builds the requests itself, in process.** Saves the process start per call, but needs every command split into "arguments to request" and "response to output" functions that both front ends call, since today each command parses, talks to the daemon, prints and picks its exit code in one place. Rejected: a large refactor of every command for a few milliseconds per call, and a second path that could drift from the CLI.

## Consequences

- Each tool call pays a process start on top of the daemon round trip: about the cost of one CLI call (a few milliseconds, the `CLI round trip` metric), where an in-process request would cost only the round trip.
- A tool result is what the command printed. Exit 0 is a success; any other exit is a tool error whose content is the JSON error the command printed, so an assertion failure (2), a timeout (3), human control (4) and an exited program (5) are told apart by that JSON's `code`, not by MCP.
- The child inherits the MCP server's environment and working directory, fixed when the client started the server: `AGENT_TERMINAL_SESSION` and relative paths resolve against those.
- Calls run the binary at the server's own path, so after `upgrade` replaces it, calls run the new version while the tool list is still the one the server built at start; restart the MCP server after an upgrade.
