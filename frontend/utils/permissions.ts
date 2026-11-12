const GROUP_ORDER = ['wildcard', 'settings'];

export function groupPermissions<T extends {name: string; group: string}>(permissions: T[]): {id: string; permissions: T[]}[] {
	const byGroup = new Map<string, T[]>();
	for (const permission of permissions) {
		byGroup.set(permission.group, [...(byGroup.get(permission.group) ?? []), permission]);
	}
	const rank = (id: string) => (GROUP_ORDER.includes(id) ? GROUP_ORDER.indexOf(id) : GROUP_ORDER.length);
	return [...byGroup.entries()].map(([id, grouped]) => ({id, permissions: grouped})).sort((a, b) => rank(a.id) - rank(b.id) || a.id.localeCompare(b.id));
}
