import type { ThemeCssVars } from '~/types/chat';

export type ThemeMode = 'dark' | 'light';

export interface ThemeState {
	currentMode: ThemeMode;
	cssVars: ThemeCssVars;
}

export function applyThemeToElement(themeState: ThemeState, element: HTMLElement) {
	if (!element) return;

	Object.entries(themeState.cssVars.theme).forEach(([key, value]) => {
		element.style.setProperty(`--${key}`, value);
	});

	const modeVars = themeState.cssVars[themeState.currentMode];
	Object.entries(modeVars).forEach(([key, value]) => {
		if (key in themeState.cssVars.theme) {
			return;
		}
		element.style.setProperty(`--${key}`, value);
	});

	element.setAttribute('data-theme', themeState.currentMode);

	if (themeState.currentMode === 'dark') {
		element.classList.add('dark');
		element.classList.remove('light');
	} else {
		element.classList.add('light');
		element.classList.remove('dark');
	}
}

export function clearThemeFromElement(element: HTMLElement) {
	if (!element) return;
	
	const style = element.style;
	const propsToRemove: string[] = [];
	
	for (let i = 0; i < style.length; i++) {
		const prop = style[i];
		if (prop.startsWith('--')) {
			propsToRemove.push(prop);
		}
	}
	
	propsToRemove.forEach(prop => style.removeProperty(prop));
}

export function getSystemTheme(): ThemeMode {
	if (typeof window === 'undefined') return 'light';
	return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}
