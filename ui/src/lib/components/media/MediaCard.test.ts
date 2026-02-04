import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import MediaCard from './MediaCard.svelte';
import type { Item } from '$lib/types';

vi.mock('$app/navigation', () => ({
	goto: vi.fn()
}));

function createTestItem(overrides: Partial<Item> = {}): Item {
	return {
		id: 'test-1',
		library_id: 'lib-1',
		item_kind: 'movie',
		name: 'Test Movie',
		year: 2024,
		overview: 'A test movie',
		runtime_minutes: 120,
		community_rating: 8.0,
		images: [],
		media_files: [],
		created_at: new Date().toISOString(),
		updated_at: new Date().toISOString(),
		...overrides
	};
}

describe('MediaCard', () => {
	it('renders item name', () => {
		const item = createTestItem({ name: 'My Movie' });
		render(MediaCard, { props: { item } });
		expect(screen.getByText('My Movie')).toBeDefined();
	});

	it('renders year when available', () => {
		const item = createTestItem({ year: 2023 });
		render(MediaCard, { props: { item } });
		expect(screen.getByText('2023')).toBeDefined();
	});

	it('renders runtime when available', () => {
		const item = createTestItem({ runtime_minutes: 150 });
		render(MediaCard, { props: { item } });
		expect(screen.getByText('2h 30m')).toBeDefined();
	});

	it('renders rating when available', () => {
		const item = createTestItem({ community_rating: 7.5 });
		render(MediaCard, { props: { item } });
		expect(screen.getByText('7.5')).toBeDefined();
	});

	it('handles item with no media files', () => {
		const item = createTestItem({ media_files: [] });
		render(MediaCard, { props: { item } });
		expect(screen.getByText('Test Movie')).toBeDefined();
	});

	it('handles item with web-playable file', () => {
		const item = createTestItem({
			media_files: [
				{
					id: 'mf-1',
					item_id: 'test-1',
					file_path: '/test.mp4',
					file_name: 'test.mp4',
					file_size: 1000,
					has_dolby_vision: false,
					role: 'universal',
					profile: 'B',
					created_at: new Date().toISOString()
				}
			]
		});
		render(MediaCard, { props: { item } });
		expect(screen.getByText('Test Movie')).toBeDefined();
	});
});
