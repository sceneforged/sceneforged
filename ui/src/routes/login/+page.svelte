<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Film, Loader2, AlertCircle } from 'lucide-svelte';
  import { login, getAuthStatus } from '$lib/api';

  let username = $state('');
  let password = $state('');
  let loading = $state(false);
  let error = $state<string | null>(null);
  let checkingAuth = $state(true);

  onMount(async () => {
    try {
      const status = await getAuthStatus();
      // If auth is disabled or already authenticated, redirect to home
      if (!status.auth_enabled || status.authenticated) {
        goto('/');
      }
    } catch (e) {
      // Auth check failed, assume we need to login
    } finally {
      checkingAuth = false;
    }
  });

  async function handleSubmit(event: Event) {
    event.preventDefault();
    if (!username.trim() || !password) {
      error = 'Please enter username and password';
      return;
    }

    loading = true;
    error = null;

    try {
      const result = await login(username.trim(), password);
      if (result.success) {
        goto('/');
      } else {
        error = result.message || 'Login failed';
      }
    } catch (e) {
      if (e instanceof Error) {
        // Try to parse JSON error message
        try {
          const parsed = JSON.parse(e.message);
          error = parsed.message || 'Login failed';
        } catch {
          error = e.message;
        }
      } else {
        error = 'Login failed';
      }
    } finally {
      loading = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !loading) {
      handleSubmit(new Event('submit'));
    }
  }
</script>

<div class="min-h-screen flex items-center justify-center p-4">
  {#if checkingAuth}
    <div class="text-center">
      <Loader2 class="h-8 w-8 mx-auto animate-spin text-primary" />
    </div>
  {:else}
    <Card class="w-full max-w-md">
      <CardHeader class="text-center">
        {#snippet children()}
          <div class="flex items-center justify-center gap-2 mb-2">
            <Film class="h-8 w-8 text-primary" />
            <span class="text-2xl font-bold">Sceneforged</span>
          </div>
          <CardTitle>Sign In</CardTitle>
          <CardDescription>Enter your credentials to access the dashboard</CardDescription>
        {/snippet}
      </CardHeader>
      <CardContent>
        {#snippet children()}
          <form onsubmit={handleSubmit} class="space-y-4">
            <div class="space-y-2">
              <label for="username" class="text-sm font-medium">Username</label>
              <Input
                id="username"
                type="text"
                placeholder="admin"
                bind:value={username}
                onkeydown={handleKeydown}
                disabled={loading}
                autocomplete="username"
              />
            </div>

            <div class="space-y-2">
              <label for="password" class="text-sm font-medium">Password</label>
              <Input
                id="password"
                type="password"
                placeholder="Enter your password"
                bind:value={password}
                onkeydown={handleKeydown}
                disabled={loading}
                autocomplete="current-password"
              />
            </div>

            {#if error}
              <div class="flex items-center gap-2 text-sm text-destructive">
                <AlertCircle class="h-4 w-4" />
                <span>{error}</span>
              </div>
            {/if}

            <Button type="submit" class="w-full" disabled={loading}>
              {#if loading}
                <Loader2 class="h-4 w-4 mr-2 animate-spin" />
                Signing in...
              {:else}
                Sign In
              {/if}
            </Button>
          </form>
        {/snippet}
      </CardContent>
    </Card>
  {/if}
</div>
