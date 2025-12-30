import {defineStore} from 'pinia';
import localizedFormat from 'dayjs/plugin/localizedFormat';
import dayjs from 'dayjs';
import 'dayjs/locale/de';

dayjs.locale('de');
dayjs.extend(localizedFormat);

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
	permissions: string[];
	created_at: string;
}

interface AuthState {
	user: User | null;
	isAuthenticated: boolean;
	loading: boolean;
}

export const useMainStore = defineStore('main', {
	state: (): {
		breadcrumbs: BreadcrumbType[];
		base: any;
		initialized: boolean;
		auth: AuthState;
	} => {
		return {
			breadcrumbs: [],
			base: null,
			initialized: false,
			auth: {
				user: null,
				isAuthenticated: false,
				loading: true,
			},
		};
	},
	getters: {
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
		hasAnyPermission(permissions: string[]): boolean {
			return permissions.some(p => this.hasPermission(p));
		},
		getTranslation(name: string, language?: string, args: {[key: string]: any} = {}): string {
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
			// @ts-ignore
			this.breadcrumbs = breadcrumbs;
		},
		formatDate(timestamp: string, format = 'L') {
			if (!timestamp) return '';
			return dayjs(timestamp).format(format);
		},
		async getBaseData() {
			try {
				const {$customFetch} = useNuxtApp();
				const base = await $customFetch(`/api/v1/base`);
				if (base) {
					this.base = base;
				}
			} catch (e: any) {
				console.error('Failed to fetch base data:', e.toString());
			}
			this.initialized = true;
		},
		isOAuthEnabled(provider: string): boolean {
			return this.base?.oauth_providers.includes(provider);
		},
		async getMe() {
			this.auth.loading = true;
			try {
				const {$customFetch} = useNuxtApp();
				const user = await $customFetch('/api/v1/users/@me');
				if (user) {
					this.auth.user = user as User;
					this.auth.isAuthenticated = true;
				}
			} catch (e: any) {
				this.auth.user = null;
				this.auth.isAuthenticated = false;
			} finally {
				this.auth.loading = false;
			}
		},
		async setup(email: string, username: string, password: string) {
			const {$customFetch} = useNuxtApp();
			const response = await $customFetch('/api/v1/auth/setup', {
				method: 'POST',
				body: {email, username, password},
			});
			if (response?.user) {
				this.auth.user = response.user as User;
				this.auth.isAuthenticated = true;
				// Refresh base data to update needs_setup
				await this.getBaseData();
			}
			return response;
		},
		async register(email: string, username: string, password: string) {
			const {$customFetch} = useNuxtApp();
			const response = await $customFetch('/api/v1/auth/register', {
				method: 'POST',
				body: {email, username, password},
			});
			if (response?.user) {
				this.auth.user = response.user as User;
				this.auth.isAuthenticated = true;
			}
			return response;
		},
		async login(email: string, password: string) {
			const {$customFetch} = useNuxtApp();
			const response = await $customFetch('/api/v1/auth/login', {
				method: 'POST',
				body: {email, password},
			});
			if (response?.user) {
				this.auth.user = response.user as User;
				this.auth.isAuthenticated = true;
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

			switch (type) {
				case 'success':
					$toast.success(title, {description, duration});
					break;
				case 'error':
					$toast.error(title, {description, duration});
					break;
				case 'warning':
					$toast.warning(title, {description, duration});
					break;
				case 'loading':
					$toast.loading(title, {description, duration});
					break;
				default:
					$toast.info(title, {description, duration});
			}
		},
	},
});
