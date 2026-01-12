// https://nuxt.com/docs/api/configuration/nuxt-config
import tailwindcss from '@tailwindcss/vite';

export default defineNuxtConfig({
	compatibilityDate: '2025-12-26',
	devtools: {enabled: true},
	modules: ['@pinia/nuxt', 'floating-vue/nuxt', '@nuxt/devtools', 'shadcn-nuxt', 'vue-sonner/nuxt', 'nuxt-charts', 'nuxt-shiki'],
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
		prefix: 'shad',
		componentDir: 'components/ui',
	},
	shiki: {
		bundledThemes: ['github-dark', 'github-light'],
		bundledLangs: [
			'javascript',
			'typescript',
			'python',
			'rust',
			'html',
			'css',
			'json',
			'markdown',
			'bash',
			'sql',
			'vue',
			'jsx',
			'tsx',
			'yaml',
			'toml',
			'shell',
			'go',
			'java',
			'c',
			'cpp',
			'csharp',
		],
		defaultTheme: 'github-dark',
	},
	css: ['./assets/css/tailwind.css', 'vue-sonner/style.css'],
});
