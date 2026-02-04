import { api } from './client.js';
import type {
	Library,
	Item,
	Job,
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
	if (params.page !== undefined) searchParams.set('page', String(params.page));
	if (params.limit !== undefined) searchParams.set('limit', String(params.limit));
	if (params.search) searchParams.set('search', params.search);

	const query = searchParams.toString();
	const result = await api.get<Item[] | { items: Item[]; total: number }>(`/items${query ? `?${query}` : ''}`);
	// Backend returns a plain array; normalize to { items, total } shape
	const items = Array.isArray(result) ? result : result.items;
	return { items: items.map(normalizeItem), total: items.length };
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
	if (params?.page !== undefined) searchParams.set('page', String(params.page));
	if (params?.limit !== undefined) searchParams.set('limit', String(params.limit));

	const query = searchParams.toString();
	return api.get<{ jobs: Job[]; total: number }>(`/jobs${query ? `?${query}` : ''}`, {
		skipCache: true
	});
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
	return api.get<DashboardStats>('/dashboard', { skipCache: true });
}

// --- Tools ---

export async function getTools(): Promise<ToolInfo[]> {
	return api.get<ToolInfo[]>('/tools');
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
