import {test, expect} from './support/fixtures';
import {createKeyViaApi, uniqueName} from './support/gateway';

test.describe('developer area', () => {
	test('the navigation lands on API keys, shows endpoints and has one gateway controls link', async ({page, t}) => {
		await page.goto('/developer');
		await expect(page).toHaveURL(/\/developer\/keys$/);
		await expect(page.getByTestId('endpoint-openai')).toContainText('/openai/v1');
		await expect(page.getByRole('link', {name: t('developer.nav.gateway_admin')})).toHaveAttribute('href', '/settings/gateway-admin');
		await expect(page.locator('a[href="/settings/admin-config"]')).toHaveCount(0);
	});

	test('models can be found by search and opened in the playground', async ({page, api, seed}) => {
		await createKeyViaApi(api, seed.teamId, uniqueName('models-key'));
		await page.goto('/developer/models');
		await page.getByTestId('model-search').fill(seed.model.publicId);
		const row = page.getByTestId(`model-row-${seed.model.publicId}`);
		await expect(row).toBeVisible();

		await page.getByTestId('model-search').fill('no-such-model-exists');
		await expect(row).toHaveCount(0);

		await page.getByTestId('model-search').fill(seed.model.publicId);
		await row.getByRole('link').click();
		await expect(page).toHaveURL(/\/developer\/playground\?model=/);
		await expect(page.getByRole('combobox').filter({hasText: seed.model.displayName})).toBeVisible();
	});

	test('models show their price and can be filtered and sorted by real attributes', async ({page, seed}) => {
		await page.goto('/developer/models');
		const row = page.getByTestId(`model-row-${seed.model.publicId}`);
		await expect(row).toContainText(seed.model.displayName);
		await expect(row).toContainText(`$${seed.pricing.input}`);
		await expect(row).toContainText(`$${seed.pricing.output}`);

		const count = page.getByTestId('model-count');
		const unfiltered = await count.textContent();
		await page.getByTestId('model-filter-image').click();
		await expect(page.getByTestId('model-filter-image')).toHaveAttribute('aria-pressed', 'true');
		await expect(row).toHaveCount(0);
		expect(await count.textContent()).not.toBe(unfiltered);
		await page.getByTestId('model-filter-image').click();
		await expect(row).toBeVisible();

		const firstRow = page.locator('tbody tr').first();
		const firstByName = await firstRow.getAttribute('data-testid');
		await page.getByTestId('model-sort-name').click();
		const header = page.locator('th[aria-sort]');
		await expect(header).toHaveAttribute('aria-sort', 'descending');
		expect(await page.locator('tbody tr').last().getAttribute('data-testid')).toBe(firstByName);
	});

	test('the model list scrolls instead of being cut off', async ({page, seed}) => {
		await page.setViewportSize({width: 1440, height: 400});
		await page.goto('/developer/models');
		await page.getByTestId('model-search').fill(seed.model.publicId);
		const row = page.getByTestId(`model-row-${seed.model.publicId}`);
		await row.scrollIntoViewIfNeeded();
		await expect(row).toBeInViewport();
	});

	test('the playground sends a conversation, reports metrics and generates code', async ({page, t, api, seed}) => {
		const key = await createKeyViaApi(api, seed.teamId, uniqueName('playground-key'));
		await page.goto(`/developer/playground?key=${key.id}&model=${encodeURIComponent(seed.model.publicId)}`);

		await page.getByLabel(t('developer.playground.token')).fill(key.token);
		await page.getByTestId('playground-prompt').fill('Reply with the single word: pong');
		await page.getByTestId('playground-prompt').press('Control+Enter');

		await expect(page.getByTestId('playground-token-known')).toBeVisible();
		await expect(page.getByTestId('playground-user')).toContainText('pong');
		const reply = page.getByTestId('playground-assistant').last();
		await expect(reply.getByTestId('playground-metrics')).toBeVisible({timeout: 30_000});
		await expect(reply.getByTestId('playground-metrics')).toContainText(/\d+ \S+ \/ \d+/);
		await expect(reply.locator('[role=alert]')).toHaveCount(0);

		await page.getByTestId('playground-regenerate').click();
		await expect(page.getByTestId('playground-assistant')).toHaveCount(1);
		await expect(page.getByTestId('playground-assistant').getByTestId('playground-metrics')).toBeVisible({timeout: 30_000});

		await page.getByTestId('playground-tab-code').click();
		await expect(page.getByTestId('playground-code-curl')).toContainText(seed.model.publicId);
		await expect(page.getByTestId('playground-code-curl')).toContainText('YOUR_API_KEY');
		await expect(page.getByTestId('playground-code-curl')).not.toContainText(key.token);

		await page.getByTestId('playground-tab-details').click();
		await expect(page.getByTestId('playground-request')).toContainText(seed.model.publicId);
	});

	test('a rejected request shows the error with a retry', async ({page, t, api, seed}) => {
		const key = await createKeyViaApi(api, seed.teamId, uniqueName('playground-bad-key'));
		await page.goto(`/developer/playground?key=${key.id}`);
		await page.getByLabel(t('developer.playground.token')).fill('invalid-token');
		await page.getByTestId('playground-prompt').fill('hello');
		await page.getByTestId('playground-send').click();
		const alert = page.getByTestId('playground-assistant').locator('[role=alert]');
		await expect(alert).toBeVisible();
		await expect(alert.getByRole('button', {name: t('developer.playground.retry')})).toBeVisible();
	});
});
