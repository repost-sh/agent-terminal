const PLATFORMS = [
  { target: 'aarch64-apple-darwin', os: 'darwin', cpu: 'arm64', libc: null },
  { target: 'x86_64-apple-darwin', os: 'darwin', cpu: 'x64', libc: null },
  { target: 'aarch64-unknown-linux-gnu', os: 'linux', cpu: 'arm64', libc: 'glibc' },
  { target: 'x86_64-unknown-linux-gnu', os: 'linux', cpu: 'x64', libc: 'glibc' },
  { target: 'aarch64-unknown-linux-musl', os: 'linux', cpu: 'arm64', libc: 'musl' },
  { target: 'x86_64-unknown-linux-musl', os: 'linux', cpu: 'x64', libc: 'musl' },
];

function packageNameFor(mainName, platform) {
  const suffix = platform.libc === 'musl' ? '-musl' : '';
  return `${mainName}-${platform.os}-${platform.cpu}${suffix}`;
}

function platformPackageName(mainName, host) {
  const platform = PLATFORMS.find(
    (candidate) => candidate.os === host.platform && candidate.cpu === host.arch && candidate.libc === host.libc,
  );
  return platform ? packageNameFor(mainName, platform) : null;
}

function hostPlatform() {
  if (process.platform !== 'linux') {
    return { platform: process.platform, arch: process.arch, libc: null };
  }
  process.report.excludeNetwork = true;
  const { header } = process.report.getReport();
  const libc = header.glibcVersionRuntime ? 'glibc' : 'musl';
  return { platform: process.platform, arch: process.arch, libc };
}

module.exports = { PLATFORMS, packageNameFor, platformPackageName, hostPlatform };
