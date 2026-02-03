import { browser } from '$app/environment';

type Theme = 'light' | 'dark' | 'system';
type EffectiveTheme = 'light' | 'dark';

function getSystemTheme(): EffectiveTheme {
	if (!browser) return 'light';
	return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function applyTheme(effective: EffectiveTheme): void {
	if (!browser) return;
	document.documentElement.classList.toggle('dark', effective === 'dark');
}

function resolveTheme(theme: Theme): EffectiveTheme {
	return theme === 'system' ? getSystemTheme() : theme;
}

function createThemeStore() {
	const stored = browser ? (localStorage.getItem('theme') as Theme | null) : null;
	let theme = $state<Theme>(stored ?? 'system');

	// Apply initial theme
	if (browser) {
		applyTheme(resolveTheme(theme));

		// Listen for system preference changes
		const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
		mediaQuery.addEventListener('change', () => {
			if (theme === 'system') {
				applyTheme(getSystemTheme());
			}
		});
	}

	return {
		get theme() {
			return theme;
		},

		get current(): EffectiveTheme {
			return resolveTheme(theme);
		},

		set(value: Theme) {
			theme = value;
			if (browser) {
				localStorage.setItem('theme', value);
				applyTheme(resolveTheme(value));
			}
		},

		toggle() {
			const effective = resolveTheme(theme);
			const next: EffectiveTheme = effective === 'dark' ? 'light' : 'dark';
			this.set(next);
		}
	};
}

export const themeStore = createThemeStore();
