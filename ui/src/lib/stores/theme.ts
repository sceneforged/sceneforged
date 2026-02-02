import { writable } from 'svelte/store';
import { browser } from '$app/environment';

export type Theme = 'light' | 'dark' | 'system';

function getSystemTheme(): 'light' | 'dark' {
  if (!browser) return 'light';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function applyTheme(theme: Theme) {
  if (!browser) return;

  const effectiveTheme = theme === 'system' ? getSystemTheme() : theme;
  document.documentElement.classList.toggle('dark', effectiveTheme === 'dark');
}

function createThemeStore() {
  const stored = browser ? (localStorage.getItem('theme') as Theme | null) : null;
  const initial = stored || 'system';
  const { subscribe, set, update } = writable<Theme>(initial);

  // Apply initial theme on creation
  if (browser) {
    applyTheme(initial);

    // Listen for system theme changes
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    mediaQuery.addEventListener('change', () => {
      // Re-apply if currently using system theme
      const currentTheme = localStorage.getItem('theme') as Theme | null;
      if (!currentTheme || currentTheme === 'system') {
        applyTheme('system');
      }
    });
  }

  return {
    subscribe,
    set: (value: Theme) => {
      if (browser) {
        localStorage.setItem('theme', value);
        applyTheme(value);
      }
      set(value);
    },
    toggle: () => {
      update((current) => {
        const effectiveCurrent = current === 'system' ? getSystemTheme() : current;
        const next = effectiveCurrent === 'dark' ? 'light' : 'dark';
        if (browser) {
          localStorage.setItem('theme', next);
          applyTheme(next);
        }
        return next;
      });
    },
    // Cycle through: system -> light -> dark -> system
    cycle: () => {
      update((current) => {
        const order: Theme[] = ['system', 'light', 'dark'];
        const currentIndex = order.indexOf(current);
        const next = order[(currentIndex + 1) % order.length];
        if (browser) {
          localStorage.setItem('theme', next);
          applyTheme(next);
        }
        return next;
      });
    },
  };
}

export const theme = createThemeStore();
