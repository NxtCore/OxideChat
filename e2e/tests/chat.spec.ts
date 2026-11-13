import type {Page} from '@playwright/test';
import {test, expect} from './support/fixtures';
import {expectedCost} from './support/cost';
import {uniqueName} from './support/gateway';
import {createBrokenProvider} from './support/providers';

async function pickModel(page: Page, t: (key: string) => string, displayName: string) {
	await page.getByTestId('model-selector').click();
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

	test('a web chat reply is priced by Omniference with the model price', async ({page, t, api, seed}) => {
		const spentBefore = (await api.get('/api/v1/me/analytics')).reduce((sum: number, row: any) => sum + Number(row.cost_total), 0);
		await page.goto('/');
		await pickModel(page, t, seed.model.displayName);
		await send(page, 'Say ok.');
		await expectReplyCount(page, 1);
		const chatId = new URL(page.url()).pathname.split('/').pop();
		const messages = await api.get(`/api/v1/chats/${chatId}/messages`);
		const reply = messages.find((message: any) => message.role === 'assistant');
		const usage = {input: reply.usage_details.input_tokens, output: reply.usage_details.output_tokens, reasoning: reply.usage_details.reasoning_tokens ?? 0};
		const expected = expectedCost(seed.pricing, usage);
		expect(expected).toBeGreaterThan(0);
		expect(Number(reply.cost_details.total)).toBeCloseTo(expected, 9);
		await expect
			.poll(async () => (await api.get('/api/v1/me/analytics')).reduce((sum: number, row: any) => sum + Number(row.cost_total), 0) - spentBefore, {timeout: 15_000})
			.toBeCloseTo(expected, 9);
	});

	test('an image dropped onto the composer is attached', async ({page, t, seed}) => {
		await page.route(/\/api\/v1\/models(\?|$)/, async route => {
			const response = await route.fetch();
			const body = await response.json();
			for (const model of body.items ?? []) {
				if (model.display_name === seed.model.displayName) model.input_modalities = [...new Set([...model.input_modalities, 'IMAGE'])];
			}
			await route.fulfill({response, json: body});
		});
		await page.goto('/');
		await pickModel(page, t, seed.model.displayName);

		const png = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';
		const dataTransfer = await page.evaluateHandle(base64 => {
			const bytes = Uint8Array.from(atob(base64), char => char.charCodeAt(0));
			const transfer = new DataTransfer();
			transfer.items.add(new File([bytes], 'dot.png', {type: 'image/png'}));
			return transfer;
		}, png);
		const composer = page.getByTestId('chat-composer');
		await composer.dispatchEvent('dragenter', {dataTransfer});
		await composer.dispatchEvent('dragover', {dataTransfer});
		await composer.dispatchEvent('drop', {dataTransfer});

		await expect(page.getByTestId('attached-image')).toHaveCount(1);
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

	test('the first reply gives an untitled chat a generated title', async ({page, t, api, seed}) => {
		await page.goto('/settings/admin-config');
		await page.getByTestId('config-title-generation').click();
		await page.getByTestId('settings-save-bar-save').click();
		await expect(page.getByTestId('settings-save-bar')).toBeHidden();
		try {
			expect((await api.get('/api/v1/base')).title_generation.enabled).toBe(true);
			await page.goto('/');
			await pickModel(page, t, seed.model.displayName);
			await send(page, 'Suggest a name for a pet turtle in one word.');
			await expectReplyCount(page, 1);
			const chatId = page.url().split('/chats/')[1];
			await expect.poll(async () => (await api.get(`/api/v1/chats/${chatId}`)).chat.title, {timeout: 60_000}).toBeTruthy();
			const {title} = (await api.get(`/api/v1/chats/${chatId}`)).chat;
			await expect(page.locator('[data-active="true"]')).toContainText(title);
		} finally {
			await api.patch('/api/v1/admin/config', {title_generation: {enabled: false, model_id: null}});
		}
	});
});
