<template>
	<!-- Boot screen: server loading, auth resolving, or chat data not yet ready -->
	<BootScreen v-if="store.bootState !== 'online' || store.auth.loading || (store.auth.isAuthenticated && !chatStore.initialized)" />
	<!-- Auth pages: server up, session confirmed as unauthenticated -->
	<div v-else-if="!store.auth.isAuthenticated">
		<slot />
	</div>
	<!-- Full app: everything ready -->
	<ShadSidebarProvider v-else class="flex h-screen overflow-hidden w-full">
		<AppSidebar />
		<ShadSidebarInset class="flex-1 flex flex-col min-w-0">
			<header class="flex items-center gap-2 px-3 py-2 border-b border-border md:hidden shrink-0">
				<ShadSidebarTrigger class="h-8 w-8" />
				<span class="font-semibold text-sm">OxideChat</span>
			</header>
			<div class="flex-1 min-h-0">
				<slot />
			</div>
		</ShadSidebarInset>
	</ShadSidebarProvider>
	<ShadToaster
		position="top-center"
		:visible-toasts="5"
		:toast-options="{
			descriptionClass: '!text-white',
		}"
	/>
</template>

<script setup lang="ts">
import {useHead} from 'nuxt/app';
import {useMainStore} from '@/stores';
import {useChatStore} from '@/stores/chatStore';
import 'vue-sonner/style.css';

useHead({
	title: 'OxideChat',
	link: [{rel: 'icon', type: 'image/svg+xml', href: '/light_transparent.svg'}],
	bodyAttrs: {
		class: 'dark',
	},
});

const store = useMainStore();
const chatStore = useChatStore();
const router = useRouter();
const route = useRoute();

let getMePromise: Promise<void> | null = null;

const checkAuth = async () => {
	try {
		if (store.bootState !== 'online') {
			await store.getBaseData();
		}
		
		if (!store.auth.user && !store.auth.isAuthenticated) {
			if (!getMePromise) {
				getMePromise = store.getMe().finally(() => {
					getMePromise = null;
				});
			}
			await getMePromise;
		}

		if (store.base?.needs_setup) {
			if (route.path !== '/auth/setup') {
				await router.push('/auth/setup');
				return {authenticated: false};
			}
			return {authenticated: false};
		}

		if (route.path === '/auth/setup') {
			await router.push('/auth/login');
			return {authenticated: false};
		}

		const isAuthPage = route.path.startsWith('/auth');

		if (store.auth.isAuthenticated && isAuthPage) {
			await router.push('/');
			return {authenticated: false};
		}

		if (!store.auth.isAuthenticated && !isAuthPage) {
			await router.push('/auth/login');
			return {authenticated: false};
		}
		if (!store.auth.isAuthenticated && isAuthPage) {
			return {authenticated: false};
		}
		return {authenticated: true};
	} catch (error) {
		console.error('Auth check error:', error);
		if (isServerUnreachable(error)) {
			// Server is down — keep boot screen, do not redirect to login
			return {authenticated: false};
		}
		const isAuthPage = route.path.startsWith('/auth');
		if (!isAuthPage) {
			await router.push('/auth/login');
		}
		return {authenticated: false};
	}
};

watch(
	() => route.path,
	async () => {
		await checkAuth();
	}
);

onMounted(async () => {
	const authResult = await checkAuth();
	if (!authResult?.authenticated) return;
	await chatStore.init();
});
</script>
