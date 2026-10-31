import {test, expect} from './support/fixtures';
import {AUTH_FILES, MEMBER} from './support/constants';

test.describe('user settings', () => {
	test.use({storageState: AUTH_FILES.member});

	test('profile shows the signed-in user', async ({page}) => {
		await page.goto('/settings/profile');
		await expect(page.getByText(MEMBER.username).first()).toBeVisible();
		await expect(page.getByText(MEMBER.email).first()).toBeVisible();
	});

	test('display mode is saved to preferences and survives a reload', async ({page, t}) => {
		await page.goto('/settings/appearance');
		await page.getByText(t('settings.appearance.theme_dark'), {exact: true}).click();
		await expect(page.locator('body')).toHaveClass(/dark/);
		await page.getByRole('button', {name: t('common.save'), exact: true}).click();
		await expect(page.getByText(t('settings.theme_saved')).first()).toBeVisible();

		await page.context().clearCookies({name: 'theme'});
		await page.evaluate(() => localStorage.clear());
		await page.reload();
		await expect(page.locator('body')).toHaveClass(/dark/);

		await page.getByText(t('settings.appearance.theme_light'), {exact: true}).click();
		await page.getByRole('button', {name: t('common.save'), exact: true}).click();
		await expect(page.locator('body')).not.toHaveClass(/dark/);
	});
});
