---
status: accepted
---

# The npm package is @repost/agent-terminal

The unscoped npm name `agent-terminal` belongs to another account (versions 1.0.0 and 1.0.1), so publishing under it is impossible, and every command that installs it by that name would install someone else's package. agent-terminal publishes as `@repost/agent-terminal` instead, in the `@repost` npm organisation that already publishes `@repost/cli`. The command stays `agent-terminal`: the scope changes how the package is installed (`npm i -g @repost/agent-terminal`, `npx @repost/agent-terminal`), not what the binary is called. The platform packages follow `@repost/cli`'s pattern, `@repost/agent-terminal-<os>-<cpu>[-musl]`, so the whole set lives in one namespace that only the organisation can publish to.

## Considered options

- **Ask the owner of `agent-terminal` to transfer it.** Depends on a stranger, has no deadline, and users who already installed 1.0.1 would be upgraded across two unrelated programs.
- **Another unscoped name.** Unscoped platform package names can be claimed by anyone before the first release, which is a name-squatting risk a scope removes.

## Consequences

- Scoped packages publish as private by default, so every package needs `publishConfig.access: public`. agent-browser's first scoped publish failed on exactly this.
- `upgrade`, `doctor`, the launcher's reinstall hint, the README, the skill stub and `skill-data` `allowed-tools` (`npx @repost/agent-terminal`) all name `@repost/agent-terminal`.
- Trusted publishing is configured per package on npmjs.com in the `@repost` organisation, as it already is for `@repost/cli`.
