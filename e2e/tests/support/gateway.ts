import {expect, type APIRequestContext, type Page} from '@playwright/test';
import type {Api} from './fixtures';

export type CreatedKey = {id: string; name: string; token: string};

export function uniqueName(prefix: string): string {
	return `${prefix}-${Date.now().toString(36)}-${Math.floor(Math.random() * 1e4)}`;
}

export async function createKeyInUi(page: Page, t: (key: string) => string, name: string): Promise<string> {
	await page.goto('/developer/keys');
	await page.getByRole('button', {name: t('developer.keys.create')}).first().click();
	const dialog = page.getByRole('dialog');
	await dialog.getByLabel(t('developer.create.name')).fill(name);
	await dialog.getByRole('button', {name: t('developer.create.submit')}).click();

	const token = (await page.getByTestId('revealed-token').textContent())?.trim() ?? '';
	expect(token.length).toBeGreaterThan(20);
	await page.getByLabel(t('developer.reveal.confirm')).check();
	await page.getByRole('dialog').getByRole('button', {name: t('common.close'), exact: true}).first().click();
	await expect(page.getByTestId(`key-${name}`)).toBeVisible();
	return token;
}

export async function createKeyViaApi(api: Api, teamId: string, name: string): Promise<CreatedKey> {
	const created = await api.post('/api/v1/developer/keys', {name, team_id: teamId, scopes: ['inference:read', 'inference:write']});
	return {id: created.id, name, token: created.token};
}

export async function keyIdByName(api: Api, name: string): Promise<string> {
	const keys: {id: string; name: string}[] = await api.get('/api/v1/developer/keys');
	const key = keys.find(candidate => candidate.name === name);
	if (!key) throw new Error(`key ${name} not found`);
	return key.id;
}

export async function chatCompletion(request: APIRequestContext, token: string, body: Record<string, unknown>) {
	return request.post('/openai/v1/chat/completions', {
		headers: {authorization: `Bearer ${token}`},
		data: {messages: [{role: 'user', content: 'ping from e2e'}], ...body},
		failOnStatusCode: false,
	});
}

export async function waitForLog(api: Api, apiKeyId: string, requestId: string) {
	let found: any;
	await expect
		.poll(
			async () => {
				const page = await api.get(`/api/v1/developer/request-logs?api_key_id=${apiKeyId}&size=100`);
				found = page.items.find((item: any) => item.request_id === requestId);
				return found ? 'logged' : 'missing';
			},
			{message: `request log for ${requestId}`, timeout: 15_000},
		)
		.toBe('logged');
	return found;
}
