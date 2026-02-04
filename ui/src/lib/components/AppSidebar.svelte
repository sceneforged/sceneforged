<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import {
		Home,
		Film,
		Tv,
		Music,
		FolderOpen,
		LayoutDashboard,
		Library,
		Briefcase,
		Settings,
		Sun,
		Moon,
		LogOut
	} from '@lucide/svelte';
	import * as Sidebar from '$lib/components/ui/sidebar/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { libraryStore } from '$lib/stores/library.svelte.js';
	import { themeStore } from '$lib/stores/theme.svelte.js';
	import { authStore } from '$lib/stores/auth.svelte.js';
	import { eventsService } from '$lib/services/events.svelte.js';

	const isDark = $derived(themeStore.current === 'dark');

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	function getMediaTypeIcon(type: string): any {
		switch (type) {
			case 'movies':
				return Film;
			case 'tvshows':
				return Tv;
			case 'music':
				return Music;
			default:
				return FolderOpen;
		}
	}

	function isActive(href: string, exact = false): boolean {
		return exact ? page.url.pathname === href : page.url.pathname.startsWith(href);
	}

	let unsubscribe: (() => void) | null = null;

	onMount(() => {
		libraryStore.loadLibraries();
		authStore.checkStatus();

		unsubscribe = eventsService.subscribe('all', (event) => {
			const { payload } = event;
			if (
				payload.type === 'library_created' ||
				payload.type === 'library_deleted' ||
				payload.type === 'library_scan_complete'
			) {
				libraryStore.loadLibraries();
			}
		});
	});

	onDestroy(() => {
		unsubscribe?.();
	});

	async function handleLogout() {
		await authStore.logout();
		goto('/login');
	}
</script>

<Sidebar.Sidebar>
	<!-- Header -->
	<Sidebar.Header class="flex flex-row items-center justify-between border-b border-sidebar-border px-4 py-3">
		<a href="/" class="text-lg font-semibold text-sidebar-foreground">SceneForged</a>
		<div class="flex items-center gap-1">
			<Button
				variant="ghost"
				size="icon"
				class="size-7 text-sidebar-foreground/70 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
				onclick={() => themeStore.toggle()}
				aria-label="Toggle theme"
			>
				{#if isDark}
					<Sun class="h-4 w-4" />
				{:else}
					<Moon class="h-4 w-4" />
				{/if}
			</Button>
			{#if authStore.authenticated}
				<Button
					variant="ghost"
					size="icon"
					class="size-7 text-sidebar-foreground/70 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
					onclick={handleLogout}
					aria-label="Logout"
				>
					<LogOut class="h-4 w-4" />
				</Button>
			{/if}
		</div>
	</Sidebar.Header>

	<!-- Navigation -->
	<Sidebar.Content>
		<!-- Home -->
		<Sidebar.Group>
			<Sidebar.Menu>
				<Sidebar.MenuItem>
					<Sidebar.MenuButton isActive={isActive('/', true)} tooltipContent="Home">
						{#snippet child({ props })}
							<a href="/" {...props}>
								<Home class="h-4 w-4" />
								<span>Home</span>
							</a>
						{/snippet}
					</Sidebar.MenuButton>
				</Sidebar.MenuItem>
			</Sidebar.Menu>
		</Sidebar.Group>

		<Sidebar.Separator />

		<!-- Libraries -->
		<Sidebar.Group>
			<Sidebar.GroupLabel>Libraries</Sidebar.GroupLabel>
			<Sidebar.Menu>
				{#if libraryStore.libraries.length === 0}
					<Sidebar.MenuItem>
						<span class="flex items-center gap-2 px-2 py-1.5 text-sm text-sidebar-foreground/50">
							No Libraries
						</span>
					</Sidebar.MenuItem>
				{:else}
					{#each libraryStore.libraries as lib (lib.id)}
						{@const Icon = getMediaTypeIcon(lib.media_type)}
						<Sidebar.MenuItem>
							<Sidebar.MenuButton isActive={isActive(`/browse/${lib.id}`)} tooltipContent={lib.name}>
								{#snippet child({ props })}
									<a href="/browse/{lib.id}" {...props}>
										<Icon class="h-4 w-4" />
										<span>{lib.name}</span>
									</a>
								{/snippet}
							</Sidebar.MenuButton>
						</Sidebar.MenuItem>
					{/each}
				{/if}
			</Sidebar.Menu>
		</Sidebar.Group>

		<Sidebar.Separator />

		<!-- Admin -->
		<Sidebar.Group>
			<Sidebar.GroupLabel>Admin</Sidebar.GroupLabel>
			<Sidebar.Menu>
				<Sidebar.MenuItem>
					<Sidebar.MenuButton isActive={isActive('/admin', true)} tooltipContent="Dashboard">
						{#snippet child({ props })}
							<a href="/admin" {...props}>
								<LayoutDashboard class="h-4 w-4" />
								<span>Dashboard</span>
							</a>
						{/snippet}
					</Sidebar.MenuButton>
				</Sidebar.MenuItem>
				<Sidebar.MenuItem>
					<Sidebar.MenuButton isActive={isActive('/admin/libraries')} tooltipContent="Libraries">
						{#snippet child({ props })}
							<a href="/admin/libraries" {...props}>
								<Library class="h-4 w-4" />
								<span>Libraries</span>
							</a>
						{/snippet}
					</Sidebar.MenuButton>
				</Sidebar.MenuItem>
				<Sidebar.MenuItem>
					<Sidebar.MenuButton isActive={isActive('/admin/jobs')} tooltipContent="Jobs">
						{#snippet child({ props })}
							<a href="/admin/jobs" {...props}>
								<Briefcase class="h-4 w-4" />
								<span>Jobs</span>
							</a>
						{/snippet}
					</Sidebar.MenuButton>
				</Sidebar.MenuItem>
				<Sidebar.MenuItem>
					<Sidebar.MenuButton isActive={isActive('/settings')} tooltipContent="Settings">
						{#snippet child({ props })}
							<a href="/settings" {...props}>
								<Settings class="h-4 w-4" />
								<span>Settings</span>
							</a>
						{/snippet}
					</Sidebar.MenuButton>
				</Sidebar.MenuItem>
			</Sidebar.Menu>
		</Sidebar.Group>
	</Sidebar.Content>

	<Sidebar.Footer class="border-t border-sidebar-border px-4 py-2">
		<span class="font-mono text-xs text-sidebar-foreground/40">{__GIT_SHA__}</span>
	</Sidebar.Footer>
</Sidebar.Sidebar>
