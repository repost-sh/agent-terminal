#!/usr/bin/env node
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { name } = require('../package.json');
const { platformPackageName, hostPlatform } = require('../lib/platform');

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
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
        `reinstall with optional dependencies: npm i -g ${name}`,
    );
  }
}

const binary = binaryPath();
const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
if (result.error) {
  fail(`could not run ${binary}: ${result.error.message}`);
}
if (result.signal) {
  process.kill(process.pid, result.signal);
}
process.exit(result.status);
