import {test, expect} from './support/fixtures';
import {AUTH_FILES} from './support/constants';
import {uniqueName} from './support/gateway';

type Assignments = {kinds: string[]; single: boolean; targets: {kind: string; id: string | null; inherited: boolean; role: string | null}[]};

test.describe('shared assignments', () => {
	test('a budget is assigned to a team and a user through the picker and unassigned again', async ({page, api, seed, t}) => {
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

		await expect(page.getByTestId('budget-create-name')).toBeHidden();
		await expect(page.getByTestId('budget-assignments-empty')).toBeVisible();
		await page.getByTestId('budget-assignments-add').click();
		await page.getByTestId(`budget-assignments-option-team:${team.id}`).click();
		await expect(page.getByTestId('budget-assignments-label')).toHaveText([team.name]);
		await expect(page.getByTestId('budget-assignments-row')).toContainText(t('settings.budgets.assign.effect_team_pooled'));
		await page.getByTestId(`budget-assignments-option-user:${seed.memberId}`).click();
		await expect(page.getByTestId('budget-assignments-row')).toHaveCount(2);
		expect(await stored()).toEqual([`team:${team.id}`, `user:${seed.memberId}`]);

		await page.keyboard.press('Escape');
		await page.getByTestId('budget-assignments-remove').first().click();
		await expect(page.getByTestId('budget-assignments-row')).toHaveCount(1);
		expect(await stored()).toEqual([`user:${seed.memberId}`]);
	});

	test('a model is granted to a team on its Access tab', async ({page, api, seed, t}) => {
		const team = await api.post('/api/v1/admin/teams', {name: uniqueName('AccessTeam')});
		await page.goto(`/settings/models/${seed.model.id}`);
		await page.getByRole('tab', {name: t('settings.models.editor.access')}).click();
		await page.getByTestId('model-access-add').click();
		await page.getByTestId(`model-access-option-team:${team.id}`).click();
		await expect(page.getByTestId('model-access-row').filter({hasText: team.name})).toContainText(t('settings.models.editor.access_effect'));

		const detail = await api.get(`/api/v1/admin/teams/${team.id}`);
		expect(detail.model_access.model_ids).toEqual([seed.model.id]);
		const access = await api.get<Assignments>(`/api/v1/admin/models/${seed.model.id}/assignments`);
		expect(access.targets.find(target => target.id === team.id)?.inherited).toBe(false);
	});

	test('a personal preset is shared with two teams and a user, and a member of an editor team changes it', async ({page, browser, baseURL, api, seed, t}) => {
		const first = await api.post('/api/v1/admin/teams', {name: uniqueName('ShareOne'), member_ids: [seed.memberId]});
		const second = await api.post('/api/v1/admin/teams', {name: uniqueName('ShareTwo')});
		const slug = uniqueName('shareable').toLowerCase().replace(/[^a-z0-9]+/g, '-');
		const preset = await api.post('/api/v1/presets', {slug, name: uniqueName('Shareable'), base_model_id: seed.model.id});
		const stored = async () => (await api.get<Assignments>(`/api/v1/presets/${preset.id}/assignments`)).targets.map(target => `${target.kind}:${target.id}`).sort();
		const member = await browser.newContext({baseURL, storageState: AUTH_FILES.member});
		try {
			await page.goto(`/settings/presets/${preset.id}`);
			await expect(page.getByTestId('preset-sharing-badge')).toHaveText(t('presets.scope.personal'));
			await page.getByTestId('preset-sharing-badge').click();
			await expect(page.getByTestId('preset-sharing-row')).toHaveCount(1);
			await expect(page.getByTestId('preset-sharing-row')).toContainText(t('assignments.owner_badge'));
			await expect(page.getByTestId('preset-sharing-remove')).toHaveCount(0);

			await page.getByTestId('preset-sharing-add').click();
			await page.getByTestId(`preset-sharing-option-team:${first.id}`).click();
			await expect(page.getByTestId('preset-sharing-row')).toHaveCount(2);
			await page.getByTestId(`preset-sharing-option-team:${second.id}`).click();
			await expect(page.getByTestId('preset-sharing-row')).toHaveCount(3);
			await page.getByTestId(`preset-sharing-option-user:${seed.memberId}`).click();
			await expect(page.getByTestId('preset-sharing-row')).toHaveCount(4);
			await expect(page.getByTestId('preset-sharing-count')).toHaveText('+3');
			expect(await stored()).toEqual([`team:${first.id}`, `team:${second.id}`, `user:${seed.adminId}`, `user:${seed.memberId}`].sort());
			expect((await api.get(`/api/v1/presets/${preset.id}`)).scope).toBe('personal');

			await page.keyboard.press('Escape');
			await expect(page.getByTestId('preset-sharing-search')).toBeHidden();
			await page.getByTestId('preset-sharing-remove').first().click();
			await expect(page.getByTestId('preset-sharing-row')).toHaveCount(3);
			expect(await stored()).toEqual([`team:${first.id}`, `team:${second.id}`, `user:${seed.adminId}`].sort());

			const memberPage = await member.newPage();
			await memberPage.goto('/settings/presets?tab=shared');
			const row = memberPage.locator(`[data-testid="preset-row"][data-preset-key="presets/${slug}"]`);
			await expect(row.getByTestId('preset-row-scope')).toHaveText(t('presets.scope.shared'));
			await row.getByRole('link').first().click();
			await expect(memberPage.getByTestId('preset-sharing-badge')).toHaveText(t('presets.scope.shared'));
			await expect(memberPage.getByTestId('preset-editor-delete')).toHaveCount(0);
			await expect(memberPage.getByTestId('preset-name')).toBeDisabled();

			await expect(page.getByTestId('preset-sharing-role').first()).toHaveText(t('assignments.role.viewer'));
			await page.getByTestId('preset-sharing-role').first().click();
			await page.getByTestId('preset-sharing-role-editor').click();
			await expect(page.getByTestId('preset-sharing-row').filter({hasText: first.name})).toContainText(t('presets.editor.share_effect.team_editor'));
			const roles = (await api.get<Assignments>(`/api/v1/presets/${preset.id}/assignments`)).targets.map(target => `${target.id}:${target.role}`);
			expect(roles).toEqual(expect.arrayContaining([`${first.id}:editor`, `${second.id}:viewer`, `${seed.adminId}:null`]));

			const renamed = uniqueName('Renamed');
			await memberPage.reload();
			await expect(memberPage.getByTestId('preset-editor-delete')).toHaveCount(0);
			await memberPage.getByTestId('preset-name').fill(renamed);
			await memberPage.getByTestId('settings-save-bar-save').click();
			await expect(memberPage.getByText(t('presets.editor.saved')).first()).toBeVisible();
			expect((await api.get(`/api/v1/presets/${preset.id}`)).name).toBe(renamed);
			await memberPage.getByTestId('preset-sharing-badge').click();
			await expect(memberPage.getByTestId('preset-sharing-add')).toHaveCount(0);
			await expect(memberPage.getByTestId('preset-sharing-remove')).toHaveCount(0);
		} finally {
			await member.close();
			await api.delete(`/api/v1/presets/${preset.id}`);
		}
	});
});
