const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { PLATFORMS, packageNameFor } = require('../agent-terminal/lib/platform');
const { tempDir } = require('./support');

const SCRIPT = path.resolve(__dirname, '..', '..', 'scripts', 'one-time-setup.sh');
const MAIN = '@repost/agent-terminal';
const PACKAGES = [MAIN, ...PLATFORMS.map((platform) => packageNameFor(MAIN, platform))];
const OIDC_PUBLISHER = 'GitHub Actions <npm-oidc-no-reply@github.com>';
const OWNER_PUBLISHER = 'owner <owner@example.com>';
const TAP_TOKEN = 'github_pat_valid';
const NPM_TOKEN = 'npm_valid';
const HOUR = 60 * 60;

const GH_STUB = `
const fs = require('node:fs');
const path = require('node:path');
const world = process.env.FAKE_WORLD;
const args = process.argv.slice(2);
const flag = (name) => (args.includes(name) ? args[args.indexOf(name) + 1] : null);
fs.appendFileSync(path.join(world, 'calls'), JSON.stringify({ tool: 'gh', args, token: process.env.GH_TOKEN || null }) + '\\n');
const [group, command] = args;
if (group === 'auth' && command === 'status') process.exit(0);
if (group === 'repo' && command === 'view') {
  if (args[2] === 'repost-sh/homebrew-tap' && fs.existsSync(path.join(world, 'tap'))) process.exit(0);
  console.error("GraphQL: Could not resolve to a Repository with the name '" + args[2] + "'. (repository)");
  process.exit(1);
}
if (group === 'repo' && command === 'create') {
  fs.writeFileSync(path.join(world, 'tap'), JSON.stringify(args));
  process.exit(0);
}
if (group === 'secret') {
  const repo = flag('-R') || process.env.GH_REPO;
  const dir = path.join(world, 'secrets', encodeURIComponent(repo));
  fs.mkdirSync(dir, { recursive: true });
  if (command === 'list') {
    for (const name of fs.readdirSync(dir)) console.log(name);
    process.exit(0);
  }
  if (command === 'set') {
    if (fs.existsSync(path.join(world, 'secret-set-refused'))) {
      console.error('HTTP 403: Resource not accessible by personal access token (https://api.github.com/repos/' + repo + '/actions/secrets/public-key)');
      process.exit(1);
    }
    fs.writeFileSync(path.join(dir, args[2]), fs.readFileSync(0, 'utf8'));
    process.exit(0);
  }
}
if (group === 'api' && args[1] === 'repos/repost-sh/homebrew-tap') {
  if (fs.existsSync(path.join(world, 'tap')) && process.env.GH_TOKEN === '${TAP_TOKEN}') process.exit(0);
  console.error('HTTP 401: Bad credentials (https://api.github.com/repos/repost-sh/homebrew-tap)');
  process.exit(1);
}
console.error('unexpected gh call: ' + args.join(' '));
process.exit(2);
`;

const NPM_STUB = `
const fs = require('node:fs');
const path = require('node:path');
const world = process.env.FAKE_WORLD;
const args = process.argv.slice(2);
fs.appendFileSync(path.join(world, 'calls'), JSON.stringify({ tool: 'npm', args }) + '\\n');
const userconfig = fs.readFileSync(process.env.npm_config_userconfig, 'utf8');
const registry = /^@repost:registry=https:\\/\\/registry\\.npmjs\\.org\\/$/m.test(userconfig);
const authed = userconfig.includes('//registry.npmjs.org/:_authToken=\${NPM_BOOTSTRAP_TOKEN}') && process.env.NPM_BOOTSTRAP_TOKEN === '${NPM_TOKEN}';
const record = (name) => path.join(world, 'npm', encodeURIComponent(name) + '.json');
if (!registry) {
  console.error('npm error code E404');
  process.exit(1);
}
if (args[0] === 'view' && args[2] === '_npmUser') {
  if (fs.existsSync(record(args[1]))) {
    console.log(JSON.parse(fs.readFileSync(record(args[1]), 'utf8')).npmUser);
    process.exit(0);
  }
  console.error('npm error code E404');
  process.exit(1);
}
if (args[0] === 'whoami') {
  if (authed) {
    console.log('owner');
    process.exit(0);
  }
  console.error('npm error code E401');
  process.exit(1);
}
if (args[0] === 'publish') {
  if (!authed) {
    console.error('npm error code ENEEDAUTH');
    process.exit(1);
  }
  const manifest = JSON.parse(fs.readFileSync(path.join(args[1], 'package.json'), 'utf8'));
  fs.mkdirSync(path.join(world, 'npm'), { recursive: true });
  fs.writeFileSync(record(manifest.name), JSON.stringify({ npmUser: '${OWNER_PUBLISHER}', manifest, flags: args.slice(2) }));
  process.exit(0);
}
console.error('unexpected npm call: ' + args.join(' '));
process.exit(2);
`;

const OPEN_STUB = `
const fs = require('node:fs');
const path = require('node:path');
fs.appendFileSync(path.join(process.env.FAKE_WORLD, 'calls'), JSON.stringify({ tool: 'open', args: process.argv.slice(2) }) + '\\n');
`;

function setup(t) {
  const dir = tempDir(t);
  const repo = path.join(dir, 'repo');
  const world = path.join(dir, 'world');
  const bin = path.join(dir, 'bin');
  fs.mkdirSync(path.join(repo, 'scripts'), { recursive: true });
  fs.copyFileSync(SCRIPT, path.join(repo, 'scripts', 'one-time-setup.sh'));
  fs.chmodSync(path.join(repo, 'scripts', 'one-time-setup.sh'), 0o755);
  fs.mkdirSync(world);
  fs.writeFileSync(path.join(world, 'calls'), '');
  fs.mkdirSync(bin);
  for (const [name, source] of [['gh', GH_STUB], ['npm', NPM_STUB], ['wslview', OPEN_STUB]]) {
    fs.writeFileSync(path.join(bin, name), `#!${process.execPath}\n${source}`, { mode: 0o755 });
  }
  return { repo, world, bin };
}

function run(env, args, answers) {
  const { GH_TOKEN, GITHUB_TOKEN, GH_REPO, ...inherited } = process.env;
  const before = calls(env).length;
  const result = spawnSync(path.join(env.repo, 'scripts', 'one-time-setup.sh'), args, {
    input: answers.map((answer) => `${answer}\n`).join(''),
    env: { ...inherited, PATH: `${env.bin}:${process.env.PATH}`, FAKE_WORLD: env.world },
    encoding: 'utf8',
  });
  return { ...result, calls: calls(env).slice(before) };
}

function calls(env) {
  return fs
    .readFileSync(path.join(env.world, 'calls'), 'utf8')
    .split('\n')
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

function opened(result) {
  return result.calls.filter((call) => call.tool === 'open').map((call) => call.args[0]);
}

function writes(result) {
  return result.calls.filter(
    (call) =>
      (call.tool === 'gh' && call.args[0] === 'repo' && call.args[1] === 'create') ||
      (call.tool === 'gh' && call.args[0] === 'secret' && call.args[1] === 'set') ||
      (call.tool === 'npm' && call.args[0] === 'publish'),
  );
}

function published(env) {
  const dir = path.join(env.world, 'npm');
  if (!fs.existsSync(dir)) return {};
  return Object.fromEntries(
    fs.readdirSync(dir).map((file) => {
      const entry = JSON.parse(fs.readFileSync(path.join(dir, file), 'utf8'));
      return [decodeURIComponent(file.slice(0, -'.json'.length)), entry];
    }),
  );
}

function onNpm(env, name, npmUser) {
  fs.mkdirSync(path.join(env.world, 'npm'), { recursive: true });
  fs.writeFileSync(path.join(env.world, 'npm', `${encodeURIComponent(name)}.json`), JSON.stringify({ npmUser }));
}

function tapReady(env) {
  fs.writeFileSync(path.join(env.world, 'tap'), '[]');
  const secrets = path.join(env.world, 'secrets', encodeURIComponent('repost-sh/agent-terminal'));
  fs.mkdirSync(secrets, { recursive: true });
  fs.writeFileSync(path.join(secrets, 'HOMEBREW_TAP_TOKEN'), 'earlier');
}

function previousRun(env, values) {
  const dir = path.join(env.repo, '.superpowers', 'one-time-setup');
  fs.mkdirSync(dir, { recursive: true });
  const lines = Object.entries(values).map(([key, value]) => `${key}='${value}'\n`);
  fs.writeFileSync(path.join(dir, 'state.env'), lines.join(''));
}

function trustedPublisherKey(name) {
  return `NPM_TRUSTED_PUBLISHER_${name.slice('@repost/'.length).toUpperCase().replaceAll('-', '_')}`;
}

function secondsAgo(seconds) {
  return Math.floor(Date.now() / 1000) - seconds;
}

function filesUnder(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(dir, entry.name);
    return entry.isDirectory() ? filesUnder(full) : [full];
  });
}

const accessPage = (name) => `https://www.npmjs.com/package/${name}/access`;
const TOKENS_PAGE = 'https://www.npmjs.com/settings/owner/tokens';

const FIRST_RUN = [
  '',
  'y',
  'github_pat_wrong',
  TAP_TOKEN,
  'owner',
  'npm_wrong',
  NPM_TOKEN,
  'y',
  'y',
  ...PACKAGES.map(() => 'y'),
];

test('a first run creates the tap, stores its token and sets up trusted publishing for the seven packages', (t) => {
  const env = setup(t);

  const result = run(env, [], FIRST_RUN);

  assert.equal(result.status, 0, result.stdout + result.stderr);
  const tap = JSON.parse(fs.readFileSync(path.join(env.world, 'tap'), 'utf8'));
  assert.deepEqual(tap.slice(0, 3), ['repo', 'create', 'repost-sh/homebrew-tap']);
  assert.ok(tap.includes('--public') && tap.includes('--add-readme'), tap.join(' '));
  const secret = path.join(env.world, 'secrets', encodeURIComponent('repost-sh/agent-terminal'), 'HOMEBREW_TAP_TOKEN');
  assert.equal(fs.readFileSync(secret, 'utf8'), TAP_TOKEN);
  assert.deepEqual(
    writes(result)
      .filter((call) => call.args[0] === 'secret')
      .map((call) => call.args),
    [['secret', 'set', 'HOMEBREW_TAP_TOKEN', '-R', 'repost-sh/agent-terminal']],
  );

  const packages = published(env);
  assert.deepEqual(Object.keys(packages).sort(), [...PACKAGES].sort());
  for (const name of PACKAGES) {
    const { manifest, flags } = packages[name];
    assert.equal(manifest.version, '0.0.0-bootstrap', name);
    assert.equal(manifest.repository.url, 'git+https://github.com/repost-sh/agent-terminal.git', name);
    assert.deepEqual(flags, ['--access', 'public', '--tag', 'bootstrap', '--loglevel', 'error'], name);
  }

  const urls = opened(result);
  assert.match(
    urls[0],
    /^https:\/\/github\.com\/settings\/personal-access-tokens\/new\?.*target_name=repost-sh&.*contents=write/,
  );
  assert.deepEqual(urls.slice(1), [TOKENS_PAGE, TOKENS_PAGE, ...PACKAGES.map(accessPage)]);
  for (const text of [result.stdout, result.stderr, ...filesUnder(env.repo).map((file) => fs.readFileSync(file, 'utf8'))]) {
    assert.ok(!text.includes(TAP_TOKEN) && !text.includes(NPM_TOKEN), 'a token was printed or written to the checkout');
  }

  const check = run(env, ['--check'], []);
  assert.equal(check.status, 0, check.stdout + check.stderr);
});

test('a secret gh refuses to set is left to do by hand, with what gh said', (t) => {
  const env = setup(t);
  fs.writeFileSync(path.join(env.world, 'tap'), '[]');
  fs.writeFileSync(path.join(env.world, 'secret-set-refused'), '');
  for (const name of PACKAGES) onNpm(env, name, OIDC_PUBLISHER);

  const result = run(env, [], ['', TAP_TOKEN]);

  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(
    result.stderr,
    /^HTTP 403: Resource not accessible by personal access token \(https:\/\/api\.github\.com\/repos\/repost-sh\/agent-terminal\/actions\/secrets\/public-key\)$/m,
  );
  assert.match(result.stdout, /gh couldn't set the GitHub secret HOMEBREW_TAP_TOKEN; what it said is above/);
  assert.match(result.stdout, /GitHub secret HOMEBREW_TAP_TOKEN \(set it by hand: gh secret set HOMEBREW_TAP_TOKEN -R repost-sh\/agent-terminal\)/);
  assert.ok(!result.stdout.includes(TAP_TOKEN) && !result.stderr.includes(TAP_TOKEN), 'the token was printed');
  assert.equal(fs.existsSync(path.join(env.world, 'secrets', encodeURIComponent('repost-sh/agent-terminal'), 'HOMEBREW_TAP_TOKEN')), false);
});

test('a second run finds everything done and changes nothing', (t) => {
  const env = setup(t);
  assert.equal(run(env, [], FIRST_RUN).status, 0);

  const again = run(env, [], []);

  assert.equal(again.status, 0, again.stdout + again.stderr);
  assert.match(again.stdout, /Nothing left to do/);
  assert.deepEqual(writes(again), []);
  assert.deepEqual(opened(again), []);
});

test('a run after a partial setup does only what is left', (t) => {
  const env = setup(t);
  tapReady(env);
  onNpm(env, MAIN, OIDC_PUBLISHER);

  const result = run(env, [], ['', 'owner', NPM_TOKEN, 'y', 'y', ...PACKAGES.slice(1).map(() => 'y')]);

  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.deepEqual(
    writes(result).map((call) => `${call.tool} ${call.args[0]}`),
    PACKAGES.slice(1).map(() => 'npm publish'),
  );
  const packages = published(env);
  assert.equal(packages[MAIN].npmUser, OIDC_PUBLISHER);
  for (const name of PACKAGES.slice(1)) assert.equal(packages[name].manifest.version, '0.0.0-bootstrap', name);
  assert.deepEqual(opened(result), [TOKENS_PAGE, TOKENS_PAGE, ...PACKAGES.slice(1).map(accessPage)]);
});

test('a trusted publisher no release used within two days is set up again', (t) => {
  const env = setup(t);
  tapReady(env);
  for (const name of PACKAGES) onNpm(env, name, OWNER_PUBLISHER);
  previousRun(
    env,
    Object.fromEntries(
      PACKAGES.map((name) => [trustedPublisherKey(name), secondsAgo(name === MAIN ? 49 * HOUR : HOUR)]),
    ),
  );

  const check = run(env, ['--check'], []);
  assert.equal(check.status, 1, check.stdout + check.stderr);
  assert.match(check.stdout, /@repost\/agent-terminal: .*expired/);

  const result = run(env, [], ['', 'y']);

  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.deepEqual(opened(result), [accessPage(MAIN)]);
  assert.deepEqual(writes(result), []);
  assert.equal(run(env, ['--check'], []).status, 0);
});

test('a bootstrap token left over from a stopped run is deleted on the next run', (t) => {
  const env = setup(t);
  tapReady(env);
  for (const name of PACKAGES) onNpm(env, name, OWNER_PUBLISHER);
  previousRun(env, {
    NPM_USERNAME: 'owner',
    NPM_BOOTSTRAP_TOKEN_CREATED: secondsAgo(600),
    ...Object.fromEntries(PACKAGES.map((name) => [trustedPublisherKey(name), secondsAgo(500)])),
  });

  const check = run(env, ['--check'], []);
  assert.equal(check.status, 1, check.stdout + check.stderr);
  assert.match(check.stdout, /bootstrap token/);

  const result = run(env, [], ['', 'y']);

  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.deepEqual(opened(result), [TOKENS_PAGE]);
  assert.equal(run(env, ['--check'], []).status, 0);
});

test('--check reports what is left and only reads', (t) => {
  const env = setup(t);

  const check = run(env, ['--check'], []);

  assert.equal(check.status, 1, check.stdout + check.stderr);
  assert.match(check.stdout, /repost-sh\/homebrew-tap/);
  assert.match(check.stdout, /HOMEBREW_TAP_TOKEN/);
  for (const name of PACKAGES) assert.match(check.stdout, new RegExp(`${name}: not on npm`));
  assert.deepEqual(writes(check), []);
  assert.deepEqual(opened(check), []);
});

test('declining to create the tap stops before anything is created', (t) => {
  const env = setup(t);

  const result = run(env, [], ['', 'n']);

  assert.equal(result.status, 1, result.stdout + result.stderr);
  assert.match(result.stdout, /Stopped/);
  assert.deepEqual(writes(result), []);
  assert.equal(fs.existsSync(path.join(env.world, 'tap')), false);
});
