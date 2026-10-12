// The panel's theme: the system's by default, or the one the owner picked (kept in this browser).

export type Theme = 'system' | 'dark' | 'light';

export function currentTheme(): Theme {
  try {
    const theme = localStorage.getItem('omnarc-theme');
    return theme === 'dark' || theme === 'light' ? theme : 'system';
  } catch {
    return 'system';
  }
}

export function setTheme(theme: Theme): void {
  if (theme === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
  try {
    if (theme === 'system') localStorage.removeItem('omnarc-theme');
    else localStorage.setItem('omnarc-theme', theme);
  } catch {
    // private windows may refuse storage; the theme still applies for this visit
  }
}
