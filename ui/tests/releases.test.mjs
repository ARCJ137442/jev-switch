import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';

const { checkLatestRelease, compareVersions } = await loadTs('../src/api/releases.ts');

test('compares release versions without treating a leading v as meaningful', () => {
  assert.equal(compareVersions('v0.6.0', '0.5.0'), 1);
  assert.equal(compareVersions('0.5.0', 'v0.5.0'), 0);
  assert.equal(compareVersions('0.4.9', '0.5.0'), -1);
  assert.equal(compareVersions('development', '0.5.0'), 0);
});

test('checks public release metadata and reports whether an update is available', async () => {
  const result = await checkLatestRelease('0.5.0', async (input, init) => {
    assert.equal(input, 'https://api.github.com/repos/ARCJ137442/jev-switch/releases/latest');
    assert.equal(init?.headers?.Accept, 'application/vnd.github+json');
    return new Response(JSON.stringify({
      tag_name: 'v0.6.0',
      name: 'Jev-Switch 0.6.0',
      html_url: 'https://github.com/ARCJ137442/jev-switch/releases/tag/v0.6.0',
      published_at: '2026-10-01T00:00:00Z',
    }), { status: 200 });
  });
  assert.deepEqual(result, {
    currentVersion: '0.5.0',
    latestVersion: '0.6.0',
    releaseUrl: 'https://github.com/ARCJ137442/jev-switch/releases/tag/v0.6.0',
    releaseName: 'Jev-Switch 0.6.0',
    publishedAt: '2026-10-01T00:00:00Z',
    updateAvailable: true,
  });
});

test('rejects malformed release responses and failed requests', async () => {
  await assert.rejects(
    checkLatestRelease('0.5.0', async () => new Response('{}', { status: 200 })),
    /no valid version/,
  );
  await assert.rejects(
    checkLatestRelease('0.5.0', async () => new Response('', { status: 503 })),
    /GitHub 503/,
  );
});
