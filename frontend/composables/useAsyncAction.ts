import {useMainStore} from '~/stores';

export interface AsyncState {
	loading: boolean;
	error: string | null;
}

export async function trackAsync<T>(state: AsyncState, action: () => Promise<T>): Promise<T> {
	state.loading = true;
	state.error = null;
	try {
		return await action();
	} catch (e: any) {
		state.error = errorMessage(e, String(e));
		throw e;
	} finally {
		state.loading = false;
	}
}

export function useAsyncAction() {
	const store = useMainStore();
	const state = reactive<AsyncState>({loading: false, error: null});

	async function run(action: () => Promise<unknown>, successKey: string | null, errorKey: string | null): Promise<boolean> {
		try {
			await trackAsync(state, action);
			if (successKey) store.toast(store.getTranslation(successKey), {type: 'success'});
			return true;
		} catch (e: any) {
			if (errorKey) store.toast(store.getTranslation(errorKey), {type: 'error', description: errorMessage(e, '')});
			return false;
		}
	}

	return {loading: toRef(state, 'loading'), run};
}
