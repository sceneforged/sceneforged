import type { Library, Item } from '$lib/types.js';
import { getLibraries, getItems } from '$lib/api/index.js';

function createLibraryStore() {
	let libraries = $state<Library[]>([]);
	let selectedLibrary = $state<Library | null>(null);
	let items = $state<Item[]>([]);
	let loading = $state(false);
	let searchQuery = $state('');
	let page = $state(0);
	let totalItems = $state(0);
	let limit = $state(50);

	async function loadItemsInternal() {
		loading = true;
		try {
			const result = await getItems({
				library_id: selectedLibrary?.id,
				page,
				limit,
				search: searchQuery || undefined
			});
			items = result.items;
			totalItems = result.total;
		} catch (e) {
			console.error('Failed to load items:', e);
			items = [];
			totalItems = 0;
		} finally {
			loading = false;
		}
	}

	return {
		get libraries() {
			return libraries;
		},
		get selectedLibrary() {
			return selectedLibrary;
		},
		get items() {
			return items;
		},
		get loading() {
			return loading;
		},
		get searchQuery() {
			return searchQuery;
		},
		get page() {
			return page;
		},
		get totalItems() {
			return totalItems;
		},
		get limit() {
			return limit;
		},

		async loadLibraries() {
			loading = true;
			try {
				libraries = await getLibraries();
			} catch (e) {
				console.error('Failed to load libraries:', e);
				libraries = [];
			} finally {
				loading = false;
			}
		},

		async selectLibrary(id: string) {
			const lib = libraries.find((l) => l.id === id) ?? null;
			selectedLibrary = lib;
			page = 0;
			searchQuery = '';
			await loadItemsInternal();
		},

		async loadItems() {
			await loadItemsInternal();
		},

		async search(query: string) {
			searchQuery = query;
			page = 0;
			await loadItemsInternal();
		},

		async setPage(n: number) {
			page = n;
			await loadItemsInternal();
		}
	};
}

export const libraryStore = createLibraryStore();
