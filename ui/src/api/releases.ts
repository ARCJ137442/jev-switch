export const GITHUB_REPOSITORY_URL = 'https://github.com/ARCJ137442/jev-switch';
export const GITHUB_RELEASES_URL = `${GITHUB_REPOSITORY_URL}/releases`;
const LATEST_RELEASE_API_URL = 'https://api.github.com/repos/ARCJ137442/jev-switch/releases/latest';

export type ReleaseCheckResult = {
  currentVersion: string;
  latestVersion: string;
  releaseUrl: string;
  releaseName: string | null;
  publishedAt: string | null;
  updateAvailable: boolean;
};

function parseVersion(value: string): [number, number, number] | null {
  const match = value.trim().replace(/^v/i, '').match(/^(\d+)\.(\d+)\.(\d+)(?:[-+].*)?$/);
  if (!match) return null;
  return [Number(match[1]), Number(match[2]), Number(match[3])];
}

export function compareVersions(left: string, right: string): number {
  const a = parseVersion(left);
  const b = parseVersion(right);
  if (!a || !b) return 0;
  for (let index = 0; index < a.length; index += 1) {
    if (a[index] !== b[index]) return a[index] > b[index] ? 1 : -1;
  }
  return 0;
}

/** Read only public release metadata; credentials are never sent. */
export async function checkLatestRelease(
  currentVersion: string,
  fetcher: typeof fetch = fetch,
): Promise<ReleaseCheckResult> {
  const response = await fetcher(LATEST_RELEASE_API_URL, {
    headers: { Accept: 'application/vnd.github+json' },
  });
  if (!response.ok) throw new Error(`GitHub ${response.status}`);
  const body: unknown = await response.json();
  if (!body || typeof body !== 'object') throw new Error('GitHub returned an invalid release');
  const record = body as Record<string, unknown>;
  const tag = typeof record.tag_name === 'string' ? record.tag_name : null;
  if (!tag || !parseVersion(tag)) throw new Error('GitHub release has no valid version');
  return {
    currentVersion,
    latestVersion: tag.replace(/^v/i, ''),
    releaseUrl: typeof record.html_url === 'string' ? record.html_url : GITHUB_RELEASES_URL,
    releaseName: typeof record.name === 'string' ? record.name : null,
    publishedAt: typeof record.published_at === 'string' ? record.published_at : null,
    updateAvailable: compareVersions(tag, currentVersion) > 0,
  };
}
