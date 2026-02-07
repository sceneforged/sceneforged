import { getAuthStatus, login as apiLogin, logout as apiLogout } from '$lib/api/index.js';

function createAuthStore() {
	let authenticated = $state(false);
	let username = $state('');
	let userId = $state('');
	let role = $state('');
	let authEnabled = $state(false);
	let initialized = $state(false);

	return {
		get authenticated() {
			return authenticated;
		},
		get username() {
			return username;
		},
		get userId() {
			return userId;
		},
		get role() {
			return role;
		},
		get isAdmin() {
			return role === 'admin';
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
				userId = status.user_id ?? '';
				role = status.role ?? '';
				authEnabled = status.auth_enabled;
				initialized = true;
			} catch {
				// On error, assume no auth required
				authenticated = true;
				username = '';
				userId = '';
				role = 'admin';
				authEnabled = false;
				initialized = true;
			}
		},

		async login(user: string, pass: string) {
			const result = await apiLogin(user, pass);
			if (result.success) {
				authenticated = true;
				username = user;
				// Re-fetch to get userId and role
				await this.checkStatus();
			}
			return result;
		},

		async logout() {
			await apiLogout();
			authenticated = false;
			username = '';
			userId = '';
			role = '';
		}
	};
}

export const authStore = createAuthStore();
