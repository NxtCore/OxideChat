import type {Page} from '@playwright/test';
import {test, expect} from './support/fixtures';
import {uniqueName} from './support/gateway';
import {createBrokenProvider} from './support/providers';

async function pickModel(page: Page, t: (key: string) => string, displayName: string) {
	await page.locator('button[aria-haspopup="dialog"]').filter({has: page.locator('svg[class*="chevron-down"]')}).first().click();
	const search = page.getByPlaceholder(t('chat.model_selector.search_models'));
	await search.fill(displayName);
	await page.getByText(displayName, {exact: true}).last().click();
	await expect(search).toBeHidden();
}

async function send(page: Page, text: string) {
	const composer = page.locator('textarea').first();
	await composer.fill(text);
	await composer.press('Enter');
}

const assistantMessages = (page: Page) => page.locator('[data-testid="message"][data-role="assistant"]');

async function expectReplyCount(page: Page, count: number) {
	await expect(assistantMessages(page)).toHaveCount(count, {timeout: 60_000});
	await expect(page.getByTestId('streaming-message')).toHaveCount(0, {timeout: 60_000});
	await expect(assistantMessages(page).nth(count - 1).locator('.prose').first()).not.toBeEmpty();
}

test.describe('web chat', () => {
	test('sends a message, streams a reply and keeps both after reload', async ({page, t, seed}) => {
		await page.goto('/');
		await pickModel(page, t, seed.model.displayName);
		const prompt = `Reply with one short word. Ref ${Date.now()}`;
		await send(page, prompt);

		await expect(page).toHaveURL(/\/chats\/[0-9a-f-]{36}/);
		await expectReplyCount(page, 1);
		await expect(page.locator('textarea').first()).toHaveValue('');
		const reply = (await assistantMessages(page).first().locator('.prose').first().innerText()).trim();

		await page.reload();
		await expect(page.locator('[data-testid="message"][data-role="user"]').filter({hasText: prompt})).toHaveCount(1);
		await expect(assistantMessages(page).first()).toContainText(reply);
	});

	test('a follow-up message in the same chat gets its own reply', async ({page, t, seed}) => {
		await page.goto('/');
		await pickModel(page, t, seed.model.displayName);
		await send(page, 'Say hi in one word.');
		await expectReplyCount(page, 1);
		const url = page.url();
		await send(page, 'Now say bye in one word.');
		await expectReplyCount(page, 2);
		expect(page.url()).toBe(url);
	});

	test('a provider failure is shown to the user instead of hanging', async ({page, t, api, allowErrors}) => {
		allowErrors(/stream|upstream|40\d|50\d/i);
		const broken = await createBrokenProvider(api, uniqueName('BrokenChat'));
		try {
			await page.goto('/');
			await pickModel(page, t, broken.model.displayName);
			await send(page, 'this will fail');
			await expect(page.getByText('this will fail').first()).toBeVisible();
			await expect(page.locator('.animate-spin')).toHaveCount(0, {timeout: 30_000});
			await expect(page.getByText(/upstream provider error|not found/i).first()).toBeVisible();
		} finally {
			await api.delete(`/api/v1/admin/providers/${broken.providerId}`);
		}
	});

	test('web chat usage shows up in personal analytics', async ({page, t, api, seed}) => {
		const before = JSON.stringify(await api.get('/api/v1/me/analytics'));
		await page.goto('/');
		await pickModel(page, t, seed.model.displayName);
		await send(page, 'Say ok.');
		await expectReplyCount(page, 1);
		await expect.poll(async () => JSON.stringify(await api.get('/api/v1/me/analytics')) !== before, {timeout: 15_000}).toBe(true);
	});
});

test.describe('chat navigation', () => {
	const activeRow = (page: Page) => page.locator('[data-active="true"]');

	for (const [menuKey, backKey] of [['settings.title', 'settings.back'], ['developer.title', 'developer.back']]) {
		test(`leaving ${menuKey} returns to the chat it was opened from`, async ({page, t, api}) => {
			const chat = await api.post('/api/v1/chats');
			try {
				await page.goto(`/chats/${chat.id}`);
				await expect(activeRow(page)).toHaveCount(1);
				await page.locator('[data-slot="sidebar"] button[aria-haspopup="menu"]').click();
				await page.getByRole('menuitem', {name: t(menuKey)}).click();
				await page.getByRole('button', {name: t(backKey)}).click();
				await expect(page).toHaveURL(new RegExp(`/chats/${chat.id}$`));
				await expect(activeRow(page)).toHaveCount(1);
			} finally {
				await api.delete(`/api/v1/chats/${chat.id}`);
			}
		});
	}

	test('reaching the start page with the browser back button clears the active chat', async ({page, api}) => {
		const title = uniqueName('BackChat');
		const chat = await api.post('/api/v1/chats');
		await api.patch(`/api/v1/chats/${chat.id}`, {title});
		try {
			await page.goto('/');
			await page.getByRole('button', {name: title}).click();
			await expect(page).toHaveURL(new RegExp(`/chats/${chat.id}$`));
			await expect(activeRow(page)).toHaveCount(1);
			await page.goBack();
			await expect(page).toHaveURL(/\/$/);
			await expect(activeRow(page)).toHaveCount(0);
		} finally {
			await api.delete(`/api/v1/chats/${chat.id}`);
		}
	});

	test('the sidebar or its toggle is available at every width', async ({page, t}) => {
		const newChat = page.getByRole('button', {name: t('chat.list.new_chat')}).first();
		const toggle = page.locator('[data-sidebar="trigger"]');
		await page.goto('/');
		for (const width of [1280, 1000, 900, 768]) {
			await page.setViewportSize({width, height: 800});
			await expect(newChat).toBeVisible();
		}
		await page.setViewportSize({width: 767, height: 800});
		await expect(newChat).toBeHidden();
		await toggle.click();
		await expect(newChat).toBeVisible();
	});
});
