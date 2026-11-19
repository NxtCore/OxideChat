import type {ArenaCandidateResponse, ArenaRoundResponse} from '~/types/presets';
import type {ArenaRoundState, ChatMessage} from '~/types/chat';

export function arenaLabel(index: number): string {
	return String.fromCharCode(65 + index);
}

export function arenaMessage(id: string, roundId: string, candidate?: ArenaCandidateResponse): ChatMessage {
	return {
		id,
		client_id: id,
		role: 'assistant',
		content: candidate?.content ?? '',
		reasoning_content: candidate?.reasoning ?? null,
		model_id: candidate?.model_id ?? null,
		model_key: candidate?.model_key ?? null,
		cost_details: {input: null, output: null, reasoning: null, total: null},
		usage_details: {input_tokens: null, output_tokens: null, reasoning_tokens: null, latency_ms: null, reasoning_latency_ms: null, ...(candidate?.usage as object)},
		reasoning_details: {effort: null, budget_tokens: null},
		request_settings: {},
		tool_calls: [],
		content_parts: [],
		created_at: new Date().toISOString(),
		parent_id: null,
		fork_index: 1,
		sibling_count: 1,
		arena_round_id: roundId,
	};
}

export function arenaRoundState(round: ArenaRoundResponse): ArenaRoundState {
	return {
		id: round.id,
		user_message_id: round.user_message_id,
		decided: round.decided,
		stream_responses: round.stream_responses,
		show_reasoning: round.show_reasoning,
		candidates: round.candidates.map(candidate => ({
			message: arenaMessage(candidate.message_id, round.id, candidate),
			label: candidate.label,
			model_name: candidate.model_name,
			failed: false,
		})),
	};
}
