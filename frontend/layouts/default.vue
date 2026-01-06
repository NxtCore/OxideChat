<template>
	<div v-if="!store.initialized" class="flex h-screen items-center justify-center">
		<!-- Simple loading state -->
		<svg
			xmlns="http://www.w3.org/2000/svg"
			width="24"
			height="24"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			stroke-linejoin="round"
			class="animate-spin"
		>
			<path d="M21 12a9 9 0 1 1-6.219-8.56" />
		</svg>
	</div>
	<ShadSidebarProvider v-else class="flex h-screen overflow-hidden w-full">
		<AppSidebar v-if="store.auth.isAuthenticated" />
		<ShadSidebarInset class="flex-1">
			<slot />
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
	link: [{rel: 'icon', type: 'image/png', href: '/favicon.png'}],
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
		if (!store.initialized) {
			await store.getBaseData();
		}

		if (store.base?.needs_setup) {
			if (route.path !== '/auth/setup') {
				return router.push('/auth/setup');
			}
			return;
		}

		if (route.path === '/auth/setup') {
			return router.push('/auth/login');
		}

		if (!store.auth.user && !store.auth.isAuthenticated) {
			if (!getMePromise) {
				getMePromise = store.getMe().finally(() => {
					getMePromise = null;
				});
			}
			await getMePromise;
		}

		const isAuthPage = route.path.startsWith('/auth');

		if (store.auth.isAuthenticated && isAuthPage) {
			return router.push('/');
		}

		if (!store.auth.isAuthenticated && !isAuthPage) {
			return router.push('/auth/login');
		}
	} catch (error) {
		console.error('Auth check error:', error);
		const isAuthPage = route.path.startsWith('/auth');
		if (!isAuthPage) {
			return router.push('/auth/login');
		}
	}
};

watch(
	() => route.path,
	async () => {
		await checkAuth();
	}
);

onMounted(async () => {
	await checkAuth();
	await chatStore.init();
});
</script>
