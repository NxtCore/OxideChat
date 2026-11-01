import {test, expect} from './support/fixtures';
import {AUTH_FILES, MEMBER} from './support/constants';

test.describe('user settings', () => {
	test.use({storageState: AUTH_FILES.member});

	test('profile shows the signed-in user', async ({page}) => {
		await page.goto('/settings/profile');
		await expect(page.getByText(MEMBER.username).first()).toBeVisible();
		await expect(page.getByText(MEMBER.email).first()).toBeVisible();
	});

	test('display mode previews live, can be reset and survives a reload once saved', async ({page, t}) => {
		await page.goto('/settings/appearance');
		const bar = page.getByTestId('settings-save-bar');
		await page.getByText(t('settings.appearance.theme_light'), {exact: true}).click();
		if (await bar.isVisible()) await page.getByTestId('settings-save-bar-save').click();
		await expect(bar).toBeHidden();

		await page.getByText(t('settings.appearance.theme_dark'), {exact: true}).click();
		await expect(page.locator('body')).toHaveClass(/dark/);
		await page.getByTestId('settings-save-bar-reset').click();
		await expect(bar).toBeHidden();
		await expect(page.locator('body')).not.toHaveClass(/dark/);

		await page.getByText(t('settings.appearance.theme_dark'), {exact: true}).click();
		await page.getByTestId('settings-save-bar-save').click();
		await expect(page.getByText(t('settings.theme_saved')).first()).toBeVisible();
		await expect(bar).toBeHidden();

		await page.reload();
		await expect(page.locator('body')).toHaveClass(/dark/);
		await expect(bar).toBeHidden();
	});
});
