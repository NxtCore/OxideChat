import {useMainStore} from '@/stores';

export function translateOr(key: string, fallback: string): string {
	const value = useMainStore().getTranslation(key);
	return value === key ? fallback : value;
}
