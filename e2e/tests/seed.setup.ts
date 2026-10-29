import {mkdirSync, writeFileSync} from 'node:fs';
import {request as playwrightRequest} from '@playwright/test';
import {test, expect, Api, type Seed} from './support/fixtures';
import {ADMIN, AUTH_FILES, MEMBER, MOCK_LLM_URL, MOCK_PRICING} from './support/constants';

test.use({storageState: {cookies: [], origins: []}});

test('first-run setup creates the admin and seeds test data', async ({page, t, baseURL}) => {
	mkdirSync('.auth', {recursive: true});

	await page.goto('/');
	await expect(page).toHaveURL(/\/auth\/setup/);
	await expect(page.getByText(t('auth.setup.title'))).toBeVisible();

	await page.getByLabel(t('auth.login.email')).fill(ADMIN.email);
	await page.getByLabel(t('auth.register.username')).fill(ADMIN.username);
	await page.locator('#password').fill(ADMIN.password);
	await page.locator('#confirmPassword').fill(ADMIN.password);
	await page.getByRole('button', {name: t('auth.setup.submit')}).click();
	await expect(page).not.toHaveURL(/\/auth\//);
	await page.context().storageState({path: AUTH_FILES.admin});

	const api = new Api(page.request);
	const me = await api.get('/api/v1/users/@me');

	const provider = await api.post('/api/v1/admin/providers', {
		kind: 'OPENAI_COMPAT',
		name: 'Mock',
		base_url: MOCK_LLM_URL,
		api_key: 'mock-key',
	});
	const sync = await api.post(`/api/v1/admin/providers/${provider.id}/sync`);
	expect(sync.success, JSON.stringify(sync)).toBe(true);

	const providerModels: any[] = await api.get(`/api/v1/admin/providers/${provider.id}/models`);
	const models: Seed['models'] = {};
	for (const model of providerModels) {
		await api.put(`/api/v1/admin/models/${model.id}/pricing`, {
			pricing: {input: MOCK_PRICING.input, output: MOCK_PRICING.output, cache_read: null, cache_write: null, reasoning: null, input_audio: null, output_audio: null, tiers: []},
		});
		models[String(model.model_id).split('/').pop()!] = {id: model.id, publicId: model.model_id};
	}
	expect(Object.keys(models).sort()).toEqual(['mock-chat', 'mock-error', 'mock-slow']);

	const member = await api.post('/api/v1/admin/users', {
		email: MEMBER.email,
		username: MEMBER.username,
		password: MEMBER.password,
		roles: ['user'],
	});

	const teamName = 'E2E Team';
	const team = await api.post('/api/v1/admin/teams', {
		name: teamName,
		description: 'Seeded by the e2e suite',
		allow_all_models: true,
		member_ids: [me.id, member.id],
	});

	const memberContext = await playwrightRequest.newContext({baseURL});
	const login = await memberContext.post('/api/v1/auth/login', {data: {email: MEMBER.email, password: MEMBER.password}});
	expect(login.ok(), await login.text()).toBe(true);
	await memberContext.storageState({path: AUTH_FILES.member});
	await memberContext.dispose();

	const seed: Seed = {adminId: me.id, memberId: member.id ?? member.user?.id, providerId: provider.id, teamId: team.id ?? team.team?.id, teamName, models};
	writeFileSync(AUTH_FILES.seed, JSON.stringify(seed, null, 2));
	console.log('[seed]', JSON.stringify(seed));
});
