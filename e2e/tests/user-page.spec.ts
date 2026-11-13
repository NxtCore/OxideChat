import {test, expect, untranslatedKeys} from './support/fixtures';
import {uniqueName} from './support/gateway';

type Overview = {roles: {name: string}[]; teams: {id: string; role: string}[]; permissions: {name: string; sources: {role: string; through: string}[]}[]};

test.describe('per-user admin page', () => {
	test('a user opened from the list shows their role, team, budget and model grant, and a role is assigned in place', async ({page, api, seed, t}) => {
		const username = uniqueName('pageuser').replace(/-/g, '');
		const team = await api.post('/api/v1/admin/teams', {name: uniqueName('PageTeam')});
		await api.put(`/api/v1/admin/teams/${team.id}/models`, {provider_ids: [], model_ids: [seed.model.id]});
		const budgetName = uniqueName('PageBudget');
		const budget = await api.post('/api/v1/admin/budgets', {name: budgetName, amount: '25', kind: 'pooled', interval: 'monthly', reset_strategy: 'calendar', on_exceed: 'block'});
		await api.put(`/api/v1/admin/budgets/${budget.id}/assignments`, {targets: [{kind: 'team', id: team.id}]});
		const role = await api.post('/api/v1/admin/roles', {name: uniqueName('PageRole'), permissions: ['admin.teams.view']});
		const user = await api.post('/api/v1/admin/users', {email: `${username}@e2e.test`, username, password: 'Page-User-Passw0rd!', roles: ['user'], team_ids: [team.id]});
		const overview = () => api.get<Overview>(`/api/v1/admin/users/${user.id}/overview`);

		try {
			await page.goto('/settings/users');
			await page.getByPlaceholder(t('settings.admin_users.search_placeholder')).fill(username);
			await page.getByTestId('user-row-link').filter({hasText: username}).click();
			await expect(page).toHaveURL(new RegExp(`/settings/users/${user.id}$`));
			await expect(page.getByRole('heading', {name: username})).toBeVisible();
			await expect(page.getByTestId('user-email')).toHaveValue(`${username}@e2e.test`);
			await expect(page.getByTestId('user-auth-method')).toHaveText(t('settings.user_detail.auth_local'));
			await expect(page.getByTestId('user-sessions-empty')).toBeVisible();
			expect(await untranslatedKeys(page)).toEqual([]);

			await page.getByTestId('user-tab-roles').click();
			await expect(page.getByTestId('user-roles-label')).toHaveText(['user']);
			await expect(page.locator('[data-testid="user-permission"][data-permission="admin.teams.view"]')).toHaveCount(0);
			await page.getByTestId('user-roles-add').click();
			await page.getByTestId(`user-roles-option-role:${role.id}`).click();
			await expect(page.getByTestId('user-roles-label')).toHaveText(['user', role.name]);
			await expect(page.getByTestId('user-roles-row').filter({hasText: role.name})).toContainText(t('roles.permission_count', {count: 1}));
			await page.keyboard.press('Escape');
			const granted = page.locator('[data-testid="user-permission"][data-permission="admin.teams.view"]');
			await expect(granted).toContainText(t('roles.permission.admin.teams.view'));
			await expect(granted.getByTestId('user-permission-source')).toHaveText([role.name]);
			expect((await overview()).roles.map(held => held.name).sort()).toEqual([role.name, 'user'].sort());
			expect(await untranslatedKeys(page)).toEqual([]);

			await page.getByTestId('user-tab-teams').click();
			const teamRow = page.getByTestId('user-teams-row').filter({hasText: team.name});
			await expect(teamRow).toContainText(t('settings.user_detail.team_role.member'));
			await expect(page.locator('[data-testid="user-teams-row"][data-inherited="true"]')).toContainText(t('settings.user_detail.teams_default_hint'));
			expect((await overview()).teams.find(member => member.id === team.id)?.role).toBe('member');

			await page.getByTestId('user-tab-models').click();
			const grant = page.locator('[data-testid="user-model-grant"][data-kind="model"]').filter({hasText: seed.model.displayName});
			await expect(grant).toContainText(t('settings.user_detail.grant_through', {team: team.name}));
			await expect(grant).toHaveAttribute('href', `/settings/models/${seed.model.id}`);

			await page.getByTestId('user-tab-budgets').click();
			const budgetRow = page.getByTestId('user-budget').filter({hasText: budgetName});
			await expect(budgetRow.getByTestId('user-budget-source')).toHaveText(t('settings.user_detail.budget_source_team', {team: team.name}));
			await expect(budgetRow.getByTestId('user-budget-spend')).toHaveText(t('settings.user_detail.budget_used', {spent: '$0.00', amount: '$25.00'}));
			expect(await untranslatedKeys(page)).toEqual([]);

			await page.getByTestId('user-tab-roles').click();
			await page.getByTestId('user-roles-row').filter({hasText: role.name}).getByTestId('user-roles-remove').click();
			await expect(page.getByTestId('user-roles-label')).toHaveText(['user']);
			await expect(granted).toHaveCount(0);
			expect((await overview()).roles.map(held => held.name)).toEqual(['user']);
		} finally {
			await api.delete(`/api/v1/admin/users/${user.id}`);
			await api.delete(`/api/v1/admin/roles/${role.id}`);
			await api.delete(`/api/v1/admin/budgets/${budget.id}`);
			await api.delete(`/api/v1/admin/teams/${team.id}`);
		}
	});

	test('an admin edits the profile with the save bar but cannot change their own roles', async ({page, api, seed, t}) => {
		const username = uniqueName('editme').replace(/-/g, '');
		const renamed = `${username}x`;
		const user = await api.post('/api/v1/admin/users', {email: `${username}@e2e.test`, username, password: 'Page-User-Passw0rd!', roles: ['user']});
		try {
			await page.goto(`/settings/users/${user.id}`);
			await page.getByTestId('user-username').fill(renamed);
			await page.getByTestId('settings-save-bar-save').click();
			await expect(page.getByText(t('settings.admin_users.edit_success')).first()).toBeVisible();
			await expect(page.getByRole('heading', {name: renamed})).toBeVisible();
			expect((await api.get(`/api/v1/admin/users/${user.id}`)).username).toBe(renamed);

			await page.goto(`/settings/users/${seed.adminId}?tab=roles`);
			await expect(page.getByTestId('user-roles-label').first()).toBeVisible();
			await expect(page.getByTestId('user-roles-add')).toHaveCount(0);
			await expect(page.getByTestId('user-roles-remove')).toHaveCount(0);
			await expect(page.getByText(t('settings.user_detail.roles_self_note'))).toBeVisible();
			await expect(page.getByTestId('user-delete')).toHaveCount(0);
			await expect(page.locator('[data-testid="user-permission"][data-permission="admin.users.edit"]').getByTestId('user-permission-source')).toContainText(
				t('settings.user_detail.via', {permission: 'admin.*'}),
			);
		} finally {
			await api.delete(`/api/v1/admin/users/${user.id}`);
		}
	});
});
