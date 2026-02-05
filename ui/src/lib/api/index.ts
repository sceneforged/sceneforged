import { api } from './client.js';
import type {
	Library,
	Item,
	Job,
	ConversionJob,
	PlaybackState,
	FavoriteState,
	UserData,
	Rule,
	DashboardStats,
	ToolInfo
} from '../types.js';

export { api, ApiError } from './client.js';

// --- Libraries ---

export async function getLibraries(): Promise<Library[]> {
	return api.get<Library[]>('/libraries');
}

export async function getLibrary(id: string): Promise<Library> {
	return api.get<Library>(`/libraries/${id}`);
}

export async function createLibrary(data: {
	name: string;
	media_type: string;
	paths: string[];
}): Promise<Library> {
	const result = await api.post<Library>('/libraries', data);
	api.invalidate('/libraries');
	return result;
}

export async function deleteLibrary(id: string): Promise<void> {
	await api.delete(`/libraries/${id}`);
	api.invalidate('/libraries');
}

export async function scanLibrary(id: string): Promise<void> {
	await api.post<void>(`/libraries/${id}/scan`);
}

// --- Items ---

function normalizeItem(item: Item): Item {
	return {
		...item,
		media_files: item.media_files ?? [],
		images: item.images ?? []
	};
}

export async function getItems(params: {
	library_id?: string;
	page?: number;
	limit?: number;
	search?: string;
}): Promise<{ items: Item[]; total: number }> {
	const searchParams = new URLSearchParams();
	if (params.library_id) searchParams.set('library_id', params.library_id);
	const limit = params.limit ?? 50;
	if (params.page !== undefined) {
		searchParams.set('offset', String(params.page * limit));
	}
	searchParams.set('limit', String(limit));
	if (params.search) searchParams.set('search', params.search);

	const query = searchParams.toString();
	const result = await api.get<Item[] | { items: Item[]; total: number }>(`/items${query ? `?${query}` : ''}`);
	// Backend returns a plain array; normalize to { items, total } shape
	const items = Array.isArray(result) ? result : result.items;
	const total = Array.isArray(result) ? result.length : (result.total ?? items.length);
	return { items: items.map(normalizeItem), total };
}

export async function getItem(id: string): Promise<Item> {
	const item = await api.get<Item>(`/items/${id}`);
	return normalizeItem(item);
}

// --- Jobs ---

export async function getJobs(params?: {
	status?: string;
	page?: number;
	limit?: number;
}): Promise<{ jobs: Job[]; total: number }> {
	const searchParams = new URLSearchParams();
	if (params?.status) searchParams.set('status', params.status);
	const limit = params?.limit ?? 50;
	if (params?.page !== undefined) {
		searchParams.set('offset', String(params.page * limit));
	}
	searchParams.set('limit', String(limit));

	const query = searchParams.toString();
	const result = await api.get<Job[] | { jobs: Job[]; total: number }>(`/jobs${query ? `?${query}` : ''}`, {
		skipCache: true
	});
	// Backend returns a plain array; normalize to { jobs, total } shape
	const jobs = Array.isArray(result) ? result : result.jobs;
	const total = Array.isArray(result) ? result.length : (result.total ?? jobs.length);
	return { jobs, total };
}

export async function submitJob(data: { file_path: string }): Promise<Job> {
	const result = await api.post<Job>('/jobs/submit', data);
	api.invalidate('/jobs');
	return result;
}

export async function retryJob(id: string): Promise<Job> {
	const result = await api.post<Job>(`/jobs/${id}/retry`);
	api.invalidate('/jobs');
	return result;
}

export async function deleteJob(id: string): Promise<void> {
	await api.delete(`/jobs/${id}`);
	api.invalidate('/jobs');
}

// --- Conversions ---

export async function getConversions(params?: {
	status?: string;
	page?: number;
	limit?: number;
}): Promise<ConversionJob[]> {
	const searchParams = new URLSearchParams();
	if (params?.status) searchParams.set('status', params.status);
	const limit = params?.limit ?? 50;
	if (params?.page !== undefined) {
		searchParams.set('offset', String(params.page * limit));
	}
	searchParams.set('limit', String(limit));
	const query = searchParams.toString();
	return api.get<ConversionJob[]>(`/conversions${query ? `?${query}` : ''}`, { skipCache: true });
}

export async function getConversion(id: string): Promise<ConversionJob> {
	return api.get<ConversionJob>(`/conversions/${id}`, { skipCache: true });
}

export async function submitConversion(data: {
	item_id: string;
	media_file_id?: string;
}): Promise<ConversionJob> {
	const result = await api.post<ConversionJob>('/conversions/submit', data);
	api.invalidate('/conversions');
	return result;
}

export async function deleteConversion(id: string): Promise<void> {
	await api.delete(`/conversions/${id}`);
	api.invalidate('/conversions');
}

// --- Items: children ---

export async function getItemChildren(id: string): Promise<Item[]> {
	const items = await api.get<Item[]>(`/items/${id}/children`);
	return items.map(normalizeItem);
}

// --- Playback ---

export async function getContinueWatching(limit = 20): Promise<PlaybackState[]> {
	return api.get<PlaybackState[]>(`/playback/continue?limit=${limit}`, { skipCache: true });
}

export async function getPlayback(itemId: string): Promise<PlaybackState> {
	return api.get<PlaybackState>(`/playback/${itemId}`, { skipCache: true });
}

export async function updateProgress(
	itemId: string,
	positionSecs: number,
	completed = false
): Promise<PlaybackState> {
	return api.post<PlaybackState>(`/playback/${itemId}/progress`, {
		position_secs: positionSecs,
		completed
	});
}

export async function markPlayed(itemId: string): Promise<PlaybackState> {
	return api.post<PlaybackState>(`/playback/${itemId}/played`);
}

export async function markUnplayed(itemId: string): Promise<void> {
	await api.post<void>(`/playback/${itemId}/unplayed`);
}

export async function getUserData(itemId: string): Promise<UserData> {
	return api.get<UserData>(`/playback/${itemId}/user-data`, { skipCache: true });
}

// --- Favorites ---

export async function getFavorites(limit = 50): Promise<FavoriteState[]> {
	return api.get<FavoriteState[]>(`/favorites?limit=${limit}`, { skipCache: true });
}

export async function addFavorite(itemId: string): Promise<FavoriteState> {
	return api.post<FavoriteState>(`/favorites/${itemId}`);
}

export async function removeFavorite(itemId: string): Promise<void> {
	await api.delete(`/favorites/${itemId}`);
}

// --- Config / Rules ---

export async function getConfigRules(): Promise<Rule[]> {
	return api.get<Rule[]>('/config/rules');
}

export async function updateConfigRules(rules: Rule[]): Promise<Rule[]> {
	const result = await api.put<Rule[]>('/config/rules', rules);
	api.invalidate('/config/rules');
	return result;
}

// --- Dashboard ---

export async function getDashboard(): Promise<DashboardStats> {
	return api.get<DashboardStats>('/admin/dashboard', { skipCache: true });
}

// --- Tools ---

export async function getTools(): Promise<ToolInfo[]> {
	return api.get<ToolInfo[]>('/admin/tools');
}

// --- Auth ---

export async function login(
	username: string,
	password: string
): Promise<{ success: boolean }> {
	return api.post<{ success: boolean }>('/auth/login', { username, password });
}

export async function logout(): Promise<void> {
	await api.post<void>('/auth/logout');
}

export async function getAuthStatus(): Promise<{
	authenticated: boolean;
	username?: string;
	auth_enabled: boolean;
}> {
	return api.get('/auth/status', { skipCache: true });
}
