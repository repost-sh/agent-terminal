#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { name } = require('../package.json');
const { platformPackageName, hostPlatform } = require('../lib/platform');

const INSTALLERS = [
  { layout: /\/node_modules\/\.pnpm\/|\/v\d+\/links\//, reinstall: () => `pnpm add -g ${name}` },
  {
    layout: /\/yarn\/global\/node_modules\//,
    reinstall: () => `yarn global remove ${name} && yarn global add ${name}`,
  },
  { layout: /\/install\/global\/node_modules\//, reinstall: () => `bun remove -g ${name} && bun add -g ${name}` },
];

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}

function reinstallCommand() {
  const installer = INSTALLERS.find(({ layout }) => layout.test(__dirname));
  return installer ? installer.reinstall() : `npm i -g ${name}`;
}

function shellQuoted(text) {
  return `'${text.replaceAll("'", "'\\''")}'`;
}

function binaryPath() {
  const host = hostPlatform();
  const packageName = platformPackageName(name, host);
  if (!packageName) {
    fail(
      `agent-terminal has no prebuilt binary for ${host.platform} ${host.arch}. ` +
        'Build it from source: cargo install --locked --git https://github.com/repost-sh/agent-terminal agent-terminal',
    );
  }
  try {
    return path.join(path.dirname(require.resolve(`${packageName}/package.json`)), 'bin', 'agent-terminal');
  } catch {
    return fail(
      `${packageName} is not installed. It holds the agent-terminal binary for this platform; ` +
        `reinstall with optional dependencies: ${reinstallCommand()}`,
    );
  }
}

function run(binary) {
  return spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
}

function runRestoringExecBit(binary) {
  try {
    fs.chmodSync(binary, 0o755);
  } catch (error) {
    fail(
      `could not run ${binary}: it is not executable, and agent-terminal could not make it executable ` +
        `(${error.message}). Make it executable as the user who owns it, then run agent-terminal again: ` +
        `chmod +x ${shellQuoted(binary)}`,
    );
  }
  const result = run(binary);
  if (result.error && result.error.code === 'EACCES') {
    fail(
      `could not run ${binary}: ${result.error.message}, even after agent-terminal set its exec bit. ` +
        'Check that its file system allows running programs (it may be mounted noexec), ' +
        `then reinstall: ${reinstallCommand()}`,
    );
  }
  return result;
}

const binary = binaryPath();
let result = run(binary);
if (result.error && result.error.code === 'EACCES') {
  result = runRestoringExecBit(binary);
}
if (result.error) {
  fail(`could not run ${binary}: ${result.error.message}`);
}
if (result.signal) {
  process.kill(process.pid, result.signal);
}
process.exit(result.status);
