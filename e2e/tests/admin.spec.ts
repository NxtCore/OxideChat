import {test, expect} from './support/fixtures';
import {LLM} from './support/constants';
import {uniqueName} from './support/gateway';

test.describe('admin settings', () => {
	test('an OpenAI-compatible provider can be added in the UI and syncs its models', async ({page, t, api}) => {
		const name = uniqueName('UiProvider');
		await page.goto('/settings/providers');
		await page.getByRole('button', {name: t('settings.providers.add_custom')}).click();
		const dialog = page.getByRole('dialog');
		await dialog.locator('#provider-name').fill(name);
		await dialog.locator('#api-key').fill(LLM.apiKey);
		await dialog.locator('#base-url').fill(LLM.baseUrl);
		await dialog.getByRole('button', {name: t('common.save'), exact: true}).click();
		await expect(page.getByText(t('settings.providers.save_success')).first()).toBeVisible();
		await expect(page.getByText(name).first()).toBeVisible();

		const providers: {id: string; name: string}[] = await api.get('/api/v1/admin/providers');
		const created = providers.find(provider => provider.name === name);
		expect(created).toBeTruthy();
		await expect
			.poll(async () => ((await api.get(`/api/v1/admin/providers/${created!.id}/models`)) as unknown[]).length, {timeout: 15_000})
			.toBeGreaterThan(0);

		await api.delete(`/api/v1/admin/providers/${created!.id}`);
	});

	test('the synced test model is listed on the models page', async ({page, t, seed}) => {
		await page.goto('/settings/models');
		await page.getByPlaceholder(t('settings.models.search')).fill(seed.model.displayName);
		await expect(page.getByText(seed.model.displayName, {exact: true}).first()).toBeVisible();
	});

	test('a team can be created in the UI', async ({page, t, api}) => {
		const name = uniqueName('UiTeam');
		await page.goto('/settings/teams');
		await page.getByRole('button', {name: t('settings.teams.create')}).click();
		const dialog = page.getByRole('dialog');
		await dialog.locator('input').first().fill(name);
		await dialog.getByRole('button', {name: t('common.create'), exact: true}).click();
		await expect(page.getByText(t('settings.teams.create_success')).first()).toBeVisible();
		await expect(page.getByText(name).first()).toBeVisible();

		const response = await api.get('/api/v1/admin/teams');
		const teams: {id: string; name: string}[] = Array.isArray(response) ? response : (response.items ?? response.teams);
		expect(teams.some(team => team.name === name)).toBe(true);
	});

	test('an admin can create a user who can then log in', async ({page, t, browser, baseURL}) => {
		const username = uniqueName('uiuser').replace(/-/g, '');
		const password = 'Created-Passw0rd!';
		await page.goto('/settings/users');
		await page.getByRole('button', {name: t('settings.admin_users.create_user')}).click();
		const dialog = page.getByRole('dialog');
		await dialog.locator('#create-email').fill(`${username}@e2e.test`);
		await dialog.locator('#create-username').fill(username);
		await dialog.locator('#create-password').fill(password);
		await dialog.getByRole('button', {name: t('common.create'), exact: true}).click();
		await expect(page.getByText(t('settings.admin_users.create_success')).first()).toBeVisible();
		await expect(page.getByText(`${username}@e2e.test`).first()).toBeVisible();

		const context = await browser.newContext({baseURL, storageState: {cookies: [], origins: []}});
		const login = await context.request.post('/api/v1/auth/login', {data: {email: `${username}@e2e.test`, password}});
		expect(login.status()).toBe(200);
		await context.close();
	});

	test('the gateway admin page lists keys of all users', async ({page, api, seed}) => {
		const name = uniqueName('admin-visible');
		await api.post('/api/v1/developer/keys', {name, team_id: seed.teamId, scopes: ['inference:read', 'inference:write']});
		await page.goto('/settings/gateway-admin');
		await expect(page.getByText(name).first()).toBeVisible();
	});
});
