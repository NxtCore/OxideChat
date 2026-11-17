import {test, expect} from './support/fixtures';
import {uniqueName} from './support/gateway';

test.describe('local registration', () => {
	test('an admin can turn registration off, which hides the sign-up link and refuses new accounts', async ({page, t, api, browser}) => {
		await page.goto('/settings/admin-config');
		await page.getByTestId('config-registration').click();
		await page.getByTestId('settings-save-bar-save').click();
		await expect(page.getByTestId('settings-save-bar')).toBeHidden();
		try {
			expect((await api.get('/api/v1/base')).registration.enabled).toBe(false);

			const context = await browser.newContext({storageState: {cookies: [], origins: []}});
			const visitor = await context.newPage();
			await visitor.goto('/auth/login');
			await expect(visitor.getByLabel(t('auth.login.email'))).toBeVisible();
			await expect(visitor.getByTestId('login-register-link')).toHaveCount(0);

			await visitor.goto('/auth/register');
			await expect(visitor).toHaveURL(/\/auth\/login/);

			const username = uniqueName('closed').replace(/-/g, '');
			const response = await context.request.post('/api/v1/auth/register', {data: {email: `${username}@e2e.test`, username, password: 'Reg1ster-Passw0rd!'}});
			expect(response.status()).toBe(403);
			await context.close();
		} finally {
			await api.patch('/api/v1/admin/config', {registration: {enabled: true}});
		}
		expect((await api.get('/api/v1/base')).registration.enabled).toBe(true);
	});
});
