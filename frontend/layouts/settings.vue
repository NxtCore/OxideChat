<template>
	<BootScreen v-if="store.bootState !== 'online'" />
	<div v-else class="container mx-auto flex max-w-12xl flex-1 flex-col p-3 pb-6 lg:max-h-dvh lg:overflow-y-hidden lg:p-6">
		<slot />
	</div>
	<ShadToaster
		position="top-center"
		:visible-toasts="5"
		:toast-options="{
			descriptionClass: '!text-foreground/80',
			class: 'bg-background border-border text-foreground',
		}"
	/>
</template>

<script setup lang="ts">
import {useHead} from 'nuxt/app';
import {useMainStore} from '@/stores';
import 'vue-sonner/style.css';

useHead({
	title: 'OxideChat - Settings',
	link: [{rel: 'icon', type: 'image/svg+xml', href: '/light_transparent.svg'}],
	bodyAttrs: {
		class: 'dark', // Force dark mode for now as per preference
	},
});

const store = useMainStore();
const router = useRouter();
const route = useRoute();

let getMePromise: Promise<void> | null = null;

const checkAuth = async () => {
	try {
		if (store.bootState !== 'online') {
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
});
</script>
