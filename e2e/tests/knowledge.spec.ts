import type {Page} from '@playwright/test';
import {test, expect, Api} from './support/fixtures';
import {uniqueName} from './support/gateway';
import {AUTH_FILES, LLM, MEMBER, MOCK_LLM_KEY, MOCK_LLM_URL} from './support/constants';

const CONNECTION = 'E2E handbook store';
const BASE = 'E2E handbook';

type Embed = {providerId: string; modelId: string; name: string};

async function embeddingModel(api: Api): Promise<Embed> {
	const provider = await api.post('/api/v1/admin/providers', {kind: 'OPENAI_COMPAT', name: uniqueName('KnowledgeEmbed'), base_url: `${MOCK_LLM_URL}/media`, api_key: MOCK_LLM_KEY});
	const sync = await api.post(`/api/v1/admin/providers/${provider.id}/sync`);
	expect(sync.success, JSON.stringify(sync)).toBe(true);
	const synced: {id: string; model_id: string; display_name: string}[] = await api.get(`/api/v1/admin/providers/${provider.id}/models`);
	const embed = synced.find(model => model.model_id.endsWith('/mock-embed'))!;
	await api.patch(`/api/v1/admin/models/${embed.id}`, {output_modalities: ['EMBEDDINGS']});
	await api.put(`/api/v1/admin/models/${embed.id}/pricing`, {pricing: {input: 10, output: 0, tiers: []}});
	return {providerId: provider.id, modelId: embed.id, name: embed.display_name};
}

async function connection(api: Api): Promise<string> {
	const setup = await api.post('/api/v1/admin/rag/connections', {name: CONNECTION, kind: 'qdrant', mode: 'read_only', url: `${MOCK_LLM_URL}/qdrant`, secret: null, embedding: null});
	return setup.connections.find((item: {name: string}) => item.name === CONNECTION).id;
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

async function choose(page: Page, testId: string, option: string) {
	await page.getByTestId(testId).click();
	await page.getByRole('option', {name: option, exact: true}).click();
}

function view(connectionId: string, collection: string, modelId: string) {
	return {connection_id: connectionId, collection, vector_name: null, distance: 'cosine', embedding: {model_id: modelId, dimensions: null}, fields: {text: 'page_content', title: null, url: null, section: null}, filter: []};
}

test.describe.configure({mode: 'serial'});

test.describe('knowledge bases', () => {
	let embed: Embed;
	let connectionId: string;
	let baseId: string;
	let chatId: string;
	let toolId: string;

	test.beforeAll(async ({browser, baseURL}) => {
		const context = await browser.newContext({baseURL, storageState: AUTH_FILES.admin});
		const api = new Api(context.request);
		embed = await embeddingModel(api);
		connectionId = await connection(api);
		toolId = await knowledgeTool(api);
		await api.put('/api/v1/admin/rag', {reranker: {model_id: null, top_n: 6}, defaults: {connection_id: null}});
		await context.close();
	});

	test.afterAll(async ({browser, baseURL}) => {
		const context = await browser.newContext({baseURL, storageState: AUTH_FILES.admin});
		const api = new Api(context.request);
		for (const base of await api.get('/api/v1/knowledge-bases')) await api.delete(`/api/v1/knowledge-bases/${base.id}`);
		await api.delete(`/api/v1/admin/rag/connections/${connectionId}`);
		await api.delete(`/api/v1/admin/tools/${toolId}`);
		await api.put('/api/v1/admin/rag', {reranker: {model_id: null, top_n: 6}, defaults: {connection_id: null}});
		await api.delete(`/api/v1/admin/providers/${embed.providerId}`);
		await context.close();
	});

	test('an admin maps a Qdrant collection and the test search finds its records', async ({page, t}) => {
		await page.goto('/settings/knowledge');
		await page.getByTestId('knowledge-create').click();
		await page.getByTestId('knowledge-kind').click();
		await page.getByRole('option', {name: t('knowledge.editor.kind_external')}).click();
		await page.getByTestId('knowledge-name').fill(BASE);
		await page.getByTestId('knowledge-tab-source').click();

		await choose(page, 'knowledge-source-connection', CONNECTION);
		await choose(page, 'knowledge-source-collection', 'handbook');
		await expect(page.getByTestId('knowledge-source-sample')).toHaveCount(3);
		await expect(page.getByTestId('knowledge-source-access-note')).toBeVisible();

		await choose(page, 'knowledge-source-field-text', 'page_content');
		await choose(page, 'knowledge-source-field-title', 'metadata.title');
		await choose(page, 'knowledge-source-field-url', 'metadata.url');
		await page.getByTestId('knowledge-source-filter-add').click();
		await choose(page, 'knowledge-source-filter-field-0', 'metadata.site');
		await page.getByTestId('knowledge-source-filter-value-0').fill('HR');
		await page.getByTestId('knowledge-source-filter-apply').click();
		await expect(page.getByTestId('knowledge-source-sample')).toHaveCount(2);

		await page.getByRole('button', {name: t('knowledge.source.model_none')}).click();
		await page.getByPlaceholder(t('chat.model_selector.search_models')).fill(embed.name);
		await page.getByText(embed.name, {exact: true}).last().click();
		await page.getByTestId('settings-save-bar-save').click();
		await expect(page).toHaveURL(/\/settings\/knowledge\/[0-9a-f-]{36}\?tab=sharing/);
		baseId = page.url().match(/knowledge\/([0-9a-f-]{36})/)![1]!;

		await page.getByTestId('knowledge-tab-search').click();
		await page.getByTestId('knowledge-search-query').fill('How do I book holidays?');
		await page.getByTestId('knowledge-search-run').click();
		const hits = page.getByTestId('knowledge-search-hit');
		await expect(hits).toHaveCount(2);
		await expect(hits.first()).toContainText('Holidays are booked in the HR portal.');
		await expect(hits.first().getByRole('link', {name: 'Holidays'})).toHaveAttribute('href', 'https://intranet.example/holidays');
		await expect(page.getByText('Servers are patched on Tuesdays.')).toHaveCount(0);
	});

	test('a model whose vectors do not fit the collection is refused, and a used connection stays', async ({api}) => {
		const wide = await api.request.post('/api/v1/knowledge-bases', {data: {name: 'Too wide', view: view(connectionId, 'wide', embed.modelId)}});
		expect(wide.status()).toBe(400);
		expect(JSON.stringify(await wide.json())).toContain('collection_dimensions_mismatch');

		const removed = await api.request.delete(`/api/v1/admin/rag/connections/${connectionId}`);
		expect(removed.status()).toBe(400);
		expect(JSON.stringify(await removed.json())).toContain('connection_in_use');
	});

	test('a member finds the knowledge base only after an admin confirms sharing it with everyone', async ({page, browser, baseURL, t}) => {
		const member = await browser.newContext({baseURL, storageState: AUTH_FILES.member});
		try {
			const memberPage = await member.newPage();
			expect(await (await memberPage.request.get('/api/v1/knowledge-bases')).json()).toEqual([]);
			expect((await memberPage.request.get(`/api/v1/knowledge-bases/${baseId}`)).status()).toBe(404);

			await page.goto(`/settings/knowledge/${baseId}?tab=sharing`);
			await page.getByTestId('knowledge-sharing-add').click();
			await page.getByTestId('knowledge-sharing-option-everyone').click();
			await expect(page.getByText(t('knowledge.sharing.confirm_description'))).toBeVisible();
			await page.getByTestId('knowledge-sharing-confirm').click();
			await expect(page.getByText(t('assignments.save_success')).first()).toBeVisible();

			await memberPage.goto('/settings/knowledge');
			await memberPage.getByTestId(`knowledge-row-${BASE}`).click();
			await expect(memberPage.getByTestId('knowledge-tab-search')).toBeVisible();
			await expect(memberPage.getByTestId('knowledge-tab-source')).toHaveCount(0);
			await expect(memberPage.getByTestId('knowledge-tab-sharing')).toHaveCount(0);
			await expect(memberPage.getByTestId('knowledge-name')).toHaveAttribute('readonly', '');
			await expect(memberPage.getByTestId('knowledge-delete')).toHaveCount(0);
			const response = await (await memberPage.request.get(`/api/v1/knowledge-bases/${baseId}`)).json();
			expect(response).toMatchObject({name: BASE, can_edit: false, can_manage: false, view: null, connection: null});
		} finally {
			await member.close();
		}
	});
	test('the knowledge tool on the tools page leads to the knowledge setup', async ({page}) => {
		await page.goto('/settings/tools');
		await page.getByTestId('tool-knowledge-setup').click();
		await expect(page).toHaveURL(/\/settings\/tools\/knowledge$/);
	});

	test('a member attaches the knowledge base in the composer and the answer cites its record', async ({browser, baseURL, t, seed}) => {
		test.skip(!LLM.isMock, 'only the local stand-in calls the knowledge tool on cue');
		const member = await browser.newContext({baseURL, storageState: AUTH_FILES.member});
		try {
			const page = await member.newPage();
			await page.goto('/');
			await page.getByTestId('model-selector').click();
			const search = page.getByPlaceholder(t('chat.model_selector.search_models'));
			await search.fill(seed.model.displayName);
			await page.getByText(seed.model.displayName, {exact: true}).last().click();
			await expect(search).toBeHidden();

			await page.getByTestId('composer-settings').click();
			await page.getByTestId(`composer-knowledge-${BASE}`).click();
			await expect(page.getByTestId('composer-chip-knowledge')).toHaveText(t('chat.composer.knowledge_chip', {count: 1}));
			await page.keyboard.press('Escape');

			const composer = page.locator('textarea').first();
			await composer.fill('How do I book holidays?');
			await composer.press('Enter');
			await expect(page).toHaveURL(/\/chats\/[0-9a-f-]{36}/);
			const reply = page.locator('[data-testid="message"][data-role="assistant"]').first();
			await expect(reply).toContainText('Mock answer: Holidays are booked in the HR portal. [1]', {timeout: 60_000});
			await expect(page.getByTestId('knowledge-source-chip').first()).toContainText('[1] Holidays');

			chatId = page.url().match(/chats\/([0-9a-f-]{36})/)![1]!;
			expect((await (await page.request.get(`/api/v1/chats/${chatId}`)).json()).knowledge_base_ids).toEqual([baseId]);
		} finally {
			await member.close();
		}
	});

	test('model access limits which models see the knowledge, also in a chat that already holds excerpts', async ({page, browser, baseURL, t, seed, allowErrors}) => {
		test.skip(!LLM.isMock, 'needs the chat with excerpts from the stand-in');
		allowErrors(/403|knowledge_model_not_allowed/);
		await page.goto(`/settings/knowledge/${baseId}?tab=models`);
		await expect(page.getByTestId('knowledge-models-unrestricted')).toBeVisible();
		const embedProvider = (await (await page.request.get(`/api/v1/knowledge-bases/${baseId}/model-access`)).json()).providers.find((provider: {id: string}) => provider.id === embed.providerId);
		await page.getByTestId(`knowledge-models-provider-${embedProvider.name}`).click();
		await page.getByTestId('settings-save-bar-save').click();
		await expect(page.getByText(t('knowledge.editor.saved')).first()).toBeVisible();
		await expect(page.getByTestId('knowledge-models-unrestricted')).toHaveCount(0);

		const member = await browser.newContext({baseURL, storageState: AUTH_FILES.member});
		try {
			const memberPage = await member.newPage();
			await memberPage.goto(`/chats/${chatId}`);
			await memberPage.getByTestId('model-selector').click();
			const search = memberPage.getByPlaceholder(t('chat.model_selector.search_models'));
			await search.fill(seed.model.displayName);
			await memberPage.getByText(seed.model.displayName, {exact: true}).last().click();
			await expect(search).toBeHidden();
			await memberPage.getByTestId('composer-settings').click();
			await expect(memberPage.getByTestId(`composer-knowledge-${BASE}`)).toHaveAttribute('data-disabled', 'true');
			await memberPage.keyboard.press('Escape');

			const composer = memberPage.locator('textarea').first();
			await composer.fill('And sick days?');
			await composer.press('Enter');
			await expect(memberPage.getByText(t('chat.errors.knowledge_model_not_allowed')).first()).toBeVisible({timeout: 60_000});
		} finally {
			await member.close();
			await page.request.put(`/api/v1/knowledge-bases/${baseId}/model-access`, {data: {providers: [], upstream_providers: [], models: []}});
		}
	});

	test('records list who may see them: an admin maps labels to people and searches as a member', async ({page, browser, baseURL, t}) => {
		const access = {...view(connectionId, 'intranet', embed.modelId), access: 'oxide_access'};
		const created = await page.request.post('/api/v1/knowledge-bases', {data: {name: 'E2E intranet', view: access}});
		expect(created.status()).toBe(200);
		const intranetId = (await created.json()).id;
		await page.request.put(`/api/v1/knowledge-bases/${intranetId}/assignments`, {data: {targets: [{kind: 'everyone'}]}});
		const users = await (await page.request.get('/api/v1/admin/users', {params: {page: 1, size: 5, search: MEMBER.username}})).json();
		const member = users.items.find((user: {username: string}) => user.username === MEMBER.username);

		await page.goto(`/settings/knowledge/${intranetId}?tab=source`);
		await expect(page.getByTestId('knowledge-source-access-field')).toContainText('oxide_access');
		await expect(page.getByTestId('knowledge-source-access-note')).toHaveCount(0);

		await page.getByTestId('knowledge-tab-access').click();
		await expect(page.getByTestId('knowledge-access-unassigned')).toContainText(t('knowledge.access.unassigned', {count: '3'}));
		await page.getByTestId('knowledge-access-label-HR Members').click();
		await page.getByTestId('knowledge-access-holders-add').click();
		await page.getByTestId('knowledge-access-holders-search').fill(MEMBER.username);
		await page.getByTestId(`knowledge-access-holders-option-user:${member.id}`).click();
		await expect(page.getByText(t('assignments.save_success')).first()).toBeVisible();
		await expect(page.getByTestId('knowledge-access-unassigned')).toContainText(t('knowledge.access.unassigned', {count: '2'}));

		await page.getByTestId('knowledge-tab-search').click();
		await page.getByTestId('knowledge-search-query').fill('What is confidential?');
		await page.getByTestId('knowledge-search-run').click();
		await expect(page.getByTestId('knowledge-search-empty')).toBeVisible();

		await page.getByTestId('knowledge-search-as').click();
		await page.getByTestId('knowledge-search-as-search').fill(MEMBER.username);
		await page.getByTestId(`knowledge-search-as-user-${member.id}`).click();
		await page.getByTestId('knowledge-search-run').click();
		const hits = page.getByTestId('knowledge-search-hit');
		await expect(hits).toHaveCount(1);
		await expect(hits.first()).toContainText('Salary bands are reviewed in March.');
		await expect(page.getByTestId('knowledge-search-explanation')).toContainText(t('knowledge.search.explain_held', {name: MEMBER.username, held: '1', labels: '3'}));

		const memberContext = await browser.newContext({baseURL, storageState: AUTH_FILES.member});
		try {
			const own = await (await memberContext.request.post(`/api/v1/knowledge-bases/${intranetId}/search`, {data: {query: 'anything'}})).json();
			expect(own.hits.map((hit: {text: string}) => hit.text)).toEqual(['Salary bands are reviewed in March.']);
			const spying = await memberContext.request.post(`/api/v1/knowledge-bases/${intranetId}/search`, {data: {query: 'anything', as_user: member.id}});
			expect(spying.status()).toBe(403);
		} finally {
			await memberContext.close();
		}
	});
});
