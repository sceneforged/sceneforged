import { getAuthStatus, login as apiLogin, logout as apiLogout } from '$lib/api/index.js';

function createAuthStore() {
	let authenticated = $state(false);
	let username = $state('');
	let authEnabled = $state(false);
	let initialized = $state(false);

	return {
		get authenticated() {
			return authenticated;
		},
		get username() {
			return username;
		},
		get authEnabled() {
			return authEnabled;
		},
		get initialized() {
			return initialized;
		},

		async checkStatus() {
			try {
				const status = await getAuthStatus();
				authenticated = status.authenticated;
				username = status.username ?? '';
				authEnabled = status.auth_enabled;
				initialized = true;
			} catch {
				// On error, assume no auth required
				authenticated = true;
				username = '';
				authEnabled = false;
				initialized = true;
			}
		},

		async login(user: string, pass: string) {
			const result = await apiLogin(user, pass);
			if (result.success) {
				authenticated = true;
				username = user;
			}
			return result;
		},

		async logout() {
			await apiLogout();
			authenticated = false;
			username = '';
		}
	};
}

export const authStore = createAuthStore();
