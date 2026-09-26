/** A deployed console follows its serving origin, including custom ports and HTTPS proxies. */
export function resolveApiBase(pageUrl: string | undefined, isDevelopment: boolean, override: unknown, developmentBase: string): string {
  if (typeof override === 'string' && override.length > 0) return override;
  if (pageUrl && !isDevelopment) {
    const protocol = new URL(pageUrl).protocol;
    if (protocol === 'http:' || protocol === 'https:') return '';
  }
  return developmentBase;
}
