import type {Page} from '@playwright/test';
import {test, expect} from './support/fixtures';

async function pickModel(page: Page, t: (key: string) => string, name: string) {
	await page.locator('button[aria-haspopup="dialog"]').filter({hasText: new RegExp(`${t('chat.model_selector.select_model')}|mock-`)}).first().click();
	await page.getByPlaceholder(t('chat.model_selector.search_models')).fill(name);
	await page.getByText(name, {exact: true}).last().click();
	await expect(page.getByPlaceholder(t('chat.model_selector.search_models'))).toBeHidden();
}

async function send(page: Page, text: string) {
	const composer = page.locator('textarea').first();
	await composer.fill(text);
	await composer.press('Enter');
}

test.describe('web chat', () => {
	test('sends a message, streams the reply and keeps it after reload', async ({page, t}) => {
		await page.goto('/');
		await pickModel(page, t, 'mock-chat');
		const prompt = `hello e2e ${Date.now()}`;
		await send(page, prompt);

		await expect(page).toHaveURL(/\/chats\/[0-9a-f-]{36}/);
		await expect(page.getByText(`Mock reply: ${prompt}`)).toBeVisible();
		await expect(page.locator('textarea').first()).toHaveValue('');

		await page.reload();
		await expect(page.getByText(prompt, {exact: true}).first()).toBeVisible();
		await expect(page.getByText(`Mock reply: ${prompt}`)).toBeVisible();
	});

	test('a follow-up message in the same chat gets its own reply', async ({page, t}) => {
		await page.goto('/');
		await pickModel(page, t, 'mock-chat');
		await send(page, 'first message');
		await expect(page.getByText('Mock reply: first message')).toBeVisible();
		const url = page.url();
		await send(page, 'second message');
		await expect(page.getByText('Mock reply: second message')).toBeVisible();
		expect(page.url()).toBe(url);
	});

	test('an upstream failure is shown to the user instead of hanging', async ({page, t, allowErrors}) => {
		allowErrors(/stream|upstream|500/i);
		await page.goto('/');
		await pickModel(page, t, 'mock-error');
		await send(page, 'this will fail');
		await expect(page.getByText('this will fail').first()).toBeVisible();
		await expect(page.locator('.animate-spin')).toHaveCount(0, {timeout: 20_000});
		await expect(page.getByText('upstream provider error (500): mock upstream failure').first()).toBeVisible();
		await expect(page.getByText('Mock reply: this will fail')).toHaveCount(0);
	});

	test('web chat usage shows up in personal analytics', async ({page, t, api}) => {
		const before = await api.get('/api/v1/me/analytics');
		await page.goto('/');
		await pickModel(page, t, 'mock-chat');
		await send(page, 'count my usage');
		await expect(page.getByText('Mock reply: count my usage')).toBeVisible();
		await expect.poll(async () => JSON.stringify(await api.get('/api/v1/me/analytics')) !== JSON.stringify(before), {timeout: 15_000}).toBe(true);
	});
});
