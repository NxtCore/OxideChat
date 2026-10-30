export type Pricing = {
	input: number;
	output: number;
	reasoning?: number | null;
	tiers?: {min_context_tokens: number; input?: number | null; output?: number | null; reasoning?: number | null}[];
};

export type Usage = {input: number; output: number; reasoning?: number};

export function expectedCost(pricing: Pricing, usage: Usage): number {
	const tier = (pricing.tiers ?? []).filter(candidate => usage.input >= candidate.min_context_tokens).sort((a, b) => b.min_context_tokens - a.min_context_tokens)[0];
	const inputRate = tier?.input ?? pricing.input;
	const outputRate = tier?.output ?? pricing.output;
	const reasoningRate = tier?.reasoning ?? pricing.reasoning ?? null;
	const reasoning = Math.min(usage.reasoning ?? 0, usage.output);
	const prompt = (usage.input * inputRate) / 1_000_000;
	if (reasoning > 0 && reasoningRate !== null) {
		return prompt + ((usage.output - reasoning) * outputRate + reasoning * reasoningRate) / 1_000_000;
	}
	return prompt + (usage.output * outputRate) / 1_000_000;
}

export function formatMoney(value: number): string {
	const small = value > 0 && value < 0.01;
	return `$${value.toFixed(small ? 6 : 2)}`;
}
