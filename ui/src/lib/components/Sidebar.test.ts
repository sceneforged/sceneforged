import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import AppSidebar from './AppSidebar.svelte';

// Mock the stores and services
vi.mock('$lib/stores/library.svelte.js', () => ({
	libraryStore: {
		libraries: [],
		loadLibraries: vi.fn()
	}
}));

vi.mock('$lib/stores/theme.svelte.js', () => ({
	themeStore: {
		current: 'light',
		toggle: vi.fn()
	}
}));

vi.mock('$lib/stores/auth.svelte.js', () => ({
	authStore: {
		authenticated: true,
		logout: vi.fn(),
		checkStatus: vi.fn()
	}
}));

vi.mock('$lib/services/events.svelte.js', () => ({
	eventsService: {
		subscribe: vi.fn(() => vi.fn())
	}
}));

vi.mock('$app/navigation', () => ({
	goto: vi.fn()
}));

vi.mock('$app/state', () => ({
	page: {
		url: { pathname: '/' }
	}
}));

// Mock the sidebar context since AppSidebar is used inside a Provider
vi.mock('$lib/components/ui/sidebar/context.svelte.js', () => ({
	useSidebar: () => ({
		state: 'expanded',
		open: true,
		isMobile: false,
		openMobile: false,
		setOpen: vi.fn(),
		setOpenMobile: vi.fn(),
		toggle: vi.fn(),
		handleShortcutKeydown: vi.fn()
	}),
	setSidebar: vi.fn(() => ({
		state: 'expanded',
		open: true,
		isMobile: false,
		openMobile: false,
		setOpen: vi.fn(),
		setOpenMobile: vi.fn(),
		toggle: vi.fn(),
		handleShortcutKeydown: vi.fn()
	}))
}));

// Mock the tooltip index module to avoid runed context issues in tests.
// The tooltip module is used by SidebarMenuButton; in tests we don't need real tooltips.
vi.mock('$lib/components/ui/tooltip/index.js', async () => {
	return await import('../../test-mocks/tooltip-index.js');
});

describe('AppSidebar', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it('renders the SceneForged brand link', () => {
		render(AppSidebar);
		expect(screen.getByText('SceneForged')).toBeDefined();
	});

	it('renders Home link', () => {
		render(AppSidebar);
		expect(screen.getByText('Home')).toBeDefined();
	});

	it('renders admin section links', () => {
		render(AppSidebar);
		expect(screen.getByText('Dashboard')).toBeDefined();
		expect(screen.getByText('Jobs')).toBeDefined();
		expect(screen.getByText('Settings')).toBeDefined();
	});

	it('shows "No Libraries" when library list is empty', () => {
		render(AppSidebar);
		expect(screen.getByText('No Libraries')).toBeDefined();
	});

	it('renders theme toggle button', () => {
		render(AppSidebar);
		expect(screen.getByLabelText('Toggle theme')).toBeDefined();
	});

	it('renders logout button when authenticated', () => {
		render(AppSidebar);
		expect(screen.getByLabelText('Logout')).toBeDefined();
	});

	it('renders section headings', () => {
		render(AppSidebar);
		// "Libraries" appears both as a group label and a nav link — check the group label specifically
		const librariesLabel = screen.getAllByText('Libraries').find(
			(el) => el.dataset.sidebar === 'group-label'
		);
		expect(librariesLabel).toBeDefined();
		expect(screen.getByText('Admin')).toBeDefined();
	});
});
