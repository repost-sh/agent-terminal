#!/usr/bin/env node
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { PLATFORMS, packageNameFor } = require('./agent-terminal/lib/platform');

const ROOT = path.resolve(__dirname, '..');
const TEMPLATE = path.join(__dirname, 'agent-terminal');
const BINARY = 'agent-terminal';
const USAGE = 'usage: build-packages.js --manifest <dist-manifest.json> --artifacts <dir> --out <dir>';

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i];
    const value = argv[i + 1];
    if (!['--manifest', '--artifacts', '--out'].includes(flag) || value === undefined) {
      throw new Error(USAGE);
    }
    args[flag.slice(2)] = value;
  }
  if (!args.manifest || !args.artifacts || !args.out) {
    throw new Error(USAGE);
  }
  return args;
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function writeJson(file, value) {
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function releaseVersion(manifest) {
  const release = manifest.releases.find((candidate) => candidate.app_name === BINARY);
  if (!release) {
    throw new Error(`the dist manifest has no ${BINARY} release`);
  }
  return release.app_version;
}

function archiveFor(manifest, artifactsDir, target) {
  for (const artifact of Object.values(manifest.artifacts)) {
    if (artifact.kind !== 'executable-zip' || !artifact.target_triples.includes(target)) {
      continue;
    }
    const executable = artifact.assets.find((asset) => asset.kind === 'executable' && asset.name === BINARY);
    if (!executable) {
      continue;
    }
    const archive = path.join(artifactsDir, artifact.name);
    if (!fs.existsSync(archive)) {
      throw new Error(`${archive} is listed in the dist manifest but is not in ${artifactsDir}`);
    }
    return { archive, executable: executable.path };
  }
  throw new Error(`the dist manifest has no ${BINARY} archive for ${target}`);
}

function extractBinary({ archive, executable }, destination) {
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), `${BINARY}-`));
  try {
    const tar = spawnSync('tar', ['-xf', archive, '-C', scratch, '--strip-components=1'], { encoding: 'utf8' });
    if (tar.status !== 0) {
      throw new Error(`could not extract ${archive}: ${tar.stderr || tar.error}`);
    }
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.copyFileSync(path.join(scratch, executable), destination);
    fs.chmodSync(destination, 0o755);
  } finally {
    fs.rmSync(scratch, { recursive: true, force: true });
  }
}

function platformManifest(main, platform, version) {
  const variant = platform.libc === 'musl' ? ' (musl)' : platform.libc === 'glibc' ? ' (glibc)' : '';
  const manifest = {
    name: packageNameFor(main.name, platform),
    version,
    description: `The ${BINARY} binary for ${platform.os} ${platform.cpu}${variant}`,
    license: main.license,
    repository: main.repository,
    os: [platform.os],
    cpu: [platform.cpu],
  };
  if (platform.libc) {
    manifest.libc = [platform.libc];
  }
  manifest.files = [`bin/${BINARY}`, 'licenses'];
  return manifest;
}

function build({ manifest: manifestPath, artifacts, out }) {
  const dist = readJson(manifestPath);
  const version = releaseVersion(dist);
  const archives = PLATFORMS.map((platform) => archiveFor(dist, artifacts, platform.target));
  const main = readJson(path.join(TEMPLATE, 'package.json'));

  fs.mkdirSync(out, { recursive: true });
  PLATFORMS.forEach((platform, index) => {
    const manifest = platformManifest(main, platform, version);
    const directory = path.join(out, manifest.name);
    fs.mkdirSync(directory, { recursive: true });
    writeJson(path.join(directory, 'package.json'), manifest);
    fs.copyFileSync(path.join(ROOT, 'LICENSE'), path.join(directory, 'LICENSE'));
    fs.cpSync(path.join(ROOT, 'licenses'), path.join(directory, 'licenses'), { recursive: true });
    extractBinary(archives[index], path.join(directory, 'bin', BINARY));
  });

  const directory = path.join(out, main.name);
  fs.cpSync(TEMPLATE, directory, { recursive: true });
  const optionalDependencies = Object.fromEntries(
    PLATFORMS.map((platform) => [packageNameFor(main.name, platform), version]),
  );
  writeJson(path.join(directory, 'package.json'), { name: main.name, version, ...main, optionalDependencies });
  fs.copyFileSync(path.join(ROOT, 'README.md'), path.join(directory, 'README.md'));
  fs.copyFileSync(path.join(ROOT, 'LICENSE'), path.join(directory, 'LICENSE'));
}

try {
  build(parseArgs(process.argv.slice(2)));
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
