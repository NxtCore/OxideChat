import type {Page} from '@playwright/test';
import {test, expect} from './support/fixtures';
import {chatCompletion, createKeyViaApi, uniqueName, waitForLog} from './support/gateway';
import {LLM} from './support/constants';

const REPORTED_CREDITS = 86;

async function openSeedProvider(page: Page, t: (key: string) => string, providerName: string) {
	await page.goto('/settings/providers');
	await expect(page.getByText(t('settings.providers.configured'), {exact: true}).first()).toBeVisible();
	const card = page
		.locator('div')
		.filter({has: page.getByRole('heading', {name: providerName, exact: true})})
		.filter({has: page.locator('.lucide-settings-2')})
		.last();
	await card.getByRole('button').filter({has: page.locator('.lucide-settings-2')}).click();
	await expect(page.getByTestId('provider-cost-conversion')).toBeVisible();
}

async function loggedCost(page: Page, api: any, teamId: string, model: string) {
	const key = await createKeyViaApi(api, teamId, uniqueName('cost-conversion'));
	try {
		const response = await chatCompletion(page.request, key.token, {model, max_tokens: 20});
		expect(response.status(), await response.text()).toBe(200);
		const log = await waitForLog(api, key.id, response.headers()['x-request-id']);
		return Number(log.cost_total);
	} finally {
		await api.post(`/api/v1/developer/keys/${key.id}/revoke`);
	}
}

test.describe('provider cost conversion', () => {
	test('the provider and model factors are saved from their settings pages', async ({page, t, api, seed}) => {
		const provider = await api.get(`/api/v1/admin/providers/${seed.providerId}`);
		const modelUrl = `/api/v1/admin/models/${seed.model.id}`;
		try {
			await openSeedProvider(page, t, provider.name);
			await page.getByTestId('provider-cost-conversion').fill('0.0001');
			await page.getByRole('dialog').getByRole('button', {name: t('common.save'), exact: true}).click();
			await expect(page.getByText(t('settings.providers.save_success')).first()).toBeVisible();
			expect(Number((await api.get(`/api/v1/admin/providers/${seed.providerId}`)).cost_conversion_factor)).toBe(0.0001);

			await page.goto(`/settings/models/${seed.model.id}`);
			await page.getByRole('tab', {name: t('settings.models.editor.providers')}).click();
			const modelFactor = page.getByTestId('pricing-cost-conversion');
			await expect(modelFactor).toHaveAttribute('placeholder', /0\.0001/);
			await modelFactor.fill('0.001');
			await page.getByTestId('settings-save-bar-save').click();
			await expect(page.getByTestId('settings-save-bar')).toBeHidden();
			expect(Number((await api.get(`${modelUrl}/pricing`)).cost_conversion_factor)).toBe(0.001);
		} finally {
			await api.patch(modelUrl, {cost_conversion_factor: null});
			await api.put(`/api/v1/admin/providers/${seed.providerId}`, {cost_conversion_factor: null});
		}
	});

	test('an invalid factor is rejected before saving', async ({page, t, api, seed}) => {
		const provider = await api.get(`/api/v1/admin/providers/${seed.providerId}`);
		await openSeedProvider(page, t, provider.name);
		await page.getByTestId('provider-cost-conversion').fill('0');
		await page.getByRole('dialog').getByRole('button', {name: t('common.save'), exact: true}).click();
		await expect(page.getByText(t('settings.providers.cost_conversion_invalid')).first()).toBeVisible();
		expect((await api.get(`/api/v1/admin/providers/${seed.providerId}`)).cost_conversion_factor).toBeNull();
	});

	test('reported credits are converted with the model factor, else the provider factor', async ({page, api, seed}) => {
		test.skip(!LLM.isMock, 'only the local stand-in reports a credit cost');
		const modelUrl = `/api/v1/admin/models/${seed.model.id}`;
		try {
			await api.put(`/api/v1/admin/providers/${seed.providerId}`, {cost_conversion_factor: '0.0001'});
			expect(await loggedCost(page, api, seed.teamId, seed.model.publicId)).toBeCloseTo(REPORTED_CREDITS * 0.0001, 9);

			await api.patch(modelUrl, {cost_conversion_factor: '0.001'});
			expect(await loggedCost(page, api, seed.teamId, seed.model.publicId)).toBeCloseTo(REPORTED_CREDITS * 0.001, 9);
		} finally {
			await api.patch(modelUrl, {cost_conversion_factor: null});
			await api.put(`/api/v1/admin/providers/${seed.providerId}`, {cost_conversion_factor: null});
		}
	});
});
