const test = require('node:test');
const assert = require('node:assert/strict');
const { PLATFORMS, packageNameFor, platformPackageName } = require('../agent-terminal/lib/platform');

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
