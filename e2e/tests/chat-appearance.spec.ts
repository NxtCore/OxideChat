import type {Page} from '@playwright/test';
import {test, expect, type Api} from './support/fixtures';
import {LLM} from './support/constants';
import {ensurePricing} from './support/providers';

const DEFAULTS = {streaming_animation: 'fade', use_remend: true};

async function streamSlowReply(page: Page, api: Api, t: (key: string) => string, providerId: string) {
	const models: {id: string; model_id: string; display_name: string}[] = await api.get(`/api/v1/admin/providers/${providerId}/models`);
	const slow = models.find(model => model.model_id.endsWith('mock-slow'));
	expect(slow, 'the stand-in serves mock-slow').toBeTruthy();
	await ensurePricing(api, slow!.id);

	await page.goto('/');
	await page.getByTestId('model-selector').click();
	const search = page.getByPlaceholder(t('chat.model_selector.search_models'));
	await search.fill(slow!.display_name);
	await page.getByText(slow!.display_name, {exact: true}).last().click();
	await expect(search).toBeHidden();

	const composer = page.locator('textarea').first();
	await composer.fill(`Stream slowly. Ref ${Date.now()}`);
	await composer.press('Enter');
	return page.locator('[data-testid="message"][data-role="assistant"]').last();
}

test.describe('chat appearance', () => {
	test.afterEach(async ({api}) => {
		await api.patch('/api/v1/users/@me/preferences', DEFAULTS);
	});

	test('reply animation and Markdown repair are saved with the appearance settings', async ({page, api, t}) => {
		await page.goto('/settings/appearance');
		await page.getByTestId('appearance-animation-typewriter').click();
		await page.getByTestId('appearance-remend').click();
		await page.getByTestId('settings-save-bar-save').click();
		await expect(page.getByText(t('settings.appearance.saved')).first()).toBeVisible();

		const saved = await api.get('/api/v1/users/@me/preferences');
		expect(saved).toMatchObject({streaming_animation: 'typewriter', use_remend: false});

		await page.reload();
		await expect(page.getByTestId('appearance-remend')).toHaveAttribute('data-state', 'unchecked');
		await expect(page.getByTestId('appearance-animation-typewriter')).toHaveClass(/ring-primary/);
		await expect(page.getByTestId('settings-save-bar')).toBeHidden();
	});

	test('fade and slide animate new text, the typewriter types behind a caret', async ({page, api, t, seed}) => {
		test.skip(!LLM.isMock, 'needs the slow model of the local stand-in');

		for (const animation of ['fade', 'slide'] as const) {
			await api.patch('/api/v1/users/@me/preferences', {streaming_animation: animation});
			const reply = await streamSlowReply(page, api, t, seed.providerId);
			await expect(reply.locator(`.stream-${animation}`).first()).toBeAttached({timeout: 30_000});
			await expect(reply.locator('.stream-caret')).toHaveCount(0);
		}

		await api.patch('/api/v1/users/@me/preferences', {streaming_animation: 'typewriter'});
		const typed = await streamSlowReply(page, api, t, seed.providerId);
		await expect(typed.locator('.stream-caret')).toBeAttached({timeout: 30_000});
		await expect(typed.locator('.stream-fade, .stream-slide')).toHaveCount(0);

		await api.patch('/api/v1/users/@me/preferences', {streaming_animation: 'none'});
		const plain = await streamSlowReply(page, api, t, seed.providerId);
		await expect(plain.locator('.prose').first()).not.toBeEmpty({timeout: 30_000});
		await expect(plain.locator('.stream-fade, .stream-slide, .stream-caret')).toHaveCount(0);
	});
});
