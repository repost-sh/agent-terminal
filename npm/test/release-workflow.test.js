const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const WORKFLOWS = path.resolve(__dirname, '..', '..', '.github', 'workflows');

function workflow(name) {
  return fs.readFileSync(path.join(WORKFLOWS, name), 'utf8');
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
