<script lang="ts">
  import '../app.css';
  import favicon from '$lib/assets/favicon.svg';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { Button } from '$lib/components/ui/button';
  import { theme } from '$lib/stores/theme';
  import { authStore } from '$lib/stores/auth';
  import { Toaster } from 'svelte-sonner';
  import {
    History,
    Settings,
    Moon,
    Sun,
    Menu,
    X,
    Film,
    Tv,
    LogOut,
    User,
    LayoutDashboard,
    FolderX
  } from 'lucide-svelte';
  import { cn } from '$lib/utils';
  import { connect, disconnect } from '$lib/services/events.svelte';
  import { getLibraries } from '$lib/api';
  import type { Library } from '$lib/types';

  let { children } = $props();

  let sidebarOpen = $state(true);
  let mobileMenuOpen = $state(false);
  let libraries = $state<Library[]>([]);

  // Connect to events and fetch libraries on mount, disconnect on destroy
  $effect(() => {
    connect();

    // Fetch libraries
    getLibraries().then((libs) => {
      libraries = libs;
    }).catch((err) => {
      console.error('Failed to fetch libraries:', err);
    });

    // Check auth status
    authStore.checkStatus();

    return () => {
      disconnect();
    };
  });

  // Redirect to login if auth required but not authenticated
  $effect(() => {
    const isLoginPage = $page.url.pathname === '/login';
    if ($authStore.initialized && $authStore.authEnabled && !$authStore.authenticated && !isLoginPage) {
      goto('/login');
    }
  });

  async function handleLogout() {
    await authStore.logout();
    goto('/login');
  }

  // Derive library nav items from libraries
  const libraryNav = $derived(
    libraries.map((lib) => ({
      href: `/browse/${lib.id}`,
      icon: lib.media_type === 'movies' ? Film : Tv,
      label: lib.name,
    }))
  );

  const adminNav = [
    { href: '/admin', icon: LayoutDashboard, label: 'Dashboard' },
    { href: '/history', icon: History, label: 'History' },
    { href: '/settings', icon: Settings, label: 'Settings' },
  ];

  function isActive(href: string, pathname: string): boolean {
    if (href === '/') return pathname === '/';
    return pathname.startsWith(href);
  }

  function toggleTheme() {
    theme.toggle();
  }

  // Close mobile menu when navigating
  $effect(() => {
    $page.url.pathname;
    mobileMenuOpen = false;
  });
</script>

<svelte:head>
  <title>Mediaforge</title>
  <link rel="icon" href={favicon} />
</svelte:head>

<Toaster richColors position="bottom-right" expand visibleToasts={9} />

<div class="flex h-screen bg-background">
  <!-- Sidebar - Desktop -->
  <aside
    class={cn(
      "hidden md:flex flex-col border-r bg-card transition-all duration-300",
      sidebarOpen ? "w-64" : "w-16"
    )}
  >
    <!-- Logo -->
    <div class="flex h-14 items-center border-b px-4">
      <Film class="h-6 w-6 text-primary" />
      {#if sidebarOpen}
        <span class="ml-2 font-semibold">Mediaforge</span>
      {/if}
    </div>

    <!-- Navigation -->
    <nav class="flex-1 space-y-1 p-2">
      <!-- Libraries Section -->
      {#if libraryNav.length > 0}
        {#each libraryNav as item}
          {@const active = isActive(item.href, $page.url.pathname)}
          <a
            href={item.href}
            class={cn(
              "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
              active
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
            )}
          >
            <item.icon class="h-5 w-5 flex-shrink-0" />
            {#if sidebarOpen}
              <span>{item.label}</span>
            {/if}
          </a>
        {/each}
      {:else}
        <a
          href="/settings"
          class={cn(
            "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
            "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
          )}
        >
          <FolderX class="h-5 w-5 flex-shrink-0" />
          {#if sidebarOpen}
            <span>No Libraries</span>
          {/if}
        </a>
      {/if}

      <!-- Separator -->
      <div class="py-2">
        <div class="border-t border-border"></div>
      </div>

      <!-- Admin Section -->
      {#each adminNav as item}
        {@const active = isActive(item.href, $page.url.pathname)}
        <a
          href={item.href}
          class={cn(
            "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
            active
              ? "bg-primary text-primary-foreground"
              : "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
          )}
        >
          <item.icon class="h-5 w-5 flex-shrink-0" />
          {#if sidebarOpen}
            <span>{item.label}</span>
          {/if}
        </a>
      {/each}
    </nav>

    <!-- Footer -->
    <div class="border-t p-2">
      {#if $authStore.authEnabled && $authStore.authenticated && sidebarOpen}
        <div class="flex items-center gap-2 px-3 py-2 mb-2 text-sm text-muted-foreground">
          <User class="h-4 w-4" />
          <span class="truncate">{$authStore.username}</span>
        </div>
      {/if}
      <div class="flex items-center justify-between">
        {#if sidebarOpen}
          <div class="flex items-center gap-1">
            <Button variant="ghost" size="icon" onclick={toggleTheme}>
              {#if $theme === 'dark'}
                <Sun class="h-5 w-5" />
              {:else}
                <Moon class="h-5 w-5" />
              {/if}
            </Button>
            {#if $authStore.authEnabled && $authStore.authenticated}
              <Button variant="ghost" size="icon" onclick={handleLogout} title="Logout">
                <LogOut class="h-5 w-5" />
              </Button>
            {/if}
          </div>
        {/if}
        <Button
          variant="ghost"
          size="icon"
          onclick={() => sidebarOpen = !sidebarOpen}
        >
          <Menu class="h-5 w-5" />
        </Button>
      </div>
    </div>
  </aside>

  <!-- Mobile Header & Menu -->
  <div class="md:hidden fixed top-0 left-0 right-0 z-50">
    <header class="flex h-14 items-center justify-between border-b bg-card px-4">
      <div class="flex items-center gap-2">
        <Film class="h-6 w-6 text-primary" />
        <span class="font-semibold">Mediaforge</span>
      </div>
      <div class="flex items-center gap-2">
        <Button variant="ghost" size="icon" onclick={toggleTheme}>
          {#if $theme === 'dark'}
            <Sun class="h-5 w-5" />
          {:else}
            <Moon class="h-5 w-5" />
          {/if}
        </Button>
        {#if $authStore.authEnabled && $authStore.authenticated}
          <Button variant="ghost" size="icon" onclick={handleLogout} title="Logout">
            <LogOut class="h-5 w-5" />
          </Button>
        {/if}
        <Button variant="ghost" size="icon" onclick={() => mobileMenuOpen = !mobileMenuOpen}>
          {#if mobileMenuOpen}
            <X class="h-5 w-5" />
          {:else}
            <Menu class="h-5 w-5" />
          {/if}
        </Button>
      </div>
    </header>

    {#if mobileMenuOpen}
      <nav class="border-b bg-card p-4 shadow-lg">
        <!-- Libraries Section -->
        {#if libraryNav.length > 0}
          {#each libraryNav as item}
            {@const active = isActive(item.href, $page.url.pathname)}
            <a
              href={item.href}
              class={cn(
                "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                active
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
              )}
            >
              <item.icon class="h-5 w-5" />
              <span>{item.label}</span>
            </a>
          {/each}
        {:else}
          <a
            href="/settings"
            class={cn(
              "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
              "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
            )}
          >
            <FolderX class="h-5 w-5" />
            <span>No Libraries</span>
          </a>
        {/if}

        <!-- Separator -->
        <div class="py-2">
          <div class="border-t border-border"></div>
        </div>

        <!-- Admin Section -->
        {#each adminNav as item}
          {@const active = isActive(item.href, $page.url.pathname)}
          <a
            href={item.href}
            class={cn(
              "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
              active
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
            )}
          >
            <item.icon class="h-5 w-5" />
            <span>{item.label}</span>
          </a>
        {/each}
      </nav>
    {/if}
  </div>

  <!-- Main Content -->
  <main class="flex-1 overflow-auto md:pt-0 pt-14">
    <div class="container mx-auto p-6">
      {@render children()}
    </div>
  </main>
</div>
