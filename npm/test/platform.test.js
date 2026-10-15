const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { PLATFORMS, packageNameFor, platformPackageName } = require('../agent-terminal/lib/platform');

const ROOT = path.resolve(__dirname, '..', '..');

test('the release builds every platform the npm packages cover, and the install test installs each one', () => {
  const dist = fs.readFileSync(path.join(ROOT, 'dist-workspace.toml'), 'utf8');
  const targets = dist.match(/^targets = \[([^\]]*)\]/m);
  assert.ok(targets, 'dist-workspace.toml has no targets list');
  assert.deepEqual(
    [...targets[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]).sort(),
    PLATFORMS.map((platform) => platform.target).sort(),
  );

  const workflow = fs.readFileSync(path.join(ROOT, '.github', 'workflows', 'npm-packages.yml'), 'utf8');
  assert.deepEqual(
    [...workflow.matchAll(/^ +package: (\S+)$/gm)].map((match) => match[1]).sort(),
    PLATFORMS.map((platform) => packageNameFor('agent-terminal', platform)).sort(),
  );
});

test('each supported platform maps to its own binary package', () => {
  const cases = [
    [{ platform: 'darwin', arch: 'arm64', libc: null }, 'agent-terminal-darwin-arm64'],
    [{ platform: 'darwin', arch: 'x64', libc: null }, 'agent-terminal-darwin-x64'],
    [{ platform: 'linux', arch: 'arm64', libc: 'glibc' }, 'agent-terminal-linux-arm64'],
    [{ platform: 'linux', arch: 'x64', libc: 'glibc' }, 'agent-terminal-linux-x64'],
    [{ platform: 'linux', arch: 'arm64', libc: 'musl' }, 'agent-terminal-linux-arm64-musl'],
    [{ platform: 'linux', arch: 'x64', libc: 'musl' }, 'agent-terminal-linux-x64-musl'],
  ];
  for (const [host, expected] of cases) {
    assert.equal(platformPackageName('agent-terminal', host), expected, JSON.stringify(host));
  }
});

test('platforms without a prebuilt binary map to nothing', () => {
  assert.equal(platformPackageName('agent-terminal', { platform: 'win32', arch: 'x64', libc: null }), null);
  assert.equal(platformPackageName('agent-terminal', { platform: 'linux', arch: 'ia32', libc: 'glibc' }), null);
  assert.equal(platformPackageName('agent-terminal', { platform: 'freebsd', arch: 'x64', libc: null }), null);
});

test('binary packages are named after the main package, scope included', () => {
  assert.equal(
    platformPackageName('@repost-sh/agent-terminal', { platform: 'linux', arch: 'x64', libc: 'musl' }),
    '@repost-sh/agent-terminal-linux-x64-musl',
  );
});
