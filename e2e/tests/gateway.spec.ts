import {test, expect} from './support/fixtures';
import {MOCK_COST_PER_REQUEST, MOCK_USAGE} from './support/constants';
import {chatCompletion, createKeyInUi, createKeyViaApi, keyIdByName, uniqueName, waitForLog} from './support/gateway';

test.describe('OpenAI gateway: exact usage, cost and logs', () => {
	test('a request created through the UI key is logged with exact tokens and cost', async ({page, t, api, seed}) => {
		const name = uniqueName('ui-key');
		const token = await createKeyInUi(page, t, name);
		const keyId = await keyIdByName(api, name);

		const models = await page.request.get('/openai/v1/models', {headers: {authorization: `Bearer ${token}`}});
		expect(models.ok()).toBe(true);
		const modelIds: string[] = (await models.json()).data.map((model: {id: string}) => model.id);
		expect(modelIds).toContain(seed.models['mock-chat'].publicId);

		const response = await chatCompletion(page.request, token, {model: seed.models['mock-chat'].publicId});
		expect(response.status(), await response.text()).toBe(200);
		const body = await response.json();
		expect(body.choices[0].message.content).toBe('Mock reply: ping from e2e');
		expect(body.usage.prompt_tokens).toBe(MOCK_USAGE.input);
		expect(body.usage.completion_tokens).toBe(MOCK_USAGE.output);

		const requestId = response.headers()['x-request-id'];
		expect(requestId).toBeTruthy();

		const log = await waitForLog(api, keyId, requestId);
		expect(log.status).toBe('success');
		expect(log.input_tokens).toBe(MOCK_USAGE.input);
		expect(log.output_tokens).toBe(MOCK_USAGE.output);
		expect(Number(log.cost_total)).toBeCloseTo(MOCK_COST_PER_REQUEST, 8);
		expect(log.finish_reason).toBe('stop');
		expect(log.streaming).toBe(false);
		expect(log.time_to_first_token_ms).toBeNull();

		await page.goto('/developer/logs');
		const row = page.locator('tbody tr').filter({hasText: requestId.slice(0, 8)});
		await expect(row).toHaveCount(1);
		await expect(row).toContainText(seed.models['mock-chat'].publicId);
		await expect(row).toContainText(name);
		await expect(row).toContainText('1,000 / 2,000');
		await expect(row).toContainText(`$${MOCK_COST_PER_REQUEST.toFixed(2)}`);
		await expect(row).toContainText(t('developer.logs.finish_stop'));
	});

	test('streaming requests record time to first token', async ({page, api, seed}) => {
		const key = await createKeyViaApi(api, seed.teamId, uniqueName('stream-key'));
		const response = await chatCompletion(page.request, key.token, {model: seed.models['mock-chat'].publicId, stream: true});
		expect(response.status()).toBe(200);
		const text = await response.text();
		expect(text).toContain('data: [DONE]');

		const log = await waitForLog(api, key.id, response.headers()['x-request-id']);
		expect(log.streaming).toBe(true);
		expect(log.status).toBe('success');
		expect(log.time_to_first_token_ms).not.toBeNull();
		expect(log.input_tokens).toBe(MOCK_USAGE.input);
		expect(log.output_tokens).toBe(MOCK_USAGE.output);
		expect(Number(log.cost_total)).toBeCloseTo(MOCK_COST_PER_REQUEST, 8);
	});

	test('upstream failures are logged as errors without cost', async ({page, t, api, seed}) => {
		const key = await createKeyViaApi(api, seed.teamId, uniqueName('error-key'));
		const response = await chatCompletion(page.request, key.token, {model: seed.models['mock-error'].publicId});
		expect(response.status()).toBeGreaterThanOrEqual(500);

		const log = await waitForLog(api, key.id, response.headers()['x-request-id']);
		expect(log.status).toBe('error');
		expect(log.error_category).toBe('upstream');
		expect(Number(log.cost_total)).toBe(0);

		await page.goto('/developer/logs');
		const row = page.locator('tbody tr').filter({hasText: log.request_id.slice(0, 8)});
		await expect(row).toContainText(t('developer.logs.category_upstream'));
	});

	test('spend accumulates exactly and a blocking key budget stops requests', async ({page, t, api, seed}) => {
		const key = await createKeyViaApi(api, seed.teamId, uniqueName('budget-key'));
		await api.put(`/api/v1/developer/keys/${key.id}/budget`, {amount: MOCK_COST_PER_REQUEST * 2, interval: 'daily', on_exceed: 'block'});

		for (let i = 0; i < 2; i++) {
			const ok = await chatCompletion(page.request, key.token, {model: seed.models['mock-chat'].publicId});
			expect(ok.status(), await ok.text()).toBe(200);
			await waitForLog(api, key.id, ok.headers()['x-request-id']);
		}

		await expect
			.poll(async () => Number((await api.get(`/api/v1/developer/keys/${key.id}/budget`)).spent), {timeout: 15_000})
			.toBeCloseTo(MOCK_COST_PER_REQUEST * 2, 8);

		const blocked = await chatCompletion(page.request, key.token, {model: seed.models['mock-chat'].publicId});
		expect(blocked.status()).toBe(429);
		const blockedLog = await waitForLog(api, key.id, blocked.headers()['x-request-id']);
		expect(blockedLog.error_category).toBe('budget');

		await page.goto('/developer/keys');
		await page.getByTestId(`key-${key.name}`).getByRole('button', {name: t('developer.keys.budget')}).click();
		const status = page.getByTestId('budget-status');
		await expect(status).toContainText(`$${(MOCK_COST_PER_REQUEST * 2).toFixed(2)} / $${(MOCK_COST_PER_REQUEST * 2).toFixed(2)}`);
		await expect(status).toContainText(t('developer.budget.exhausted'));
	});

	test('revoked keys are rejected and logged as auth failures', async ({page, t, api, seed}) => {
		const key = await createKeyViaApi(api, seed.teamId, uniqueName('revoke-key'));
		await page.goto('/developer/keys');
		await page.getByTestId(`key-${key.name}`).getByRole('button', {name: t('developer.keys.revoke')}).click();
		await page.getByRole('alertdialog').or(page.getByRole('dialog')).getByRole('button', {name: t('developer.keys.revoke')}).click();
		await expect(page.getByTestId(`key-${key.name}`).getByRole('button', {name: t('developer.keys.revoke')})).toHaveCount(0);

		const response = await chatCompletion(page.request, key.token, {model: seed.models['mock-chat'].publicId});
		expect(response.status()).toBe(401);
	});
});
