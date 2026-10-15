const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { tempDir } = require('./support');

const PREFLIGHT = path.resolve(__dirname, '..', 'preflight.js');
const REGISTRY = path.resolve(__dirname, '..', 'install-test', 'registry.js');

const CHANGELOG_0_1_0 = '# Changelog\n\n## 0.1.0 (2026-10-15)\n\n### Features\n\n- `open` runs a program in a terminal.\n';

function checkout(t, { version, changelog }) {
  const dir = tempDir(t);
  fs.mkdirSync(path.join(dir, 'repo', 'cli'), { recursive: true });
  fs.writeFileSync(
    path.join(dir, 'repo', 'cli', 'Cargo.toml'),
    `[package]\nname = "agent-terminal"\nversion = "${version}"\nedition = "2024"\n\n[dependencies]\nlibc = { version = "0.2" }\n`,
  );
  fs.writeFileSync(path.join(dir, 'repo', 'CHANGELOG.md'), changelog);
  fs.mkdirSync(path.join(dir, 'registry'));
  fs.writeFileSync(path.join(dir, 'user-npmrc'), '');
  fs.writeFileSync(path.join(dir, 'global-npmrc'), '');
  return dir;
}

function npmEnv(dir) {
  return {
    ...process.env,
    npm_config_userconfig: path.join(dir, 'user-npmrc'),
    npm_config_globalconfig: path.join(dir, 'global-npmrc'),
    npm_config_cache: path.join(dir, 'npm-cache'),
    npm_config_update_notifier: 'false',
  };
}

function alreadyPublished(dir, name, version) {
  const source = path.join(dir, 'published', name);
  fs.mkdirSync(source, { recursive: true });
  fs.writeFileSync(path.join(source, 'package.json'), JSON.stringify({ name, version }));
  const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
  const pack = spawnSync(npm, ['pack', source, '--pack-destination', path.join(dir, 'registry')], {
    encoding: 'utf8',
    env: npmEnv(dir),
  });
  assert.equal(pack.status, 0, pack.stderr);
}

function preflight(dir, tag) {
  return spawnSync(
    process.execPath,
    [REGISTRY, path.join(dir, 'registry'), process.execPath, PREFLIGHT, '--tag', tag],
    { cwd: path.join(dir, 'repo'), encoding: 'utf8', env: npmEnv(dir) },
  );
}

test('a tag that is not v and the CLI crate version is refused', (t) => {
  const dir = checkout(t, { version: '0.1.0', changelog: CHANGELOG_0_1_0 });

  const run = preflight(dir, 'v0.2.0');

  assert.equal(run.status, 1, run.stdout);
  assert.match(run.stderr, /^the tag v0\.2\.0 does not match version 0\.1\.0 in cli\/Cargo\.toml: tag v0\.1\.0 instead/m);
});

test('a version without its own CHANGELOG.md section is refused', (t) => {
  const changelog = `# Changelog\n\n## Unreleased\n\n- \`close\` waits for the program.\n\n## 0.2.0-rc.1 (2026-10-20)\n\n- rc\n\n${CHANGELOG_0_1_0.slice(13)}`;
  const dir = checkout(t, { version: '0.2.0', changelog });

  const run = preflight(dir, 'v0.2.0');

  assert.equal(run.status, 1, run.stdout);
  assert.match(run.stderr, /^CHANGELOG\.md has no section for 0\.2\.0: add "## 0\.2\.0 \(<date>\)"/m);
  assert.doesNotMatch(run.stderr, /the tag/);
});

test('a package npm would refuse to publish is refused, with npm\'s reason', (t) => {
  const dir = checkout(t, { version: '0.1.0', changelog: CHANGELOG_0_1_0 });
  alreadyPublished(dir, '@repost/agent-terminal-linux-x64', '0.1.0');

  const run = preflight(dir, 'v0.1.0');

  assert.equal(run.status, 1, run.stdout);
  assert.equal(
    run.stderr,
    'npm publish --dry-run refused @repost/agent-terminal-linux-x64@0.1.0: ' +
      'You cannot publish over the previously published versions: 0.1.0.\n',
  );
});

test('a release whose tag, changelog section and seven packages check out is cleared, platform packages first', (t) => {
  const dir = checkout(t, { version: '0.1.0', changelog: CHANGELOG_0_1_0 });
  alreadyPublished(dir, '@repost/agent-terminal-linux-x64', '0.0.9');

  const run = preflight(dir, 'v0.1.0');

  assert.equal(run.stderr, '');
  assert.equal(run.status, 0);
  assert.equal(
    run.stdout,
    [
      'npm publish --dry-run accepts @repost/agent-terminal-darwin-arm64@0.1.0',
      'npm publish --dry-run accepts @repost/agent-terminal-darwin-x64@0.1.0',
      'npm publish --dry-run accepts @repost/agent-terminal-linux-arm64@0.1.0',
      'npm publish --dry-run accepts @repost/agent-terminal-linux-x64@0.1.0',
      'npm publish --dry-run accepts @repost/agent-terminal-linux-arm64-musl@0.1.0',
      'npm publish --dry-run accepts @repost/agent-terminal-linux-x64-musl@0.1.0',
      'npm publish --dry-run accepts @repost/agent-terminal@0.1.0',
      'v0.1.0 can be released: it matches cli/Cargo.toml, CHANGELOG.md has its section, npm accepts all 7 packages',
      '',
    ].join('\n'),
  );
});

test('a release candidate is checked too, though npm would never let a prerelease become latest', (t) => {
  const changelog = `# Changelog\n\n## 0.2.0-rc.1 (2026-10-20)\n\n- \`close\` waits for the program.\n\n${CHANGELOG_0_1_0.slice(13)}`;
  const dir = checkout(t, { version: '0.2.0-rc.1', changelog });
  alreadyPublished(dir, '@repost/agent-terminal', '0.1.0');

  const run = preflight(dir, 'v0.2.0-rc.1');

  assert.equal(run.stderr, '');
  assert.equal(run.status, 0);
  assert.match(run.stdout, /^npm publish --dry-run accepts @repost\/agent-terminal@0\.2\.0-rc\.1$/m);
  assert.match(
    run.stdout,
    /^v0\.2\.0-rc\.1 can be released: it matches cli\/Cargo\.toml, CHANGELOG\.md has its section, npm accepts all 7 packages$/m,
  );
});

test('a release candidate rehearses the release it leads to, so its section or that release\'s section clears it', (t) => {
  const release = `# Changelog\n\n## Unreleased\n\n## 0.2.0 (2026-10-20)\n\n- \`close\` waits for the program.\n\n${CHANGELOG_0_1_0.slice(13)}`;
  const withRelease = checkout(t, { version: '0.2.0-rc.1', changelog: release });

  const cleared = preflight(withRelease, 'v0.2.0-rc.1');

  assert.equal(cleared.stderr, '');
  assert.equal(cleared.status, 0);

  const unreleased = `# Changelog\n\n## Unreleased\n\n- \`close\` waits for the program.\n\n${CHANGELOG_0_1_0.slice(13)}`;
  const withoutSection = checkout(t, { version: '0.2.0-rc.1', changelog: unreleased });

  const refused = preflight(withoutSection, 'v0.2.0-rc.1');

  assert.equal(refused.status, 1);
  assert.equal(
    refused.stderr,
    'CHANGELOG.md has no section for 0.2.0-rc.1: add "## 0.2.0-rc.1 (<date>)" as docs/releasing.md describes, ' +
      'or the "## 0.2.0 (<date>)" section it rehearses\n',
  );
});
