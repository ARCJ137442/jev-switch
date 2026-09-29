export type ThemePreference = 'light' | 'dark' | 'system';

const THEME_KEY = 'jev_theme';

export function readThemePreference(): ThemePreference {
  let stored: string | null = null;
  try { stored = localStorage.getItem(THEME_KEY); } catch { /* Session-only fallback. */ }
  return stored === 'dark' || stored === 'light' || stored === 'system' ? stored : 'system';
}

function systemTheme(): 'light' | 'dark' {
  return window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

export function applyThemePreference(preference: ThemePreference, persist = true): void {
  document.documentElement.dataset.theme = preference === 'system' ? systemTheme() : preference;
  document.documentElement.dataset.themePreference = preference;
  if (persist) {
    try { localStorage.setItem(THEME_KEY, preference); } catch { /* Session-only fallback. */ }
  }
  window.dispatchEvent(new Event('jev-theme-change'));
}

export function followSystemTheme(): void {
  const media = window.matchMedia?.('(prefers-color-scheme: dark)');
  media?.addEventListener('change', () => {
    if (readThemePreference() === 'system') applyThemePreference('system', false);
  });
}
