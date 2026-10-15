const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { tempDir } = require('./support');

const SCRIPT = path.resolve(__dirname, '..', '..', 'scripts', 'glibc-floor.sh');

const X64_ON_GLIBC_2_28 = `
Dynamic Section:
  NEEDED       libstdc++.so.6
  NEEDED       libm.so.6
  NEEDED       libc.so.6

Version References:
  required from libstdc++.so.6:
    0x08922974 0x00 31 GLIBCXX_3.4
    0x0bafd179 0x00 29 CXXABI_1.3.9
  required from libm.so.6:
    0x09691a75 0x00 13 GLIBC_2.2.5
    0x06969187 0x00 28 GLIBC_2.27
  required from libc.so.6:
    0x09691a75 0x00 03 GLIBC_2.2.5
    0x06969197 0x00 08 GLIBC_2.17
    0x06969188 0x00 22 GLIBC_2.28
    0x0d696913 0x00 07 GLIBC_2.3
`;

const ARM64_ON_GLIBC_2_28 = `
Version References:
  required from ld-linux-aarch64.so.1:
    0x06969197 0x00 19 GLIBC_2.17
  required from libc.so.6:
    0x06969197 0x00 02 GLIBC_2.17
`;

const X64_ON_GLIBC_2_35 = `
Version References:
  required from libm.so.6:
    0x09691a75 0x00 13 GLIBC_2.2.5
    0x06969189 0x00 32 GLIBC_2.29
    0x069691b5 0x00 14 GLIBC_2.35
  required from libc.so.6:
    0x09691a75 0x00 03 GLIBC_2.2.5
    0x06969188 0x02 22 GLIBC_2.28
    0x069691b0 0x02 15 GLIBC_2.30
    0x069691b4 0x00 02 GLIBC_2.34
  required from ld-linux-x86-64.so.2:
    0x0d696913 0x00 12 GLIBC_2.3
`;

function floorCheck(t, binaries) {
  const dir = tempDir(t);
  const stubs = path.join(dir, 'bin');
  fs.mkdirSync(stubs);
  fs.writeFileSync(path.join(stubs, 'objdump'), '#!/bin/sh\n[ "$1" = -p ] || exit 2\ncat "$2"\n', { mode: 0o755 });
  const files = Object.entries(binaries).map(([name, privateHeaders]) => {
    const file = path.join(dir, name);
    fs.writeFileSync(file, privateHeaders);
    return file;
  });
  const run = spawnSync('sh', [SCRIPT, ...files], {
    encoding: 'utf8',
    env: { ...process.env, PATH: `${stubs}${path.delimiter}${process.env.PATH}` },
  });
  return { ...run, files };
}

test('the floor check prints the newest glibc version each binary requires and passes at 2.28', (t) => {
  const run = floorCheck(t, { x64: X64_ON_GLIBC_2_28, arm64: ARM64_ON_GLIBC_2_28 });

  assert.equal(run.status, 0, run.stderr);
  assert.equal(
    run.stdout,
    `${run.files[0]}: newest required version GLIBC_2.28, at or below the 2.28 floor\n` +
      `${run.files[1]}: newest required version GLIBC_2.17, at or below the 2.28 floor\n`,
  );
});

test('the floor check fails on a binary built on a newer glibc, after reporting every binary', (t) => {
  const run = floorCheck(t, { newer: X64_ON_GLIBC_2_35, arm64: ARM64_ON_GLIBC_2_28 });

  assert.equal(run.status, 1);
  assert.equal(run.stdout, `${run.files[1]}: newest required version GLIBC_2.17, at or below the 2.28 floor\n`);
  assert.equal(
    run.stderr,
    `${run.files[0]}: newest required version GLIBC_2.35, above the 2.28 floor\n` +
      'the gnu binaries must run on glibc 2.28: build them in the glibc 2.28 images dist-workspace.toml names\n',
  );
});

test('the floor check fails on a binary that requires no glibc version, and on no binary at all', (t) => {
  const run = floorCheck(t, { static: '\nProgram Header:\n    LOAD off    0x0000000000000000\n' });

  assert.equal(run.status, 1);
  assert.equal(run.stdout, '');
  assert.equal(
    run.stderr,
    `${run.files[0]}: requires no GLIBC_ version, so it is not a gnu binary\n` +
      'the gnu binaries must run on glibc 2.28: build them in the glibc 2.28 images dist-workspace.toml names\n',
  );

  const none = floorCheck(t, {});
  assert.equal(none.status, 1);
  assert.match(none.stderr, /^usage: glibc-floor\.sh <gnu binary>\.\.\.$/m);
});
