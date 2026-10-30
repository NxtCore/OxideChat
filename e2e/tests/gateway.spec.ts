import {test, expect} from './support/fixtures';
import {expectedCost, formatMoney} from './support/cost';
import {chatCompletion, createKeyInUi, createKeyViaApi, keyIdByName, uniqueName, waitForLog} from './support/gateway';
import {createSyncedProvider} from './support/providers';
import {LLM} from './support/constants';

const PROMPT = 'Reply with the single word: pong';

function usageOf(body: any) {
	return {
		input: body.usage.prompt_tokens as number,
		output: body.usage.completion_tokens as number,
		reasoning: (body.usage.completion_tokens_details?.reasoning_tokens ?? 0) as number,
	};
}

function lastUsageChunk(sse: string) {
	const chunks = sse
		.split('\n')
		.filter(line => line.startsWith('data: ') && line !== 'data: [DONE]')
		.map(line => JSON.parse(line.slice(6)));
	return chunks.reverse().find(chunk => chunk.usage);
}

test.describe('OpenAI gateway: usage, cost and logs', () => {
	test('a request with a UI-created key is logged with the provider-reported tokens and matching cost', async ({page, t, api, seed}) => {
		const name = uniqueName('ui-key');
		const token = await createKeyInUi(page, t, name);
		const keyId = await keyIdByName(api, name);

		const models = await page.request.get('/openai/v1/models', {headers: {authorization: `Bearer ${token}`}});
		expect(models.ok()).toBe(true);
		expect((await models.json()).data.map((model: {id: string}) => model.id)).toContain(seed.model.publicId);

		const response = await chatCompletion(page.request, token, {model: seed.model.publicId, messages: [{role: 'user', content: PROMPT}], max_tokens: 20});
		expect(response.status(), await response.text()).toBe(200);
		const body = await response.json();
		expect(body.choices[0].message.content.trim().length).toBeGreaterThan(0);
		const usage = usageOf(body);
		expect(usage.input).toBeGreaterThan(0);
		expect(usage.output).toBeGreaterThan(0);

		const requestId = response.headers()['x-request-id'];
		expect(requestId).toBeTruthy();

		const log = await waitForLog(api, keyId, requestId);
		expect(log.status).toBe('success');
		expect(log.input_tokens).toBe(usage.input);
		expect(log.output_tokens).toBe(usage.output);
		expect(log.reasoning_tokens ?? 0).toBe(usage.reasoning);
		expect(Number(log.cost_total)).toBeGreaterThan(0);
		expect(Number(log.cost_total)).toBeCloseTo(expectedCost(seed.pricing, usage), 9);
		expect(log.streaming).toBe(false);
		expect(log.time_to_first_token_ms).toBeNull();
		expect(log.latency_ms).toBeGreaterThan(0);

		await page.goto('/developer/logs');
		const row = page.locator('tbody tr').filter({hasText: requestId.slice(0, 8)});
		await expect(row).toHaveCount(1);
		await expect(row).toContainText(seed.model.publicId);
		await expect(row).toContainText(name);
		await expect(row).toContainText(`${usage.input.toLocaleString('en-US')} / ${usage.output.toLocaleString('en-US')}`);
		await expect(row).toContainText(formatMoney(Number(log.cost_total)));
	});

	test('streaming requests record usage from the final chunk and time to first token', async ({page, api, seed}) => {
		const key = await createKeyViaApi(api, seed.teamId, uniqueName('stream-key'));
		const response = await chatCompletion(page.request, key.token, {
			model: seed.model.publicId,
			messages: [{role: 'user', content: PROMPT}],
			max_tokens: 20,
			stream: true,
			stream_options: {include_usage: true},
		});
		expect(response.status()).toBe(200);
		const sse = await response.text();
		expect(sse).toContain('data: [DONE]');
		const usageChunk = lastUsageChunk(sse);
		expect(usageChunk, 'a usage chunk in the stream').toBeTruthy();
		const usage = usageOf(usageChunk);

		const log = await waitForLog(api, key.id, response.headers()['x-request-id']);
		expect(log.streaming).toBe(true);
		expect(log.status).toBe('success');
		expect(log.time_to_first_token_ms).not.toBeNull();
		expect(log.time_to_first_token_ms).toBeLessThanOrEqual(log.latency_ms);
		expect(log.input_tokens).toBe(usage.input);
		expect(log.output_tokens).toBe(usage.output);
		expect(Number(log.cost_total)).toBeCloseTo(expectedCost(seed.pricing, usage), 9);
	});

	test('requests the provider rejects are logged as upstream errors without cost', async ({page, t, api, seed}) => {
		const key = await createKeyViaApi(api, seed.teamId, uniqueName('error-key'));
		const response = await chatCompletion(page.request, key.token, {model: seed.model.publicId, max_tokens: 5, temperature: 99});
		expect(response.status(), await response.text()).toBe(400);

		const log = await waitForLog(api, key.id, response.headers()['x-request-id']);
		expect(log.status).toBe('error');
		expect(log.status_code).toBe(400);
		expect(log.error_category).toBe('upstream');
		expect(Number(log.cost_total)).toBe(0);

		await page.goto('/developer/logs');
		const row = page.locator('tbody tr').filter({hasText: log.request_id.slice(0, 8)});
		await expect(row).toContainText(t('developer.logs.category_upstream'));
	});

	test('provider changes in admin settings apply to the next request', async ({page, api, seed}) => {
		const name = uniqueName('Editable');
		const {providerId, model} = await createSyncedProvider(api, name);
		const key = await createKeyViaApi(api, seed.teamId, uniqueName('edit-key'));
		const request = () => chatCompletion(page.request, key.token, {model: model.publicId, messages: [{role: 'user', content: PROMPT}], max_tokens: 20});
		try {
			expect((await request()).status(), 'freshly created provider').toBe(200);

			await api.put(`/api/v1/admin/providers/${providerId}`, {is_enabled: false});
			expect((await request()).status(), 'disabled provider').toBeGreaterThanOrEqual(400);

			await api.put(`/api/v1/admin/providers/${providerId}`, {is_enabled: true});
			expect((await request()).status(), 're-enabled provider').toBe(200);

			await api.put(`/api/v1/admin/providers/${providerId}`, {api_key: 'invalid-e2e-key'});
			const badKey = await request();
			expect(badKey.status(), 'provider whose API key was changed to an invalid one').toBeGreaterThanOrEqual(400);
			const log = await waitForLog(api, key.id, badKey.headers()['x-request-id']);
			expect(log.status).toBe('error');
			expect(Number(log.cost_total)).toBe(0);

			await api.put(`/api/v1/admin/providers/${providerId}`, {api_key: LLM.apiKey});
			expect((await request()).status(), 'provider with the valid key restored').toBe(200);
		} finally {
			await api.delete(`/api/v1/admin/providers/${providerId}`);
		}
		expect((await request()).status(), 'deleted provider').toBeGreaterThanOrEqual(400);
	});

	test('key spend equals the sum of logged request costs', async ({page, t, api, seed}) => {
		const key = await createKeyViaApi(api, seed.teamId, uniqueName('budget-key'));
		await api.put(`/api/v1/developer/keys/${key.id}/budget`, {amount: 1, interval: 'daily', on_exceed: 'block'});

		let total = 0;
		for (let i = 0; i < 2; i++) {
			const ok = await chatCompletion(page.request, key.token, {model: seed.model.publicId, messages: [{role: 'user', content: PROMPT}], max_tokens: 20});
			expect(ok.status(), await ok.text()).toBe(200);
			const log = await waitForLog(api, key.id, ok.headers()['x-request-id']);
			total += Number(log.cost_total);
		}
		expect(total).toBeGreaterThan(0);

		await expect.poll(async () => Number((await api.get(`/api/v1/developer/keys/${key.id}/budget`)).spent), {timeout: 15_000}).toBeCloseTo(total, 9);

		await page.goto('/developer/keys');
		await page.getByTestId(`key-${key.name}`).getByRole('button', {name: t('developer.keys.budget')}).click();
		await expect(page.getByTestId('budget-status')).toContainText(`${formatMoney(total)} / ${formatMoney(1)}`);
	});

	test('an exhausted blocking budget rejects the next request', async ({page, t, api, seed}) => {
		const key = await createKeyViaApi(api, seed.teamId, uniqueName('exhausted-key'));
		const first = await chatCompletion(page.request, key.token, {model: seed.model.publicId, messages: [{role: 'user', content: PROMPT}], max_tokens: 20});
		expect(first.status()).toBe(200);
		const firstLog = await waitForLog(api, key.id, first.headers()['x-request-id']);

		await api.put(`/api/v1/developer/keys/${key.id}/budget`, {amount: Number(firstLog.cost_total), interval: 'daily', on_exceed: 'block'});
		await expect.poll(async () => (await api.get(`/api/v1/developer/keys/${key.id}/budget`)).exhausted, {timeout: 15_000}).toBe(true);

		const blocked = await chatCompletion(page.request, key.token, {model: seed.model.publicId, messages: [{role: 'user', content: PROMPT}], max_tokens: 20});
		expect(blocked.status()).toBe(429);
		const blockedLog = await waitForLog(api, key.id, blocked.headers()['x-request-id']);
		expect(blockedLog.error_category).toBe('budget');
		expect(Number(blockedLog.cost_total)).toBe(0);

		await page.goto('/developer/keys');
		await page.getByTestId(`key-${key.name}`).getByRole('button', {name: t('developer.keys.budget')}).click();
		await expect(page.getByTestId('budget-status')).toContainText(t('developer.budget.exhausted'));
	});

	test('revoked keys are rejected', async ({page, t, api, seed}) => {
		const key = await createKeyViaApi(api, seed.teamId, uniqueName('revoke-key'));
		await page.goto('/developer/keys');
		await page.getByTestId(`key-${key.name}`).getByRole('button', {name: t('developer.keys.revoke')}).click();
		await page.getByRole('alertdialog').or(page.getByRole('dialog')).getByRole('button', {name: t('developer.keys.revoke')}).click();
		await expect(page.getByTestId(`key-${key.name}`).getByRole('button', {name: t('developer.keys.revoke')})).toHaveCount(0);

		const response = await chatCompletion(page.request, key.token, {model: seed.model.publicId});
		expect(response.status()).toBe(401);
	});
});
