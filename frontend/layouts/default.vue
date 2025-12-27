<template>
	<ShadSidebarProvider class="flex h-screen overflow-hidden w-full">
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
import 'vue-sonner/style.css';

useHead({
	title: 'OxideChat',
	link: [{rel: 'icon', type: 'image/png', href: '/favicon.png'}],
	bodyAttrs: {
		class: 'dark',
	},
});

const store = useMainStore();
</script>
