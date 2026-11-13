import {useMainStore} from '~/stores';
import type {AssignmentTarget, AssignmentsResponse} from '~/types/assignments';

export function useAssignments() {
	const store = useMainStore();
	const {$customFetch} = useNuxtApp();
	const data = ref<AssignmentsResponse | null>(null);
	let current = '';

	async function load(path: string) {
		current = path;
		data.value = null;
		try {
			const loaded = await $customFetch<AssignmentsResponse>(path);
			if (current === path) data.value = loaded;
		} catch {
			if (current === path) store.toast(store.getTranslation('assignments.load_error'), {type: 'error'});
		}
	}

	async function replace(path: string, targets: AssignmentTarget[]): Promise<boolean> {
		const started = current;
		try {
			const body = {targets: targets.filter(target => !target.inherited && !target.owner).map(({kind, id, role}) => ({kind, id, ...(role ? {role} : {})}))};
			const saved = await $customFetch<AssignmentsResponse>(path, {method: 'PUT', body});
			if (current === started) data.value = saved;
			store.toast(store.getTranslation('assignments.save_success'), {type: 'success'});
			return true;
		} catch {
			store.toast(store.getTranslation('assignments.save_error'), {type: 'error'});
			return false;
		}
	}

	return {data, load, replace};
}
