import {expect} from '@playwright/test';
import type {Api} from './fixtures';
import {LLM, MOCK_PRICING} from './constants';
import type {Pricing} from './cost';

export type SyncedModel = {id: string; publicId: string; displayName: string};

export async function createSyncedProvider(api: Api, name: string): Promise<{providerId: string; model: SyncedModel}> {
	const provider = await api.post('/api/v1/admin/providers', {kind: LLM.kind, name, base_url: LLM.baseUrl, api_key: LLM.apiKey});
	const sync = await api.post(`/api/v1/admin/providers/${provider.id}/sync`);
	expect(sync.success, `syncing ${name}: ${JSON.stringify(sync)}`).toBe(true);

	const models: {id: string; model_id: string; display_name: string}[] = await api.get(`/api/v1/admin/providers/${provider.id}/models`);
	const model = models.find(candidate => candidate.model_id === LLM.model || candidate.model_id.endsWith(`/${LLM.model}`));
	expect(model, `model "${LLM.model}" among ${models.length} synced models of ${name}`).toBeTruthy();
	return {providerId: provider.id, model: {id: model!.id, publicId: model!.model_id, displayName: model!.display_name}};
}

export async function createBrokenProvider(api: Api, name: string): Promise<{providerId: string; model: SyncedModel}> {
	const {providerId, model} = await createSyncedProvider(api, name);
	await api.put(`/api/v1/admin/providers/${providerId}`, {api_key: 'invalid-e2e-key'});
	await api.patch(`/api/v1/admin/models/${model.id}`, {display_name: name});
	return {providerId, model: {...model, displayName: name}};
}

export async function ensurePricing(api: Api, modelId: string): Promise<Pricing> {
	const effective = await api.get(`/api/v1/admin/models/${modelId}/pricing`);
	const pricing: Pricing = {...(effective.override_pricing ?? {}), input: Number(effective.effective_input), output: Number(effective.effective_output)};
	if (pricing.input > 0 || pricing.output > 0) return pricing;
	if (!LLM.isMock) throw new Error(`Model ${LLM.model} has no catalog pricing, so costs cannot be verified. Pick a model with known pricing.`);
	const override = {input: MOCK_PRICING.input, output: MOCK_PRICING.output, cache_read: null, cache_write: null, reasoning: null, input_audio: null, output_audio: null, tiers: []};
	await api.put(`/api/v1/admin/models/${modelId}/pricing`, {pricing: override});
	return override;
}
