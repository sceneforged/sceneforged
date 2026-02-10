import { browser } from '$app/environment';

interface Preferences {
	autoplayNextEpisode: boolean;
	defaultSubtitleLanguage: string;
}

const STORAGE_KEY = 'sf-preferences';

const defaults: Preferences = {
	autoplayNextEpisode: true,
	defaultSubtitleLanguage: ''
};

function load(): Preferences {
	if (!browser) return { ...defaults };
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (raw) return { ...defaults, ...JSON.parse(raw) };
	} catch {
		// ignore
	}
	return { ...defaults };
}

function save(prefs: Preferences): void {
	if (!browser) return;
	localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
}

function createPreferencesStore() {
	const initial = load();
	let autoplayNextEpisode = $state(initial.autoplayNextEpisode);
	let defaultSubtitleLanguage = $state(initial.defaultSubtitleLanguage);

	function persist() {
		save({ autoplayNextEpisode, defaultSubtitleLanguage });
	}

	return {
		get autoplayNextEpisode() {
			return autoplayNextEpisode;
		},
		set autoplayNextEpisode(value: boolean) {
			autoplayNextEpisode = value;
			persist();
		},

		get defaultSubtitleLanguage() {
			return defaultSubtitleLanguage;
		},
		set defaultSubtitleLanguage(value: string) {
			defaultSubtitleLanguage = value;
			persist();
		}
	};
}

export const preferencesStore = createPreferencesStore();
