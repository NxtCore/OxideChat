import {test, expect} from './support/fixtures';
import {uniqueName} from './support/gateway';

type Assignments = {kinds: string[]; single: boolean; targets: {kind: string; id: string | null; inherited: boolean}[]};

test.describe('shared assignments', () => {
	test('a budget is assigned to a team and a user through the picker and unassigned again', async ({page, api, seed}) => {
		const team = await api.post('/api/v1/admin/teams', {name: uniqueName('AssignTeam')});
		const name = uniqueName('AssignBudget');
		await page.goto('/settings/budgets');
		await page.getByTestId('budget-create').click();
		await page.getByTestId('budget-create-name').fill(name);
		await page.getByTestId('budget-create-submit').click();
		await expect(page.getByTestId('budget-name')).toHaveValue(name);
		const budgets = await api.get('/api/v1/admin/budgets?size=100');
		const budget = budgets.items.find((item: {name: string}) => item.name === name);
		const stored = async () => (await api.get<Assignments>(`/api/v1/admin/budgets/${budget.id}/assignments`)).targets.map(target => `${target.kind}:${target.id}`);

		await page.getByTestId('budget-assignments-add').click();
		await page.getByTestId(`budget-assignments-option-team:${team.id}`).click();
		await expect(page.getByTestId('budget-assignments-chip')).toHaveText([team.name]);
		await page.getByTestId(`budget-assignments-option-user:${seed.memberId}`).click();
		await expect(page.getByTestId('budget-assignments-chip')).toHaveCount(2);
		expect(await stored()).toEqual([`team:${team.id}`, `user:${seed.memberId}`]);

		await page.keyboard.press('Escape');
		await page.getByTestId('budget-assignments-remove').first().click();
		await expect(page.getByTestId('budget-assignments-chip')).toHaveCount(1);
		expect(await stored()).toEqual([`user:${seed.memberId}`]);
	});

	test('a model is granted to a team on its Access tab', async ({page, api, seed, t}) => {
		const team = await api.post('/api/v1/admin/teams', {name: uniqueName('AccessTeam')});
		await page.goto(`/settings/models/${seed.model.id}`);
		await page.getByRole('tab', {name: t('settings.models.editor.access')}).click();
		await page.getByTestId('model-access-add').click();
		await page.getByTestId(`model-access-option-team:${team.id}`).click();
		await expect(page.getByTestId('model-access-chip').filter({hasText: team.name})).toBeVisible();

		const detail = await api.get(`/api/v1/admin/teams/${team.id}`);
		expect(detail.model_access.model_ids).toEqual([seed.model.id]);
		const access = await api.get<Assignments>(`/api/v1/admin/models/${seed.model.id}/assignments`);
		expect(access.targets.find(target => target.id === team.id)?.inherited).toBe(false);
	});

	test('a personal preset is moved to a team from its editor', async ({page, api, seed}) => {
		const team = await api.post('/api/v1/admin/teams', {name: uniqueName('MoveTeam'), member_ids: [seed.adminId]});
		const slug = uniqueName('movable').toLowerCase().replace(/[^a-z0-9]+/g, '-');
		const preset = await api.post('/api/v1/presets', {slug, name: uniqueName('Movable'), base_model_id: seed.model.id});
		try {
			await page.goto(`/settings/presets/${preset.id}`);
			await page.getByTestId('preset-scope-add').click();
			await page.getByTestId(`preset-scope-option-team:${team.id}`).click();
			await expect(page.getByTestId('preset-scope-chip')).toHaveText([team.name]);
			const moved = await api.get(`/api/v1/presets/${preset.id}`);
			expect(moved.scope).toBe('team');
			expect(moved.team.id).toBe(team.id);
		} finally {
			await api.delete(`/api/v1/presets/${preset.id}`);
		}
	});
});
