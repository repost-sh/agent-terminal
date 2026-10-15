const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const { TARGETS } = require('./support');

const ROOT = path.resolve(__dirname, '..', '..');
const WORKFLOWS = path.join(ROOT, '.github', 'workflows');

function workflow(name) {
  return fs.readFileSync(path.join(WORKFLOWS, name), 'utf8');
}

function rootFile(name) {
  return fs.readFileSync(path.join(ROOT, name), 'utf8');
}

function buildImages() {
  return Object.fromEntries(
    [
      ...rootFile('dist-workspace.toml').matchAll(
        /^\[dist\.github-custom-runners\.(\S+)\]\n(?:[^[\n].*\n)*?container = \{ image = "([^"]+)"/gm,
      ),
    ].map((match) => [match[1], match[2]]),
  );
}

function jobNeeds(source) {
  const jobs = source.slice(source.indexOf('\njobs:\n'));
  return Object.fromEntries(
    jobs
      .split(/^ {2}(?=[a-z][\w-]*:$)/m)
      .slice(1)
      .map((block) => {
        const [header, ...body] = block.split('\n');
        const needs = body.join('\n').match(/^ {4}needs:\n((?: {6}- .+\n)+)/m);
        return [header.slice(0, -1), needs ? [...needs[1].matchAll(/- (\S+)/g)].map((match) => match[1]) : []];
      }),
  );
}

test('the release creates and publishes nothing until the preflight passes', () => {
  const release = workflow('release.yml');
  const needs = jobNeeds(release);

  assert.match(release, /^ {2}custom-preflight:\n(?: {4}.*\n)* {4}uses: \.\/\.github\/workflows\/preflight\.yml$/m);
  assert.ok(needs.host.includes('custom-preflight'), `host needs ${needs.host}`);
  assert.match(release, /needs\.custom-preflight\.result == 'success'/);
  assert.ok(needs['publish-homebrew-formula'].includes('host'));
  assert.ok(needs['custom-publish-npm'].includes('host'));
  assert.match(workflow('preflight.yml'), /run: node npm\/preflight\.js --tag "\$TAG"$/m);
});

test('the release archives are attested', () => {
  assert.match(workflow('release.yml'), /^ {8}uses: actions\/attest@v\d+$/m);
});

test('npm publishing uses trusted publishing with provenance and no token', () => {
  const publish = workflow('publish-npm.yml');

  assert.doesNotMatch(publish, /NPM_TOKEN|NODE_AUTH_TOKEN|secrets\./);
  assert.match(publish, /^ {6}id-token: write$/m);
  assert.match(publish, /npm publish "\$1" --access public --provenance$/m);
});

test('every Linux target builds in an image pinned by digest, the gnu targets in a glibc 2.28 image', () => {
  const images = buildImages();
  for (const target of TARGETS.filter((candidate) => candidate.includes('-linux-'))) {
    assert.match(images[target] ?? 'no image', /@sha256:[0-9a-f]{64}$/, `${target} builds in ${images[target]}`);
  }
  assert.match(images['x86_64-unknown-linux-gnu'], /^quay\.io\/pypa\/manylinux_2_28_x86_64:/);
  assert.match(images['aarch64-unknown-linux-gnu'], /^quay\.io\/pypa\/manylinux_2_28_aarch64:/);
});

test('before a release, each gnu binary passes the glibc floor check and runs on Debian 11, Ubuntu 20.04 and Rocky Linux 8', () => {
  const packages = workflow('npm-packages.yml');
  const jobs = packages
    .slice(packages.indexOf('\njobs:\n'))
    .split(/^ {2}(?=[a-z][\w-]*:$)/m)
    .slice(1);
  const gnu = TARGETS.filter((target) => target.endsWith('-linux-gnu'));

  const floor = jobs.find((job) => /^ +sh scripts\/glibc-floor\.sh /m.test(job));
  assert.ok(floor, 'npm-packages.yml has no job that runs scripts/glibc-floor.sh');
  for (const target of gnu) {
    assert.match(floor, new RegExp(`^ +(?:- )?target: ${target}$`, 'm'), `the floor check skips ${target}`);
  }

  const glibc = jobs.find((job) => /image: debian:11$/m.test(job));
  assert.ok(glibc, 'npm-packages.yml has no job that runs on debian:11');
  for (const image of ['debian:11', 'ubuntu:20.04', 'rockylinux/rockylinux:8']) {
    assert.match(glibc, new RegExp(`^ +image: ${image.replace(/[./]/g, '\\$&')}$`, 'm'));
  }
  for (const target of gnu) {
    assert.match(glibc, new RegExp(`^ +(?:- )?target: ${target}$`, 'm'), `the glibc job skips ${target}`);
  }
  assert.match(glibc, /docker exec clean sh npm\/install-test\/check\.sh /);

  const release = workflow('release.yml');
  assert.match(release, /^ {2}custom-npm-packages:\n(?: {4}.*\n)*? {4}uses: \.\/\.github\/workflows\/npm-packages\.yml$/m);
  assert.ok(jobNeeds(release).host.includes('custom-npm-packages'));
  assert.match(workflow('clean-install.yml'), /^ {4}uses: \.\/\.github\/workflows\/npm-packages\.yml$/m);
});

test('the release builds with a pinned Rust toolchain and a pinned dist', () => {
  assert.match(rootFile('rust-toolchain.toml'), /^channel = "\d+\.\d+\.\d+"$/m);
  assert.match(rootFile('dist-workspace.toml'), /^cargo-dist-version = "\d+\.\d+\.\d+"$/m);
});
