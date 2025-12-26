<template>
	<ShadSidebarProvider class="flex h-screen overflow-hidden w-full">
		<AppSidebar />
		<ShadSidebarInset> </ShadSidebarInset>
	</ShadSidebarProvider>
	<ShadToaster
		position="top-center"
		:visible-toasts="5"
		:toast-options="{
			descriptionClass: '!text-white',
		}"
	/>
</template>

<script async setup>
import {useHead} from 'nuxt/app';
import {ChevronRight} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {storeToRefs} from 'pinia';
import 'vue-sonner/style.css';

const {$customFetch} = useNuxtApp();
const route = useRoute();
const router = useRouter();

useHead({
	title: 'HenrikDev Systems',
	link: [{rel: 'icon', type: 'image/png', href: '/api_icon.png'}],
	bodyAttrs: {
		class: 'dark',
	},
});

const store = useMainStore();
await store.validateAuth($customFetch);

watch(
	() => route.path,
	async () => {},
	{immediate: true}
);

onMounted(() => {
	if (store.auth.need_login) {
		window.location = store.auth.login_url;
	}
});
</script>
