import type {Page} from '@playwright/test';
import {test, expect} from './support/fixtures';

const PREFIX = 'Acme Labs: ';

async function dressUpModel(page: Page, modelId: string) {
	await page.route(/\/api\/v1\/models(\?|$)/, async route => {
		const response = await route.fetch();
		const body = await response.json();
		for (const model of body.items ?? []) {
			if (model.id !== modelId) continue;
			model.display_name = `${PREFIX}${model.display_name}`;
			model.capabilities = [...new Set([...model.capabilities, 'REASONING', 'REASONING_EFFORT_LOW', 'REASONING_EFFORT_HIGH'])];
			model.provider = {...model.provider, kind: 'OPENROUTER'};
		}
		await route.fulfill({response, json: body});
	});
	await page.route(/\/api\/v1\/base(\?|$)/, async route => {
		const response = await route.fetch();
		const body = await response.json();
		body.chat_provider_selector = {enabled: true, provider_kinds: ['OPENROUTER']};
		await route.fulfill({response, json: body});
	});
	await page.route(/\/api\/v1\/models\/[^/]+\/provider-options$/, route =>
		route.fulfill({
			json: {
				gateway_model_id: null,
				availability_state: null,
				options: [
					{
						id: 'acme-cloud',
						provider_slug: 'acme-cloud',
						provider_name: 'Acme Cloud',
						endpoint_name: null,
						status: 0,
						quantization: null,
						context_length: 128000,
						max_completion_tokens: null,
						latency: 300,
						throughput: 80,
						uptime: 99.9,
						price_input: 1,
						price_output: 2,
					},
				],
			},
		}),
	);
}

async function pickSeedModel(page: Page, t: (key: string) => string, displayName: string) {
	await page.getByTestId('model-selector').click();
	const search = page.getByPlaceholder(t('chat.model_selector.search_models'));
	await search.fill(displayName);
	await page.getByTestId('picker-model').filter({hasText: displayName}).first().click();
	await expect(search).toBeHidden();
}

test.describe('chat composer bar', () => {
	test('shows the model without its provider prefix and keeps changed settings visible', async ({page, t, seed}) => {
		await dressUpModel(page, seed.model.id);
		await page.goto('/');
		await pickSeedModel(page, t, seed.model.displayName);

		const selector = page.getByTestId('model-selector');
		await expect(selector).toHaveAttribute('title', `${PREFIX}${seed.model.displayName}`);
		await expect(selector.getByTestId('model-selector-label')).toHaveText(seed.model.displayName);

		const settings = page.getByTestId('composer-settings');
		await expect(settings.getByTestId('composer-chip-reasoning')).toHaveText(t('chat.reasoning_selector.low'));
		await expect(settings.getByTestId('composer-chip-provider')).toHaveCount(0);

		await settings.click();
		const panel = page.getByTestId('composer-settings-panel');
		await expect(panel.getByRole('group', {name: t('chat.composer.reasoning')})).toBeVisible();
		await expect(panel.getByRole('group', {name: t('chat.sampling.title')})).toBeVisible();
		const provider = panel.getByRole('group', {name: t('chat.provider_selector.title')});
		await expect(provider).toContainText(t('chat.provider_selector.automatic'));

		await panel.getByTestId('reasoning-selector').click();
		await page.getByRole('option', {name: t('chat.reasoning_selector.high')}).click();
		await expect(settings.getByTestId('composer-chip-reasoning')).toHaveText(t('chat.reasoning_selector.high'));

		await provider.getByRole('button').click();
		await page.getByRole('button', {name: /Acme Cloud/}).click();
		await expect(settings.getByTestId('composer-chip-provider')).toHaveText('Acme Cloud');
		await expect(settings).toHaveAttribute('aria-label', new RegExp(`${t('chat.reasoning_selector.high')}.*Acme Cloud`));
	});

	test('every control stays reachable at 375px without overflowing the bar', async ({page, t, seed}) => {
		await dressUpModel(page, seed.model.id);
		await page.setViewportSize({width: 375, height: 812});
		await page.goto('/');
		await pickSeedModel(page, t, seed.model.displayName);

		const composer = page.getByTestId('chat-composer');
		for (const id of ['model-selector', 'composer-settings', 'context-indicator', 'composer-send']) {
			await expect(composer.getByTestId(id)).toBeInViewport({ratio: 1});
		}
		await expect(composer.getByTestId('composer-settings-count')).toHaveText('1');
		const overflow = await composer.evaluate(element => element.scrollWidth - element.clientWidth);
		expect(overflow).toBeLessThanOrEqual(0);

		await composer.getByTestId('composer-settings').click();
		const panel = page.getByTestId('composer-settings-panel');
		await expect(panel).toBeInViewport({ratio: 1});
		await panel.getByTestId('sampling-selector').click();
		await expect(page.getByTestId('sampling-panel')).toBeInViewport({ratio: 1});
	});
});
