import type {Page} from '@playwright/test';
import {test, expect} from './support/fixtures';

async function pickModel(page: Page, t: (key: string) => string, displayName: string) {
	await page.locator('button[aria-haspopup="dialog"]').filter({has: page.locator('svg[class*="chevron-down"]')}).first().click();
	const search = page.getByPlaceholder(t('chat.model_selector.search_models'));
	await search.fill(displayName);
	await page.getByText(displayName, {exact: true}).last().click();
	await expect(search).toBeHidden();
}

const assistantMessages = (page: Page) => page.locator('[data-testid="message"][data-role="assistant"]');

async function chatWithReply(page: Page, t: (key: string) => string, displayName: string) {
	await page.goto('/');
	await pickModel(page, t, displayName);
	const composer = page.locator('textarea').first();
	await composer.fill('Say ok in one word.');
	await composer.press('Enter');
	await expect(assistantMessages(page)).toHaveCount(1, {timeout: 60_000});
	await expect(page.getByTestId('streaming-message')).toHaveCount(0, {timeout: 60_000});
	const reply = assistantMessages(page).first();
	await expect(reply.getByTestId('message-action-copy')).toBeAttached();
	await reply.hover();
	return reply;
}

test.describe('message actions', () => {
	test('copy puts the reply on the clipboard', async ({page, t, seed, context}) => {
		await context.grantPermissions(['clipboard-read', 'clipboard-write']);
		const reply = await chatWithReply(page, t, seed.model.displayName);
		const text = (await reply.locator('.prose').first().innerText()).trim();
		await reply.getByTestId('message-action-copy').click();
		await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(text);
	});

	test('branch opens a new chat', async ({page, t, seed}) => {
		const reply = await chatWithReply(page, t, seed.model.displayName);
		const url = page.url();
		await reply.getByTestId('message-action-branch').click();
		await expect(page).not.toHaveURL(url);
		await expect(page).toHaveURL(/\/chats\/[0-9a-f-]{36}$/);
	});

	test('regenerate adds a second version of the reply', async ({page, t, seed}) => {
		const reply = await chatWithReply(page, t, seed.model.displayName);
		await reply.getByTestId('message-action-regenerate').click();
		await expect(assistantMessages(page).first().getByText('2/2')).toBeAttached({timeout: 60_000});
	});

	test('every action shows a tooltip on hover', async ({page, t, seed}) => {
		const reply = await chatWithReply(page, t, seed.model.displayName);
		for (const [id, key] of [
			['copy', 'chat.message_actions.copy_message'],
			['regenerate', 'chat.message_actions.regenerate'],
			['branch', 'chat.message_actions.branch_to_new_chat'],
		]) {
			await page.mouse.move(0, 0);
			await expect(page.locator('[data-slot="tooltip-content"]')).toHaveCount(0);
			await reply.getByTestId(`message-action-${id}`).hover();
			await expect(page.locator('[data-slot="tooltip-content"]').filter({hasText: t(key)})).toBeVisible();
		}
	});

	test('hovering info shows the reply details', async ({page, t, seed}) => {
		const reply = await chatWithReply(page, t, seed.model.displayName);
		await reply.getByTestId('message-action-info').hover();
		const details = page.locator('[data-slot="tooltip-content"]').filter({hasText: t('chat.message_actions.details')});
		await expect(details).toBeVisible();
		await expect(details).toContainText(t('chat.message_actions.total_cost'));
		await expect(details).toContainText(t('chat.message_actions.input_tokens'));
		await expect(details.getByTestId('message-details-ttft')).toContainText(t('chat.message_actions.time_to_first_token'));
		await expect(details.getByTestId('message-details-speed')).toContainText(/\d/);
		await expect(details.getByTestId('message-details-finish')).toContainText(t('chat.message_actions.finish_reasons.stop'));
	});
});
