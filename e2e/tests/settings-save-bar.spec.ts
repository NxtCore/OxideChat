import {test, expect} from './support/fixtures';
import {MEMBER} from './support/constants';
import {uniqueName} from './support/gateway';

test.describe('settings save bar', () => {
	test('configuration changes wait for save and can be reset', async ({page, api}) => {
		const before = await api.get('/api/v1/config');
		const bar = page.getByTestId('settings-save-bar');
		const toggle = page.getByTestId('config-provider-selector');

		await page.goto('/settings/admin-config');
		await expect(bar).toBeHidden();

		await toggle.click();
		await expect(bar).toBeVisible();
		expect((await api.get('/api/v1/config')).enable_provider_selector).toBe(before.enable_provider_selector);

		await page.getByTestId('settings-save-bar-reset').click();
		await expect(bar).toBeHidden();
		await expect(toggle).toHaveAttribute('data-state', before.enable_provider_selector ? 'checked' : 'unchecked');

		await toggle.click();
		await page.getByTestId('settings-save-bar-save').click();
		await expect(bar).toBeHidden();
		expect((await api.get('/api/v1/config')).enable_provider_selector).toBe(!before.enable_provider_selector);

		await api.patch('/api/v1/admin/config', {enable_provider_selector: before.enable_provider_selector});
	});

	test('a failed save keeps the edits and shows the error', async ({page, t}) => {
		await page.route('**/api/v1/admin/config', route => route.fulfill({status: 400, contentType: 'application/json', body: '{}'}));
		await page.goto('/settings/admin-config');
		const toggle = page.getByTestId('config-provider-selector');
		const state = await toggle.getAttribute('data-state');

		await toggle.click();
		await page.getByTestId('settings-save-bar-save').click();

		await expect(page.getByText(t('settings.admin_config.save_error')).first()).toBeVisible();
		await expect(page.getByTestId('settings-save-bar')).toBeVisible();
		await expect(toggle).not.toHaveAttribute('data-state', state ?? '');
	});

	test('leaving with unsaved changes asks first', async ({page, t}) => {
		await page.goto('/settings/admin-config');
		await page.getByTestId('config-provider-selector').click();

		const usersLink = page.getByRole('link', {name: t('settings.tabs.admin_users')});
		await usersLink.click();
		await page.getByTestId('settings-leave-keep').click();
		await expect(page).toHaveURL(/\/settings\/admin-config/);
		await expect(page.getByTestId('settings-save-bar')).toBeVisible();

		await usersLink.click();
		await page.getByTestId('settings-leave-discard').click();
		await expect(page).toHaveURL(/\/settings\/users/);
	});

	test('a team saves name, members and roles together', async ({page, api}) => {
		const name = uniqueName('SaveBarTeam');
		const renamed = uniqueName('SaveBarRenamed');
		const team = await api.post('/api/v1/admin/teams', {name});
		const bar = page.getByTestId('settings-save-bar');
		const memberRow = page.locator('label', {hasText: MEMBER.username});

		await page.goto('/settings/teams');
		await page.getByRole('button', {name}).click();
		await expect(page.locator('input').nth(1)).toHaveValue(name);
		await expect(bar).toBeHidden();

		await memberRow.getByRole('checkbox').click();
		await expect(bar).toBeVisible();
		await memberRow.getByRole('checkbox').click();
		await expect(bar).toBeHidden();

		await memberRow.getByRole('checkbox').click();
		await page.getByTestId(`member-role-${MEMBER.username}`).click();
		await page.getByRole('option').last().click();
		await page.locator('input').nth(1).fill(renamed);
		await page.getByTestId('settings-save-bar-save').click();
		await expect(bar).toBeHidden();

		const saved = await api.get(`/api/v1/admin/teams/${team.id}`);
		expect(saved.name).toBe(renamed);
		expect(saved.members.map((member: {username: string; role: string}) => [member.username, member.role])).toEqual([[MEMBER.username, 'admin']]);
	});

	test('a model saves its fields and price override together', async ({page, api, t, seed}) => {
		const url = `/api/v1/admin/models/${seed.model.id}`;
		const original = await api.get(url);
		const originalPricing = await api.get(`${url}/pricing`);
		const renamed = uniqueName('SaveBarModel');
		const bar = page.getByTestId('settings-save-bar');

		try {
			await page.goto(`/settings/models/${seed.model.id}`);
			await expect(page.locator('#display-name')).toHaveValue(original.display_name);
			await expect(bar).toBeHidden();

			await page.locator('#display-name').fill(renamed);
			await page.getByRole('tab', {name: t('settings.models.editor.providers')}).click();
			await page.getByTestId('pricing-input').fill('42');
			await expect(bar).toBeVisible();

			await page.getByTestId('model-editor-back').click();
			await page.getByTestId('settings-leave-keep').click();
			await expect(page).toHaveURL(new RegExp(seed.model.id));

			await page.getByTestId('settings-save-bar-save').click();
			await expect(bar).toBeHidden();
			expect((await api.get(url)).display_name).toBe(renamed);
			expect(Number((await api.get(`${url}/pricing`)).override_pricing.input)).toBe(42);

			await page.getByTestId('pricing-remove-override').click();
			await expect(page.getByTestId('pricing-remove-override')).toBeHidden();
			await expect(bar).toBeHidden();
			expect((await api.get(`${url}/pricing`)).override_pricing).toBeNull();
		} finally {
			await api.patch(url, {display_name: original.display_name});
			if (originalPricing.override_pricing) await api.put(`${url}/pricing`, {pricing: originalPricing.override_pricing});
			else await api.delete(`${url}/pricing`);
		}
	});

	test('an invalid price is rejected and keeps the edits', async ({page, t, seed}) => {
		await page.goto(`/settings/models/${seed.model.id}`);
		await page.getByRole('tab', {name: t('settings.models.editor.providers')}).click();
		await page.getByTestId('pricing-input').fill('-1');
		await page.getByTestId('settings-save-bar-save').click();

		await expect(page.getByText(t('settings.models.editor.pricing_invalid')).first()).toBeVisible();
		await expect(page.getByTestId('settings-save-bar')).toBeVisible();
	});

	test('a budget is created through a dialog and edited with the save bar', async ({page, api}) => {
		const name = uniqueName('SaveBarBudget');
		const renamed = uniqueName('SaveBarBudgetRenamed');
		const bar = page.getByTestId('settings-save-bar');
		const budgetField = page.getByTestId('budget-name');
		const findBudget = async (budgetName: string) => {
			const response = await api.get('/api/v1/admin/budgets?size=100');
			const budgets: {id: string; name: string}[] = Array.isArray(response) ? response : response.items;
			return budgets.find(budget => budget.name === budgetName);
		};

		await page.goto('/settings/budgets');
		await page.getByTestId('budget-create').click();
		await page.getByTestId('budget-create-name').fill(name);
		expect(await findBudget(name)).toBeUndefined();
		await page.getByTestId('budget-create-submit').click();
		await expect(budgetField).toHaveValue(name);
		await expect(bar).toBeHidden();
		expect(await findBudget(name)).toBeDefined();

		await budgetField.fill(renamed);
		await page.getByTestId('settings-save-bar-reset').click();
		await expect(budgetField).toHaveValue(name);
		await expect(bar).toBeHidden();

		await budgetField.fill(renamed);
		await page.getByTestId('settings-save-bar-save').click();
		await expect(bar).toBeHidden();
		expect(await findBudget(renamed)).toBeDefined();
	});

	test('switching teams with unsaved changes asks first', async ({page, api}) => {
		const first = uniqueName('SaveBarFirst');
		const second = uniqueName('SaveBarSecond');
		await api.post('/api/v1/admin/teams', {name: first});
		await api.post('/api/v1/admin/teams', {name: second});

		await page.goto('/settings/teams');
		await page.getByRole('button', {name: first}).click();
		await expect(page.locator('input').nth(1)).toHaveValue(first);
		await page.locator('input').nth(1).fill(`${first} edited`);

		await page.getByRole('button', {name: second}).click();
		await page.getByTestId('settings-leave-keep').click();
		await expect(page.locator('input').nth(1)).toHaveValue(`${first} edited`);

		await page.getByRole('button', {name: second}).click();
		await page.getByTestId('settings-leave-discard').click();
		await expect(page.locator('input').nth(1)).toHaveValue(second);
		await expect(page.getByTestId('settings-save-bar')).toBeHidden();
	});
});
