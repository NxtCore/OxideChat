import {test, expect, untranslatedKeys} from './support/fixtures';
import {AUTH_FILES, MEMBER} from './support/constants';

const ADMIN_PAGES = [
	'/',
	'/settings/profile',
	'/settings/appearance',
	'/settings/providers',
	'/settings/tools',
	'/settings/models',
	'/settings/teams',
	'/settings/budgets',
	'/settings/gateway-admin',
	'/settings/analytics',
	'/settings/users',
	'/settings/admin-config',
	'/developer/overview',
	'/developer/keys',
	'/developer/logs',
	'/developer/usage',
	'/developer/models',
	'/developer/playground',
	'/developer/docs',
];

const MEMBER_PAGES = ['/', '/settings/profile', '/settings/appearance', '/developer/overview', '/developer/keys', '/developer/logs', '/developer/usage', '/developer/models', '/developer/playground', '/developer/docs'];

async function visit(page: import('@playwright/test').Page, path: string) {
	await page.goto(path);
	await page.waitForLoadState('networkidle');
	await expect(page.locator('body')).not.toBeEmpty();
	await expect(page.locator('.animate-pulse')).toHaveCount(0);
}

test.describe('every page renders as admin', () => {
	for (const path of ADMIN_PAGES) {
		test(`admin ${path}`, async ({page}) => {
			await visit(page, path);
			await expect(page).not.toHaveURL(/\/auth\//);
			expect((await untranslatedKeys(page)).filter(key => !key.startsWith('gateway.')), 'untranslated i18n keys shown on the page').toEqual([]);
		});
	}

	test('model detail page renders', async ({page, seed}) => {
		await visit(page, `/settings/models/${seed.model.id}`);
		await expect(page.getByText(seed.model.displayName).first()).toBeVisible();
		expect(await untranslatedKeys(page)).toEqual([]);
	});
});

test.describe('every page renders as member', () => {
	test.use({storageState: AUTH_FILES.member});

	for (const path of MEMBER_PAGES) {
		test(`member ${path}`, async ({page}) => {
			await visit(page, path);
			await expect(page).not.toHaveURL(/\/auth\//);
			expect((await untranslatedKeys(page)).filter(key => !key.startsWith('gateway.')), 'untranslated i18n keys shown on the page').toEqual([]);
		});
	}

	test('member settings navigation hides admin tabs', async ({page, t}) => {
		await visit(page, '/settings/profile');
		await expect(page.getByRole('link', {name: t('settings.tabs.profile')})).toBeVisible();
		for (const tab of ['providers', 'tools', 'models', 'teams', 'budgets', 'gateway_admin', 'analytics', 'admin_users', 'admin_config']) {
			await expect(page.getByRole('link', {name: t(`settings.tabs.${tab}`), exact: true})).toHaveCount(0);
		}
	});

	test('member cannot see other users through admin URLs', async ({page}) => {
		await visit(page, '/settings/users');
		await expect(page.getByText('admin@e2e.test')).toHaveCount(0);
		await expect(page.getByText(MEMBER.email).first()).toBeHidden({timeout: 1}).catch(() => {});
	});
});
