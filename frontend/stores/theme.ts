import {defineStore} from 'pinia';
import type {ThemeCssVars, FetchedTheme, GlobalConfig} from '~/types/chat';
import {applyThemeToElement, applyAccentToElement, clearAccentFromElement, clearThemeFromElement, getSystemTheme, type ThemeMode, type ThemeState} from '~/lib/apply-theme';
import {fetchThemeFromUrl, THEME_URLS} from '~/lib/theme-utils';

const THEME_STORE_KEY = 'oxide-theme-store';

interface PersistedThemeState {
	currentMode: ThemeMode;
	cssVars: ThemeCssVars;
	selectedThemeUrl: string | null;
}

function getDefaultCssVars(): ThemeCssVars {
	return {
		theme: {},
		light: {},
		dark: {},
	};
}

function loadPersistedState(): PersistedThemeState | null {
	if (typeof window === 'undefined') return null;
	try {
		const stored = localStorage.getItem(THEME_STORE_KEY);
		if (stored) {
			return JSON.parse(stored);
		}
	} catch {}
	return null;
}

function persistState(state: PersistedThemeState) {
	if (typeof window === 'undefined') return;
	try {
		localStorage.setItem(THEME_STORE_KEY, JSON.stringify(state));
	} catch {}
	return;
}

export const useThemeStore = defineStore('theme', {
	state: () => {
		const persisted = loadPersistedState();
		return {
			currentMode: persisted?.currentMode ?? getSystemTheme(),
			cssVars: persisted?.cssVars ?? getDefaultCssVars(),
			selectedThemeUrl: persisted?.selectedThemeUrl ?? null,
			globalConfig: null as GlobalConfig | null,
			fetchedThemes: [] as FetchedTheme[],
			customThemeUrls: [] as string[],
			isLoadingThemes: false,
			workspaceAccent: null as string | null,
		};
	},

	getters: {
		themeState(): ThemeState {
			return {
				currentMode: this.currentMode,
				cssVars: this.cssVars,
			};
		},
		allThemeUrls(): string[] {
			const urlSet = new Set<string>(THEME_URLS);
			this.customThemeUrls.forEach(url => urlSet.add(url));
			return Array.from(urlSet);
		},
		builtInThemes(): FetchedTheme[] {
			return this.fetchedThemes.filter(t => t.type === 'built-in');
		},
		customThemes(): FetchedTheme[] {
			return this.fetchedThemes.filter(t => t.type === 'custom');
		},
		hasCustomTheme(): boolean {
			return Object.keys(this.cssVars.light).length > 0 || Object.keys(this.cssVars.dark).length > 0;
		},
	},

	actions: {
		setThemeState(state: {currentMode?: ThemeMode; cssVars?: ThemeCssVars}) {
			if (state.currentMode !== undefined) {
				this.currentMode = state.currentMode;
			}
			if (state.cssVars !== undefined) {
				this.cssVars = state.cssVars;
			}
			this.persist();
			this.apply();
		},

		toggleMode() {
			this.currentMode = this.currentMode === 'dark' ? 'light' : 'dark';
			this.persist();
			this.apply();
		},

		setMode(mode: ThemeMode) {
			this.currentMode = mode;
			this.persist();
			this.apply();
		},

		applyThemePreset(cssVars: ThemeCssVars, url?: string) {
			this.cssVars = cssVars;
			if (url) {
				this.selectedThemeUrl = url;
			}
			this.persist();
			this.apply();
		},

		resetTheme() {
			if (typeof document !== 'undefined') {
				clearThemeFromElement(document.body);
			}
			this.cssVars = getDefaultCssVars();
			this.selectedThemeUrl = null;
			this.persist();
			this.apply();
		},

		setWorkspaceAccent(color: string | null) {
			this.workspaceAccent = color;
			this.apply();
		},

		apply() {
			if (typeof document === 'undefined') return;
			clearAccentFromElement(document.body);
			applyThemeToElement(this.themeState, document.body);
			applyAccentToElement(this.workspaceAccent, document.body);
		},

		persist() {
			persistState({
				currentMode: this.currentMode,
				cssVars: this.cssVars,
				selectedThemeUrl: this.selectedThemeUrl,
			});
		},

		async loadGlobalConfig() {
			try {
				const {$customFetch} = useNuxtApp();
				const config = (await $customFetch('/api/v1/config')) as GlobalConfig;
				this.globalConfig = config;

				if (!this.hasCustomTheme && config.default_theme) {
					const hasVars = Object.keys(config.default_theme.light).length > 0 || Object.keys(config.default_theme.dark).length > 0;
					if (hasVars) {
						this.cssVars = config.default_theme;
						this.apply();
					}
				}
			} catch (e) {
				console.error('Failed to load global config:', e);
			}
		},

		async fetchAllThemes() {
			this.isLoadingThemes = true;
			try {
				const themes = await Promise.all(this.allThemeUrls.map(fetchThemeFromUrl));
				this.fetchedThemes = themes;
			} catch (e) {
				console.error('Failed to fetch themes:', e);
			} finally {
				this.isLoadingThemes = false;
			}
		},

		async importTheme(url: string) {
			const theme = await fetchThemeFromUrl(url);
			if (theme.error) {
				throw new Error(theme.error);
			}

			if (!THEME_URLS.includes(url) && !this.customThemeUrls.includes(url)) {
				this.customThemeUrls.push(url);
			}

			this.fetchedThemes.push(theme);
			this.applyThemePreset(theme.preset.cssVars, url);

			return theme;
		},

		removeCustomTheme(url: string) {
			if (THEME_URLS.includes(url)) return;

			this.customThemeUrls = this.customThemeUrls.filter(u => u !== url);
			this.fetchedThemes = this.fetchedThemes.filter(t => t.url !== url);

			if (this.selectedThemeUrl === url) {
				this.resetTheme();
			}
		},

		selectTheme(theme: FetchedTheme) {
			if (theme.error) return;
			this.applyThemePreset(theme.preset.cssVars, theme.url);
		},

		randomizeTheme() {
			const available = this.fetchedThemes.filter(t => !t.error);
			if (available.length > 0) {
				const random = available[Math.floor(Math.random() * available.length)];
				this.selectTheme(random);
			}
		},

		initFromUserPreferences(customUrls: string[], themeCssVars: ThemeCssVars) {
			this.customThemeUrls = customUrls;

			const hasVars = Object.keys(themeCssVars.light).length > 0 || Object.keys(themeCssVars.dark).length > 0;
			if (hasVars) {
				this.cssVars = themeCssVars;
				this.persist();
				this.apply();
			}
		},
	},
});
