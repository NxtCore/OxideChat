import type {APIRequestContext} from '@playwright/test';
import {test, expect} from './support/fixtures';
import {createKeyViaApi, uniqueName, waitForLog} from './support/gateway';
import {MEDIA_USAGE, MOCK_LLM_KEY, MOCK_LLM_URL} from './support/constants';

const PRICES = {
	'mock-embed': {input: 10, output: 0},
	'mock-rerank': {input: 0, output: 0, per_search: 0.002},
	'mock-decide': {input: 5, output: 0},
	'mock-image': {input: 5, output: 40, per_request: 0.01},
	'mock-whisper': {input: 0, output: 0, per_audio_second: 0.0001},
	'mock-tts': {input: 0, output: 0, per_character: 0.000015},
} as const;

type MediaModel = keyof typeof PRICES;

const SPEECH_INPUT = 'Hello from the e2e suite';
const IMAGE_COST = (MEDIA_USAGE.imageInput * 5 + MEDIA_USAGE.imageOutput * 40) / 1e6 + 0.01;
const TRANSCRIPT_COST = MEDIA_USAGE.transcriptSeconds * 0.0001;

function file(name: string, mimeType: string, size: number) {
	return {name, mimeType, buffer: Buffer.alloc(size, 7)};
}

test.describe('OpenAI gateway: non-chat models', () => {
	test('every non-chat route is admitted, billed and logged like chat', async ({page, api, seed}) => {
		const provider = await api.post('/api/v1/admin/providers', {kind: 'OPENAI_COMPAT', name: uniqueName('Media'), base_url: `${MOCK_LLM_URL}/media`, api_key: MOCK_LLM_KEY});
		const key = await createKeyViaApi(api, seed.teamId, uniqueName('media-key'));
		try {
			const sync = await api.post(`/api/v1/admin/providers/${provider.id}/sync`);
			expect(sync.success, JSON.stringify(sync)).toBe(true);
			const synced: {id: string; model_id: string}[] = await api.get(`/api/v1/admin/providers/${provider.id}/models`);
			const ids = {} as Record<MediaModel, string>;
			for (const [native, price] of Object.entries(PRICES) as [MediaModel, (typeof PRICES)[MediaModel]][]) {
				const model = synced.find(candidate => candidate.model_id.endsWith(`/${native}`));
				expect(model, `${native} among the synced models`).toBeTruthy();
				await api.put(`/api/v1/admin/models/${model!.id}/pricing`, {pricing: {...price, tiers: []}});
				ids[native] = model!.model_id;
			}

			const auth = {authorization: `Bearer ${key.token}`};
			const listed = await page.request.get('/openai/v1/models', {headers: auth});
			expect((await listed.json()).data.map((model: {id: string}) => model.id)).toEqual(expect.arrayContaining(Object.values(ids)));
			const badType = await page.request.get('/openai/v1/models?type=video', {headers: auth});
			expect(badType.status()).toBe(400);
			expect((await badType.json()).error.code).toBe('invalid_type');

			const call = (request: APIRequestContext, path: string, options: {data?: unknown; multipart?: Record<string, unknown>}) =>
				request.post(`/openai/v1${path}`, {headers: auth, failOnStatusCode: false, ...options} as Parameters<APIRequestContext['post']>[1]);
			const cases: {path: string; type: string; send: () => ReturnType<typeof call>; cost: number; check?: (response: any) => Promise<void>}[] = [
				{
					path: '/embeddings',
					type: 'gateway.embeddings',
					send: () => call(page.request, '/embeddings', {data: {model: ids['mock-embed'], input: ['first', 'second']}}),
					cost: (MEDIA_USAGE.embeddingTokens * 10) / 1e6,
					check: async response => expect((await response.json()).data).toHaveLength(2),
				},
				{
					path: '/rerank',
					type: 'gateway.rerank',
					send: () => call(page.request, '/rerank', {data: {model: ids['mock-rerank'], query: 'oxide', documents: ['iron', 'rust']}}),
					cost: MEDIA_USAGE.searchUnits * 0.002,
					check: async response => expect((await response.json()).results).toHaveLength(2),
				},
				{
					path: '/decisions',
					type: 'gateway.decisions',
					send: () => call(page.request, '/decisions', {data: {model: ids['mock-decide'], input: 'Is rust a metal oxide?', questions: [{type: 'predicate', instructions: 'Answer yes or no.'}]}}),
					cost: (MEDIA_USAGE.decisionTokens * 5) / 1e6,
				},
				{
					path: '/images/generations',
					type: 'gateway.image_generations',
					send: () => call(page.request, '/images/generations', {data: {model: ids['mock-image'], prompt: 'a rusty gear'}}),
					cost: IMAGE_COST,
					check: async response => expect((await response.json()).data[0].b64_json).toBeTruthy(),
				},
				{
					path: '/images/edits',
					type: 'gateway.image_edits',
					send: () => call(page.request, '/images/edits', {multipart: {model: ids['mock-image'], prompt: 'polish it', image: file('gear.png', 'image/png', 2048)}}),
					cost: IMAGE_COST,
				},
				{
					path: '/audio/transcriptions',
					type: 'gateway.transcriptions',
					send: () => call(page.request, '/audio/transcriptions', {multipart: {model: ids['mock-whisper'], file: file('speech.mp3', 'audio/mpeg', 3 * 1024 * 1024)}}),
					cost: TRANSCRIPT_COST,
					check: async response => expect((await response.json()).text).toBe('mock transcript'),
				},
				{
					path: '/audio/translations',
					type: 'gateway.translations',
					send: () => call(page.request, '/audio/translations', {multipart: {model: ids['mock-whisper'], file: file('speech.mp3', 'audio/mpeg', 4096)}}),
					cost: TRANSCRIPT_COST,
				},
				{
					path: '/audio/speech',
					type: 'gateway.speech',
					send: () => call(page.request, '/audio/speech', {data: {model: ids['mock-tts'], input: SPEECH_INPUT, voice: 'alloy'}}),
					cost: SPEECH_INPUT.length * 0.000015,
					check: async response => {
						expect(response.headers()['content-type']).toBe('audio/mpeg');
						expect((await response.body()).toString()).toContain(SPEECH_INPUT);
					},
				},
			];

			for (const route of cases) {
				const response = await route.send();
				expect(response.status(), `${route.path}: ${await response.text()}`).toBe(200);
				await route.check?.(response);
				const log = await waitForLog(api, key.id, response.headers()['x-request-id']);
				expect(log.request_type, route.path).toBe(route.type);
				expect(log.status, route.path).toBe('success');
				expect(Number(log.cost_total), route.path).toBeCloseTo(route.cost, 9);
			}
		} finally {
			await api.post(`/api/v1/developer/keys/${key.id}/revoke`);
			await api.delete(`/api/v1/admin/providers/${provider.id}`);
		}
	});
});
