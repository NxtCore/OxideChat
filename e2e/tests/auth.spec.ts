import {test, expect} from './support/fixtures';
import {ADMIN, MEMBER} from './support/constants';
import {uniqueName} from './support/gateway';

test.describe('authentication', () => {
	test.use({storageState: {cookies: [], origins: []}});

	test('signed-out visitors are sent to the login page', async ({page}) => {
		await page.goto('/settings/profile');
		await expect(page).toHaveURL(/\/auth\/login/);
	});

	test('the setup page is closed once an admin exists', async ({page}) => {
		await page.goto('/auth/setup');
		await expect(page).not.toHaveURL(/\/auth\/setup/);
	});

	test('a wrong password shows an error and stays on the login page', async ({page, t}) => {
		await page.goto('/auth/login');
		await page.getByLabel(t('auth.login.email')).fill(MEMBER.email);
		await page.getByLabel(t('auth.login.password')).fill('wrong-password-123!');
		await page.getByRole('button', {name: t('auth.login.submit')}).click();
		await expect(page.locator('[data-sonner-toast]').first()).toBeVisible();
		await expect(page).toHaveURL(/\/auth\/login/);
	});

	test('a member can log in and log out', async ({page, t}) => {
		await page.goto('/auth/login');
		await page.getByLabel(t('auth.login.email')).fill(MEMBER.email);
		await page.getByLabel(t('auth.login.password')).fill(MEMBER.password);
		await page.getByRole('button', {name: t('auth.login.submit')}).click();
		await expect(page).not.toHaveURL(/\/auth\//);

		await page.getByText(MEMBER.username).first().click();
		await page.getByText(t('sidebar.logout')).click();
		await expect(page).toHaveURL(/\/auth\/login/);

		await page.goto('/settings/profile');
		await expect(page).toHaveURL(/\/auth\/login/);
	});

	test('logging in as someone else without a reload does not reuse the previous user\'s data', async ({page, t}) => {
		const login = async (user: {email: string; password: string}) => {
			await page.getByLabel(t('auth.login.email')).fill(user.email);
			await page.getByLabel(t('auth.login.password')).fill(user.password);
			const budget = page.waitForResponse(response => new URL(response.url()).pathname === '/api/v1/me/budget');
			await page.getByRole('button', {name: t('auth.login.submit')}).click();
			await expect(page).not.toHaveURL(/\/auth\//);
			await budget;
		};
		const cachedUserData = () =>
			page.evaluate(() => {
				const state = (document.querySelector('#__nuxt') as any).__vue_app__.config.globalProperties.$pinia.state.value;
				return {budget: state.budget.myStatus, tools: state.preferences.userTools.length, mcpServers: state.preferences.userMcpServers.length, models: state.model.models.length, keys: state.developer.keys.length};
			});

		await page.goto('/auth/login');
		await login(MEMBER);
		await page.getByText(MEMBER.username).first().click();
		await page.getByText(t('sidebar.logout')).click();
		await expect(page).toHaveURL(/\/auth\/login/);
		expect(await cachedUserData()).toEqual({budget: null, tools: 0, mcpServers: 0, models: 0, keys: 0});

		await login(ADMIN);
	});

	test('a visitor can register a new account', async ({page, t}) => {
		const username = uniqueName('reg').replace(/-/g, '');
		await page.goto('/auth/register');
		await page.getByLabel(t('auth.login.email')).fill(`${username}@e2e.test`);
		await page.getByLabel(t('auth.register.username')).fill(username);
		await page.locator('#password').fill('Reg1ster-Passw0rd!');
		await page.locator('#confirmPassword').fill('Reg1ster-Passw0rd!');
		await page.getByRole('button', {name: t('auth.register.submit')}).click();
		await expect(page).not.toHaveURL(/\/auth\//);
	});

	test('mismatched passwords are rejected before submitting', async ({page, t}) => {
		await page.goto('/auth/register');
		await page.getByLabel(t('auth.login.email')).fill('mismatch@e2e.test');
		await page.getByLabel(t('auth.register.username')).fill('mismatch');
		await page.locator('#password').fill('Reg1ster-Passw0rd!');
		await page.locator('#confirmPassword').fill('Different-Passw0rd!');
		await page.getByRole('button', {name: t('auth.register.submit')}).click();
		await expect(page.getByText(t('auth.register.passwords_mismatch'))).toBeVisible();
		await expect(page).toHaveURL(/\/auth\/register/);
	});
});
