const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { TARGETS, tempDir, distRelease, build, readJson, packedFiles } = require('./support');

const printTarget = (target) => `#!/bin/sh\necho ${target}\n`;

test('one package per platform, each holding only its own binary', (t) => {
  const dir = tempDir(t);
  distRelease(dir, TARGETS, printTarget);

  const run = build(dir);
  assert.equal(run.status, 0, run.stderr);

  const out = path.join(dir, 'npm');
  assert.deepEqual(fs.readdirSync(out), ['@repost']);
  assert.deepEqual(fs.readdirSync(path.join(out, '@repost')).sort(), [
    'agent-terminal',
    'agent-terminal-darwin-arm64',
    'agent-terminal-darwin-x64',
    'agent-terminal-linux-arm64',
    'agent-terminal-linux-arm64-musl',
    'agent-terminal-linux-x64',
    'agent-terminal-linux-x64-musl',
  ]);

  const musl = path.join(out, '@repost', 'agent-terminal-linux-x64-musl');
  assert.deepEqual(readJson(path.join(musl, 'package.json')), {
    name: '@repost/agent-terminal-linux-x64-musl',
    version: '0.1.0',
    description: 'The agent-terminal binary for linux x64 (musl)',
    license: 'Apache-2.0',
    repository: { type: 'git', url: 'git+https://github.com/repost-sh/agent-terminal.git' },
    publishConfig: { access: 'public' },
    os: ['linux'],
    cpu: ['x64'],
    libc: ['musl'],
    files: ['bin/agent-terminal', 'licenses'],
  });
  const binary = path.join(musl, 'bin', 'agent-terminal');
  assert.equal(fs.readFileSync(binary, 'utf8'), '#!/bin/sh\necho x86_64-unknown-linux-musl\n');
  assert.equal(fs.statSync(binary).mode & 0o777, 0o755);

  const mac = readJson(path.join(out, '@repost', 'agent-terminal-darwin-arm64', 'package.json'));
  assert.deepEqual([mac.os, mac.cpu, mac.libc], [['darwin'], ['arm64'], undefined]);
  const glibc = readJson(path.join(out, '@repost', 'agent-terminal-linux-arm64', 'package.json'));
  assert.deepEqual([glibc.os, glibc.cpu, glibc.libc], [['linux'], ['arm64'], ['glibc']]);
});

test('every platform package publishes the licences of the fonts and OpenH264 built into its binary', (t) => {
  const dir = tempDir(t);
  distRelease(dir, TARGETS, printTarget);

  const run = build(dir);
  assert.equal(run.status, 0, run.stderr);

  const scope = path.join(dir, 'npm', '@repost');
  const platformPackages = fs.readdirSync(scope).filter((entry) => entry !== 'agent-terminal');
  assert.equal(platformPackages.length, TARGETS.length);
  for (const name of platformPackages) {
    assert.deepEqual(packedFiles(path.join(scope, name)), [
      'LICENSE',
      'bin/agent-terminal',
      'licenses/LICENSE-OpenH264.txt',
      'licenses/LICENSE-SymbolsNerdFont.txt',
      'licenses/OFL-JetBrainsMono.txt',
      'licenses/OFL-NotoEmoji.txt',
      'licenses/README.md',
      'package.json',
    ]);
  }
  const openh264 = path.join(scope, 'agent-terminal-linux-x64', 'licenses', 'LICENSE-OpenH264.txt');
  assert.match(fs.readFileSync(openh264, 'utf8'), /^Copyright \(c\) 2013, Cisco Systems/);
});

test('the main package @repost/agent-terminal exposes agent-terminal and pins every platform package to the release', (t) => {
  const dir = tempDir(t);
  distRelease(dir, TARGETS, printTarget);

  const run = build(dir);
  assert.equal(run.status, 0, run.stderr);

  const main = path.join(dir, 'npm', '@repost', 'agent-terminal');
  const manifest = readJson(path.join(main, 'package.json'));
  assert.equal(manifest.name, '@repost/agent-terminal');
  assert.equal(manifest.version, '0.1.0');
  assert.deepEqual(manifest.bin, { 'agent-terminal': 'bin/agent-terminal.js' });
  assert.deepEqual(manifest.optionalDependencies, {
    '@repost/agent-terminal-darwin-arm64': '0.1.0',
    '@repost/agent-terminal-darwin-x64': '0.1.0',
    '@repost/agent-terminal-linux-arm64': '0.1.0',
    '@repost/agent-terminal-linux-x64': '0.1.0',
    '@repost/agent-terminal-linux-arm64-musl': '0.1.0',
    '@repost/agent-terminal-linux-x64-musl': '0.1.0',
  });
  assert.ok(fs.existsSync(path.join(main, 'bin', 'agent-terminal.js')));
  assert.ok(fs.existsSync(path.join(main, 'README.md')));
  assert.ok(fs.existsSync(path.join(main, 'LICENSE')));
});

test('every package publishes publicly under @repost with the repository npm provenance checks', (t) => {
  const dir = tempDir(t);
  distRelease(dir, TARGETS, printTarget);

  const run = build(dir);
  assert.equal(run.status, 0, run.stderr);

  const scope = path.join(dir, 'npm', '@repost');
  const packages = fs.readdirSync(scope);
  assert.equal(packages.length, TARGETS.length + 1);
  for (const name of packages) {
    const manifest = readJson(path.join(scope, name, 'package.json'));
    assert.equal(manifest.name, `@repost/${name}`);
    assert.deepEqual(manifest.publishConfig, { access: 'public' }, manifest.name);
    assert.deepEqual(
      manifest.repository,
      { type: 'git', url: 'git+https://github.com/repost-sh/agent-terminal.git' },
      manifest.name,
    );
  }
});

test('a release missing a platform is refused, naming the platform', (t) => {
  const dir = tempDir(t);
  distRelease(
    dir,
    TARGETS.filter((target) => target !== 'aarch64-unknown-linux-musl'),
    printTarget,
  );

  const run = build(dir);
  assert.equal(run.status, 1);
  assert.match(run.stderr, /no agent-terminal archive for aarch64-unknown-linux-musl/);
  assert.equal(fs.existsSync(path.join(dir, 'npm')), false);
});

test('a release with a platform no npm package carries is refused, naming the platform', (t) => {
  const dir = tempDir(t);
  distRelease(dir, [...TARGETS, 'riscv64gc-unknown-linux-gnu'], printTarget);

  const run = build(dir);
  assert.equal(run.status, 1);
  assert.match(run.stderr, /dist builds riscv64gc-unknown-linux-gnu, which has no entry in npm\/agent-terminal\/lib\/platform\.js/);
  assert.equal(fs.existsSync(path.join(dir, 'npm')), false);
});
