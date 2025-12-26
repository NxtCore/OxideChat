// https://nuxt.com/docs/api/configuration/nuxt-config
import tailwindcss from '@tailwindcss/vite';

export default defineNuxtConfig({
	compatibilityDate: '2025-12-26',
	devtools: {enabled: true},
	modules: ['@pinia/nuxt', 'floating-vue/nuxt', '@nuxt/devtools', 'shadcn-nuxt', 'vue-sonner/nuxt', 'nuxt-charts'],
	plugins: ['plugins/fetch.ts'],
	vite: {
		server: {
			allowedHosts: ['.local'],
		},
		plugins: [tailwindcss()],
	},
	vueSonner: {
		css: true,
	},
	shadcn: {
		theme: 'dark',
		prefix: 'shad',
		componentDir: 'components/ui',
	},
	css: ['./assets/css/tailwind.css', 'vue-sonner/style.css'],
});
