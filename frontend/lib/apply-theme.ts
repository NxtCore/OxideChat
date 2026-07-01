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

const ACCENT_VARS = ['--primary', '--primary-foreground', '--ring', '--sidebar-primary', '--sidebar-primary-foreground'];

function readableForeground(hex: string): string {
	const normalized = hex.replace('#', '');
	const full = normalized.length === 3 ? normalized.split('').map(c => c + c).join('') : normalized;
	if (full.length !== 6) return 'rgb(255, 255, 255)';
	const r = parseInt(full.slice(0, 2), 16) / 255;
	const g = parseInt(full.slice(2, 4), 16) / 255;
	const b = parseInt(full.slice(4, 6), 16) / 255;
	const channel = (c: number) => (c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4));
	const luminance = 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
	return luminance > 0.45 ? 'rgb(23, 23, 23)' : 'rgb(255, 255, 255)';
}

export function clearAccentFromElement(element: HTMLElement) {
	if (!element) return;
	ACCENT_VARS.forEach(prop => element.style.removeProperty(prop));
}

export function applyAccentToElement(color: string | null, element: HTMLElement) {
	if (!element || !color) return;
	const foreground = readableForeground(color);
	element.style.setProperty('--primary', color);
	element.style.setProperty('--primary-foreground', foreground);
	element.style.setProperty('--ring', color);
	element.style.setProperty('--sidebar-primary', color);
	element.style.setProperty('--sidebar-primary-foreground', foreground);
}
