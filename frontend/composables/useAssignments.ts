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
		const loaded = await $customFetch<AssignmentsResponse>(path);
		if (current === path) data.value = loaded;
	}

	async function replace(path: string, targets: AssignmentTarget[]): Promise<boolean> {
		try {
			const body = {targets: targets.filter(target => !target.inherited && !target.owner).map(({kind, id, role}) => ({kind, id, ...(role ? {role} : {})}))};
			data.value = await $customFetch<AssignmentsResponse>(path, {method: 'PUT', body});
			store.toast(store.getTranslation('assignments.save_success'), {type: 'success'});
			return true;
		} catch {
			store.toast(store.getTranslation('assignments.save_error'), {type: 'error'});
			return false;
		}
	}

	return {data, load, replace};
}
