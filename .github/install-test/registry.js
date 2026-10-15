#!/usr/bin/env node
const crypto = require('node:crypto');
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');
const { spawn, spawnSync } = require('node:child_process');

function manifestOf(tarball) {
  const tar = spawnSync('tar', ['-xzOf', tarball, 'package/package.json'], { encoding: 'utf8' });
  if (tar.status !== 0) {
    throw new Error(`could not read package/package.json from ${tarball}: ${tar.stderr || tar.error}`);
  }
  return JSON.parse(tar.stdout);
}

function loadPackages(tarballDir) {
  const packages = new Map();
  for (const file of fs.readdirSync(tarballDir).filter((name) => name.endsWith('.tgz'))) {
    const tarball = path.join(tarballDir, file);
    const bytes = fs.readFileSync(tarball);
    const manifest = manifestOf(tarball);
    const versions = packages.get(manifest.name) ?? [];
    versions.push({ manifest, file, bytes });
    packages.set(manifest.name, versions);
  }
  return packages;
}

function packument(name, versions, origin) {
  return {
    name,
    'dist-tags': { latest: versions[versions.length - 1].manifest.version },
    versions: Object.fromEntries(
      versions.map(({ manifest, file, bytes }) => [
        manifest.version,
        {
          ...manifest,
          dist: {
            tarball: `${origin}/${encodeURIComponent(name)}/-/${file}`,
            shasum: crypto.createHash('sha1').update(bytes).digest('hex'),
            integrity: `sha512-${crypto.createHash('sha512').update(bytes).digest('base64')}`,
          },
        },
      ]),
    ),
  };
}

function handler(packages) {
  return (request, response) => {
    const [name, file] = decodeURIComponent(request.url.slice(1)).split('/-/');
    const versions = packages.get(name);
    const tarball = versions && file && versions.find((version) => version.file === file);
    if (versions && file === undefined) {
      response.writeHead(200, { 'content-type': 'application/json' });
      response.end(JSON.stringify(packument(name, versions, `http://${request.headers.host}`)));
    } else if (tarball) {
      response.writeHead(200, { 'content-type': 'application/octet-stream' });
      response.end(tarball.bytes);
    } else {
      response.writeHead(404, { 'content-type': 'application/json' });
      response.end(JSON.stringify({ error: 'not found' }));
    }
  };
}

function main([tarballDir, ...command]) {
  if (!tarballDir || command.length === 0) {
    throw new Error('usage: registry.js <tarball-dir> <command> [args...]');
  }
  const server = http.createServer(handler(loadPackages(tarballDir)));
  server.listen(0, '127.0.0.1', () => {
    const registry = `http://127.0.0.1:${server.address().port}/`;
    const child = spawn(command[0], command.slice(1), {
      stdio: 'inherit',
      env: { ...process.env, npm_config_registry: registry, pnpm_config_registry: registry },
    });
    child.on('error', (error) => {
      console.error(`could not run ${command[0]}: ${error.message}`);
      process.exit(1);
    });
    child.on('exit', (code, signal) => {
      server.close();
      process.exit(signal ? 1 : code);
    });
  });
}

try {
  main(process.argv.slice(2));
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
