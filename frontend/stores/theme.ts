import {defineStore} from 'pinia';
import type {ThemeCssVars, FetchedTheme} from '~/types/chat';
import {applyThemeToElement, applyAccentToElement, clearAccentFromElement, clearThemeFromElement, getSystemTheme, type ThemeMode, type ThemePreference, type ThemeState} from '~/lib/apply-theme';
import {fetchThemeFromUrl, hasThemeVars, isSameTheme, THEME_URLS} from '~/lib/theme-utils';

const THEME_STORE_KEY = 'oxide-theme-store';

interface PersistedThemeState {
	currentMode: ThemeMode;
	modePreference: ThemePreference;
	cssVars: ThemeCssVars;
	selectedThemeUrl: string | null;
}

export interface ThemeSnapshot extends PersistedThemeState {
	customThemeUrls: string[];
}

function getDefaultCssVars(): ThemeCssVars {
	return {
		theme: {},
		light: {},
		dark: {},
	};
}

function loadPersistedState(): Partial<PersistedThemeState> | null {
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

function resolveMode(preference: ThemePreference): ThemeMode {
	return preference === 'system' ? getSystemTheme() : preference;
}

export const useThemeStore = defineStore('theme', {
	state: () => {
		const persisted = loadPersistedState();
		const modePreference: ThemePreference = persisted?.modePreference ?? persisted?.currentMode ?? 'system';
		return {
			modePreference,
			currentMode: resolveMode(modePreference),
			cssVars: persisted?.cssVars ?? getDefaultCssVars(),
			selectedThemeUrl: persisted?.selectedThemeUrl ?? null,
			defaultTheme: getDefaultCssVars(),
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
				cssVars: this.hasCustomTheme ? this.cssVars : this.defaultTheme,
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
			return hasThemeVars(this.cssVars);
		},
	},

	actions: {
		setThemeState(state: {currentMode?: ThemeMode; cssVars?: ThemeCssVars}) {
			if (state.currentMode !== undefined) {
				this.modePreference = state.currentMode;
				this.currentMode = state.currentMode;
			}
			if (state.cssVars !== undefined) {
				this.cssVars = state.cssVars;
			}
			this.persist();
			this.apply();
		},

		toggleMode() {
			this.setMode(this.currentMode === 'dark' ? 'light' : 'dark');
		},

		setMode(mode: ThemePreference) {
			this.modePreference = mode;
			this.currentMode = resolveMode(mode);
			this.persist();
			this.apply();
		},

		followSystemMode() {
			if (this.modePreference !== 'system') return;
			this.currentMode = getSystemTheme();
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
				modePreference: this.modePreference,
				cssVars: this.cssVars,
				selectedThemeUrl: this.selectedThemeUrl,
			});
		},

		setDefaultTheme(theme: ThemeCssVars) {
			if (typeof document !== 'undefined') {
				clearThemeFromElement(document.body);
			}
			this.defaultTheme = theme;
			this.apply();
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

		restoreSnapshot(snapshot: ThemeSnapshot) {
			if (typeof document !== 'undefined') {
				clearThemeFromElement(document.body);
			}
			this.modePreference = snapshot.modePreference;
			this.currentMode = resolveMode(snapshot.modePreference);
			this.cssVars = snapshot.cssVars;
			this.selectedThemeUrl = snapshot.selectedThemeUrl;
			this.customThemeUrls = snapshot.customThemeUrls;
			this.persist();
			this.apply();
		},

		initFromUserPreferences(customUrls: string[], themeCssVars: ThemeCssVars) {
			this.customThemeUrls = customUrls;
			if (isSameTheme(this.cssVars, themeCssVars)) return;

			if (typeof document !== 'undefined') {
				clearThemeFromElement(document.body);
			}
			this.cssVars = themeCssVars;
			this.selectedThemeUrl = null;
			this.persist();
			this.apply();
		},
	},
});
