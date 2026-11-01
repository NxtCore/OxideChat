import {test, expect} from './support/fixtures';
import {AUTH_FILES} from './support/constants';

const SWITCHABLE_THEME = {theme: {}, light: {background: 'rgb(250, 250, 250)'}, dark: {background: 'rgb(10, 10, 10)'}};

test.describe('admin configuration', () => {
	test('the default language applies straight away and can be changed back', async ({page, api, t}) => {
		await page.goto('/settings/admin-config');
		await page.getByTestId('config-language').click();
		await page.getByRole('option', {name: t('settings.appearance.language_de')}).click();
		await page.getByTestId('settings-save-bar-save').click();
		await expect(page.getByTestId('settings-save-bar')).toBeHidden();

		try {
			expect((await api.get('/api/v1/base')).language).toBe('de');
			await expect(page.getByRole('link', {name: 'Konfiguration'})).toBeVisible();
		} finally {
			await api.patch('/api/v1/admin/config', {language: 'en'});
		}
	});

	test('a default theme reaches users without their own theme and can be cleared', async ({page, api, t}) => {
		await api.patch('/api/v1/admin/config', {default_theme: SWITCHABLE_THEME});
		try {
			await page.goto('/settings/admin-config');
			await expect(page.getByTestId('config-default-theme')).toContainText(t('settings.admin_config.default_theme_custom'));
			await expect.poll(() => page.evaluate(() => document.body.style.getPropertyValue('--background'))).toMatch(/rgb\((250, 250, 250|10, 10, 10)\)/);

			await page.getByTestId('config-default-theme').click();
			await page.getByRole('option', {name: t('settings.admin_config.default_theme_none')}).click();
			await page.getByTestId('settings-save-bar-save').click();
			await expect(page.getByTestId('settings-save-bar')).toBeHidden();

			expect((await api.get('/api/v1/config')).default_theme.light).toEqual({});
			await expect.poll(() => page.evaluate(() => document.body.style.getPropertyValue('--background'))).toBe('');
		} finally {
			await api.patch('/api/v1/admin/config', {default_theme: {theme: {}, light: {}, dark: {}}});
		}
	});

	test('sign-in credentials are saved without ever returning the secret', async ({page, api, t}) => {
		const secret = 'e2e-oauth-secret-value';
		await page.goto('/settings/admin-config');
		await page.getByTestId('oauth-configure-GOOGLE').click();
		await page.getByTestId('oauth-client-id').fill('e2e-client-id');
		await expect(page.getByTestId('oauth-save')).toBeDisabled();
		await page.getByTestId('oauth-client-secret').fill(secret);
		await page.getByTestId('oauth-save').click();
		await expect(page.getByTestId('oauth-client-id')).toBeHidden();

		try {
			const saved = (await api.get('/api/v1/admin/config')).oauth.find((item: {provider: string}) => item.provider === 'GOOGLE');
			expect(saved).toMatchObject({client_id: 'e2e-client-id', has_client_secret: true, configured: true});
			expect(JSON.stringify(await api.get('/api/v1/admin/config'))).not.toContain(secret);
			expect((await api.get('/api/v1/base')).oauth_providers).toContain('GOOGLE');
		} finally {
			await page.getByTestId('oauth-configure-GOOGLE').click();
			await page.getByTestId('oauth-remove').click();
			await expect(page.getByTestId('oauth-client-id')).toBeHidden();
		}
		expect((await api.get('/api/v1/admin/config')).oauth.find((item: {provider: string}) => item.provider === 'GOOGLE')).toMatchObject({client_id: null, has_client_secret: false});
		await expect(page.getByTestId('oauth-card')).toContainText(t('settings.admin_config.oauth.configure'));
	});
});

test.describe('translations', () => {
	test('an edit is marked, applies live and can be found by the filter', async ({page, api, t}) => {
		const key = 'settings.tabs.budgets';
		const original = t(key);
		const edited = `${original} edited`;
		await page.goto('/settings/translations');
		await page.getByTestId('translations-search').fill(key);
		await page.getByTestId(`translation-${key}`).click();
		await page.getByTestId('translation-value').fill(edited);
		await page.getByTestId('translation-save').click();

		try {
			await expect(page.getByTestId(`translation-${key}`)).toContainText(t('settings.translations.edited'));
			await expect(page.getByRole('link', {name: edited})).toBeVisible();
			await page.getByTestId('translations-edited-only').click();
			await expect(page.getByTestId(`translation-${key}`)).toBeVisible();
		} finally {
			await api.put('/api/v1/admin/i18n/translations', {language: 'en', key_path: key, value: original, is_override: false});
		}
	});
});

test.describe('connection checks', () => {
	test('a saved provider can be tested from its dialog', async ({page, t}) => {
		await page.goto('/settings/providers');
		await expect(page.getByText(t('settings.providers.configured'), {exact: true}).first()).toBeVisible();
		await page.getByRole('button').filter({has: page.locator('.lucide-settings-2')}).first().click();
		const response = page.waitForResponse(res => /\/api\/v1\/admin\/providers\/[^/]+\/test$/.test(res.url()));
		await page.getByTestId('provider-test').click();
		const result = await (await response).json();
		const expected = result.success ? t('settings.providers.test_success', {count: result.models_found}) : t('settings.providers.test_failed');
		await expect(page.getByText(expected).first()).toBeVisible();
	});

	test('an MCP server health check updates its status', async ({page, api, t}) => {
		const server = await api.post('/api/v1/admin/mcp-servers', {name: `health-${Date.now()}`, transport: 'http', connection_config: {url: 'http://127.0.0.1:9/mcp', headers: {}}});
		try {
			await page.goto('/settings/tools');
			await page.getByRole('button', {name: t('sidebar.manage_mcp')}).first().click();
			await page.getByTestId(`mcp-health-${server.id}`).click();
			await expect(page.getByText(t('mcp.health_unhealthy')).first()).toBeVisible();
		} finally {
			await api.delete(`/api/v1/admin/mcp-servers/${server.id}`);
		}
	});
});

test.describe('member settings', () => {
	test.use({storageState: AUTH_FILES.member});

	test('members see Appearance and Sessions but not admin pages', async ({page, t}) => {
		await page.goto('/settings/profile');
		await expect(page.getByRole('link', {name: t('settings.tabs.appearance')})).toBeVisible();
		await expect(page.getByRole('link', {name: t('settings.tabs.sessions')})).toBeVisible();
		await expect(page.getByRole('link', {name: t('settings.tabs.translations')})).toBeHidden();
	});

	test('other sessions can be ended and the current one stays', async ({page, api, t}) => {
		await page.goto('/settings/sessions');
		const before = await api.get<{id: string; is_current: boolean}[]>('/api/v1/users/@me/sessions');
		expect(before.some(session => session.is_current)).toBe(true);
		for (const session of before.filter(item => !item.is_current)) await api.delete(`/api/v1/users/@me/sessions/${session.id}`);
		await page.reload();
		await expect(page.getByTestId('session-row')).toHaveCount(1);
		await expect(page.getByTestId('session-sign-out')).toHaveCount(0);
		await expect(page.getByTestId('session-row')).toContainText(t('settings.sessions.current'));
		await expect(api.delete('/api/v1/users/@me/sessions/not-a-session')).rejects.toThrow('404');
	});

	test('system mode follows the operating system', async ({page}) => {
		await page.emulateMedia({colorScheme: 'dark'});
		await page.goto('/settings/appearance');
		await page.getByTestId('appearance-mode-system').click();
		await expect(page.locator('body')).toHaveClass(/dark/);

		await page.emulateMedia({colorScheme: 'light'});
		await expect(page.locator('body')).not.toHaveClass(/dark/);
	});
});
