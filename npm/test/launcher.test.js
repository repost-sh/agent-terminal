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
  const built = path.join(dir, 'npm');
  const prefix = path.join(dir, 'prefix');
  for (const name of fs.readdirSync(built).filter(packages)) {
    fs.cpSync(path.join(built, name), path.join(prefix, 'node_modules', name), { recursive: true });
  }
  return path.join(prefix, 'node_modules', 'agent-terminal', 'bin', 'agent-terminal.js');
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
  const launcher = installed(t, (name) => name === 'agent-terminal');

  const run = launch(launcher, ['snapshot'], '');

  assert.equal(run.status, 1);
  assert.equal(run.stdout, '');
  assert.match(
    run.stderr,
    /^agent-terminal-(darwin|linux)-(arm64|x64)(-musl)? is not installed\. It holds the agent-terminal binary for this platform; reinstall with optional dependencies: npm i -g agent-terminal\n$/,
  );
});
