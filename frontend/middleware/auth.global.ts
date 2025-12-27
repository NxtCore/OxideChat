/**
 * Global auth middleware
 *
 * Handles authentication flow:
 * 1. Fetches base data to check if setup is needed
 * 2. Redirects to /auth/setup if no users exist
 * 3. Redirects to /auth/login if not authenticated (except auth pages)
 * 4. Redirects away from auth pages if already authenticated
 */
export default defineNuxtRouteMiddleware(async to => {
	const store = useMainStore();
	const {$customFetch} = useNuxtApp();

	// Skip middleware for initial loading
	if (process.server) {
		return;
	}

	// Check if we're on an auth page
	const isAuthPage = to.path.startsWith('/auth');

	try {
		// Fetch base data if not initialized
		if (!store.initialized) {
			await store.getBaseData();
		}

		// Check if setup is needed
		if (store.base?.needs_setup) {
			if (to.path !== '/auth/setup') {
				return navigateTo('/auth/setup');
			}
			return;
		}

		// If on setup page but setup is complete, redirect to login
		if (to.path === '/auth/setup') {
			return navigateTo('/auth/login');
		}

		// Try to get current user if not already fetched
		if (!store.auth.user && !store.auth.loading) {
			try {
				await store.getMe();
			} catch {
				// Not authenticated, handled below
			}
		}

		// If authenticated and on auth page, redirect to home
		if (store.auth.isAuthenticated && isAuthPage) {
			return navigateTo('/');
		}

		// If not authenticated and not on auth page, redirect to login
		if (!store.auth.isAuthenticated && !isAuthPage) {
			return navigateTo('/auth/login');
		}
	} catch (error) {
		console.error('Auth middleware error:', error);
		// On error, allow navigation to auth pages
		if (!isAuthPage) {
			return navigateTo('/auth/login');
		}
	}
});
