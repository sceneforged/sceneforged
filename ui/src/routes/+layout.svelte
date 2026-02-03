<script lang="ts">
	import '../app.css';
	import { cn } from '$lib/utils';
	import {
		Home,
		Library,
		ScrollText,
		Settings,
		Shield,
		Menu,
		X
	} from 'lucide-svelte';

	let { children } = $props();

	let sidebarOpen = $state(false);

	const navItems = [
		{ href: '/', label: 'Home', icon: Home },
		{ href: '/browse/default', label: 'Browse', icon: Library },
		{ href: '/rules', label: 'Rules', icon: ScrollText },
		{ href: '/settings', label: 'Settings', icon: Settings },
		{ href: '/admin', label: 'Admin', icon: Shield }
	];
</script>

<div class="flex h-screen overflow-hidden">
	<!-- Mobile overlay -->
	{#if sidebarOpen}
		<button
			class="fixed inset-0 z-30 bg-black/50 md:hidden"
			onclick={() => (sidebarOpen = false)}
			aria-label="Close sidebar"
		></button>
	{/if}

	<!-- Sidebar -->
	<aside
		class={cn(
			'fixed inset-y-0 left-0 z-40 flex w-64 flex-col border-r border-border bg-card transition-transform duration-200 md:relative md:translate-x-0',
			sidebarOpen ? 'translate-x-0' : '-translate-x-full'
		)}
	>
		<div class="flex h-14 items-center border-b border-border px-4">
			<a href="/" class="text-lg font-semibold text-foreground">SceneForged</a>
			<button
				class="ml-auto md:hidden"
				onclick={() => (sidebarOpen = false)}
				aria-label="Close sidebar"
			>
				<X class="h-5 w-5" />
			</button>
		</div>
		<nav class="flex-1 space-y-1 p-2">
			{#each navItems as item}
				<a
					href={item.href}
					class="flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-foreground/70 transition-colors hover:bg-accent hover:text-foreground"
				>
					<item.icon class="h-4 w-4" />
					{item.label}
				</a>
			{/each}
		</nav>
	</aside>

	<!-- Main content -->
	<div class="flex flex-1 flex-col overflow-hidden">
		<!-- Mobile header -->
		<header class="flex h-14 items-center border-b border-border px-4 md:hidden">
			<button onclick={() => (sidebarOpen = true)} aria-label="Open sidebar">
				<Menu class="h-5 w-5" />
			</button>
			<span class="ml-3 text-lg font-semibold">SceneForged</span>
		</header>

		<!-- Page content -->
		<main class="flex-1 overflow-y-auto p-4 md:p-6">
			{@render children()}
		</main>
	</div>
</div>
