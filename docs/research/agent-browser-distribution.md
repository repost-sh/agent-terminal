# How agent-browser is distributed, compared with agent-terminal

Research date: 2026-10-15. Input for the distribution interview and spec; it records facts and leaves decisions open.

Sources and the short names used below:

- **AB** = `vercel-labs/agent-browser` at commit `dac13bb87468fe9d74a040d862720f6cc17a9565` (cloned 2026-10-15). Paths like `AB:package.json` refer to that commit.
- **AT** = this repo at commit `0b2cc6ff83cd8757d2372fc53b59f6cbfca84004`, plus the working tree (README.md has an uncommitted edit; the README facts below are from the working tree).
- **npm-view** = `npm view agent-browser --json` (run 2026-10-15).
- **tarball** = `https://registry.npmjs.org/agent-browser/-/agent-browser-0.39.1.tgz`, listed with `tar -tvzf`.
- **gh-release** = `gh release view v0.39.1 -R vercel-labs/agent-browser --json tagName,publishedAt,author,assets`.
- **gh-run** = `gh run view 38443848444 -R vercel-labs/agent-browser --json jobs` (the release run for v0.39.1).

## Summary

agent-browser ships one npm package, `agent-browser`, that contains all seven platform binaries (about 119 MB unpacked) plus a Node launcher, a postinstall script, and the skill files. Releases happen when a merged PR changes `version` in `package.json`; CI notices the npm version differs, builds the binaries with plain `cargo`/`cargo-zigbuild` on GitHub runners, waits for a human approval on the `Release` environment, publishes to npm with provenance via OIDC (no npm token secret), and creates a GitHub release with the raw binaries. Homebrew (homebrew-core, bumped by BrewTestBot) and Scoop (ScoopInstaller/Main) are maintained outside the repo; crates.io is stale at 0.19.0. There is no curl|sh installer, no winget, no Docker image, no signing, no checksum files. Skills ship as a thin `skills/agent-browser/SKILL.md` stub for `npx skills add vercel-labs/agent-browser` (and a Claude Code plugin marketplace file), with the real content served by `agent-browser skills get ...` from files next to the binary. `agent-browser upgrade` detects the installer and reruns it.

agent-terminal already has a complete, untested-in-production pipeline that is in several ways stricter than agent-browser's: cargo-dist on tag push, per-platform npm packages as `optionalDependencies`, a clean-machine install test, a Homebrew tap, `upgrade`, `install` (skill stub), and skills embedded in the binary. Nothing has been released yet. The biggest blockers are external: the npm name `agent-terminal` and the crates.io name `agent-terminal` both belong to other people, the `repost-sh/homebrew-tap` repo does not exist, and no repository secrets are set.

## agent-browser mechanism

### 1. Release trigger and versioning

- **Trigger: version diff against npm, on every push to `main`.** `AB:.github/workflows/release.yml` runs on `push: branches: [main]` and `workflow_dispatch`. Job `check-release` reads `package.json` version, runs `npm view agent-browser version`, and sets `should_release=true` when they differ. If they match but `gh release view v<version>` fails, it rebuilds just to create the missing GitHub release. No tag push triggers anything; the tag `v<version>` is created by `gh release create` at the end.
- **No changesets or release-please.** `AB:AGENTS.md` ("Releasing"): "Releases are manual, single-PR affairs. There is no changesets automation." Steps: branch, bump `package.json` `version`, run `pnpm version:sync`, write the `CHANGELOG.md` entry wrapped in `<!-- release:start -->`/`<!-- release:end -->` (only the newest entry keeps markers), add a matching entry to `docs/content/docs/changelog.mdx`, open a PR, merge. Example: PR #2105 "Prepare v0.39.1 release" by `ctate` touched `CHANGELOG.md`, `cli/Cargo.lock`, `cli/Cargo.toml`, `docs/content/docs/changelog.mdx`, `package.json`, and three sub-package manifests (`gh pr view 2105 -R vercel-labs/agent-browser --json files,author`).
- **Changelog.** Hand-written `AB:CHANGELOG.md`, sections `### New Features`, `### Bug Fixes`, `### Contributors`, etc. The GitHub release body is the text between the markers, extracted with `awk` in the `github-release` job (`AB:.github/workflows/release.yml`).
- **Version sync.** `package.json` is the single source (`AB:scripts/sync-version.js`: "Read version from package.json (single source of truth)"). The script rewrites `version` in `cli/Cargo.toml`, `packages/dashboard/package.json`, `packages/@agent-browser/sandbox/package.json`, `packages/@agent-browser/eve/package.json`, `packages/@agent-browser/sandbox/src/version.ts`, then runs `cargo update -p agent-browser` for `Cargo.lock`. CI job `version-sync` in `AB:.github/workflows/ci.yml` runs `AB:scripts/check-version-sync.js`, which fails on any mismatch. The npm `version` lifecycle script also syncs and stages `cli/Cargo.toml` (`AB:package.json`). The release workflow runs `pnpm run version:sync` again before building.
- **Cadence.** Releases v0.38.0 (2026-09-16) to v0.39.1 (2026-10-13), five in a month (`gh release list -R vercel-labs/agent-browser --limit 5`). npm lists 130 versions (npm-view).

### 2. CI build matrix

From `AB:.github/workflows/release.yml`, job `build-binaries`:

| Name | Runner | Rust target | Tool | Asset name |
|---|---|---|---|---|
| Linux x64 | ubuntu-latest | x86_64-unknown-linux-gnu (built as `.2.28`) | cargo-zigbuild 0.23.2 | `agent-browser-linux-x64` |
| Linux ARM64 | ubuntu-latest | aarch64-unknown-linux-gnu (`.2.28`) | cargo-zigbuild | `agent-browser-linux-arm64` |
| Linux musl x64 | ubuntu-latest | x86_64-unknown-linux-musl | cargo-zigbuild | `agent-browser-linux-musl-x64` |
| Linux musl ARM64 | ubuntu-latest | aarch64-unknown-linux-musl | cargo-zigbuild | `agent-browser-linux-musl-arm64` |
| Windows x64 | ubuntu-latest | x86_64-pc-windows-gnu | cargo + mingw-w64 | `agent-browser-win32-x64.exe` |
| macOS x64 | macos-latest | x86_64-apple-darwin | cargo | `agent-browser-darwin-x64` |
| macOS ARM64 | macos-latest | aarch64-apple-darwin | cargo | `agent-browser-darwin-arm64` |

- **Cross-compilation.** Linux targets use `cargo zigbuild` (ziglang via pip); the glibc builds pin glibc 2.28 through the `.2.28` target suffix, and a step fails the build if `objdump -p` shows a `GLIBC_2.29`+ symbol. Windows cross-compiles from Linux with mingw-w64. A dashboard (`pnpm --filter dashboard build`) is built before `cargo build` in every matrix row.
- **Artifacts.** Raw executables, not archives, named `agent-browser-<os>-<arch>[.exe]` (gh-release lists exactly these seven assets, 15.6 to 18.3 MB each). No Windows ARM64 build; the launcher and postinstall fall back to the x64 binary under emulation (`AB:bin/agent-browser.js`, `AB:scripts/postinstall.js`).
- **Checksums.** None generated. GitHub's own per-asset `digest` (sha256) is present in gh-release output, but no `SHA256SUMS` file is uploaded.
- **Signing / notarization.** None in the workflow (no codesign, notarytool, cosign, or minisign step in `AB:.github/workflows/release.yml`).
- **Provenance.** npm only: `pnpm publish --provenance` in the `publish-packages` job, which has `id-token: write`. The registry shows `dist.attestations.provenance.predicateType = https://slsa.dev/provenance/v1` for 0.39.1 (npm-view). No GitHub artifact attestations for the release binaries: `gh api repos/vercel-labs/agent-browser/attestations/sha256:<darwin-arm64 digest>` returned 404.
- **Gate.** The publish job runs in environment `Release`, which has a `required_reviewers` rule (reviewers `ctate`, `Railly`) and a branch policy (`gh api repos/vercel-labs/agent-browser/environments`). In gh-run the builds finished at 18:13 and publish started at 19:51, consistent with a manual approval wait.
- **Order.** Build, then publish npm, then create the GitHub release (`needs: [check-release, build-binaries, publish-packages]`). npm's `postinstall` downloads from the GitHub release only as a fallback (see 3), so the brief window where npm is ahead of the release does not matter for normal installs.

### 3. npm distribution

- **One fat package, no per-platform packages.** `AB:package.json` has no `optionalDependencies`; npm-view confirms `optionalDependencies: null`, `dependencies: null`. `files` is `bin`, `scripts`, `skill-data`, `skills`, and two licence files. The release job moves all seven binaries into `bin/` before `pnpm publish`. The 0.39.1 tarball holds all seven binaries plus `bin/agent-browser.js`; npm-view reports `fileCount: 45`, `unpackedSize: 119447219`.
- **Bin shim.** `"bin": {"agent-browser": "./bin/agent-browser.js"}`. The launcher (`AB:bin/agent-browser.js`) maps `os.platform()`/`os.arch()` to `agent-browser-<os>-<arch>`, detects musl with `ldd --version` (falling back to checking `/lib/ld-musl-*.so.1`), chmods the binary if it lost its execute bit (comment: bun blocks lifecycle scripts), and `spawn`s it with inherited stdio, forwarding the exit code.
- **postinstall** (`AB:scripts/postinstall.js`, run via `"postinstall": "node scripts/postinstall.js"`):
  1. If the platform binary is already in `bin/` (the normal case), chmod it.
  2. Otherwise download `https://github.com/vercel-labs/agent-browser/releases/download/v<version>/<binaryName>`; on failure print instructions to build with Rust. No checksum verification of the download.
  3. Write `bin/.install-method` (`npm`/`pnpm`/`yarn`/`bun`, from `npm_config_user_agent`) for `upgrade`.
  4. On global installs, replace npm's symlink in `$(npm prefix -g)/bin` with a symlink straight to the native binary (Unix), or rewrite the `.cmd`/`.ps1` shims (Windows), so global runs skip Node.
  5. Look for a system Chrome; if none, tell the user to run `agent-browser install` (`--with-deps` on Linux).
- **Node requirement.** `engines: {"node": ">=24.0.0", "pnpm": ">=11.0.0"}` in the published manifest (npm-view). The README says Node 24+ is "Only needed when building from source" (`AB:README.md`, Requirements), but the engines field applies to the published package too.
- **Other npm packages** published from the same workflow: `@agent-browser/sandbox` and `@agent-browser/eve` (Vercel Sandbox helpers), each version-checked separately (`AB:.github/workflows/release.yml`). Both are at 0.39.1 (`npm view @agent-browser/sandbox`, `npm search agent-browser`).

### 4. Other channels

Install commands from `AB:README.md` ("Installation"):

```bash
npm install -g agent-browser        # global, recommended
npm install agent-browser           # project-local
brew install agent-browser          # Homebrew (macOS)
cargo install agent-browser         # Cargo
agent-browser install               # after any of the above: download Chrome
```

`AB:docs/content/docs/installation.mdx` adds `npx agent-browser install` / `npx agent-browser open example.com` (no install) and says `cargo install` "Compiles from source (~2-3 min)".

- **Homebrew: homebrew-core, not a tap.** Formula `Formula/a/agent-browser.rb` in `Homebrew/homebrew-core` (read with `gh api repos/Homebrew/homebrew-core/contents/Formula/a/agent-browser.rb`): builds from the GitHub source tarball of the tag, `depends_on "rust" => :build`, `depends_on "node"`, runs `npm run build:native` then `npm install`, and has bottles for macOS arm64 (three releases) and Linux arm64/x86_64. No macOS Intel bottle is listed. Updates are made by `BrewTestBot` ("agent-browser 0.39.1" on 2026-10-13, about 3 h after the release; `gh api 'repos/Homebrew/homebrew-core/commits?path=Formula/a/agent-browser.rb'`). Nothing in AB pushes to Homebrew.
- **Cargo.** The crate exists on crates.io but its newest version is 0.19.0, published 2026-03-14 by user `ctate`, with no trusted-publishing data (`https://crates.io/api/v1/crates/agent-browser`). CI never runs `cargo publish`. So `cargo install agent-browser` installs a version about 20 minor releases old, and the `skills` command finds no skill files in that layout (see 5).
- **Scoop.** `ScoopInstaller/Main` has `bucket/agent-browser.json` at 0.39.1, downloading `agent-browser-win32-x64.exe` plus the source zip and moving `skills/` and `skill-data/` next to the exe, with `checkver`/`autoupdate` on GitHub releases (`gh api repos/ScoopInstaller/Main/contents/bucket/agent-browser.json`). AB's README does not mention Scoop; it is community-maintained.
- **winget.** No entry: `microsoft/winget-pkgs` `manifests/v/Vercel` holds only `Hyper` and `Zero`.
- **curl | sh installer.** None. No `install.sh` in `AB:scripts/`, and no such command in the README or docs.
- **Docker.** `AB:docker/Dockerfile.build` and `docker-compose.yml` are a cross-compilation build image for local `build:linux`/`build:windows` scripts, not a published runtime image.
- **From source.** `git clone`, `pnpm install`, `pnpm build`, `pnpm build:native`, `pnpm link --global` (`AB:README.md`).

### 5. Skills distribution and post-install downloads

- **Two directories.** `skills/agent-browser/SKILL.md` is a "discovery stub" with `hidden: true`, `allowed-tools: Bash(agent-browser:*), Bash(npx agent-browser:*)`, that tells the agent to run `agent-browser skills get core` (`AB:skills/agent-browser/SKILL.md`). `skill-data/` holds the real content: `core`, `electron`, `slack`, `dogfood`, `derive-client`, `vercel-sandbox`, `protected-vercel-deployments`, `agentcore`, `webmcp-gen` (`ls AB:skill-data`).
- **Install path for agents.** `npx skills add vercel-labs/agent-browser` (`AB:README.md`, "AI Coding Assistants" and "Claude Code"): fetches the stub from the GitHub repo into e.g. `.claude/skills/agent-browser/SKILL.md`; the README names Claude Code, Codex, Cursor, Gemini CLI, GitHub Copilot, Goose, OpenCode, Windsurf, and warns not to copy `SKILL.md` from `node_modules`. The README carries a skills.sh badge (`https://skills.sh/vercel-labs/agent-browser`).
- **Claude Code plugin marketplace.** `AB:.claude-plugin/marketplace.json` declares plugin `agent-browser` with `skills: ["./skills/agent-browser"]`.
- **Runtime serving.** `agent-browser skills list|get <name> [--full]|get --all|path [name]` (`AB:README.md`, "Skills"). The binary reads skills from disk, not from embedded data: `AB:cli/src/skills.rs` searches `AGENT_BROWSER_SKILLS_DIR`, then `<exe dir>/../skills` and `../skill-data` (the npm layout), then walks up from the exe (dev builds). The npm tarball contains both directories (26 `skill-data` entries in the tarball listing). A plain `cargo install` binary has no such directory next to it; Scoop works around this by copying the folders from the source zip.
- **No built-in skill installer.** AB has no `install --skills` subcommand; `agent-browser install` downloads Chrome for Testing, and `install --with-deps` also installs Linux system libraries (`AB:README.md`).
- **Post-install downloads.** Chrome only, and only on demand via `agent-browser install`; postinstall just prints a reminder when no system Chrome is found (`AB:scripts/postinstall.js`, `showInstallReminder`).

### 6. Self-update and version check

- `agent-browser upgrade` (`AB:cli/src/upgrade.rs`): fetches `https://registry.npmjs.org/agent-browser/latest`, exits early if already current, detects the install method (the `.install-method` marker, then path heuristics for `.cargo/bin`, `Cellar`/`homebrew`/`linuxbrew`, pnpm, yarn, bun, `node_modules/agent-browser`, then probing `brew list`, `pnpm list -g`, etc.), and runs `npm install -g agent-browser@latest`, `pnpm add -g ...`, `yarn global add ...`, `bun install -g ...`, `brew upgrade agent-browser`, or `cargo install agent-browser --force`. Unknown method prints all the commands.
- No passive "update available" check: a grep of `AB:cli/src` for `registry.npmjs`, `update available`, `newer version` finds only `upgrade.rs`.
- `agent-browser doctor` reports, among other things, a daemon version mismatch with the CLI (`AB:docs/content/docs/installation.mdx`, Doctor table).

### 7. Secrets and one-time human setup

- **npm: trusted publishing (OIDC), no token secret.** The only secret referenced in `AB:.github/workflows/` is `secrets.GITHUB_TOKEN` (grep for `secrets.`, `NODE_AUTH_TOKEN`, `NPM_TOKEN`). The publish job has `id-token: write` and runs `pnpm publish --provenance`. This implies a trusted-publisher entry for this repo/workflow/environment configured on npmjs.com for `agent-browser`, `@agent-browser/sandbox`, and `@agent-browser/eve` (the npm-side setting is not visible from the repo). Publisher on npm: maintainers include `vercel-release-bot` (npm-view).
- **GitHub environment `Release`** with required reviewers and a branch policy (see 2).
- **Homebrew and Scoop:** no setup in AB; both are maintained by those projects' bots.
- **crates.io:** manual, last done for 0.19.0.
- **Signing certificates:** none used.

## agent-terminal today

- **Version source.** `version` in `cli/Cargo.toml` only (`AT:docs/releasing.md`); currently `0.1.0` (`AT:cli/Cargo.toml`). The npm template `AT:npm/agent-terminal/package.json` has no `version`; `AT:npm/build-packages.js` injects the version from the dist manifest. No changelog file; release notes come from dist's announcement (generated `AT:.github/workflows/release.yml`, `announce`/`host` jobs).
- **Trigger.** Push of a tag matching `**[0-9]+.[0-9]+.[0-9]+*`; `dist plan` also runs on every pull request (`AT:.github/workflows/release.yml`, `on:`). Prerelease tags build everything and create a GitHub prerelease but publish nothing (`AT:docs/releasing.md`). No tags or GitHub releases exist yet (`git tag` empty; `gh release list -R repost-sh/agent-terminal` empty).
- **Build.** cargo-dist 0.33.0 (`AT:dist-workspace.toml`): six targets, macOS arm64/x64 and Linux arm64/x64 in glibc and musl. No Windows target. musl builds run in digest-pinned `ghcr.io/rust-cross/rust-musl-cross` images because OpenH264 needs a musl C++ compiler (`AT:docs/releasing.md`). Archives include `licenses/` (`include = ["licenses/"]`). `precise-builds = true`, `install-path = "CARGO_HOME"`. Checksums, signing and GitHub attestations are not configured in `dist-workspace.toml`; whatever dist does by default applies and has not been observed in a real release.
- **npm.** One package per platform, `agent-terminal-<os>-<cpu>[-musl]`, each with `os`/`cpu`/`libc` fields and only the binary plus licences, and a main `agent-terminal` package with those as exact-version `optionalDependencies` (`AT:npm/build-packages.js`). No postinstall. Launcher `AT:npm/agent-terminal/bin/agent-terminal.js` resolves `<platform package>/bin/agent-terminal` via `require.resolve`, detects musl through `process.report` (`AT:npm/agent-terminal/lib/platform.js`), and on failure tells the user to reinstall with optional deps or `cargo install --git ...`. `engines.node >= 18`. Unit tests in `AT:npm/test/`; CI runs them (`AT:.github/workflows/ci.yml`, "npm packages").
- **Install test.** Before any publish, `AT:.github/workflows/npm-packages.yml` installs the packed tarballs from a local registry with `npm i -g` on clean Debian and Alpine containers (x64, arm64) and macOS arm64/x64 runners, then drives a fixture with `open` and `snapshot` (`AT:npm/install-test/run.sh`). AB's nearest equivalent is the `global-install` job in `AB:.github/workflows/ci.yml`, which installs a packed tarball holding a binary built in that job and checks `--version` on Ubuntu, macOS and Windows; it does not test the release artifacts.
- **Publish.** `AT:.github/workflows/publish-npm.yml` publishes platform packages first, then the main one, skipping versions already on npm, with `--provenance` and `NODE_AUTH_TOKEN: secrets.NPM_TOKEN`. Homebrew formula pushed by dist to `repost-sh/homebrew-tap` with `secrets.HOMEBREW_TAP_TOKEN` (`brew install repost-sh/tap/agent-terminal`) (`AT:docs/releasing.md`, generated `release.yml` job `publish-homebrew-formula`). No approval environment.
- **Skills.** Embedded in the binary with `include_str!` (`AT:cli/src/skills.rs`): `agent-terminal` (hidden stub, `AT:skills/agent-terminal/SKILL.md`), `core`, `tui-qa`, `cli-qa` (`AT:skill-data/`). `skills list|get [--full]|path`, and `agent-terminal install [--skills]` writes the stub to `~/.claude/skills/agent-terminal/SKILL.md`, stamping `metadata.version` and refusing to overwrite a newer stub (`AT:cli/src/skills.rs` `install_stub`, `AT:cli/src/commands.rs` line 216, `AT:docs/commands.md`). Because skills are embedded, every install channel (npm, brew, cargo) gets them. No `.claude-plugin/marketplace.json`.
- **Upgrade.** `agent-terminal upgrade` (`AT:cli/src/upgrade.rs`) detects npm (`node_modules` in the resolved path) or Homebrew (`Cellar`) and runs `npm install -g agent-terminal@latest` or `brew upgrade repost-sh/tap/agent-terminal`, then reports the new version. It does not check the registry first and does not handle pnpm/yarn/bun/cargo. `doctor` exists (`AT:cli/src/commands.rs` line 1155).
- **README.** "There is no release yet. Build from source: `cargo install --locked --git https://github.com/repost-sh/agent-terminal agent-terminal`" (`AT:README.md`, working tree).
- **CI.** Conventional Commits check, npm unit tests, dashboard build check, browser tests (installs `agent-browser@0.39.0` globally), fmt/clippy/test, perf comparison (`AT:.github/workflows/ci.yml`).
- **External state checked 2026-10-15.**
  - npm `agent-terminal` is taken: version 1.0.1, maintainer `jasonkneen`, repo `github.com/jkneen/agent-terminal`, published 2026-01-13 (`npm view agent-terminal --json`).
  - npm `agent-terminal-darwin-arm64`, `agent-terminal-linux-x64`, `agent-terminal-linux-x64-musl`, `@repost-sh/agent-terminal`, `@repost/agent-terminal` return E404, so they are free or unpublished (`npm view <name> version`).
  - crates.io `agent-terminal` is taken: 0.1.0 by `ohwxyz`, repo `github.com/ohwang/agent-terminal`, created 2026-03-30 (`https://crates.io/api/v1/crates/agent-terminal`).
  - `repost-sh/homebrew-tap` does not exist (`gh repo view repost-sh/homebrew-tap`: "Could not resolve").
  - `gh secret list -R repost-sh/agent-terminal` printed nothing, so `NPM_TOKEN` and `HOMEBREW_TAP_TOKEN` are not set, or this account cannot see them.

## Gap table

| Capability | agent-browser | agent-terminal | Gap |
|---|---|---|---|
| Release trigger | Merge a version-bump PR; CI diffs against npm | Push a `v*` tag | Different model, both work; AT has no tag yet |
| Version source of truth | `package.json`, synced into Cargo.toml and others by script, CI check | `cli/Cargo.toml`, npm version injected at build | None functionally |
| Changelog / release notes | Hand-written `CHANGELOG.md`, markers extracted into the GitHub release | dist-generated announcement; no changelog file | No changelog |
| Human approval gate | `Release` environment with required reviewers | None; the tag push publishes | Missing, if wanted |
| Targets | 7: macOS x64/arm64, Linux gnu/musl x64/arm64, Windows x64 | 6: same minus Windows | No Windows |
| glibc floor | 2.28, enforced by zigbuild and an objdump check | Set by dist's chosen runner, not checked | No explicit floor |
| Artifacts | Raw binaries on the GitHub release | dist archives (with `licenses/`) | Different, AT is richer |
| Checksums | None (GitHub digests only) | dist default, unverified | Neither verified |
| Signing / notarization | None | None | Same |
| Provenance | npm `--provenance` (SLSA v1) | npm `--provenance` configured | Same; neither attests GitHub binaries |
| npm auth | Trusted publishing (OIDC), no token | `NPM_TOKEN` secret | Not set up; token vs OIDC open |
| npm package name | `agent-browser` owned | `agent-terminal` owned by someone else | **Blocker** |
| npm layout | One 119 MB package with all binaries, postinstall fallback download | Main + 6 platform packages via `optionalDependencies`, no postinstall | Different by design; AT is smaller per install |
| Launcher | Node spawn; postinstall rewrites global symlink/shims to the native binary | Node `spawnSync` always | AT always pays Node startup per call |
| Node engines | `>=24` | `>=18` | None |
| Clean-machine install test | `npm pack` + `npm install -g` + `--version` on ubuntu-latest, macos-latest, windows-latest, with a CI-built binary, not on PRs (`AB:.github/workflows/ci.yml` job `global-install`) | Packed tarballs from a local registry on Debian, Alpine (x64, arm64) and macOS arm64/x64, driving a fixture, before publish | AT's matrix covers more platforms and the release artifacts on paper, but `npm-packages.yml` is `workflow_call` only, runs only inside a tag-triggered release, and has never run (0 runs); AB's runs on every push to main and covers Windows |
| Homebrew | homebrew-core, bumped by BrewTestBot, builds from source | Own tap via dist | Tap repo and token missing |
| crates.io | Stale 0.19.0, manual | Name taken by someone else | Blocker for `cargo install agent-terminal` |
| cargo install from git | Not documented | README's only install path | None |
| curl \| sh | None | None (dist can generate one) | Same |
| Scoop / winget | Scoop Main (community), no winget | None, no Windows build | Depends on Windows |
| Docker image | None | None | Same |
| Skills: stub for agents | `skills/` stub, `npx skills add vercel-labs/agent-browser`, skills.sh listing | Same stub layout in `skills/`; `agent-terminal install --skills` writes `~/.claude/skills` | `npx skills add repost-sh/agent-terminal` untested; not listed on skills.sh |
| Skills: runtime content | Files on disk next to the binary (npm and Scoop only) | Embedded in the binary (every channel) | AT ahead |
| Claude Code plugin marketplace | `.claude-plugin/marketplace.json` | None | Missing |
| Post-install download | Chrome via `agent-browser install` | Nothing to download | Not applicable |
| Upgrade | npm, pnpm, yarn, bun, brew, cargo; checks npm latest first | npm and brew only; no latest check | Fewer installers |
| Doctor | Yes | Yes | None |

## Open questions for the owner

1. **npm name.** `agent-terminal` on npm belongs to `jasonkneen`. Options include asking for a transfer, a scope (`@repost-sh/agent-terminal`, free today), or another name. Which one? The choice also renames the six platform packages, `upgrade`'s npm command, the stub's `allowed-tools` (`npx agent-terminal`), and every README/skill install line.
2. **crates.io.** `agent-terminal` there belongs to `ohwxyz`. Should agent-terminal be on crates.io at all (agent-browser lets theirs go stale), under another crate name, or only via `cargo install --git`?
3. **Release trigger.** Keep tag push (dist's model), or switch to agent-browser's "merge a version-bump PR, CI publishes when npm differs"?
4. **Approval gate.** Add a GitHub environment with required reviewers before npm/Homebrew publish, as agent-browser does?
5. **npm auth.** Keep the `NPM_TOKEN` secret or move to npm trusted publishing (OIDC), as agent-browser does? Trusted publishing has to be configured per package on npmjs.com, which here means seven packages, and the first publish of a new package name may need a token anyway (not verified here).
6. **npm package layout.** Keep per-platform `optionalDependencies` (smaller installs, depends on the package manager honouring optional deps and `libc`), or copy agent-browser's single fat package with a postinstall download fallback?
7. **Launcher overhead.** agent-browser's postinstall points the global symlink at the native binary so each call skips Node. agent-terminal always goes through Node. Is per-call Node startup acceptable for an agent CLI called many times per task, and if not, is a postinstall script acceptable (bun and some setups skip it)?
8. **Homebrew.** Own tap (`repost-sh/homebrew-tap`, needs creating plus a token) or aim for homebrew-core (needs notability; formula builds from source and BrewTestBot bumps it)? Both?
9. **Windows.** agent-browser ships Windows x64 and is in Scoop. Is Windows in scope for agent-terminal (PTY layer, daemon sockets), and when?
10. **Other channels.** Want a curl|sh installer (dist can generate shell/PowerShell installers), Docker image, or winget? agent-browser has none of these.
11. **Changelog.** Keep dist's generated notes, or hand-written `CHANGELOG.md` with markers like agent-browser?
12. **Signing and attestations.** Add GitHub artifact attestations (dist `github-attestations`), checksums, or macOS signing/notarization? agent-browser does none; the npm provenance is its only supply-chain evidence.
13. **glibc floor.** Which minimum glibc should the gnu builds support, and should CI check it as agent-browser does with objdump?
14. **Skills reach.** Rely on `agent-terminal install --skills` (writes only `~/.claude/skills`), or also support `npx skills add repost-sh/agent-terminal` (multi-agent), a skills.sh listing, and a `.claude-plugin/marketplace.json`? Should `install` cover agents other than Claude Code?
15. **Upgrade coverage.** Should `upgrade` check the registry before running the installer and support pnpm/yarn/bun/cargo like agent-browser, or stay npm and Homebrew only?
16. **First release version.** Ship as 0.1.0, or another number, and with a prerelease rehearsal tag first (`docs/releasing.md` supports `v0.x.y-rc.n`)?

## Published artifacts (forensics, 2026-10-15)

The sections above read configuration. This section and the next two read what agent-browser actually published and what broke. Scope: the last five versions, 0.38.0 to 0.39.1. Tools: macOS `file`, `shasum`, `otool`, `codesign`, `spctl`, Apple LLVM `objdump` (ELF and PE), `gh`, `npm` 11.16.0. Scratch logs are kept locally in the gitignored `.superpowers/research/forensics/`; every fact below names the command that reproduces it.

- **npm and GitHub copies are byte-identical.** For all 35 binaries (7 targets × 5 versions), the sha256 of the GitHub release asset, the `digest` field in `gh release view v<v> -R vercel-labs/agent-browser --json assets`, and the same-named file inside the npm tarball agree. No checksum file is published; GitHub's per-asset digest is the only published sha256.
- **No binary is signed.** darwin-arm64: `codesign -dv` gives `Signature=adhoc`, `flags=0x20002(adhoc,linker-signed)`, `TeamIdentifier=not set`. darwin-x64: "code object is not signed at all". `spctl -a -t exec -vv`: `rejected` for both, all versions. Windows exe: PE Security Directory size 0 (`objdump -p`), so no Authenticode. Files fetched by `npm` or `gh release download` carry no `com.apple.quarantine`, so Gatekeeper does not block those installs; a browser download is quarantined and blocked.
- **glibc floor 2.28 on both gnu targets, all versions.** `objdump -p` Version References: `GLIBC_2.28` with flags `0x00` (hard), although the only 2.28 symbol is a weak `statx`; the newest non-weak symbol is `GLIBC_2.27` (`log2f`, `exp2f`). NEEDED: `libm.so.6 libc.so.6 libpthread.so.0 libdl.so.2`. musl builds are fully static.
- **macOS minimum:** 11.0 for arm64 (`LC_BUILD_VERSION minos 11.0`), 10.12 for x64 (`LC_VERSION_MIN_MACOSX`). From 0.39.0 the macOS builds link `Security.framework` and the Windows exe imports `crypt32.dll` (`otool -L`, `objdump -p`, 0.38.2 vs 0.39.0).
- **Sizes are flat:** under 1% growth from 0.38.0 to 0.39.1 (all seven assets together 118,483,504 → 119,062,744 bytes, `stat -f %z`).

| 0.39.1 asset | Bytes | sha256 (first 8) | Format | glibc (non-weak) |
|---|---|---|---|---|
| darwin-arm64 | 15,803,440 | `cc18fac4` | Mach-O, macOS 11.0+ | n/a |
| darwin-x64 | 17,441,576 | `339cb9aa` | Mach-O, macOS 10.12+ | n/a |
| linux-arm64 | 15,844,112 | `9d8fdc58` | ELF PIE | 2.28 (2.27) |
| linux-x64 | 18,305,528 | `6906d3a2` | ELF PIE | 2.28 (2.27) |
| linux-musl-arm64 | 15,642,552 | `c4959f3e` | ELF static | none |
| linux-musl-x64 | 18,102,976 | `74507cda` | ELF static | none |
| win32-x64.exe | 17,922,560 | `af1ff5f2` | PE32+ console | n/a |

**npm tarball** (`npm pack agent-browser@<v>`, `tar -tvzf`): 0.39.1 is 53,561,140 bytes, 45 files, `dist.unpackedSize` 119,447,219. All seven binaries are stored mode `0644`; the postinstall chmods only the host's binary, and with `--ignore-scripts` it stays `-rw-r--r--` until the launcher chmods it on first run. Build-time scripts (`check-version-sync.js`, `copy-native.js`, `sync-version.js`, `build-all-platforms.sh`) ship to users. 0.38.0 also shipped five `scripts/windows-debug/*.sh`. `postinstall.js` and `bin/agent-browser.js` are byte-identical across all five versions.

**postinstall, read from the published file** (not executed): the fallback download (`https.get` of the release asset) has no checksum check, and a non-200 response leaves the partial file at the final path. It also runs `npm prefix -g` and, if `<prefix>/bin/agent-browser` is a symlink, repoints it at *this* package's binary without checking that the install is global, so a project-local install can retarget a global command.

**Homebrew ships a different build.** The homebrew-core bottle is built from the tag source by Homebrew's CI: darwin-arm64 binary 13,508,048 bytes, sha256 `42d340a3…`, versus 15,803,440 for the release asset, and `bin/agent-browser` goes through the Node launcher (`libexec/bin/agent-browser -> …/agent-browser.js`). The Linux bottle needs `GLIBC_2.39` (Homebrew's own glibc). Bottles carry GitHub attestations: `gh attestation verify <bottle> -R Homebrew/homebrew-core` succeeds with signer `publish-commit-bottles.yml`. Scoop's manifest pins the release exe by its correct sha256 `af1ff5f2…` (`https://raw.githubusercontent.com/ScoopInstaller/Main/master/bucket/agent-browser.json`).

## Provenance as published

| Version | Built commit (SLSA `gitCommit` = run headSha) | Tag `v<v>` points at | Release run | Builds done → publish start |
|---|---|---|---|---|
| 0.39.1 | `1165952d` | same | 38443848444 | 97 min |
| 0.39.0 | `44af3984` | same | 37970996381 | 18 min |
| 0.38.2 | `39a74c70` | same | 36919928752 | 25 min |
| 0.38.1 | `aff6125c` | same | 35143035258 | 0 min |
| 0.38.0 | `cd063d1c` | **`0473bfdc`**, one commit later | 35051995184 | 502 min |

- **npm provenance verifies for every version.** Each version has SLSA v1 provenance (Fulcio cert, SAN `https://github.com/vercel-labs/agent-browser/.github/workflows/release.yml@refs/heads/main`, trigger `push`, GitHub-hosted runner) and an npm publish attestation, both in Rekor (for 0.39.1, log indexes 3246145672 and 3246202404). `gh attestation verify <tgz> --bundle <slsa bundle> --digest-alg sha512 -R vercel-labs/agent-browser --signer-workflow vercel-labs/agent-browser/.github/workflows/release.yml` exits 0 for all five (it fails without `--digest-alg sha512`, because npm subjects are sha512). `npm audit signatures` after `npm install agent-browser@latest --ignore-scripts`: "1 package has a verified registry signature / 1 package has a verified attestation".
- **Release binaries have no attestations.** `gh attestation verify <asset> -R vercel-labs/agent-browser` returns `HTTP 404: Not Found` for darwin-arm64, linux-x64 and win32-x64 of every version (15 calls). The only cryptographic link from a binary to CI is indirect: binary = file in npm tarball, whose sha512 is the SLSA subject.
- **npm publisher:** `_npmUser = "GitHub Actions <npm-oidc-no-reply@github.com>"` (trusted publishing). Built commits are GitHub-verified (`gh api repos/vercel-labs/agent-browser/commits/<sha>`: `verified=true`).
- **The v0.38.0 tag points at the wrong commit.** `gh api repos/vercel-labs/agent-browser/git/ref/tags/v0.38.0` gives `0473bfdc` ("fix: restore release CI (#1862)", six Rust files changed), while the binaries and provenance come from `cd063d1c`. Builds finished at 05:00, `0473bfdc` landed on main at 06:05, approval came at 13:23, and `gh release create "$TAG"` (`AB:.github/workflows/release.yml` line 426, no `--target`) tagged whatever `main` was then. Homebrew builds from the tag source, so its 0.38.0 was built from `0473bfdc`, not `cd063d1c` (inferred from the formula's source URL; no 0.38.0 bottle inspected). Scoop's 0.38.0 manifest downloaded only the release exe (`5a4b95cc…`), byte-identical to npm's; Scoop added the tag source zip on 2026-10-14 (ScoopInstaller/Main `ab5377ed3b`). Nothing in AB's workflow checks the tag against the built commit.
- **Publish order.** The npm publish job finished before the GitHub release in all five runs (0.39.1: job 19:52:07, release 19:52:20). The registry's version `time` (0.39.1: 19:56:09) trails the job by 2–4 min and matches the npm publish-attestation Rekor time, so it may record post-processing rather than availability. When each version became installable was not measured.

## Incident history

Sources: `gh issue list` / `gh pr list -R vercel-labs/agent-browser --state all` (731 issues, 1,364 PRs, filtered by install and release keywords, 75 issues and 40 PRs read in full), `git log -p` on the release files, `gh run list` for `release.yml` (579 runs), npm and Homebrew analytics. **AB#n** means `https://github.com/vercel-labs/agent-browser/issues/n` (GitHub redirects to the PR when n is a PR). Time to fix = issue creation → npm publish of the first version containing the fix (`git tag --contains`). "Open" = still open on 2026-10-15.

| Issues | Channel | Platform | Failure | Root cause | Fix | Time to fix |
|---|---|---|---|---|---|---|
| AB#25, #108, #148, #262, #43, #208 | npm global | Windows x64 | `'/bin/sh.exe' is not recognized` right after `npm i -g` | `bin` was a POSIX `#!/bin/sh` wrapper | Node launcher + Windows global-install CI job, AB#293 (0.8.3) | 1.9–14.6 d |
| AB#278 | npm, volta | Windows | exit 126, path not found | `.cmd` ran a file not in the package | AB#289 (0.8.2) | 0.4 d |
| npm 0.7.0 (AB#285 body) | npm | all | published with no binaries (unpacked 465 KB vs ~4 MB) | first run of a new release workflow; artifact path clash | AB#221 (0.7.2, same day); 100 KB size check AB#285 | same day |
| AB#222 | npm upgrade | macOS arm64 | "No binary found" | exec bit lost in tarball | postinstall chmod, AB#229 (0.7.4) | 0.1 d |
| AB#258 | pnpm 10+, bun, volta, `--ignore-scripts` | macOS arm64 | same | the fix relied on postinstall, which these skip | chmod at run time, AB#285 (0.8.1) | 2.0 d |
| AB#344 | bun | macOS arm64 | `spawn … EACCES` | new JS launcher dropped the run-time chmod | AB#354 (0.8.10) | 0.1 d |
| AB#296 | npm global | macOS | "Daemon not found" | postinstall symlinked the native binary; daemon path resolved through the symlink | `canonicalize()`, AB#299 (0.8.4) | 0.1 d |
| AB#324 | volta | Linux, macOS | command not found | postinstall symlink targets Volta's staging dir | none | open 258 d |
| AB#382, #514, #2047, #549 | npm | Windows x64 | Defender quarantine; Smart App Control blocks (MCP exit 199) | unsigned exe | none | open since 2026-02-06 |
| AB#1729 | npm | macOS arm64 | `spctl` rejects; commands hang with no output | reported as the ad-hoc signature; not confirmed (an npm-installed ad-hoc binary ran here) | none | open since 2026-08-26 |
| AB#1256, #1593 | npm | Windows ARM64 | 0-byte binary; "This app can't run on your PC" | no arm64 asset; non-atomic fallback download | x64 fallback AB#1269 (0.27.3), launcher fix + tests AB#1725 (0.35.1) | 56.9 d, then 35.7 d |
| AB#1587, #1477, #1391 | npm | Android, ppc64le | unsupported platform; empty file shipped | no case for the platform | none | open |
| AB#1415 | npm | RHEL 8 (glibc 2.28) | ``version `GLIBC_2.29' not found`` | no glibc floor | zigbuild `.2.28` + objdump guard, AB#1417 (0.27.2) | 5.8 d |
| AB#1548 | npm, pnpm | all | `EBADENGINE`; strict engines refuse Node 22 | dev constraint `node>=24` published (AB#1377) | none | open since 2026-07-14 |
| AB#1127 | upgrade | all | after upgrade, silently wrong results | old daemon kept serving; only the socket was checked | version handshake, AB#1134 (0.24.1) | 1.2 d |
| AB#1616 | upgrade | all | dashboard stops working after upgrade | running dashboard bound to replaced binary | none | open |
| AB#954 | upgrade | pnpm, yarn, bun | "could not detect installation method" | path heuristics only | `.install-method` marker, AB#960 (0.22.0) | 2.5 d |
| AB#1412, #2075 | brew | macOS, Linux | "Dashboard not built" | homebrew-core switched to a source build on 2026-05-28 that never builds the dashboard | none | open 133 d |
| AB#147 | brew, source | all | packagers can't build: no tags, wrong tag, Cargo.toml version behind npm | manual tagging, unsynced versions | retag; version-sync CI, AB#277, AB#302 | 2 d (still open) |
| AB#1560 | cargo | all | `cargo install` gives 0.19.0 | crates.io published by hand once | none | open since 2026-07-16 |
| AB#1983 | direct (mise, aqua), cargo | all | "Skills directory not found" | skills read from disk next to the binary; bare assets have none | none (Scoop worked around it) | open since 2026-09-23 |
| AB#963, #773, #1459, #1337 | skills | all, OpenCode | too many skills installed; renamed skills break locks; stub hidden in OpenCode | everything under `skills/` discoverable; `hidden: true` | moved to `skill-data/`, AB#1227 (0.25.4); rest none | 20.8 d; #773, #1337 open; #1459 closed without a fix |
| AB#178 | npm | macOS arm64 under Rosetta Node | looks for the x64 binary on Apple Silicon | not established in this research | not established | — |
| AB#1422, #1577 | npm fallback | all | truncated download left as the binary | writes straight to destination, no `SHA256SUMS` | none | open since 2026-06-06 |
| AB#39, #107, #1433, #1984, #1650, #716 | post-install Chrome download | various | browser missing, TLS proxy, macOS 12, no sudo, China CDN | browser download step | partly fixed (AB#754, AB#2006) | 60–121 d or open |

The post-install browser rows don't apply to agent-terminal, which downloads nothing after install. They are kept because they are the largest class of AB "install" complaints.

**Failed release runs** (`gh run list -R vercel-labs/agent-browser --workflow release.yml`): 579 runs from 2026-01-23 to 2026-10-15, 523 success, 21 failure, 35 cancelled. Logs before mid-July have expired (HTTP 410), so older causes come from check-run annotations and the fix commit that followed.

| Run(s) | Date | Cause | Fix |
|---|---|---|---|
| 21297928113 + 3 more | 2026-01-23 | first day of a new release workflow: artifact path clash (0.7.0 without binaries), `gh release create` on an existing tag | AB#221, AB#224, AB#232 |
| 22643080766 | 2026-03-03 | OpenSSL cross-compile for Linux | switch to rustls, AB#595 |
| 23076702783 + 3 more | 2026-03-14 | stale lockfile, four attempts in 17 min | AB#771 |
| 23657388973 | 2026-03-27 | workspace file left out the root package | AB#1053 |
| 24036596480 | 2026-04-06 | GitHub release failed after npm published | recovery path, AB#1168 |
| 25504746218 | 2026-05-07 | first trusted-publishing run: missing environment claim; an `if: always()` job created GitHub v0.27.0 30 min before npm had it | `environment: Release`, AB#1333 |
| 27887228882 | 2026-06-20 | first scoped-package publish: provenance needs `repository` and `publishConfig.access` | AB#1468 |
| 30023384759 | 2026-07-23 | re-runs could not fetch artifacts (`workflow run not found`); GitHub release 86 min after npm | re-runs |
| 32778098524 | 2026-08-24 | unpinned Rust 1.98 passed a linker flag Zig rejected (zigbuild ARM64) | pin cargo-zigbuild 0.23.2, AB#1716 |
| 22985182909, 23509761228 | March | GitHub flakiness (401 on an action download, `HttpError`) | re-run |

31 of the 35 cancelled runs never started a job: the workflow sets `concurrency: ${{ github.workflow }}-${{ github.ref }}` (`AB:.github/workflows/release.yml` line 9, since 2026-01-23) without `cancel-in-progress`, so GitHub keeps one pending run per group and drops older ones. 18 of the 31 came before the approval gate (2026-02-23 to 03-23); 13 came after it, when a run waiting for approval holds the group. Run 34240075240 (0.37.1) waited 8 days for approval and then published nothing.

**Timing.** Run start → npm publish, median / p90 / max: 5.0 / 5.7 / 13.3 min in the changesets era (66 runs, endpoint = npm registry `time`), 6.6 / 6.9 / 21.9 min in the version-diff era without a gate (11 runs), 18.4 / 91.3 / 2,749.6 min with the gate (32 runs from 2026-05-07, endpoint = publish-job end). In the gated era the median approval wait is 6.4 min (max 2,741.7 min, 45.7 h), and builds grew from 4.5 to 7.6 min. The homebrew-core bump follows npm by a median 1.7 h (p90 3.5 h, 82 versions); Scoop by 2.1 h (p90 5.4 h, 31 versions) (`gh api 'repos/Homebrew/homebrew-core/commits?path=Formula/a/agent-browser.rb'`, the same for ScoopInstaller/Main).

**Usage.**
- npm: 7,731,016 downloads from 2026-09-13 to 10-12 (`https://api.npmjs.org/downloads/range/last-month/agent-browser`).
- Of last week's 1,726,779 downloads, 55.1% were versions older than 0.27.1, and the latest (0.39.0) had 2.3% (`https://api.npmjs.org/versions/agent-browser/last-week`).
- Homebrew: 3,734 installs in 30 days (`https://formulae.brew.sh/api/formula/agent-browser.json`).
- GitHub release assets: 435,180 downloads all time, linux-x64 56% (`gh api repos/vercel-labs/agent-browser/releases`).

**Patterns.**
1. **npm launcher failures:** almost all came in the first three weeks, and each fix tended to cause the next (AB#229 → #258, AB#293 → #296 and #344). They stopped once CI ran a packed global install on three OSes and the release checked binary sizes.
2. **Platforms not built for:** Windows ARM64, Android and ppc64le keep returning as empty or wrong binaries.
3. **Unsigned binaries:** Windows Defender and Smart App Control issues are open; the macOS report (AB#1729) has an unconfirmed cause.
4. **Channels nobody tests drift silently:** homebrew-core, crates.io, and bare binaries without skills.
5. **Upgrade:** the old daemon kept serving after an upgrade until AB#1134 added a version handshake, and installer detection failed until AB#960 added a marker.
6. **Release pipeline failures:** about half came on the first run of a new mechanism; one (run 32778098524) came from an unpinned toolchain and four from a stale lockfile. Against the directly preceding ungated era the gate raised the median from 6.6 to 18.4 min (2.8×), of which 6.4 min is the approval wait; it also holds the concurrency group, so queued runs are dropped while it waits.
