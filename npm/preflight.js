#!/usr/bin/env node
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { writePackagesWithoutBinaries } = require('./build-packages');
const { PLATFORMS } = require('./agent-terminal/lib/platform');

const CRATE = 'cli/Cargo.toml';
const CHANGELOG = 'CHANGELOG.md';
const USAGE = 'usage: preflight.js --tag <tag>, run from the repository root';

function parseArgs(argv) {
  if (argv.length !== 2 || argv[0] !== '--tag' || !argv[1]) {
    throw new Error(USAGE);
  }
  return { tag: argv[1] };
}

function crateVersion() {
  if (!fs.existsSync(CRATE)) {
    throw new Error(`${CRATE} not found: ${USAGE}`);
  }
  const lines = fs.readFileSync(CRATE, 'utf8').split('\n');
  const start = lines.indexOf('[package]');
  if (start === -1) {
    throw new Error(`${CRATE} has no [package] table`);
  }
  for (const line of lines.slice(start + 1)) {
    if (line.startsWith('[')) {
      break;
    }
    const version = line.match(/^version\s*=\s*"([^"]+)"/);
    if (version) {
      return version[1];
    }
  }
  throw new Error(`${CRATE} has no version in its [package] table`);
}

function checkTag(tag, version) {
  if (tag === `v${version}`) {
    return [];
  }
  return [`the tag ${tag} does not match version ${version} in ${CRATE}: tag v${version} instead, or bump the version`];
}

function changelogSections() {
  const lines = fs.existsSync(CHANGELOG) ? fs.readFileSync(CHANGELOG, 'utf8').split('\n') : [];
  const sections = [];
  for (const line of lines) {
    if (line.startsWith('## ')) {
      sections.push({ heading: line, entries: [] });
    } else if (sections.length > 0 && line.trim() !== '') {
      sections[sections.length - 1].entries.push(line);
    }
  }
  return sections;
}

function checkChangelog(version) {
  const heading = `## ${version}`;
  const prerelease = version.includes('-');
  const sections = changelogSections();
  const own = sections.some((section) => section.heading === heading || section.heading.startsWith(`${heading} `));
  const unreleased = sections.some((section) => section.heading === '## Unreleased' && section.entries.length > 0);
  if (own || (prerelease && unreleased)) {
    return [];
  }
  const alternative = prerelease ? ', or entries under "## Unreleased"' : '';
  return [`${CHANGELOG} has no section for ${version}: add "${heading} (<date>)" as docs/releasing.md describes${alternative}`];
}

function npmErrors(stderr) {
  return stderr
    .split('\n')
    .filter((line) => line.startsWith('npm error ') && !line.startsWith('npm error A complete log of this run'))
    .map((line) => line.slice('npm error '.length))
    .join(' ');
}

function dryRun(directory) {
  const { name, version } = JSON.parse(fs.readFileSync(path.join(directory, 'package.json'), 'utf8'));
  const distTag = version.includes('-') ? ['--tag', 'next'] : [];
  const publish = spawnSync('npm', ['publish', '--dry-run', ...distTag, directory], { encoding: 'utf8' });
  if (publish.error) {
    throw new Error(`could not run npm: ${publish.error.message}`);
  }
  if (publish.status !== 0) {
    const reason = npmErrors(publish.stderr) || publish.stderr.trim() || `npm exited with status ${publish.status}`;
    return [`npm publish --dry-run refused ${name}@${version}: ${reason}`];
  }
  console.log(`npm publish --dry-run accepts ${name}@${version}`);
  return [];
}

function checkPackages(version) {
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'agent-terminal-preflight-'));
  try {
    const { platforms, main } = writePackagesWithoutBinaries(version, scratch);
    return [...platforms, main].flatMap(dryRun);
  } finally {
    fs.rmSync(scratch, { recursive: true, force: true });
  }
}

function preflight({ tag }) {
  const version = crateVersion();
  const problems = [...checkTag(tag, version), ...checkChangelog(version), ...checkPackages(version)];
  for (const problem of problems) {
    console.error(problem);
  }
  if (problems.length === 0) {
    console.log(
      `${tag} can be released: it matches ${CRATE}, ${CHANGELOG} has its section, ` +
        `npm accepts all ${PLATFORMS.length + 1} packages`,
    );
  }
  return problems.length === 0;
}

try {
  process.exit(preflight(parseArgs(process.argv.slice(2))) ? 0 : 1);
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
