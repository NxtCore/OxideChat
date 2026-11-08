import {writeFileSync, mkdirSync} from 'node:fs';
import type {Page, Request} from '@playwright/test';
import {test, expect} from './support/fixtures';

type Step = {label: string; requests: string[]; duplicates: string[]; ms: number};

async function settle(page: Page) {
	await page.waitForLoadState('networkidle');
	await expect(page.locator('.animate-pulse')).toHaveCount(0);
}

async function navigate(page: Page, path: string) {
	await page.evaluate(target => (document.querySelector('#__nuxt') as any).__vue_app__.config.globalProperties.$router.push(target), path);
	await expect(page).toHaveURL(new RegExp(`${path.replace(/[/]/g, '\\/')}$`));
}

async function sendInNewChat(page: Page, t: (key: string) => string, modelName: string) {
	await page.locator('button[aria-haspopup="dialog"]').filter({has: page.locator('svg[class*="chevron-down"]')}).first().click();
	const search = page.getByPlaceholder(t('chat.model_selector.search_models'));
	await search.fill(modelName);
	await page.getByText(modelName, {exact: true}).last().click();
	const composer = page.locator('textarea').first();
	await composer.fill(`Reply with one word. ${Date.now()}`);
	const budgetRefresh = page.waitForResponse(response => new URL(response.url()).pathname === '/api/v1/me/budget');
	await composer.press('Enter');
	await expect(page).toHaveURL(/\/chats\/[0-9a-f-]{36}/);
	await expect(page.locator('[data-testid="message"][data-role="assistant"]')).toHaveCount(1, {timeout: 60_000});
	await expect(page.getByTestId('streaming-message')).toHaveCount(0, {timeout: 60_000});
	await budgetRefresh;
}

test('main views do not repeat or refetch /api/v1 requests', async ({page, t, seed}) => {
	test.setTimeout(240_000);
	await page.route('**/api/v1/models?size=100', async route => {
		const response = await route.fetch();
		const body = await response.json();
		for (const model of body.items ?? []) if (!model.capabilities.includes('TOOLS')) model.capabilities.push('TOOLS');
		await route.fulfill({response, json: body});
	});

	const all: string[] = [];
	const log: string[] = [];
	const onRequest = (request: Request) => {
		const url = new URL(request.url());
		if (!url.pathname.startsWith('/api/v1/')) return;
		const entry = `${request.method()} ${url.pathname}${url.search}`;
		log.push(entry);
		all.push(entry);
	};
	page.on('request', onRequest);
	const steps: Step[] = [];

	async function step(label: string, action: () => Promise<void>) {
		log.length = 0;
		const started = Date.now();
		await action();
		await settle(page);
		const requests = [...log];
		const seen = new Set<string>();
		const duplicates = requests.filter(r => r.startsWith('GET') && (seen.has(r) || !seen.add(r)));
		steps.push({label, requests, duplicates, ms: Date.now() - started});
	}

	await step('cold /', () => page.goto('/').then(() => undefined));
	await step('send first message', () => sendInNewChat(page, t, seed.model.displayName));
	const firstChat = new URL(page.url()).pathname;
	await step('nav / (new chat)', () => navigate(page, '/'));
	await step('send second message', () => sendInNewChat(page, t, seed.model.displayName));
	const secondChat = new URL(page.url()).pathname;
	const firstMarker = all.length;
	await step('switch to first chat', () => navigate(page, firstChat));
	await step('switch to second chat', () => navigate(page, secondChat));
	await step('switch to first chat again', () => navigate(page, firstChat));
	const afterSwitches = all.length;

	await step('reload chat', () => page.reload().then(() => undefined));
	await step('nav /settings/profile', () => navigate(page, '/settings/profile'));
	await step('nav /settings/models', () => navigate(page, '/settings/models'));
	await step('nav /settings/providers', () => navigate(page, '/settings/providers'));
	await step('nav /developer/keys', () => navigate(page, '/developer/keys'));
	await step('nav /developer/logs', () => navigate(page, '/developer/logs'));
	await step('nav /developer/playground', () => navigate(page, '/developer/playground'));
	await step('nav /', () => navigate(page, '/'));
	await step('nav chat', () => navigate(page, firstChat));
	await step('admin edit in settings', async () => {
		await navigate(page, '/settings/models');
		await page.evaluate(
			({id, name}) => (document.querySelector('#__nuxt') as any).__vue_app__.config.globalProperties.$customFetch(`/api/v1/admin/models/${id}`, {method: 'PATCH', body: {display_name: name}}),
			{id: seed.model.id, name: seed.model.displayName}
		);
	});
	await step('nav / after admin edit', () => navigate(page, '/'));
	await step('cold /settings/profile', () => page.goto('/settings/profile').then(() => undefined));
	await step('cold /developer/keys', () => page.goto('/developer/keys').then(() => undefined));

	page.off('request', onRequest);
	if (process.env.AUDIT_LABEL) {
		mkdirSync('test-results', {recursive: true});
		writeFileSync(`test-results/request-audit-${process.env.AUDIT_LABEL}.json`, JSON.stringify(steps, null, 2));
		for (const s of steps) {
			console.log(`${s.label.padEnd(28)} ${String(s.requests.length).padStart(3)} requests  ${String(s.duplicates.length).padStart(2)} duplicate GETs  ${s.ms} ms`);
		}
	}

	const requestsOf = (label: string) => steps.find(s => s.label === label)?.requests ?? [];
	for (const s of steps) expect(s.duplicates, `duplicate GETs in "${s.label}"`).toEqual([]);
	expect(requestsOf('cold /')).not.toContain('GET /api/v1/users/@me/preferences');

	const firstLoad = all.slice(0, firstMarker);
	expect(firstLoad.filter(r => r === 'GET /api/v1/tools'), 'tools load once').toHaveLength(1);
	for (const switched of all.slice(firstMarker, afterSwitches)) expect(switched, 'a chat switch only loads that chat').toMatch(/^GET \/api\/v1\/chats\/[0-9a-f-]{36}$/);

	const sharedData = /\/(users\/@me\/preferences|mcp-servers|tools|me\/budget)$|\/models\?size=100$/;
	expect(requestsOf('nav /').filter(r => sharedData.test(r)), 'returning to chat reuses the store').toEqual([]);
	expect(requestsOf('nav chat').filter(r => sharedData.test(r)), 'returning to chat reuses the store').toEqual([]);
	expect(requestsOf('nav / after admin edit'), 'an admin model change refreshes the chat models').toContain('GET /api/v1/models?size=100');
	for (const label of ['nav /developer/logs', 'nav /developer/playground']) {
		expect(requestsOf(label).filter(r => /\/developer\/(teams|keys|team-keys)$/.test(r)), label).toEqual([]);
	}
});
