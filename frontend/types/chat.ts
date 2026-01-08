// Chat system types

export interface Workspace {
	id: string;
	name: string;
	icon: string | null;
	color: string | null;
	sort_order: number;
	is_default: boolean;
	chat_count: number;
	created_at: string;
	updated_at: string;
}

export interface Chat {
	id: string;
	workspace_id: string | null;
	title: string | null;
	is_pinned: boolean;
	is_archived: boolean;
	message_count: number;
	last_message_at: string | null;
	created_at: string;
	updated_at: string;
}

export interface ChatMessage {
	id: string;
	client_id: string;
	role: 'user' | 'assistant' | 'system';
	content: string;
	reasoning_content: string | null;
	model_id: string | null;
	reasoning_effort: string | null;
	reasoning_budget_tokens: number | null;
	input_tokens: number | null;
	output_tokens: number | null;
	reasoning_tokens: number | null;
	input_cost_usd: string | null;
	output_cost_usd: string | null;
	reasoning_cost_usd: string | null;
	total_cost_usd: string | null;
	latency_ms: number | null;
	reasoning_latency_ms: number | null;
	created_at: string;
}

export interface ChatWithMessages {
	chat: Chat;
	messages: ChatMessage[];
}

export type StreamingAnimation = 'fade' | 'typewriter' | 'slide' | 'none';

export interface UserPreferences {
	default_model_key: string | null;
	favorite_model_keys: string[];
	streaming_animation: StreamingAnimation;
	use_remend: boolean;
}

// Request types
export interface CreateWorkspaceRequest {
	name: string;
	icon?: string;
	color?: string;
	is_default?: boolean;
}

export interface UpdateWorkspaceRequest {
	name?: string;
	icon?: string;
	color?: string;
	sort_order?: number;
	is_default?: boolean;
}

export interface CreateChatRequest {
	workspace_id?: string;
	title?: string;
	model_id?: string;
}

export interface UpdateChatRequest {
	title?: string;
	workspace_id?: string;
	is_pinned?: boolean;
	is_archived?: boolean;
}

export interface SendMessageRequest {
	content: string;
	model_id?: string;
	reasoning_effort?: string;
	tools_enabled?: string[];
}

export interface UpdatePreferencesRequest {
	default_model_key?: string;
	favorite_model_keys?: string[];
	streaming_animation?: StreamingAnimation;
	use_remend?: boolean;
	reasoning_effort?: string | null;
}

// Model types (from backend)
export interface Model {
	id: string;
	provider_id: string;
	model_id: string;
	display_name: string;
	capabilities: string[];
	context_length: number | null;
	max_tokens: number | null;
	provider_name: string;
	provider_kind: string;
	provider_display_name: string;
	provider_icon_svg: string | null;
	provider_brand_color: string | null;
	user_display_name: string | null;
	user_icon_override: string | null;
	is_favorite: boolean;
	is_hidden: boolean;
	default_temperature: number | null;
	default_max_tokens: number | null;
}

// Helper types
export interface ChatListParams {
	workspace_id?: string;
	include_archived?: boolean;
	limit?: number;
	offset?: number;
}

export interface MessageListParams {
	limit?: number;
	before?: string;
	after?: string;
}
