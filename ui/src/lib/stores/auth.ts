import { writable } from 'svelte/store';
import type { AuthStatus } from '$lib/types';
import { getAuthStatus, logout as apiLogout } from '$lib/api';

interface AuthState {
  initialized: boolean;
  authEnabled: boolean;
  authenticated: boolean;
  username: string | null;
}

function createAuthStore() {
  const { subscribe, set, update } = writable<AuthState>({
    initialized: false,
    authEnabled: false,
    authenticated: true,
    username: null,
  });

  return {
    subscribe,

    async checkStatus() {
      try {
        const status = await getAuthStatus();
        set({
          initialized: true,
          authEnabled: status.auth_enabled,
          authenticated: status.authenticated,
          username: status.username,
        });
        return status;
      } catch (e) {
        // On error, assume no auth required
        set({
          initialized: true,
          authEnabled: false,
          authenticated: true,
          username: null,
        });
        return null;
      }
    },

    setAuthenticated(username: string) {
      update((state) => ({
        ...state,
        authenticated: true,
        username,
      }));
    },

    async logout() {
      await apiLogout();
      update((state) => ({
        ...state,
        authenticated: false,
        username: null,
      }));
    },
  };
}

export const authStore = createAuthStore();
