import {test, expect} from './support/fixtures';

test.describe('model pricing', () => {
	test('an admin adds context, service-tier and time-window rates without retyping the base price', async ({page, api, t, seed}) => {
		const url = `/api/v1/admin/models/${seed.model.id}/pricing`;
		const original = await api.get(url);
		const bar = page.getByTestId('settings-save-bar');

		try {
			await api.delete(url);
			await page.goto(`/settings/models/${seed.model.id}`);
			await page.getByRole('tab', {name: t('settings.models.editor.providers')}).click();

			await page.getByTestId('pricing-input').fill(String(seed.pricing.input));
			await page.getByTestId('pricing-output').fill(String(seed.pricing.output));
			await page.getByTestId('pricing-tiers-custom').click();
			await page.getByTestId('pricing-tier-add').click();
			await page.getByTestId('pricing-tier-threshold-0').fill('100000');
			await page.getByTestId('pricing-tier-input-0').fill('99');
			await page.getByTestId('pricing-service-custom').click();
			await page.getByTestId('pricing-service-add').click();
			await page.getByTestId('pricing-service-name-0').fill('Flex');
			await page.getByTestId('pricing-service-output-0').fill('0.5');
			await page.getByTestId('pricing-window-add').click();
			await page.getByTestId('pricing-window-day-mon-0').click();
			await page.getByTestId('pricing-window-start-0').fill('16:30');
			await page.getByTestId('pricing-window-end-0').fill('00:30');
			await page.getByTestId('pricing-window-input-0').fill('0.001');

			await page.getByTestId('settings-save-bar-save').click();
			await expect(bar).toBeHidden();

			const saved = await api.get(url);
			expect(saved.override_pricing.tiers).toEqual([{min_context_tokens: 100000, input: 99}]);
			expect(saved.override_pricing.service_tiers).toEqual({flex: {output: 0.5, tiers: []}});
			expect(saved.override_pricing.windows).toEqual([{days: ['mon'], start: '16:30', end: '00:30', timezone: 'UTC', rates: {input: 0.001}}]);
			expect(saved.effective.summary.input).toEqual({min: 0.001, max: 99});

			const billed = page.getByTestId('pricing-summary-billed');
			await expect(billed).toContainText('$0.001–$99');
			await expect(billed.getByTestId('pricing-special-rates')).toBeVisible();

			await page.goto('/developer/models');
			await page.getByTestId('model-search').fill(seed.model.publicId);
			const row = page.getByTestId(`model-row-${seed.model.publicId}`);
			await expect(row).toContainText('$0.001–$99');
			await row.getByTestId('pricing-special-rates').hover();
			await expect(page.getByText(t('pricing.summary_service', {names: 'flex'})).first()).toBeVisible();
		} finally {
			if (original.override_pricing) await api.put(url, {pricing: original.override_pricing});
			else await api.delete(url);
		}
	});

	test('a partial override keeps the catalog rates it leaves empty', async ({page, api, t, seed}) => {
		const url = `/api/v1/admin/models/${seed.model.id}/pricing`;
		const original = await api.get(url);

		try {
			await api.put(url, {pricing: {input: seed.pricing.input, output: seed.pricing.output, cache_read: 0.01}});
			await page.goto(`/settings/models/${seed.model.id}`);
			await page.getByRole('tab', {name: t('settings.models.editor.providers')}).click();
			await page.getByTestId('pricing-cache_read').fill('');
			await page.getByTestId('pricing-reasoning').fill('3');
			await page.getByTestId('settings-save-bar-save').click();
			await expect(page.getByTestId('settings-save-bar')).toBeHidden();

			const saved = await api.get(url);
			expect(saved.override_pricing.reasoning).toBe(3);
			expect(saved.override_pricing.cache_read).toBeUndefined();
			expect(saved.effective.price.cache_read).toBe(saved.catalog?.price.cache_read);
		} finally {
			if (original.override_pricing) await api.put(url, {pricing: original.override_pricing});
			else await api.delete(url);
		}
	});

	test('overlapping time windows are refused with a reason and keep the edits', async ({page, t, seed}) => {
		await page.goto(`/settings/models/${seed.model.id}`);
		await page.getByRole('tab', {name: t('settings.models.editor.providers')}).click();
		await page.getByTestId('pricing-input').fill(String(seed.pricing.input));
		await page.getByTestId('pricing-output').fill(String(seed.pricing.output));
		await page.getByTestId('pricing-window-add').click();
		await page.getByTestId('pricing-window-add').click();
		await page.getByTestId('settings-save-bar-save').click();

		await expect(page.getByText(t('pricing.error.overlapping_windows')).first()).toBeVisible();
		await expect(page.getByTestId('settings-save-bar')).toBeVisible();
	});
});
