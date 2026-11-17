import {test, expect} from './support/fixtures';

test.describe('model description', () => {
	test('an admin writes a markdown description that the new-chat page renders for the model', async ({page, api, t, seed}) => {
		const url = `/api/v1/admin/models/${seed.model.id}`;
		const original = await api.get(url);

		try {
			await page.goto(`/settings/models/${seed.model.id}`);
			await page.locator('#description').fill('Good at **careful reviews**.');
			await page.getByTestId('markdown-preview-tab').click();
			await expect(page.getByTestId('markdown-preview').locator('strong')).toHaveText('careful reviews');
			await page.getByTestId('settings-save-bar-save').click();
			await expect(page.getByTestId('settings-save-bar')).toBeHidden();
			expect((await api.get(url)).description).toBe('Good at **careful reviews**.');

			await page.goto('/');
			await page.getByTestId('model-selector').click();
			const search = page.getByPlaceholder(t('chat.model_selector.search_models'));
			await search.fill(seed.model.displayName);
			await page.getByText(seed.model.displayName, {exact: true}).last().click();
			await expect(page.getByTestId('model-empty-state')).toBeVisible();
			await expect(page.getByTestId('model-empty-state-name')).not.toBeEmpty();
			await expect(page.getByTestId('chat-description').locator('strong')).toHaveText('careful reviews');
		} finally {
			await api.patch(url, {description: original.description ?? null});
		}
	});
});
