import type {Page} from '@playwright/test';
import {test, expect, type Api} from './support/fixtures';
import {uniqueName} from './support/gateway';
import {MOCK_LLM_KEY, MOCK_LLM_URL} from './support/constants';

const MANAGED = 'E2E main store';
const QDRANT = 'E2E qdrant';
const KEY = 'e2e-qdrant-api-key-value';

async function fillConnection(page: Page, fields: {name?: string; kind?: string; url?: string; secret?: string}) {
	if (fields.name !== undefined) await page.getByTestId('knowledge-connection-name').fill(fields.name);
	if (fields.kind) {
		await page.getByTestId('knowledge-connection-target').click();
		await page.getByTestId('knowledge-connection-target-external').click();
		await page.getByTestId('knowledge-connection-kind').click();
		await page.getByRole('option', {name: fields.kind}).click();
	}
	if (fields.url !== undefined) await page.getByTestId('knowledge-connection-url').fill(fields.url);
	if (fields.secret !== undefined) await page.getByTestId('knowledge-connection-secret').fill(fields.secret);
}

async function reset(api: Api) {
	for (const base of await api.get('/api/v1/knowledge-bases')) await api.delete(`/api/v1/knowledge-bases/${base.id}`);
	const setup = await api.get('/api/v1/admin/rag');
	await api.put('/api/v1/admin/rag', {reranker: {model_id: null, top_n: 6}, defaults: {connection_id: null}});
	for (const connection of setup.connections) await api.delete(`/api/v1/admin/rag/connections/${connection.id}`);
}

test.describe.configure({mode: 'serial'});

test.describe('knowledge setup', () => {
	test('an admin connects the main database as managed storage and tests it', async ({page, api, t}) => {
		await reset(api);
		await page.goto('/settings/knowledge/new');
		await expect(page.getByTestId('knowledge-managed-unavailable')).toBeVisible();
		await page.goto('/settings/tools/knowledge');
		await expect(page.getByTestId('knowledge-connections-empty')).toBeVisible();

		await page.getByTestId('knowledge-connection-add').click();
		await expect(page.getByTestId('knowledge-connection-url')).toBeHidden();
		await fillConnection(page, {name: MANAGED});
		await page.getByTestId('knowledge-connection-save').click();
		await expect(page.getByTestId(`knowledge-connection-${MANAGED}`)).toContainText(t('settings.knowledge_setup.connections.main_database'));

		await page.getByTestId(`knowledge-connection-test-${MANAGED}`).click();
		await expect(page.getByTestId(`knowledge-connection-result-${MANAGED}`)).toContainText(t('settings.knowledge_setup.connections.test_ok'));
	});

	test('a managed connection keeps the vector size of its test embedding', async ({page, api, t}) => {
		const provider = await api.post('/api/v1/admin/providers', {kind: 'OPENAI_COMPAT', name: uniqueName('Embed'), base_url: `${MOCK_LLM_URL}/media`, api_key: MOCK_LLM_KEY});
		try {
			const sync = await api.post(`/api/v1/admin/providers/${provider.id}/sync`);
			expect(sync.success, JSON.stringify(sync)).toBe(true);
			const synced: {id: string; model_id: string; display_name: string}[] = await api.get(`/api/v1/admin/providers/${provider.id}/models`);
			const embed = synced.find(model => model.model_id.endsWith('/mock-embed'))!;
			await api.patch(`/api/v1/admin/models/${embed.id}`, {output_modalities: ['EMBEDDINGS']});
			await api.put(`/api/v1/admin/models/${embed.id}/pricing`, {pricing: {input: 10, output: 0, tiers: []}});

			await page.goto('/settings/tools/knowledge');
			await page.getByTestId(`knowledge-connection-configure-${MANAGED}`).click();
			const listRequest = page.waitForRequest(request => /\/api\/v1\/admin\/models\?/.test(request.url()));
			await page.getByRole('button', {name: t('settings.knowledge_setup.connections.embedding_none')}).click();
			expect(new URL((await listRequest).url()).searchParams.get('type')).toBe('embedding');
			await page.getByPlaceholder(t('chat.model_selector.search_models')).fill(embed.display_name);
			await page.getByText(embed.display_name, {exact: true}).last().click();
			await page.getByTestId('knowledge-connection-save').click();
			await expect(page.getByTestId(`knowledge-connection-${MANAGED}`)).toContainText(t('settings.knowledge_setup.connections.vector_size', {size: 3}));
		} finally {
			await api.delete(`/api/v1/admin/providers/${provider.id}`);
		}
	});

	test('a Qdrant key is refused over http and never returned', async ({page, api, t, allowErrors}) => {
		allowErrors(/400/);
		await page.goto('/settings/tools/knowledge');
		await page.getByTestId('knowledge-connection-add').click();
		await fillConnection(page, {name: QDRANT, kind: 'Qdrant', url: 'http://qdrant.e2e.test:6333', secret: KEY});
		await page.getByTestId('knowledge-connection-save').click();
		await expect(page.getByText(t('rag.error.insecure_secret'))).toBeVisible();

		await fillConnection(page, {url: 'https://qdrant.e2e.test:6333'});
		await page.getByTestId('knowledge-connection-save').click();
		await expect(page.getByTestId(`knowledge-connection-${QDRANT}`)).toContainText(t('settings.knowledge_setup.connections.mode_read_only'));

		const setup = await api.get('/api/v1/admin/rag');
		expect(setup.connections.find((connection: {name: string}) => connection.name === QDRANT)).toMatchObject({kind: 'qdrant', mode: 'read_only', has_secret: true});
		expect(JSON.stringify(setup)).not.toContain(KEY);
	});

	test('moving a connection to another host needs its key again', async ({page, t, allowErrors}) => {
		allowErrors(/400/);
		await page.goto('/settings/tools/knowledge');
		await page.getByTestId(`knowledge-connection-configure-${QDRANT}`).click();
		await fillConnection(page, {url: 'https://elsewhere.e2e.test:6333'});
		await page.getByTestId('knowledge-connection-save').click();
		await expect(page.getByTestId('knowledge-connection-save')).toBeVisible();
		await expect(page.getByText(t('settings.knowledge_setup.connections.save_error'))).toBeVisible();
		await expect(page.getByText(t('settings.errors.secret_required_for_new_host'))).toBeVisible();
	});

	test('the default storage is saved through the save bar', async ({page, api}) => {
		expect((await api.get('/api/v1/admin/rag')).defaults.connection_id, 'the first storage became the default').toBeTruthy();
		await api.put('/api/v1/admin/rag', {reranker: {model_id: null, top_n: 6}, defaults: {connection_id: null}});
		await page.goto('/settings/tools/knowledge');
		await page.getByTestId('knowledge-default-connection').click();
		await page.getByRole('option', {name: MANAGED}).click();
		await page.getByTestId('settings-save-bar-save').click();
		await expect(page.getByTestId('settings-save-bar')).toBeHidden();

		const setup = await api.get('/api/v1/admin/rag');
		const managed = setup.connections.find((connection: {name: string}) => connection.name === MANAGED);
		expect(setup).toMatchObject({defaults: {connection_id: managed.id}});
		await reset(api);
	});

	test('knowledge is on while the Knowledge Search tool is enabled', async ({page, api, t}) => {
		expect((await api.get('/api/v1/base')).rag).toEqual({enabled: false});
		const tool = await api.post('/api/v1/admin/tools', {
			name: 'knowledge_search',
			display_name: 'Knowledge Search',
			source_kind: 'BUILTIN',
			source_config: {builtin_id: 'knowledge'},
			functions: [{name: 'knowledge_search', input_schema: {type: 'object', properties: {query: {type: 'string'}}, required: ['query']}}],
		});
		try {
			expect((await api.get('/api/v1/base')).rag).toEqual({enabled: true});
			await page.goto('/settings/tools/knowledge');
			await expect(page.getByTestId('knowledge-status')).toHaveText(t('settings.knowledge_setup.status_on'));
			await page.getByTestId('knowledge-setup-back').click();
			await expect(page).toHaveURL(/\/settings\/tools$/);
			await api.put(`/api/v1/admin/tools/${tool.id}`, {is_enabled: false});
			expect((await api.get('/api/v1/base')).rag).toEqual({enabled: false});
		} finally {
			await api.delete(`/api/v1/admin/tools/${tool.id}`);
		}
	});

	test('the old setup address leads to the page under Tools', async ({page}) => {
		await page.goto('/settings/knowledge-setup');
		await expect(page).toHaveURL(/\/settings\/tools\/knowledge$/);
	});
});
