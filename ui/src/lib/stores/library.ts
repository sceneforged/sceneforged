import { writable, derived } from 'svelte/store';
import type { Library, Item, ItemsPage } from '../types';
import * as api from '../api';

interface LibraryState {
  libraries: Library[];
  selectedLibraryId: string | null;
  items: Item[];
  totalCount: number;
  currentPage: number;
  pageSize: number;
  loading: boolean;
  error: string | null;
}

function createLibraryStore() {
  const initialState: LibraryState = {
    libraries: [],
    selectedLibraryId: null,
    items: [],
    totalCount: 0,
    currentPage: 0,
    pageSize: 24,
    loading: false,
    error: null,
  };

  const { subscribe, set, update } = writable<LibraryState>(initialState);

  return {
    subscribe,

    async loadLibraries() {
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        const libraries = await api.getLibraries();
        update((s) => ({ ...s, libraries, loading: false }));
        return libraries;
      } catch (e) {
        const error = e instanceof Error ? e.message : 'Failed to load libraries';
        update((s) => ({ ...s, error, loading: false }));
        return [];
      }
    },

    selectLibrary(libraryId: string | null) {
      update((s) => ({
        ...s,
        selectedLibraryId: libraryId,
        items: [],
        totalCount: 0,
        currentPage: 0,
      }));
    },

    async loadItems(params?: {
      libraryId?: string;
      parentId?: string;
      search?: string;
      page?: number;
    }) {
      update((s) => ({ ...s, loading: true, error: null }));

      try {
        let state: LibraryState = initialState;
        update((s) => {
          state = s;
          return s;
        });

        const page = params?.page ?? 0;
        const response = await api.getItems({
          library_id: params?.libraryId ?? state.selectedLibraryId ?? undefined,
          parent_id: params?.parentId,
          search: params?.search,
          limit: state.pageSize,
          offset: page * state.pageSize,
        });

        update((s) => ({
          ...s,
          items: response.items,
          totalCount: response.total_count,
          currentPage: page,
          loading: false,
        }));

        return response;
      } catch (e) {
        const error = e instanceof Error ? e.message : 'Failed to load items';
        update((s) => ({ ...s, error, loading: false }));
        return null;
      }
    },

    async search(query: string) {
      if (!query.trim()) {
        return this.loadItems();
      }
      return this.loadItems({ search: query });
    },

    setPage(page: number) {
      return this.loadItems({ page });
    },

    reset() {
      set(initialState);
    },
  };
}

export const libraryStore = createLibraryStore();

// Derived stores for convenience
export const selectedLibrary = derived(libraryStore, ($store) =>
  $store.libraries.find((l) => l.id === $store.selectedLibraryId)
);

export const hasMorePages = derived(
  libraryStore,
  ($store) => ($store.currentPage + 1) * $store.pageSize < $store.totalCount
);

export const totalPages = derived(libraryStore, ($store) =>
  Math.ceil($store.totalCount / $store.pageSize)
);
