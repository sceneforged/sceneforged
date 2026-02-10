<script lang="ts">
	import { onMount } from 'svelte';
	import '../app.css';
	import * as Sidebar from '$lib/components/ui/sidebar/index.js';
	import AppSidebar from '$lib/components/AppSidebar.svelte';
	import { authStore } from '$lib/stores/auth.svelte.js';

	let { children } = $props();

	onMount(() => {
		authStore.checkStatus();
	});
</script>

<Sidebar.Provider>
	<AppSidebar />
	<Sidebar.Inset>
		<!-- Mobile header with sidebar trigger -->
		<header class="flex h-14 items-center gap-2 border-b px-4 md:hidden">
			<Sidebar.Trigger />
			<span class="text-lg font-semibold">SceneForged</span>
		</header>

		<!-- Page content -->
		<main class="flex-1 overflow-y-auto p-4 md:p-6">
			{@render children()}
		</main>
	</Sidebar.Inset>
</Sidebar.Provider>
