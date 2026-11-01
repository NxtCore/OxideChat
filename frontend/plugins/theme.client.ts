import { useThemeStore } from '~/stores/theme';

export default defineNuxtPlugin(() => {
	const themeStore = useThemeStore();

	themeStore.apply();

	window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => themeStore.followSystemMode());
});
