import { api } from './client.js';
import type {
	Library,
	Item,
	MediaFile,
	Job,
	ConversionJob,
	PlaybackState,
	FavoriteState,
	UserData,
	User,
	Rule,
	DashboardStats,
	ToolInfo,
	ArrConfig,
	JellyfinConfig,
	ConversionConfig,
	ContinueWatchingEntry,
	FavoriteEntry,
	DirEntry,
	LibraryStats,
	Invitation
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

export async function getJob(id: string): Promise<Job> {
	return api.get<Job>(`/jobs/${id}`, { skipCache: true });
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

export async function getContinueWatching(limit = 20): Promise<ContinueWatchingEntry[]> {
	return api.get<ContinueWatchingEntry[]>(`/playback/continue?limit=${limit}`, { skipCache: true });
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

export async function getFavorites(limit = 50): Promise<FavoriteEntry[]> {
	return api.get<FavoriteEntry[]>(`/favorites?limit=${limit}`, { skipCache: true });
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

export async function createRule(rule: Omit<Rule, 'id'>): Promise<Rule> {
	const result = await api.post<Rule>('/config/rules', rule);
	api.invalidate('/config/rules');
	return result;
}

export async function updateRule(name: string, rule: Omit<Rule, 'id'>): Promise<Rule> {
	const result = await api.put<Rule>(`/config/rules/${encodeURIComponent(name)}`, rule);
	api.invalidate('/config/rules');
	return result;
}

export async function deleteRule(name: string): Promise<void> {
	await api.delete(`/config/rules/${encodeURIComponent(name)}`);
	api.invalidate('/config/rules');
}

// --- Config / Arrs ---

export async function getConfigArrs(): Promise<ArrConfig[]> {
	return api.get<ArrConfig[]>('/config/arrs');
}

export async function createArr(data: ArrConfig): Promise<ArrConfig> {
	const result = await api.post<ArrConfig>('/config/arrs', data);
	api.invalidate('/config/arrs');
	return result;
}

export async function updateArr(name: string, data: ArrConfig): Promise<ArrConfig> {
	const result = await api.put<ArrConfig>(`/config/arrs/${encodeURIComponent(name)}`, data);
	api.invalidate('/config/arrs');
	return result;
}

export async function deleteArr(name: string): Promise<void> {
	await api.delete(`/config/arrs/${encodeURIComponent(name)}`);
	api.invalidate('/config/arrs');
}

export async function testArr(name: string): Promise<{ success: boolean; message: string }> {
	return api.post<{ success: boolean; message: string }>(`/config/arrs/${encodeURIComponent(name)}/test`);
}

// --- Config / Jellyfins ---

export async function getConfigJellyfins(): Promise<JellyfinConfig[]> {
	return api.get<JellyfinConfig[]>('/config/jellyfins');
}

export async function createJellyfin(data: JellyfinConfig): Promise<JellyfinConfig> {
	const result = await api.post<JellyfinConfig>('/config/jellyfins', data);
	api.invalidate('/config/jellyfins');
	return result;
}

export async function updateJellyfin(name: string, data: JellyfinConfig): Promise<JellyfinConfig> {
	const result = await api.put<JellyfinConfig>(`/config/jellyfins/${encodeURIComponent(name)}`, data);
	api.invalidate('/config/jellyfins');
	return result;
}

export async function deleteJellyfin(name: string): Promise<void> {
	await api.delete(`/config/jellyfins/${encodeURIComponent(name)}`);
	api.invalidate('/config/jellyfins');
}

// --- Config / Conversion ---

export async function getConversionConfig(): Promise<ConversionConfig> {
	return api.get<ConversionConfig>('/config/conversion');
}

export async function updateConversionConfig(data: ConversionConfig): Promise<ConversionConfig> {
	const result = await api.put<ConversionConfig>('/config/conversion', data);
	api.invalidate('/config/conversion');
	return result;
}

// --- Config / Reload ---

export async function reloadConfig(): Promise<void> {
	await api.post<void>('/config/reload');
	api.invalidate('/config');
}

// --- Config / Browse ---

export async function browsePath(path: string): Promise<{ entries: { name: string; path: string; is_dir: boolean }[] }> {
	return api.get<{ entries: { name: string; path: string; is_dir: boolean }[] }>(`/config/browse?path=${encodeURIComponent(path)}`);
}

// --- Dashboard ---

export async function getDashboard(): Promise<DashboardStats> {
	return api.get<DashboardStats>('/admin/dashboard', { skipCache: true });
}

// --- Tools ---

export async function getTools(): Promise<ToolInfo[]> {
	return api.get<ToolInfo[]>('/admin/tools');
}

// --- Items: files ---

export async function getItemFiles(itemId: string): Promise<MediaFile[]> {
	return api.get<MediaFile[]>(`/items/${itemId}/files`);
}

// --- Search ---

export async function searchItems(
	query: string,
	limit = 20,
	opts?: { library_id?: string; item_kind?: string }
): Promise<Item[]> {
	const searchParams = new URLSearchParams({ q: query, limit: String(limit) });
	if (opts?.library_id) searchParams.set('library_id', opts.library_id);
	if (opts?.item_kind) searchParams.set('item_kind', opts.item_kind);
	return api.get<Item[]>(`/search?${searchParams}`);
}

// --- Conversions: item-specific ---

export async function convertItem(itemId: string): Promise<ConversionJob> {
	const result = await api.post<ConversionJob>('/conversions/submit', { item_id: itemId });
	api.invalidate('/conversions');
	return result;
}

export async function batchConvert(itemIds: string[]): Promise<ConversionJob[]> {
	const result = await api.post<ConversionJob[]>('/conversions/batch', { item_ids: itemIds });
	api.invalidate('/conversions');
	return result;
}

export async function getConversionsForItem(itemId: string): Promise<ConversionJob[]> {
	return api.get<ConversionJob[]>(`/conversions?item_id=${encodeURIComponent(itemId)}`, { skipCache: true });
}

// --- Admin stats ---

export async function getLibraryStats(): Promise<LibraryStats> {
	return api.get<LibraryStats>('/admin/stats', { skipCache: true });
}

// --- Config / Validate ---

export async function validateConfig(): Promise<{ valid: boolean; errors: string[] }> {
	return api.post<{ valid: boolean; errors: string[] }>('/config/validate');
}

// --- Config / Browse (directory listing) ---

export async function browsePaths(path: string = '/', search?: string): Promise<DirEntry[]> {
	const params = new URLSearchParams({ path });
	if (search) params.set('search', search);
	const result = await api.get<DirEntry[] | { entries: DirEntry[] }>(`/config/browse?${params}`, { skipCache: true });
	// Handle both { entries: [...] } and [...] response shapes
	return Array.isArray(result) ? result : result.entries;
}

// --- Utility formatters ---

export function formatBytes(bytes: number): string {
	if (bytes === 0) return '0 B';
	const k = 1024;
	const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}

export function formatRuntime(minutes: number | null | undefined): string {
	if (!minutes) return '';
	const h = Math.floor(minutes / 60);
	const m = minutes % 60;
	if (h > 0) return `${h}h ${m}m`;
	return `${m}m`;
}

export function formatDurationSecs(secs: number | null | undefined): string {
	if (secs == null || secs <= 0) return '-';
	const s = Math.round(secs);
	if (s < 60) return `${s}s`;
	const m = Math.floor(s / 60);
	const rs = s % 60;
	if (m < 60) return `${m}m ${rs}s`;
	const h = Math.floor(m / 60);
	const rm = m % 60;
	return `${h}h ${rm}m`;
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
	user_id?: string;
	role?: string;
	auth_enabled: boolean;
}> {
	return api.get('/auth/status', { skipCache: true });
}

// --- Admin: Users ---

export async function listUsers(): Promise<User[]> {
	return api.get<User[]>('/admin/users', { skipCache: true });
}

export async function createUser(data: {
	username: string;
	password: string;
	role?: string;
}): Promise<User> {
	const result = await api.post<User>('/admin/users', data);
	api.invalidate('/admin/users');
	return result;
}

export async function updateUser(
	id: string,
	data: { role?: string; password?: string }
): Promise<void> {
	await api.put<void>(`/admin/users/${id}`, data);
	api.invalidate('/admin/users');
}

export async function deleteUser(id: string): Promise<void> {
	await api.delete(`/admin/users/${id}`);
	api.invalidate('/admin/users');
}

// --- TMDB ---

export async function searchTmdb(
	query: string,
	type: string = 'movie'
): Promise<{
	results: Array<{
		tmdb_id: number;
		title: string | null;
		year: string | null;
		overview: string | null;
		poster_path: string | null;
	}>;
}> {
	const params = new URLSearchParams({ q: query, type });
	return api.get(`/tmdb/search?${params}`, { skipCache: true });
}

export async function enrichItem(
	itemId: string,
	tmdbId: number,
	mediaType: string
): Promise<{ updated: boolean; tmdb_id: number | null; images_downloaded: number }> {
	return api.post(`/items/${itemId}/enrich`, { tmdb_id: tmdbId, type: mediaType });
}

// --- Invitations ---

export async function createInvitation(data?: {
	role?: string;
	expires_in_days?: number;
}): Promise<Invitation> {
	const result = await api.post<Invitation>('/admin/invitations', data ?? {});
	api.invalidate('/admin/invitations');
	return result;
}

export async function listInvitations(): Promise<Invitation[]> {
	return api.get<Invitation[]>('/admin/invitations', { skipCache: true });
}

export async function deleteInvitation(id: string): Promise<void> {
	await api.delete(`/admin/invitations/${id}`);
	api.invalidate('/admin/invitations');
}

export async function register(data: {
	code: string;
	username: string;
	password: string;
}): Promise<{ success: boolean; token: string }> {
	return api.post('/auth/register', data);
}

// --- Conversions: reorder ---

export async function reorderConversions(jobIds: string[]): Promise<void> {
	await api.put<void>('/conversions/reorder', { job_ids: jobIds });
}
