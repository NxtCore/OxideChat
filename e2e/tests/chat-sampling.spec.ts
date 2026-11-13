import type {Page} from '@playwright/test';
import {test, expect} from './support/fixtures';

async function pickModel(page: Page, t: (key: string) => string, displayName: string) {
	await page.getByTestId('model-selector').click();
	const search = page.getByPlaceholder(t('chat.model_selector.search_models'));
	await search.fill(displayName);
	await page.getByText(displayName, {exact: true}).last().click();
	await expect(search).toBeHidden();
}

const panel = (page: Page) => page.getByTestId('sampling-panel');

async function openPanel(page: Page) {
	if (!(await page.getByTestId('composer-settings-panel').isVisible())) await page.getByTestId('composer-settings').click();
	await page.getByTestId('composer-settings-panel').getByTestId('sampling-selector').click();
	await expect(panel(page)).toBeVisible();
}

const chip = (page: Page) => page.getByTestId('composer-settings').getByTestId('composer-chip-sampling');

async function enableParam(page: Page, key: string) {
	await panel(page).getByTestId(`sampling-param-${key}`).getByRole('button').last().click();
}

async function addCustomField(page: Page, key: string, value: string) {
	await panel(page).getByTestId('sampling-add-custom').click();
	const field = panel(page).getByTestId('sampling-custom-field').last();
	await field.getByTestId('sampling-custom-key').fill(key);
	await field.getByTestId('sampling-custom-value').fill(value);
}

test.describe('chat sampling options', () => {
	test('saves sampling values and custom fields on the chat', async ({page, t, api}) => {
		const chat = await api.post('/api/v1/chats');
		try {
			await page.goto(`/chats/${chat.id}`);
			await openPanel(page);
			await enableParam(page, 'temperature');
			await addCustomField(page, 'min_p', '0.05');
			await panel(page).getByTestId('sampling-save').click();

			await expect(page.getByText(t('chat.sampling.saved')).first()).toBeVisible();
			await expect(panel(page)).toBeHidden();
			await expect(chip(page)).toHaveText(t('chat.composer.sampling_chip', {count: 2}));
			const saved = await api.get(`/api/v1/chats/${chat.id}`);
			expect(saved.chat.sampling).toEqual({temperature: 0.7, custom: {min_p: 0.05}});

			await page.reload();
			await expect(chip(page)).toHaveText(t('chat.composer.sampling_chip', {count: 2}));
			await openPanel(page);
			await expect(panel(page).getByTestId('sampling-custom-key')).toHaveValue('min_p');
			await panel(page).getByTestId('sampling-reset').click();
			await expect(chip(page)).toHaveCount(0);
			expect((await api.get(`/api/v1/chats/${chat.id}`)).chat.sampling).toEqual({});
		} finally {
			await api.delete(`/api/v1/chats/${chat.id}`);
		}
	});

	test('a reserved custom field is rejected and nothing is saved', async ({page, t, api, allowErrors}) => {
		allowErrors(/400|sampling/i);
		const chat = await api.post('/api/v1/chats');
		try {
			await page.goto(`/chats/${chat.id}`);
			await openPanel(page);
			await addCustomField(page, 'model', '"other-model"');
			await panel(page).getByTestId('sampling-save').click();

			await expect(page.getByText(t('chat.errors.sampling_invalid')).first()).toBeVisible();
			await expect(panel(page)).toBeVisible();
			expect((await api.get(`/api/v1/chats/${chat.id}`)).chat.sampling).toEqual({});
		} finally {
			await api.delete(`/api/v1/chats/${chat.id}`);
		}
	});

	test('values set before the first message are saved on the new chat', async ({page, t, api, seed}) => {
		await page.goto('/');
		await pickModel(page, t, seed.model.displayName);
		await openPanel(page);
		await enableParam(page, 'seed');
		await panel(page).getByTestId('sampling-save').click();
		await expect(chip(page)).toHaveText(t('chat.composer.sampling_chip', {count: 1}));

		const composer = page.locator('textarea').first();
		await composer.fill('Say ok.');
		await composer.press('Enter');
		await expect(page).toHaveURL(/\/chats\/[0-9a-f-]{36}/);
		const chatId = page.url().split('/chats/')[1] ?? '';
		try {
			await expect.poll(async () => (await api.get(`/api/v1/chats/${chatId}`)).chat.sampling).toEqual({seed: 42});
		} finally {
			await api.delete(`/api/v1/chats/${chatId}`);
		}
	});
});
