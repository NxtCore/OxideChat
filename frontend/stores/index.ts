import {defineStore} from 'pinia';
import localizedFormat from 'dayjs/plugin/localizedFormat';
import dayjs from 'dayjs';
import 'dayjs/locale/de';
import type {TeamSummary, UserPreferences} from '~/types/chat';

dayjs.locale('de');
dayjs.extend(localizedFormat);

// Module-level signal for the retry loop — set by retryBoot(), read by _pollUntilHealthy()
let _retryNow = false;

type BootState = 'idle' | 'booting' | 'server-starting' | 'online';

interface BreadcrumbType {
	to: object;
	name: string;
	icon?: string;
	current?: boolean;
}

interface User {
	id: string;
	email: string;
	username: string;
	auth_method: string;
	roles: string[];
	teams: TeamSummary[];
	permissions: string[];
	preferences: UserPreferences;
	created_at: string;
}

interface AuthState {
	user: User | null;
	isAuthenticated: boolean;
	loading: boolean;
}

interface Role {
	name: string;
	id: string;
	created_at: string;
}

interface Base {
	i18n: any;
	language: string;
	needs_setup: boolean;
	oauth_providers: string[];
	roles: Role[];
	enable_provider_selector: boolean;
}

export const useMainStore = defineStore('main', {
	state: (): {
		breadcrumbs: BreadcrumbType[];
		base: Base | null;
		bootState: BootState;
		retryCount: number;
		lastBootError: string | null;
		preferences: UserPreferences | null;
		auth: AuthState;
	} => {
		return {
			breadcrumbs: [],
			base: null,
			bootState: 'idle',
			retryCount: 0,
			lastBootError: null,
			preferences: null,
			auth: {
				user: null,
				isAuthenticated: false,
				loading: true,
			},
		};
	},
	getters: {
		initialized(): boolean {
			return this.bootState === 'online';
		},
		roles(): Role[] {
			return this.base?.roles ?? [];
		},
		isAdmin(): boolean {
			return this.auth.user?.roles?.includes('admin') ?? false;
		},
	},
	actions: {
		hasPermission(permission: string): boolean {
			const userPerms = this.auth.user?.permissions ?? [];
			if (userPerms.includes(permission)) return true;
			const parts = permission.split('.');
			for (let i = 1; i < parts.length; i++) {
				const wildcard = parts.slice(0, i).join('.') + '.*';
				if (userPerms.includes(wildcard)) return true;
			}
			return false;
		},
		getTranslation(name: string, args: {[key: string]: any} = {}, language?: string): string {
			const lang = language ?? this.base?.language ?? 'en';
			let currentTranslation: any = this.base?.i18n?.[lang];

			for (const translationKey of name.split('.')) {
				const field = currentTranslation?.[translationKey];
				if (field !== undefined) {
					if (typeof field === 'string') {
						return field.replace(/{(\w+)}/g, (match, key) => args[key] ?? match);
					}
					if (typeof field === 'object' && field !== null) {
						currentTranslation = field;
					}
				} else {
					return name;
				}
			}
			return name;
		},
		updateBreadcrumbs(breadcrumbs: BreadcrumbType[]) {
			this.breadcrumbs = breadcrumbs;
		},
		formatDate(timestamp: string, format = 'L') {
			if (!timestamp) return '';
			return dayjs(timestamp).format(format);
		},
		async getBaseData() {
			this.bootState = 'booting';
			try {
				const {$customFetch} = useNuxtApp();
				const base = await $customFetch<Base>('/api/v1/base');
				if (base) {
					this.base = base;
				}
				this.bootState = 'online';
			} catch (e: any) {
				if (isServerUnreachable(e)) {
					this.bootState = 'server-starting';
					this.lastBootError = e?.message ?? 'Network error';
					await this._pollUntilHealthy();
					await this.getBaseData();
				} else {
					// Server responded with a non-5xx error — proceed so auth pages still render
					console.error('Failed to fetch base data:', e.toString());
					this.bootState = 'online';
				}
			}
		},
		async _pollUntilHealthy() {
			let delay = 1000;
			const maxDelay = 10000;
			while (true) {
				// Wait with early-exit: check _retryNow every 100ms
				const deadline = Date.now() + delay;
				while (Date.now() < deadline) {
					if (_retryNow) {
						_retryNow = false;
						break;
					}
					await new Promise<void>(r => setTimeout(r, Math.min(100, deadline - Date.now())));
				}
				_retryNow = false;
				this.retryCount++;
				try {
					const {$customFetch} = useNuxtApp();
					await $customFetch('/api/v1/health');
					return; // Server is healthy
				} catch {
					delay = Math.min(delay * 2, maxDelay);
				}
			}
		},
		retryBoot() {
			_retryNow = true;
		},
		isOAuthEnabled(provider: string): boolean {
			return this.base?.oauth_providers.includes(provider) ?? false;
		},
		async getMe() {
			this.auth.loading = true;
			try {
				const {$customFetch} = useNuxtApp();
				const user = await $customFetch<User>('/api/v1/users/@me');
				if (user) {
					this.auth.user = user;
					this.preferences = user.preferences;
					this.auth.isAuthenticated = true;
				}
			} catch (e: any) {
				if (isServerUnreachable(e)) {
					// Propagate so checkAuth keeps the boot screen rather than redirecting to login
					throw e;
				}
				this.auth.user = null;
				this.auth.isAuthenticated = false;
			} finally {
				this.auth.loading = false;
			}
		},
		async setup(email: string, username: string, password: string) {
			const {$customFetch} = useNuxtApp();
			const response = await $customFetch<{user: User}>('/api/v1/auth/setup', {
				method: 'POST',
				body: {email, username, password},
			});
			if (response) {
				this.auth.user = response.user;
				this.auth.isAuthenticated = true;
				await this.getBaseData();
			}
			return response;
		},
		async register(email: string, username: string, password: string) {
			const {$customFetch} = useNuxtApp();
			const response = await $customFetch<{user: User}>('/api/v1/auth/register', {
				method: 'POST',
				body: {email, username, password},
			});
			if (response) {
				this.auth.user = response.user;
				this.auth.isAuthenticated = true;
			}
			return response;
		},
		async login(email: string, password: string) {
			const {$customFetch} = useNuxtApp();
			const chatStore = useChatStore();
			const response = await $customFetch<{user: User}>('/api/v1/auth/login', {
				method: 'POST',
				body: {email, password},
			});
			if (response) {
				this.auth.user = response.user;
				this.auth.isAuthenticated = true;
				await chatStore.init();
			}
			return response;
		},
		async logout() {
			try {
				const {$customFetch} = useNuxtApp();
				await $customFetch('/api/v1/auth/logout', {method: 'POST'});
			} catch (e: any) {
				console.error('Logout error:', e.toString());
			} finally {
				this.auth.user = null;
				this.auth.isAuthenticated = false;
			}
		},
		async copyToClipboard(text: string) {
			try {
				if (typeof navigator === 'undefined' || !navigator.clipboard || typeof navigator.clipboard.writeText !== 'function') {
					console.error('Clipboard API is not available in this environment.');
					return;
				}
				await navigator.clipboard.writeText(text);
				this.toast(this.getTranslation('common.copy_to_clipboard'), {description: text});
			} catch (error) {
				console.error('Failed to copy text to clipboard:', error);
			}
		},
		toast(title: string, options?: {description?: string; type?: 'success' | 'error' | 'warning' | 'info' | 'loading'; duration?: number}) {
			const {$toast} = useNuxtApp();
			const {description, type = 'info', duration} = options || {};

			let toast;
			switch (type) {
				case 'success':
					toast = $toast.success(title, {description, duration});
					break;
				case 'error':
					toast = $toast.error(title, {description, duration});
					break;
				case 'warning':
					toast = $toast.warning(title, {description, duration});
					break;
				case 'loading':
					toast = $toast.loading(title, {description, duration});
					break;
				default:
					toast = $toast.info(title, {description, duration});
			}
			return toast;
		},
		dismissToast(toast: any) {
			const {$toast} = useNuxtApp();
			if (!toast) {
				console.error('Invalid toast:', toast);
				return;
			}
			$toast.dismiss(toast);
		},
	},
});
