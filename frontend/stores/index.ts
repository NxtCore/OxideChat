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

export const useMainStore = defineStore('main', {
	state: (): {
		breadcrumbs: BreadcrumbType[];
		base: any;
		initialized: boolean;
	} => {
		const config = useRuntimeConfig();
		return {
			breadcrumbs: [],
			base: null,
			initialized: false,
		};
	},
	getters: {},
	actions: {
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
		async validateAuth() {
			try {
				const {$customFetch} = useNuxtApp();
				const {data: user, error} = await useAsyncData(`auth_user`, async () => $customFetch('/api/v1/auth'));
			} catch (e: any) {
				console.log(`Error: ${e}`);
			}
			this.initialized = true;
		},
		async getBaseData() {
			try {
				const {$customFetch} = useNuxtApp();
				const {data: base, error} = await useAsyncData('base', async () => $customFetch(`/api/v1/base`));
				if (!error?.value && base?.value) {
					this.base = base.value;
				}
			} catch (e: any) {
				console.log(e.toString());
			}
		},
		async logout() {
			try {
				const {$customFetch} = useNuxtApp();
				await useAsyncData('logout', async () => $customFetch('/api/v1/users/logout', {method: 'POST'}).catch(e => e));
			} catch (e: any) {
				console.log(e.toString());
			}
		},
		copyToClipboard(text: string) {
			navigator.clipboard.writeText(text);
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
