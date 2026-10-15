const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { TARGETS, tempDir, distRelease, build } = require('./support');

const echoArgs = () => "#!/bin/sh\nprintf '%s|' \"$@\"\ncat\nexit 5\n";

function installed(t, packages) {
  const dir = tempDir(t);
  distRelease(dir, TARGETS, echoArgs);
  const run = build(dir);
  assert.equal(run.status, 0, run.stderr);
  const scope = path.join(dir, 'npm', '@repost');
  const installedScope = path.join(dir, 'prefix', 'node_modules', '@repost');
  for (const name of fs.readdirSync(scope).filter((name) => packages(`@repost/${name}`))) {
    fs.cpSync(path.join(scope, name), path.join(installedScope, name), { recursive: true });
  }
  return path.join(installedScope, 'agent-terminal', 'bin', 'agent-terminal.js');
}

function launch(launcher, args, input) {
  return spawnSync(process.execPath, [launcher, ...args], { encoding: 'utf8', input });
}

test('the launcher runs the platform binary with the same arguments, input and exit code', (t) => {
  const launcher = installed(t, () => true);

  const run = launch(launcher, ['snapshot', '-i', 'two words'], 'piped input');

  assert.equal(run.stdout, 'snapshot|-i|two words|piped input');
  assert.equal(run.stderr, '');
  assert.equal(run.status, 5);
});

test('without the platform package the launcher says which package is missing and how to get it', (t) => {
  const launcher = installed(t, (name) => name === '@repost/agent-terminal');

  const run = launch(launcher, ['snapshot'], '');

  assert.equal(run.status, 1);
  assert.equal(run.stdout, '');
  assert.match(
    run.stderr,
    /^@repost\/agent-terminal-(darwin|linux)-(arm64|x64)(-musl)? is not installed\. It holds the agent-terminal binary for this platform; reinstall with optional dependencies: npm i -g @repost\/agent-terminal\n$/,
  );
});
