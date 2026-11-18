// Install UNMODIFIED upstream Sakuga VFX components at a pinned Git commit.
// Offline rebuilds reuse existing vendored files. No third-party code is rewritten.
//
// Network resilience: raw.githubusercontent.com is blocked/reset on some
// networks (ECONNRESET), so every asset is fetched through a list of mirrors
// with per-mirror retries and exponential backoff. Once a file is on disk it is
// never fetched again, so one successful run makes later builds offline.
import { existsSync, mkdirSync, writeFileSync, readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';

const root = new URL('../public/sakuga-vfx/', import.meta.url);
const commit = 'c3dabb173928a3ad2372183103caf2e6f50953d0';
const repo = 'finktheartist/sakuga-vfx';
const names = ['button', 'toggle', 'tabs', 'checkbox', 'input', 'slider', 'toast'];
mkdirSync(root, { recursive: true });

/** Mirrors, in the order they are tried. */
const mirrors = [
  (p) => `https://raw.githubusercontent.com/${repo}/${commit}/${p}`,
  (p) => `https://cdn.jsdelivr.net/gh/${repo}@${commit}/${p}`,
  (p) => `https://gcore.jsdelivr.net/gh/${repo}@${commit}/${p}`,
];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Download one upstream path, trying every mirror up to `attempts` times. */
async function download(path, attempts = 3) {
  let lastError = null;
  for (let round = 0; round < attempts; round += 1) {
    for (const mirror of mirrors) {
      const url = mirror(path);
      try {
        const response = await fetch(url, {
          signal: AbortSignal.timeout(20000),
          headers: { 'accept-encoding': 'identity', connection: 'close' },
        });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        return Buffer.from(await response.arrayBuffer());
      } catch (error) {
        lastError = error;
        console.warn(`  ! ${url} -> ${error?.message || error}`);
      }
    }
    if (round + 1 < attempts) await sleep(600 * (round + 1));
  }
  console.warn(`  x ${path}: all mirrors failed (${lastError?.message || 'unknown'})`);
  return null;
}

let failures = 0;
const records = [];

for (const name of names) {
  const file = new URL(`${name}.js`, root);
  if (!existsSync(file)) {
    const body = await download(`${name}/${name}.js`);
    if (body === null) {
      failures += 1;
      continue;
    }
    writeFileSync(file, new Uint8Array(body));
  }
  const bytes = readFileSync(file);
  if (bytes.length < 10000) {
    throw new Error(`Sakuga ${name}.js seems incomplete (${bytes.length} bytes)`);
  }
  records.push({ component: name, sha256: createHash('sha256').update(bytes).digest('hex') });
}

const license = new URL('LICENSE', root);
if (!existsSync(license)) {
  const body = await download('LICENSE');
  if (body === null) failures += 1;
  else writeFileSync(license, body);
}

if (records.length > 0) {
  writeFileSync(new URL('manifest.json', root),
    `${JSON.stringify({ repo, commit, records }, null, 2)}\n`);
}

if (failures > 0) {
  // Never block `tauri dev` on a transient network error: report loudly and let
  // the app start with whatever is already vendored.
  console.warn(`Sakuga VFX: ${failures} asset(s) unavailable, ${records.length} ready at ${commit}`);
  console.warn('Re-run `npm run sakuga:sync` when the network recovers (cached files are kept).');
  process.exitCode = 0;
} else {
  console.log(`Sakuga VFX: ${records.length} original components at ${commit}`);
}