const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { TARGETS, tempDir, distRelease, build } = require('./support');
const { platformPackageName, hostPlatform } = require('../agent-terminal/lib/platform');

const echoArgs = () => "#!/bin/sh\nprintf '%s|' \"$@\"\ncat\nexit 5\n";
const HOST_PACKAGE = platformPackageName('@repost/agent-terminal', hostPlatform());

function installed(t, packages, layout = path.join('prefix', 'node_modules')) {
  const dir = tempDir(t);
  distRelease(dir, TARGETS, echoArgs);
  const run = build(dir);
  assert.equal(run.status, 0, run.stderr);
  const scope = path.join(dir, 'npm', '@repost');
  const modules = path.join(dir, layout);
  for (const name of fs.readdirSync(scope).filter((name) => packages(`@repost/${name}`))) {
    fs.cpSync(path.join(scope, name), path.join(modules, '@repost', name), { recursive: true });
  }
  return {
    launcher: path.join(modules, '@repost', 'agent-terminal', 'bin', 'agent-terminal.js'),
    binary: path.join(modules, HOST_PACKAGE, 'bin', 'agent-terminal'),
  };
}

function launch(launcher, args, input) {
  return spawnSync(process.execPath, [launcher, ...args], { encoding: 'utf8', input });
}

test('the launcher runs the platform binary with the same arguments, input and exit code', (t) => {
  const { launcher } = installed(t, () => true);

  const run = launch(launcher, ['snapshot', '-i', 'two words'], 'piped input');

  assert.equal(run.stdout, 'snapshot|-i|two words|piped input');
  assert.equal(run.stderr, '');
  assert.equal(run.status, 5);
});

test('a platform binary that lost its exec bit gets it back and runs', (t) => {
  const { launcher, binary } = installed(t, () => true);
  fs.chmodSync(binary, 0o644);

  const run = launch(launcher, ['snapshot', '-i', 'two words'], 'piped input');

  assert.equal(run.stdout, 'snapshot|-i|two words|piped input');
  assert.equal(run.stderr, '');
  assert.equal(run.status, 5);
  assert.equal(fs.statSync(binary).mode & 0o777, 0o755);
});

test('a platform binary that cannot be made executable gets the chmod command to run', (t) => {
  const { launcher, binary } = installed(t, () => true);
  fs.chmodSync(binary, 0o644);
  fs.chmodSync(path.dirname(binary), 0o644);

  const run = launch(launcher, ['snapshot'], '');
  fs.chmodSync(path.dirname(binary), 0o755);

  assert.equal(run.status, 1);
  assert.equal(run.stdout, '');
  assert.equal(
    run.stderr,
    `could not run ${binary}: it is not executable, and agent-terminal could not make it executable ` +
      `(EACCES: permission denied, chmod '${binary}'). ` +
      `Make it executable as the user who owns it, then run agent-terminal again: chmod +x '${binary}'\n`,
  );
});

test('a platform binary that still cannot run after the repair is reported with the reinstall command', (t) => {
  const { launcher, binary } = installed(t, () => true);
  fs.rmSync(binary);
  fs.mkdirSync(binary, { mode: 0o644 });

  const run = launch(launcher, ['snapshot'], '');

  assert.equal(run.status, 1);
  assert.equal(run.stdout, '');
  assert.equal(
    run.stderr,
    `could not run ${binary}: spawnSync ${binary} EACCES, even after agent-terminal set its exec bit. ` +
      'Check that its file system allows running programs (it may be mounted noexec), ' +
      'then reinstall: npm i -g @repost/agent-terminal\n',
  );
});

const INSTALLERS = [
  ['npm', path.join('prefix', 'lib', 'node_modules'), 'npm i -g @repost/agent-terminal'],
  ['an unknown installer', path.join('somewhere', 'node_modules'), 'npm i -g @repost/agent-terminal'],
  [
    'pnpm',
    path.join('pnpm', 'store', 'v11', 'links', '@repost', 'agent-terminal', '0.1.0', '3f9c', 'node_modules'),
    'pnpm add -g @repost/agent-terminal',
  ],
  [
    'pnpm with a .pnpm store',
    path.join('pnpm', 'global', '5', 'node_modules', '.pnpm', '@repost+agent-terminal@0.1.0', 'node_modules'),
    'pnpm add -g @repost/agent-terminal',
  ],
  [
    'yarn',
    path.join('.config', 'yarn', 'global', 'node_modules'),
    'yarn global remove @repost/agent-terminal && yarn global add @repost/agent-terminal',
  ],
  [
    'bun',
    path.join('.bun', 'install', 'global', 'node_modules'),
    'bun remove -g @repost/agent-terminal && bun add -g @repost/agent-terminal',
  ],
];

for (const [installer, layout, command] of INSTALLERS) {
  test(`without the platform package, an install by ${installer} names the package and ${command}`, (t) => {
    const { launcher } = installed(t, (name) => name === '@repost/agent-terminal', layout);

    const run = launch(launcher, ['snapshot'], '');

    assert.equal(run.status, 1);
    assert.equal(run.stdout, '');
    assert.equal(
      run.stderr,
      `${HOST_PACKAGE} is not installed. It holds the agent-terminal binary for this platform; ` +
        `reinstall with optional dependencies: ${command}\n`,
    );
  });
}
