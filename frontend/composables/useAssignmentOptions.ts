import {useMainStore} from '~/stores';
import type {AssignmentOption} from '~/types/assignments';
import {ADMIN_TEAMS_VIEW, ADMIN_USERS_VIEW} from '~/types/generated/permissions';

export function useAssignmentOptions() {
	const store = useMainStore();
	const teams = ref<AssignmentOption[]>([]);
	const users = ref<AssignmentOption[]>([]);
	const roles = computed<AssignmentOption[]>(() => store.roles.map(role => ({kind: 'role' as const, id: role.id, label: role.name})));
	let term = '';

	async function searchUsers(search = '') {
		if (!store.hasPermission(ADMIN_USERS_VIEW)) return;
		term = search;
		const list = await adminUsers({page: 1, size: 50, ...(search ? {search} : {})});
		if (term === search) users.value = list.items.map(user => ({kind: 'user' as const, id: user.id, label: user.username || user.email}));
	}

	async function load() {
		if (store.hasPermission(ADMIN_TEAMS_VIEW)) {
			const all: AssignmentOption[] = [];
			for (let page = 1; ; page++) {
				const list = await adminTeams({page, size: 100});
				all.push(...(list?.items ?? []).map(team => ({kind: 'team' as const, id: team.id, label: team.name})));
				if (!list?.has_more) break;
			}
			teams.value = all;
		}
		await searchUsers();
	}

	return {teams, roles, users, searchUsers, load};
}
