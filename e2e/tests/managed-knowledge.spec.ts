import {test, expect, Api} from './support/fixtures';
import {uniqueName} from './support/gateway';
import {AUTH_FILES, LLM, MEMBER, MOCK_LLM_KEY, MOCK_LLM_URL} from './support/constants';

const CONNECTION = 'E2E managed pgvector';
const BASE = 'E2E managed handbook';

type Embed = {providerId: string; modelId: string; name: string};

async function embeddingModel(api: Api): Promise<Embed> {
	const provider = await api.post('/api/v1/admin/providers', {
		kind: 'OPENAI_COMPAT',
		name: uniqueName('ManagedEmbed'),
		base_url: `${MOCK_LLM_URL}/media`,
		api_key: MOCK_LLM_KEY,
	});
	const sync = await api.post(`/api/v1/admin/providers/${provider.id}/sync`);
	expect(sync.success, JSON.stringify(sync)).toBe(true);
	const synced: {id: string; model_id: string; display_name: string}[] = await api.get(`/api/v1/admin/providers/${provider.id}/models`);
	const embed = synced.find(model => model.model_id.endsWith('/mock-embed'))!;
	await api.patch(`/api/v1/admin/models/${embed.id}`, {output_modalities: ['EMBEDDINGS']});
	await api.put(`/api/v1/admin/models/${embed.id}/pricing`, {pricing: {input: 10, output: 0, tiers: []}});
	return {providerId: provider.id, modelId: embed.id, name: embed.display_name};
}

async function knowledgeTool(api: Api): Promise<string> {
	const existing = (await api.get('/api/v1/admin/tools')).find((tool: {name: string}) => tool.name === 'knowledge_search');
	if (existing) return existing.id;
	const tool = await api.post('/api/v1/admin/tools', {
		name: 'knowledge_search',
		display_name: 'Knowledge Search',
		source_kind: 'BUILTIN',
		source_config: {builtin_id: 'knowledge'},
		functions: [{name: 'knowledge_search', input_schema: {type: 'object', properties: {query: {type: 'string'}}, required: ['query']}}],
	});
	return tool.id;
}

test.describe.configure({mode: 'serial'});

test.describe('managed knowledge bases', () => {
	let embed: Embed;
	let connectionId: string;
	let baseId: string;
	let toolId: string;
	let pgvectorReady = false;

	test.beforeAll(async ({browser, baseURL}) => {
		const context = await browser.newContext({baseURL, storageState: AUTH_FILES.admin});
		const api = new Api(context.request);
		embed = await embeddingModel(api);
		toolId = await knowledgeTool(api);
		const setup = await api.post('/api/v1/admin/rag/connections', {
			name: CONNECTION,
			kind: 'pgvector',
			mode: 'managed',
			url: null,
			secret: null,
			embedding: {model_id: embed.modelId, dimensions: null},
		});
		connectionId = setup.connections.find((item: {name: string}) => item.name === CONNECTION).id;
		const health = await api.post(`/api/v1/admin/rag/connections/${connectionId}/test`);
		pgvectorReady = Boolean(health.ok);
		await api.put('/api/v1/admin/rag', {
			reranker: {model_id: null, top_n: 6},
			defaults: {connection_id: connectionId},
			chunking: {target_tokens: 400, overlap_percent: 15},
			quotas: {max_uploads_per_hour: 60, storage_quota_mb: 100},
		});
		await context.close();
	});

	test.afterAll(async ({browser, baseURL}) => {
		const context = await browser.newContext({baseURL, storageState: AUTH_FILES.admin});
		const api = new Api(context.request);
		await api.put('/api/v1/admin/rag', {reranker: {model_id: null, top_n: 6}, defaults: {connection_id: null}});
		for (const base of await api.get('/api/v1/knowledge-bases')) {
			if (base.name === BASE) await api.delete(`/api/v1/knowledge-bases/${base.id}`);
		}
		if (connectionId) await api.request.delete(`/api/v1/admin/rag/connections/${connectionId}`);
		if (toolId) await api.request.delete(`/api/v1/admin/tools/${toolId}`);
		if (embed?.providerId) await api.request.delete(`/api/v1/admin/providers/${embed.providerId}`);
		await context.close();
	});

	test('uploads a document into a managed knowledge base and finds it via hybrid search', async ({page}) => {
		test.skip(!pgvectorReady, 'pgvector 0.8+ extension is required on the database');

		await page.goto('/settings/knowledge');
		await page.getByTestId('knowledge-create').click();
		await page.getByTestId('knowledge-name').fill(BASE);
		await page.getByTestId('settings-save-bar-save').click();
		await expect(page).toHaveURL(/\/settings\/knowledge\/[0-9a-f-]{36}\?tab=documents/);
		baseId = page.url().match(/knowledge\/([0-9a-f-]{36})/)![1]!;

		await page.getByTestId('knowledge-documents-input').setInputFiles({
			name: 'handbook.md',
			mimeType: 'text/markdown',
			buffer: Buffer.from('# Travel Policy\n\nRail tickets are booked through the internal travel desk and reimbursed within five business days.\n'),
		});
		await expect(page.getByTestId('knowledge-document-handbook.md')).toBeVisible();
		await expect(page.getByTestId('knowledge-document-status-handbook.md')).toContainText(/Ready|Bereit/, {timeout: 30_000});

		await page.getByTestId('knowledge-tab-search').click();
		await page.getByTestId('knowledge-search-query').fill('How are rail tickets booked?');
		await page.getByTestId('knowledge-search-run').click();
		const hits = page.getByTestId('knowledge-search-hit');
		await expect(hits.first()).toContainText('Rail tickets are booked through the internal travel desk');
	});

	test('per-document limit access hides restricted documents from other users', async ({page, browser, baseURL}) => {
		test.skip(!pgvectorReady || !baseId, 'requires indexed managed base');

		await page.request.put(`/api/v1/knowledge-bases/${baseId}/assignments`, {data: {targets: [{kind: 'everyone'}]}});
		const users = await (await page.request.get('/api/v1/admin/users', {params: {page: 1, size: 10, search: MEMBER.username}})).json();
		const member = users.items.find((user: {username: string}) => user.username === MEMBER.username);
		const docs = await (await page.request.get(`/api/v1/knowledge-bases/${baseId}/documents`)).json();
		const doc = docs.find((item: {filename: string}) => item.filename === 'handbook.md');
		expect(doc).toBeTruthy();

		const memberContext = await browser.newContext({baseURL, storageState: AUTH_FILES.member});
		try {
			const before = await (await memberContext.request.post(`/api/v1/knowledge-bases/${baseId}/search`, {data: {query: 'rail tickets'}})).json();
			expect(before.hits.length).toBeGreaterThan(0);

			const adminUsers = await (await page.request.get('/api/v1/admin/users', {params: {page: 1, size: 10}})).json();
			const admin = adminUsers.items.find((user: {id: string}) => user.id !== member.id);
			const restricted = await page.request.put(`/api/v1/knowledge-bases/${baseId}/documents/${doc.id}/assignments`, {
				data: {targets: [{kind: 'user', id: admin.id}]},
			});
			expect(restricted.status()).toBe(200);

			const after = await (await memberContext.request.post(`/api/v1/knowledge-bases/${baseId}/search`, {data: {query: 'rail tickets'}})).json();
			expect(after.hits).toEqual([]);
		} finally {
			await memberContext.close();
		}
	});

	test('attaching a managed knowledge base in chat returns cited answers', async ({browser, baseURL, t, seed}) => {
		test.skip(!pgvectorReady || !baseId || !LLM.isMock, 'requires pgvector and local mock LLM');

		const adminContext = await browser.newContext({baseURL, storageState: AUTH_FILES.admin});
		try {
			const page = await adminContext.newPage();
			await page.goto('/');
			await page.getByTestId('model-selector').click();
			const search = page.getByPlaceholder(t('chat.model_selector.search_models'));
			await search.fill(seed.model.displayName);
			await page.getByText(seed.model.displayName, {exact: true}).last().click();
			await expect(search).toBeHidden();

			await page.getByTestId('composer-settings').click();
			await page.getByTestId(`composer-knowledge-${BASE}`).click();
			await page.keyboard.press('Escape');

			const composer = page.locator('textarea').first();
			await composer.fill('How are rail tickets booked?');
			await composer.press('Enter');
			await expect(page).toHaveURL(/\/chats\/[0-9a-f-]{36}/);
			const reply = page.locator('[data-testid="message"][data-role="assistant"]').first();
			await expect(reply).toContainText('Rail tickets are booked through the internal travel desk', {timeout: 60_000});
			await expect(page.getByTestId('knowledge-source-chip').first()).toContainText('handbook.md');
		} finally {
			await adminContext.close();
		}
	});
});
