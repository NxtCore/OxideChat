<template>
	<div v-if="!store.initialized" class="flex h-screen items-center justify-center">
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
		if (!store.initialized) {
			await store.getBaseData();
		}

		if (store.base?.needs_setup) {
			if (route.path !== '/auth/setup') {
				await router.push('/auth/setup');
				return {
					authenticated: false,
				};
			}
			return {
				authenticated: false,
			};
		}

		if (route.path === '/auth/setup') {
			await router.push('/auth/login');
			return {
				authenticated: false,
			};
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
			await router.push('/');
			return {
				authenticated: false,
			};
		}

		if (!store.auth.isAuthenticated && !isAuthPage) {
			await router.push('/auth/login');
			return {
				authenticated: false,
			};
		}
		if (!store.auth.isAuthenticated && isAuthPage) {
			return {
				authenticated: false,
			};
		}
		return {
			authenticated: true,
		};
	} catch (error) {
		console.error('Auth check error:', error);
		const isAuthPage = route.path.startsWith('/auth');
		if (!isAuthPage) {
			await router.push('/auth/login');
			return {
				authenticated: false,
			};
		}
		return {
			authenticated: false,
		};
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
