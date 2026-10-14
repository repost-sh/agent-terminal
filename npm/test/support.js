const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const BUILD = path.resolve(__dirname, '..', 'build-packages.js');

const TARGETS = [
  'aarch64-apple-darwin',
  'x86_64-apple-darwin',
  'aarch64-unknown-linux-gnu',
  'x86_64-unknown-linux-gnu',
  'aarch64-unknown-linux-musl',
  'x86_64-unknown-linux-musl',
];

function tempDir(t) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'at-npm-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  return dir;
}

function distRelease(dir, targets, script) {
  const artifacts = {};
  for (const target of targets) {
    const name = `agent-terminal-${target}.tar.xz`;
    const root = path.join(dir, 'staging', `agent-terminal-${target}`);
    fs.mkdirSync(root, { recursive: true });
    fs.writeFileSync(path.join(root, 'agent-terminal'), script(target), { mode: 0o755 });
    fs.writeFileSync(path.join(root, 'README.md'), 'readme\n');
    const tar = spawnSync('tar', ['-cJf', path.join(dir, name), '-C', path.dirname(root), path.basename(root)]);
    assert.equal(tar.status, 0, String(tar.stderr));
    artifacts[name] = {
      name,
      kind: 'executable-zip',
      target_triples: [target],
      assets: [
        { name: 'README.md', path: 'README.md', kind: 'readme' },
        { name: 'agent-terminal', path: 'agent-terminal', kind: 'executable' },
      ],
    };
  }
  const manifest = {
    releases: [{ app_name: 'agent-terminal', app_version: '0.1.0', artifacts: Object.keys(artifacts) }],
    artifacts,
  };
  fs.writeFileSync(path.join(dir, 'dist-manifest.json'), JSON.stringify(manifest));
}

function build(dir) {
  return spawnSync(
    process.execPath,
    [BUILD, '--manifest', path.join(dir, 'dist-manifest.json'), '--artifacts', dir, '--out', path.join(dir, 'npm')],
    { encoding: 'utf8' },
  );
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

module.exports = { TARGETS, tempDir, distRelease, build, readJson };
