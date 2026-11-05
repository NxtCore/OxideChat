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
		await expect(page.getByText(t('settings.appearance.saved')).first()).toBeVisible();
		await expect(bar).toBeHidden();

		await page.reload();
		await expect(page.locator('body')).toHaveClass(/dark/);
		await expect(bar).toBeHidden();
	});
	test('language is picked per user and applies to the UI and backend messages', async ({page, api, t, tDe}) => {
		try {
			await page.goto('/settings/appearance');
			await page.getByTestId('appearance-language').click();
			await page.getByTestId('appearance-language-de').click();
			await page.getByTestId('settings-save-bar-save').click();
			await expect(page.getByText(tDe('settings.appearance.saved')).first()).toBeVisible();

			await page.reload();
			await expect(page.getByText(tDe('settings.appearance.language_description'))).toBeVisible();

			const response = await page.request.get('/api/v1/chats/00000000-0000-0000-0000-000000000000');
			const body = await response.json();
			expect(body.errors[0].message).toBe(tDe('errors.not_found'));
		} finally {
			await api.patch('/api/v1/users/@me/preferences', {language: null});
		}

		await page.reload();
		await expect(page.getByText(t('settings.appearance.language_description'))).toBeVisible();
	});
});
