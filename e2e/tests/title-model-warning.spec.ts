import type {Page} from '@playwright/test';
import {test, expect} from './support/fixtures';

async function markReasoning(page: Page, reasoning: boolean) {
	await page.route(/\/api\/v1\/admin\/models(\/[0-9a-f-]{36})?(\?|$)/, async route => {
		const response = await route.fetch();
		const body = await response.json();
		const models = body.items ?? [body];
		for (const model of models) {
			model.capabilities = reasoning ? [...new Set([...model.capabilities, 'REASONING'])] : model.capabilities.filter((c: string) => !c.startsWith('REASONING'));
		}
		await route.fulfill({response, json: body});
	});
}

async function pickTitleModel(page: Page, t: (key: string) => string, displayName: string) {
	await page.goto('/settings/admin-config');
	await page.getByTestId('config-title-generation').click();
	await page.getByRole('button', {name: t('settings.admin_config.title_model_chat')}).click();
	await page.getByPlaceholder(t('chat.model_selector.search_models')).fill(displayName);
	await page.getByText(displayName, {exact: true}).last().click();
}

test.describe('title model', () => {
	test('picking a reasoning model warns the admin', async ({page, t, seed}) => {
		await markReasoning(page, true);
		await pickTitleModel(page, t, seed.model.displayName);
		await expect(page.getByTestId('config-title-model-reasoning-warning')).toContainText(t('settings.admin_config.title_model_reasoning_warning'));
	});

	test('a model without reasoning shows no warning and only text models are offered', async ({page, t, seed}) => {
		await markReasoning(page, false);
		const listRequest = page.waitForRequest(request => /\/api\/v1\/admin\/models\?/.test(request.url()));
		await pickTitleModel(page, t, seed.model.displayName);
		expect(new URL((await listRequest).url()).searchParams.get('output_only')).toBe('text');
		await expect(page.getByRole('button', {name: seed.model.displayName})).toBeVisible();
		await expect(page.getByTestId('config-title-model-reasoning-warning')).toHaveCount(0);
	});
});
