import type {Page} from '@playwright/test';
import {test, expect} from './support/fixtures';
import {expectedCost} from './support/cost';
import {chatCompletion, createKeyViaApi, uniqueName, waitForLog} from './support/gateway';
import {LLM} from './support/constants';

const REPORTED_CREDITS = 86;

async function openProvider(page: Page, t: (key: string) => string, providerName: string) {
	await page.goto('/settings/providers');
	await expect(page.getByText(t('settings.providers.configured'), {exact: true}).first()).toBeVisible();
	const card = page
		.locator('div')
		.filter({has: page.getByRole('heading', {name: providerName, exact: true})})
		.filter({has: page.locator('.lucide-settings-2')})
		.last();
	await card.getByRole('button').filter({has: page.locator('.lucide-settings-2')}).click();
	await expect(page.getByTestId('provider-reported-cost-unit')).toBeVisible();
}

async function pickUnit(page: Page, t: (key: string) => string, unit: string) {
	await page.getByTestId('provider-reported-cost-unit').click();
	await page.getByRole('option', {name: t(`settings.providers.reported_cost_${unit}`)}).click();
}

async function save(page: Page, t: (key: string) => string) {
	await page.getByRole('dialog').getByRole('button', {name: t('common.save'), exact: true}).click();
}

async function gatewayRequest(page: Page, api: any, teamId: string, model: string) {
	const key = await createKeyViaApi(api, teamId, uniqueName('reported-cost'));
	try {
		const response = await chatCompletion(page.request, key.token, {model, max_tokens: 20});
		expect(response.status(), await response.text()).toBe(200);
		const body = await response.json();
		const log = await waitForLog(api, key.id, response.headers()['x-request-id']);
		return {cost: Number(log.cost_total), usage: {input: body.usage.prompt_tokens as number, output: body.usage.completion_tokens as number}};
	} finally {
		await api.post(`/api/v1/developer/keys/${key.id}/revoke`);
	}
}

test.describe('provider reported cost', () => {
	test.afterEach(async ({api, seed}) => {
		await api.put(`/api/v1/admin/providers/${seed.providerId}`, {reported_cost: null});
	});

	test('credits are saved from the provider dialog and converted with their value', async ({page, t, api, seed}) => {
		const provider = await api.get(`/api/v1/admin/providers/${seed.providerId}`);
		await openProvider(page, t, provider.name);
		await pickUnit(page, t, 'credits');
		await page.getByTestId('provider-reported-cost-credit').fill('0.0001');
		await save(page, t);
		await expect(page.getByText(t('settings.providers.save_success')).first()).toBeVisible();

		const saved = await api.get(`/api/v1/admin/providers/${seed.providerId}`);
		expect(saved.reported_cost.unit).toBe('credits');
		expect(Number(saved.reported_cost.usd_per_credit)).toBe(0.0001);
		expect(saved.reported_cost.source ?? null).toBeNull();

		if (LLM.isMock) {
			const {cost} = await gatewayRequest(page, api, seed.teamId, seed.model.publicId);
			expect(cost).toBeCloseTo(REPORTED_CREDITS * 0.0001, 9);
		}
	});

	test('a cost field that is not in the response falls back to token prices', async ({page, api, seed}) => {
		test.skip(!LLM.isMock, 'only the local stand-in reports a credit cost');
		await api.put(`/api/v1/admin/providers/${seed.providerId}`, {reported_cost: {unit: 'credits', usd_per_credit: '0.0001', source: 'usage.not_a_cost'}});
		const {cost, usage} = await gatewayRequest(page, api, seed.teamId, seed.model.publicId);
		expect(cost).toBeCloseTo(expectedCost(seed.pricing, usage), 9);
	});

	test('a scale turns provider credits into the unit, as Cortecs reports them', async ({page, api, seed}) => {
		test.skip(!LLM.isMock, 'only the local stand-in reports a credit cost');
		await api.put(`/api/v1/admin/providers/${seed.providerId}`, {reported_cost: {unit: 'usd', scale: '0.000001'}});
		const {cost} = await gatewayRequest(page, api, seed.teamId, seed.model.publicId);
		expect(cost).toBeCloseTo(REPORTED_CREDITS * 0.000001, 12);
	});

	test('a currency is saved and shows its exchange rate status', async ({page, t, api, seed}) => {
		const provider = await api.get(`/api/v1/admin/providers/${seed.providerId}`);
		await openProvider(page, t, provider.name);
		await pickUnit(page, t, 'currency');
		await expect(page.getByTestId('provider-reported-cost-rate')).not.toBeEmpty();
		await page.getByTestId('provider-reported-cost-source').fill('header:x-provider-cost');
		await page.getByTestId('provider-reported-cost-scale').fill('0.000001');
		await save(page, t);
		await expect(page.getByText(t('settings.providers.save_success')).first()).toBeVisible();
		const saved = (await api.get(`/api/v1/admin/providers/${seed.providerId}`)).reported_cost;
		expect({...saved, scale: Number(saved.scale)}).toEqual({unit: 'currency', currency: 'EUR', source: 'header:x-provider-cost', scale: 0.000001});
	});

	test('credits without a value are rejected before saving', async ({page, t, api, seed}) => {
		const provider = await api.get(`/api/v1/admin/providers/${seed.providerId}`);
		await openProvider(page, t, provider.name);
		await pickUnit(page, t, 'credits');
		await save(page, t);
		await expect(page.getByText(t('settings.providers.reported_cost_invalid')).first()).toBeVisible();
		expect((await api.get(`/api/v1/admin/providers/${seed.providerId}`)).reported_cost).toBeNull();
	});
});
