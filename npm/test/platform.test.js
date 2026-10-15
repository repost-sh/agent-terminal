const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { PLATFORMS, packageNameFor, platformPackageName } = require('../agent-terminal/lib/platform');

const ROOT = path.resolve(__dirname, '..', '..');

function cleanInstallRows() {
  const workflow = fs.readFileSync(path.join(ROOT, '.github', 'workflows', 'npm-packages.yml'), 'utf8');
  return workflow
    .split(/^ *- /m)
    .map((item) => ({
      name: item.match(/^name: (.+)$/m)?.[1],
      target: item.match(/^ *target: (\S+)$/m)?.[1],
      package: item.match(/^ *package: "([^"]+)"$/m)?.[1],
    }))
    .filter((row) => row.package);
}

test('the release builds every platform the npm packages cover, and the clean-install test installs each one', () => {
  const dist = fs.readFileSync(path.join(ROOT, 'dist-workspace.toml'), 'utf8');
  const targets = dist.match(/^targets = \[([^\]]*)\]/m);
  assert.ok(targets, 'dist-workspace.toml has no targets list');
  assert.deepEqual(
    [...targets[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]).sort(),
    PLATFORMS.map((platform) => platform.target).sort(),
  );

  const rows = cleanInstallRows();
  for (const row of rows) {
    const platform = PLATFORMS.find((candidate) => candidate.target === row.target);
    assert.ok(platform, `the clean-install row ${row.name} installs ${row.target}, which is not a release target`);
    assert.equal(row.package, packageNameFor('@repost/agent-terminal', platform), `the clean-install row ${row.name}`);
  }
  assert.deepEqual(
    [...new Set(rows.map((row) => row.package))].sort(),
    PLATFORMS.map((platform) => packageNameFor('@repost/agent-terminal', platform)).sort(),
  );
});

test('each supported platform maps to its own @repost binary package', () => {
  const cases = [
    [{ platform: 'darwin', arch: 'arm64', libc: null }, '@repost/agent-terminal-darwin-arm64'],
    [{ platform: 'darwin', arch: 'x64', libc: null }, '@repost/agent-terminal-darwin-x64'],
    [{ platform: 'linux', arch: 'arm64', libc: 'glibc' }, '@repost/agent-terminal-linux-arm64'],
    [{ platform: 'linux', arch: 'x64', libc: 'glibc' }, '@repost/agent-terminal-linux-x64'],
    [{ platform: 'linux', arch: 'arm64', libc: 'musl' }, '@repost/agent-terminal-linux-arm64-musl'],
    [{ platform: 'linux', arch: 'x64', libc: 'musl' }, '@repost/agent-terminal-linux-x64-musl'],
  ];
  for (const [host, expected] of cases) {
    assert.equal(platformPackageName('@repost/agent-terminal', host), expected, JSON.stringify(host));
  }
});

test('platforms without a prebuilt binary map to nothing', () => {
  assert.equal(platformPackageName('@repost/agent-terminal', { platform: 'win32', arch: 'x64', libc: null }), null);
  assert.equal(platformPackageName('@repost/agent-terminal', { platform: 'linux', arch: 'ia32', libc: 'glibc' }), null);
  assert.equal(platformPackageName('@repost/agent-terminal', { platform: 'freebsd', arch: 'x64', libc: null }), null);
});
