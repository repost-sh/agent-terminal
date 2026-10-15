# agent-terminal release and distribution: design input

Date: 2026-10-15. Input for `/grill-with-docs`. It records evidence and options, and leaves the decisions open: they are listed at the end, each after the decisions it depends on.

Goal: agent-terminal's distribution should be at least as reliable as agent-browser's (AB), and the proof should come from checks outside this document. AB is the baseline to beat, not the template. AB facts and incident ids (AB#n) come from `docs/research/agent-browser-distribution.md`.

Sources for agent-terminal (AT) facts:
- The repo at `0b2cc6f`.
- A local release rehearsal on 2026-10-15 (macOS arm64 host):
  - `dist plan` and `dist build` with the pinned cargo-dist 0.33.0.
  - `npm publish --dry-run` for all seven packages.
  - Installs from locally packed tarballs into a prefix (npm 11, pnpm 11.17, bun 1.3.14).
  - A daemon version-skew test between 0.1.0 and a scratch 0.2.0-skew.1 build.

  Its logs and the draft clean-install workflow are kept locally in the gitignored `.superpowers/research/rehearsal/`. Each result below quotes the command and its output.

## 1. Where agent-terminal stands

Four findings decide the order of work:

1. **A release today fails, and once the secrets exist it would half-publish** (inferred from `release.yml` and the dry run; no tag was pushed). Today `host` would create the GitHub Release, then both publish jobs fail at their first step: no `NPM_TOKEN`, no tap repo or `HOMEBREW_TAP_TOKEN` (`gh api repos/repost-sh/agent-terminal/actions/secrets`: `total_count: 0`). The generated `release.yml` runs the jobs in this order:
   1. build
   2. `custom-npm-packages`
   3. `host` (creates the GitHub Release)
   4. `publish-homebrew-formula` and `custom-publish-npm`, in parallel
   5. `announce`

   `publish-npm.yml` publishes the six platform packages and then the main one. The main one fails: `npm publish --dry-run` on `agent-terminal@0.1.0` gives `Cannot implicitly apply the "latest" tag because previously published version 1.0.1 is higher than the new version 0.1.0`, and the name belongs to another account (`npm view agent-terminal`: 1.0.0, 1.0.1, maintainer `jasonkneen`). Once the secrets and tap exist but the npm name is still foreign, the release, the formula and six platform packages would go out before the main package fails, and nothing rolls back.
2. **Every "reinstall" path points at a stranger's package.**
   - `upgrade` runs `npm install -g agent-terminal@latest` (`cli/src/upgrade.rs:48`).
   - `doctor` suggests `npm i -g agent-terminal` (`cli/src/doctor.rs:84`).
   - The skill stub and `skill-data/{core,tui-qa,cli-qa}/SKILL.md` allow `npx agent-terminal` in `allowed-tools`.
   - The launcher's hint `npm i -g ${name}` (`npm/agent-terminal/bin/agent-terminal.js:26`) takes the name from `package.json`, so it follows the name decision automatically.

   Today `upgrade` would replace a user's install with the other owner's 1.0.1. During the rehearsal, pnpm 11 ignored `npm_config_registry`, fetched that package (95 packages: it and 94 dependencies, including node-pty), and its bin was run twice with `--version`. pnpm blocked its build scripts, and it crashed at module load with `ERR_IMPORT_ATTRIBUTE_MISSING`. The directories were deleted.
3. **No daemon version handshake** (abridged rehearsal output):
   ```
   $ B --version            # B = 0.2.0-skew.1, daemon started by A = 0.1.0 (comment added)
   agent-terminal 0.2.0-skew.1
   $ B snapshot             (output omitted: succeeded against A's daemon, exit 0)
   $ B doctor
   ok       daemon   running, listening on …/daemon.sock
   ```
   A request the old daemon can't parse produces only `daemon closed the connection without replying (got 0 bytes)`. The cause appears only in `daemon.log` (`unknown variant 'future-command'`). After a real `upgrade` from 0.1.0 to 0.2.0-skew.1 from a local registry, the 0.1.0 daemon kept serving. AB had the same bug: AB#1127 produced silently wrong results and was fixed in 1.2 d with a handshake (AB#1134).
4. **Nothing checks the glibc floor.** The gnu targets build on `ubuntu-22.04` (glibc 2.35), according to `dist plan`. The floor is therefore probably 2.34–2.35, which rules out Debian 11, Ubuntu 20.04 and RHEL 8. This is inferred from the runner image and not measured: a Linux build was not possible locally without Docker. No CI run has tested a Linux floor: the bookworm row is in `npm-packages.yml`, which has never run (see below). AB pins 2.28 and fails the build above it, after AB#1415.

What already beats AB:
- **One npm package per platform.** The darwin-arm64 package is 8.4 MB packed and 17.5 MB unpacked. AB's single package is 53.5 MB packed and 119 MB unpacked.
- **No postinstall.** Every AB bug in the AB#222 → #258 → #344 and AB#296/#324 chains came from postinstall or the exec bit.
- **Skills are compiled into the binary,** so every channel has them. Missing skill files is open as AB#1983.
- **The release tag exists before the build,** because a tag push triggers the release. AB's v0.38.0 tag drift (tag created after an approval wait) can't happen.
- **The install test is designed to use the actual packed release artifacts** on Debian, Alpine and macOS before publishing. But it has never run: `npm-packages.yml` is `workflow_call` only and is called from `release.yml` behind `build-local-artifacts`, which runs only on a tag (0 runs to date). AB's `global-install` job runs on every push to main, on three OSes, with a CI-built binary.

At parity with AB: the macOS binary links only system libraries (`otool -L`: `libc++.1`, `libiconv.2`, `libSystem.B`; minos 11.0).

## 2. Failure-mode table

Status values:
- **PASS / FAIL:** observed in the rehearsal.
- **untested:** not exercised yet.
- **n/a:** does not apply.

"AB" names the matching AB incident. "Mitigation" options are listed for the owner, not decided.

### By channel and condition

| Channel | Condition | AT status (evidence) | AB precedent | Mitigation options |
|---|---|---|---|---|
| npm global | normal | PASS: 2 packages added; `--version` 0.1.0; open/snapshot/close round trip | — | — |
| npm global | `--ignore-scripts` | PASS: no lifecycle scripts to skip | AB#258 (fixed) | keep no-postinstall design |
| npm global | `--omit=optional` | PASS on npm 11: it still installed the platform package | — | add to clean-install matrix |
| npm global | registry unreachable for the platform package during install | **FAIL**: `added 1 package`, exit 0, no binary; fails on first run | AB#1256 (0-byte fallback) | launcher error naming the exact command; `doctor` check |
| npm global | exec bit lost on the binary | **FAIL**: `spawnSync … EACCES`, no chmod fallback | AB#222, #344 | chmod-and-retry in the launcher |
| npm | platform package missing | FAIL (message): "agent-terminal-darwin-arm64 is not installed… reinstall with optional dependencies: npm i -g agent-terminal". The command it names is the foreign package | — | blocked on the npm name decision |
| npm | unsupported OS or arch | PASS: clear message pointing to `cargo install --git` | AB#1587, #1391 (unclear errors) | — |
| pnpm 11 | explicit `--registry` | PASS; on release day pnpm writes 7 `minimumReleaseAgeExclude` entries to `pnpm-workspace.yaml` | AB#1548 (engines) | document pnpm's release-age gate; keep `engines.node >= 18` loose |
| pnpm 11 | `npm_config_registry` only | ignored: fetched from public npm | — | test with pnpm's own config in CI |
| bun 1.3 | normal | PASS | AB#344 | add to clean-install matrix |
| yarn | any | untested | — | add to matrix if in scope |
| volta | any | untested; no postinstall symlink, so AB#324 can't recur | AB#324 (open) | — |
| any | no TTY (`</dev/null \| cat`) | PASS; `attach` refuses with exit 1 | — | — |
| any | offline after install | PASS for all commands except `upgrade`: `` `npm install -g agent-terminal@latest` failed `` | — | `upgrade` checks the registry first and says it is offline |
| any | registry mirror (Verdaccio, Artifactory) | untested | AB#1433 was a TLS-proxy issue in a download path AT doesn't have | add a mirror row to the clean-install matrix |
| npm | Node launcher overhead | unmeasured: the one 5-call sample's native baseline named no binary; re-measure against the platform binary path | AB postinstall symlink causes AB#296, #324 | owner decision; see D10 |
| Homebrew tap | install from generated formula | untested: tap repo doesn't exist; formula generated (`agent-terminal.rb`, 1,357 B) | AB#1412 (homebrew-core source build lost a feature) | own tap avoids a third-party rebuild |
| direct archive | browser download on macOS | **FAIL expected** (inferred): ad-hoc signature (`flags=0x20002(adhoc,linker-signed)`), `spctl` rejected; quarantine would block it | none for browser downloads; AB#1729 is an npm install with an unconfirmed cause | sign and notarize, or document `xattr` and npm/brew as the only paths |
| direct archive | checksum | `dist build` writes per-archive `.sha256` and `sha256.sum` | AB#1577 (none) | verify in clean-install; note busybox `sha256sum` format risk |
| cargo install --git | source build | the README's only path today; untested in CI | AB#1560 (crates.io frozen) | owner decision on crates.io |
| Linux gnu | glibc below the build host's | **FAIL likely**, unmeasured | AB#1415 | pin a floor and check with `objdump -T` in CI |
| Linux musl (Alpine) | normal | untested: configured in `npm-packages.yml`, never run | — | run the clean-install workflow |
| macOS x64 under Rosetta | arm64 Node + x64 binary or the reverse | untested | AB#178 (x64 binary chosen under Rosetta Node) | add a `arch -x86_64` row |
| Windows | any | n/a: no Windows target | AB's most fragile platform (AB#25, #382, #1256) | scope decision D5 |

### Risks only agent-terminal has

| Risk | Status | Evidence | Mitigation options |
|---|---|---|---|
| Binary upgraded while the old daemon runs | **FAIL** | section 1, item 3 | daemon reports its version in every reply or at connect; CLI restarts or refuses on mismatch; `doctor` shows both versions; `upgrade` stops the daemon |
| Wire-format change surfaces as "closed the connection" | **FAIL** | same | a version check before the first request turns this into a named error |
| Skill stub version ≠ binary version after upgrade | **FAIL** | the stub keeps `metadata.version: 0.1.0`, while `skills get core` reports 0.2.0-skew.1 | `upgrade` re-runs `install --skills`; `doctor` flags a stale stub |
| Skill cache grows forever | from code, not observed (`cli/src/skills.rs:220-227`) | `AGENT_TERMINAL_HOME/skills/<version>/` is never pruned | prune old versions on start |
| Stub from `npx skills add` has no version | observed | `install` always overwrites it | stamp a version in the repo stub at release |
| Half-published release | inferred | section 1, item 1 | a preflight job (name ownership, `npm publish --dry-run` for every package, tap reachable) that every publishing job needs |

## 3. Support matrix

These are proposals. Each row needs an owner decision on the floor, and then the check in the last column enforces it.

| Dimension | Proposed floor | Current fact | Enforcing check |
|---|---|---|---|
| macOS arm64 | 11.0 | `minos 11.0` (`otool -l`) | `otool -l` assert in the release build; clean install on `macos-14` |
| macOS x64 | 11.0 | x86_64-apple-darwin built on `macos-15-intel` (`dist plan`) | same; clean install on `macos-15-intel` (the `macos-13` image is retired) |
| Linux gnu x64/arm64 | open (AB uses 2.28) | built on ubuntu-22.04 (glibc 2.35); unmeasured | `objdump -T` newest `GLIBC_` ≤ floor, fail the build; run on bullseye (2.31), focal (2.31), rocky8 (2.28) |
| Linux musl x64/arm64 | any musl | static, toolchain pinned by image digest (`dist-workspace.toml`) | `file` says statically linked; Alpine clean install |
| Windows | out of scope today | no target | — |
| Node (npm launcher) | 18 | `engines.node >= 18` | clean install on the oldest supported Node |
| npm clients | npm ≥ 9, pnpm ≥ 9, bun ≥ 1.1, yarn: open | npm 11, pnpm 11.17, bun 1.3.14 tested locally | clean-install matrix row per client |
| Homebrew | current | formula generated, tap missing | `brew audit --strict --online` + `brew test` on the tap formula |

## 4. Supply-chain threat model

| Threat | Today | AB today | Target options |
|---|---|---|---|
| Name squatting / confusion on npm | **Realised**: `agent-terminal` is someone else's. The six platform names are free (E404) and could be taken by anyone first | owns its name | reserve the chosen names before the first release; publish all names at once |
| Name squatting on crates.io | **Realised**: `agent-terminal` 0.1.0 by `ohwxyz` | stale own crate | another crate name, or no crates.io and say so in the README |
| Dependency confusion via `upgrade` | **Realised**: `upgrade` installs `agent-terminal@latest` from public npm | — | `upgrade` uses the final package name, checks the registry, and refuses an unexpected publisher |
| Stolen npm token | `publish-npm.yml` uses `secrets.NPM_TOKEN` (not set yet) | none: OIDC trusted publishing, `_npmUser = GitHub Actions` | trusted publishing per package. AB's first OIDC run failed on the environment claim (run 25504746218), so rehearse it |
| Binary not traceable to source | no attestations; dist `github-attestations` not set | npm tarball SLSA v1 only; release binaries 404 | npm `--provenance` plus GitHub artifact attestations on archives. That gives more coverage than AB: `gh attestation verify <archive>` would succeed |
| Gatekeeper / Defender reputation | ad-hoc signature only | same, issues open | Developer ID + notarization for macOS archives; npm and brew paths are unaffected |
| Toolchain drift in the release build | musl images pinned by digest; dist pinned 0.33.0; Rust toolchain version not checked here | AB run 32778098524 broke on unpinned Rust 1.98 | pin `rust-toolchain.toml`; `dist plan` is configured for PRs but has never run (no PRs yet) |
| Pinned dist not installable from crates.io | `cargo install cargo-dist --version 0.33.0` fails (crates.io stops at 0.32.0); the GitHub release binary verifies with `gh attestation verify` | — | install dist from the GitHub release in docs and CI |
| Approval bypass | no environment; a tag push publishes | `Release` environment with reviewers | environment with reviewers on publishing jobs. AB's data: median release went from 6.6 to 18.4 min (approval wait median 6.4 min, max 45.7 h); while a run waits it holds the `concurrency` group, and 13 queued runs were dropped after the gate arrived |

SLSA level: npm `--provenance` from GitHub-hosted runners would give the npm tarballs SLSA Build L2 once published, by analogy with GitHub's statement that artifact attestations by themselves provide SLSA v1.0 Build L2 (https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations). AB is at that level. L3 (https://slsa.dev/spec/v1.0/levels) needs runs that can't influence one another and signing keys unreachable from user-defined build steps, usually via a reusable build workflow; the target is an owner decision (D7).

## 5. Release rehearsal plan

Steps 1 and 2 have run. The rest are blocked as marked.

1. **Local plan and build.** `dist plan` PASS with 0.33.0. `dist build --artifacts=all --target=aarch64-apple-darwin` PASS in 36 s:
   - `agent-terminal-aarch64-apple-darwin.tar.xz` (6,145,968 B; binary 17,506,368 B)
   - `source.tar.gz` (4,256,582 B)
   - `agent-terminal.rb`
   - `sha256.sum` and per-archive `.sha256`
2. **Publish dry runs.** The six platform package manifests PASS (only darwin-arm64 held a real binary; the other five had placeholders). The main package FAILs (section 1, item 1).
3. **Clean-machine install on CI** (drafted, actionlint clean, not pushed; blocked on approval to push a branch):
   - Build all six targets. Report the newest `GLIBC_` with `objdump -T`.
   - Matrix: ubuntu x64 and arm64, alpine x64, macos-14, macos-15-intel. Each row installs through three channels:
     - the npm tarball from a local registry
     - the direct archive, verified against its `.sha256`
     - the Homebrew formula with a `file://` URL
   - Each row asserts:
     - `--version` and `--json` version equal the tag
     - the stub version equals the tag
     - an open/snapshot/close round trip, then `daemon stop`
     - runs with no TTY and with no network
   - A glibc-floor job runs the gnu binary on bullseye, focal and rocky8.
   - AB baseline rows run on the same matrix.
4. **Release candidate under a throwaway scope** (blocked on D1 and D3):
   - Tag `v0.1.0-rc.1`. `docs/releasing.md` says prereleases publish nothing, so publishing an rc to a throwaway scope needs a workflow change or a manual publish.
   - Measure:
     - the time from tag push to `npm view <pkg>@<v>` succeeding
     - the time to the GitHub Release
     - the time to `brew install` from the tap
5. **Time-to-installable targets.**
   - AB baseline: 6.6 min median with no gate (version-diff era), 18.4 min with the gate. Homebrew-core lags npm by a median 1.7 h, Scoop by 2.1 h.
   - AT's tap is pushed in the same run, so Homebrew should land within minutes of npm. That is unmeasured until step 4.
6. **Preflight before anything public** (proposed): one job that every publishing job needs. It checks:
   - the chosen npm names are owned or free
   - `npm publish --dry-run` passes for every package
   - the tap repo is writable
   - the tag equals `cli/Cargo.toml` version

## 6. Rollback

npm cannot reuse a version number once published, even after unpublish (https://docs.npmjs.com/policies/unpublish). Rollback therefore means moving users off the bad version and shipping a fixed one.

1. **Stop new installs of the bad version:** `npm dist-tag add <pkg>@<good> latest` for the main package. Platform packages are pinned exactly by the main package's `optionalDependencies`, so they follow it.
2. **Warn existing installs:** `npm deprecate <pkg>@<bad> "<reason>; upgrade to <good>"`, for the main package and every platform package.
3. **Unpublish only if the policy allows it** (https://docs.npmjs.com/policies/unpublish): within 72 h if no public package depends on it; later only with no dependents, under 300 downloads last week and a single owner. The policy doesn't say whether a main package listing the platform packages as `optionalDependencies` blocks unpublishing a platform version; verify before relying on it.
4. **Homebrew tap:** reverting the formula in `repost-sh/homebrew-tap` stops new installs of the bad version but does not move existing installs: `brew upgrade` only acts when the formula version is higher than the installed keg (Homebrew `Formula#outdated_kegs`). Existing users move only when a higher fixed version is published, by `brew uninstall` + `brew install`, or by bumping `version_scheme`.
5. **GitHub Release:** mark it as a prerelease or delete its assets. Never move the tag, because Homebrew and source users rely on it (AB#147 and the v0.38.0 tag drift show what a tag on the wrong commit costs packagers).
6. **Ship the fix as a new patch version** through the normal pipeline. Measure the time from bad publish to fixed `latest`; AB's reproducible regressions took 0.1–2 d.
7. **Daemons:** with the handshake in place, users' old daemons restart on the next command. Without it, the release notes must tell users to run `agent-terminal daemon stop`.

## 7. Checks that decide whether the result is right

| Check | Runnable now? | What it proves |
|---|---|---|
| `dist plan` | **yes** locally (PASS); configured for PRs but never run in CI (no PRs yet) | release workflow matches config |
| `npm publish --dry-run` for all 7 packages | **yes** locally; main package fails until D1 | packages are publishable |
| `cargo test`, npm unit tests | **yes** (existing CI on push to main) | code and launcher logic |
| `npm-packages.yml` install test | configured but never run: only inside a tag-triggered release | packed artifacts work on Debian, Alpine, macOS |
| clean-install workflow (section 5, step 3) | blocked: owner approval to push a branch | every documented install path delivers the tag's binary, skill and a working daemon |
| glibc floor check on bullseye, focal, rocky8 | blocked: same approval, then D5 sets the floor | Linux support claim is true |
| daemon version handshake test | blocked: D6 (behaviour) and code | upgrade can't leave a stale daemon |
| `brew audit --strict --online` + `brew test` | blocked: tap repo (D8) | formula is valid |
| `npm audit signatures` after install | blocked: first real publish (D1, D3) | registry signatures and provenance exist |
| `gh attestation verify` on npm tarball (`--digest-alg sha512`) | blocked: first publish | npm provenance links to this repo's workflow |
| `gh attestation verify` on release archives | blocked: D7 (enable dist `github-attestations`) | binaries link to this repo's workflow, which AB can't show |
| `spctl -a -t exec -vv` on the macOS archive | runnable now: currently `rejected`; passes only after D7 signing | direct download isn't blocked |
| OpenSSF Scorecard | blocked: needs a GitHub token (owner approval); public API returns 404 for this repo | repo-level supply-chain hygiene |
| fresh agent sandbox installs from the README alone | blocked: D1, then a real release | the documented path works for the actual user, an agent |
| post-release smoke on a schedule | blocked: first release | channels don't drift silently (AB#1412, #1560) |

## 8. Owner decisions, ordered so each follows the ones it depends on

1. **D1 — npm package name: decided.** `@repost/agent-terminal`, with platform packages `@repost/agent-terminal-<os>-<cpu>[-musl]`; the command stays `agent-terminal` (`docs/adr/0003-npm-package-is-repost-scoped.md`). The `@repost` organisation already publishes `@repost/cli` with the same per-platform `optionalDependencies` layout, through GitHub Actions trusted publishing (`npm view @repost/cli`: `_npmUser` GitHub Actions) and without provenance attestations. Unblocks:
   - the main publish, the preflight, the rc rehearsal, `npm audit signatures` and the fresh-agent check
   - renaming in `upgrade.rs:48`, `doctor.rs:84`, the README, and `allowed-tools` in the stub and three skill-data files
   - trusted-publisher setup per package (D3)
   - `publishConfig.access: public` on all seven packages

2. **D2 — release trigger: decided, tag push** (the existing cargo-dist model in `docs/releasing.md`). The tag exists before the build, so AB's tag drift can't happen; `publish-npm.yml` already skips versions on npm, so re-running a failed release is safe. Remaining effects: the preflight and D4's gate attach to the tag-triggered `release.yml`.

3. **D3 — npm auth:** `NPM_TOKEN` or trusted publishing. Depends on D1, because it is configured per package name; that means seven packages. It affects the threat model, the need for a GitHub environment, and the rehearsal (AB's first OIDC run failed).
4. **D5 — platform scope and floors:** the glibc floor, whether Windows comes in and when, Rosetta, and which npm clients (yarn?). It sets the build matrix, the support matrix, clean-install rows and the README requirements.
5. **D6 — daemon version handshake behaviour:** on mismatch, auto-restart, refuse with a named error, or warn. It affects `upgrade`, `doctor` and the rollback step for daemons. AB chose a handshake after AB#1127.
6. **D7 — signing and attestations:**
   - macOS Developer ID and notarization (needs an Apple account)
   - dist `github-attestations`
   - the SLSA target (L2 or L3)

   It affects the direct-download channel, the threat model and three checks.
7. **D4 — approval gate:** a GitHub environment with reviewers on publishing jobs. AB's evidence: safer, about 3.6× slower at the median, occasionally days, and it silently cancels queued runs together with `concurrency`. Depends on D2.
8. **D8 — channels:**
   - Homebrew: own tap (repo plus token) and/or homebrew-core later
   - crates.io: another name, or none
   - a `curl | sh` installer (dist can generate one)
   - `npx skills add`, a skills.sh listing and `.claude-plugin/marketplace.json`

   Each channel adds rows to the clean-install matrix and the scheduled smoke.
9. **D9 — skills on upgrade:** should `upgrade` refresh the stub, should `doctor` flag a stale stub, should old skill caches be pruned, and should `install --skills` support agents other than Claude Code?
10. **D10 — launcher path:** keep a Node process per call (overhead to be measured), or add a native fast path. Any postinstall-based fast path reintroduces AB's most frequent bug class (AB#222, #258, #296, #324, #344).
11. **D11 — `upgrade` coverage:** check the registry first, and support pnpm, yarn, bun and cargo, using an install-method marker (AB#954, AB#960) or path detection. Depends on D1 and D5.
12. **D12 — first version and rehearsal:** the first version number, and whether an rc goes to a throwaway scope first (requires a workflow change, because prereleases publish nothing today).

Operational approvals, outside the design:
- push the clean-install workflow to a branch
- run Scorecard with the owner's GitHub token
- delete the scratch state under `.superpowers/` when done

## 9. Decisions taken by default (2026-10-15)

The owner's direction: copy agent-browser's release pipeline, and agents run releases without asking the owner. Every remaining decision therefore defaults to what AB does, departing from it only where the incident history shows AB's way broke. These replace D3–D12 above as open questions.

| Decision | Choice | Basis |
|---|---|---|
| Changelog | `CHANGELOG.md` at the repo root, one section per version (`## <version> (<date>)`), written by the releasing agent from the Conventional Commits since the last tag. The release procedure lives in `docs/releasing.md` with a pointer from `AGENTS.md`, as AB keeps its own in `AB:AGENTS.md` ("Releasing"). cargo-dist puts the matching section into the GitHub release and uses its heading as the title (dist book, "Release Notes"); the preflight refuses a tag with no section | AB, adapted to dist |
| D3 npm auth | trusted publishing (OIDC), no token secret, `--provenance` | AB; `@repost/cli` already publishes this way |
| D4 approval gate | none: the tag push is the release decision | departs from AB: a reviewer click per release contradicts agent-run releases, and AB's gate cost a 6.4 min median wait (max 45.7 h) and dropped queued runs |
| D5 platforms | glibc floor 2.28 enforced by an `objdump` check in the release build, as AB; Windows stays out until the PTY and daemon run on Windows, which is product work rather than distribution | AB (AB#1415) |
| D6 daemon handshake | the CLI checks the daemon's version before its first request and restarts a daemon from another version; `doctor` shows both versions | AB#1127 → AB#1134 |
| D7 signing and attestations | no code signing, as AB; enable cargo-dist's GitHub artifact attestations, which AB lacks and which costs one config line | AB plus one cheap improvement |
| D8 channels | npm and the `repost-sh/homebrew-tap` formula that cargo-dist already generates; `npx skills add repost-sh/agent-terminal` and `.claude-plugin/marketplace.json` for skills, as AB; no crates.io (name taken, and AB's crate went stale), no curl installer (AB has none) | AB |
| D9 skills on upgrade | `upgrade` refreshes the installed stub; `doctor` flags a stub whose version differs from the binary | AB's skills rows (AB#73) and the rehearsal |
| D10 launcher | keep the Node launcher, no postinstall | departs from AB: its postinstall symlink caused AB#296 and AB#324 (open 258 d) |
| D11 upgrade | check the registry for the latest version first, then reinstall with the detected installer: npm, pnpm, yarn, bun or Homebrew | AB (`AB:cli/src/upgrade.rs`, AB#954) |
| D12 first release | `v0.1.0`, tagged once the clean-install workflow passes on a branch | `docs/releasing.md` |

