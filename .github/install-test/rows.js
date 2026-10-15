const { PLATFORMS, packageNameFor } = require('../../npm/agent-terminal/lib/platform');

const RUNNERS = {
  'darwin-arm64': 'macos-14',
  'darwin-x64': 'macos-15-intel',
  'linux-arm64': 'ubuntu-24.04-arm',
  'linux-x64': 'ubuntu-24.04',
};
const IMAGES = { glibc: 'node:24', musl: 'node:24-alpine' };
const CHANNELS = ['npm', 'pnpm', 'bun', 'archive'];
const LINUX_ARCH = { arm64: 'aarch64', x64: 'x86_64' };

function row(platform) {
  const runner = RUNNERS[`${platform.os}-${platform.cpu}`];
  if (!runner) {
    throw new Error(`no runner for ${platform.target}: add ${platform.os}-${platform.cpu} to RUNNERS in ${__filename}`);
  }
  const linux = platform.os === 'linux';
  return {
    target: platform.target,
    runner,
    image: linux ? IMAGES[platform.libc] : '',
    package: packageNameFor('@repost/agent-terminal', platform),
    fixture: linux ? `${LINUX_ARCH[platform.cpu]}-unknown-linux-musl` : platform.target,
  };
}

const rows = PLATFORMS.map(row);
const install = rows.flatMap((platform) => CHANNELS.map((channel) => ({ ...platform, channel })));
const fixtures = [...new Map(rows.map(({ fixture, runner }) => [fixture, { target: fixture, runner }])).values()];

console.log(`install=${JSON.stringify({ include: install })}`);
console.log(`fixtures=${JSON.stringify({ include: fixtures })}`);
