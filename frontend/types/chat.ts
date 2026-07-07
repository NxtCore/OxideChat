export interface PaginatedResponse<T> {
	has_more: boolean;
	items: T[];
}

export interface TeamSummary {
	id: string;
	name: string;
	is_default: boolean;
}

export interface TeamList {
	id: string;
	name: string;
	description: string | null;
	is_default: boolean;
	allow_all_models: boolean;
	budget_id: string | null;
	member_count: number;
	created_at: string;
	updated_at: string;
}

export interface TeamMember {
	id: string;
	email: string;
	username: string;
}

export interface TeamDetailed extends Omit<TeamList, 'member_count'> {
	default_model_key: string | null;
	members: TeamMember[];
	model_access: {
		provider_ids: string[];
		model_ids: string[];
	};
}

export interface ProviderTab {
	id: string;
	name: string;
}

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

export interface McpServer {
	id: string;
	owner_id: string | null;
	name: string;
	transport: string;
	connection_config: Record<string, any>;
	is_enabled: boolean;
	last_health_check: string | null;
	health_status: string | null;
	discovered_tools: string[];
	created_at: string;
	updated_at: string;
}

export interface CreateMcpServerRequest {
	name: string;
	transport: string;
	connection_config: Record<string, any>;
	is_enabled?: boolean;
}

export interface UpdateMcpServerRequest {
	name?: string;
	transport?: string;
	connection_config?: Record<string, any>;
	is_enabled?: boolean;
}

export interface McpDiscoveryResult {
	tools: {name: string; description: string | null; input_schema: any}[];
	server_name: string;
	server_version: string | null;
}

export interface Chat {
	id: string;
	workspace_id: string | null;
	title: string | null;
	is_pinned: boolean;
	is_archived: boolean;
	branched_from_chat_id: string | null;
	branched_from_message_id: string | null;
	message_count: number;
	last_message_at: string | null;
	created_at: string;
	updated_at: string;
}

export interface ToolCallState {
	name: string;
	args: string;
	output?: any;
	error?: string;
	isExecuting: boolean;
}

export interface ChatMessageToolCall {
	tool_call_id: string;
	tool_name: string;
	input_args: string;
	output: string | null;
	error: string | null;
	execution_ms: number | null;
	tool_id: string | null;
	tool_function: string | null;
}

export interface ChatMessageRequestSettings {
	model_key?: string | null;
	provider_slug?: string | null;
	provider_routing_mode?: 'prefer' | 'lock' | string | null;
	enabled_tools?: string[];
}

export interface ChatMessage {
	id: string;
	client_id?: string;
	role: 'user' | 'assistant' | 'system';
	content: string;
	reasoning_content: string | null;
	model_id: string | null;
	model_key: string | null;
	content_parts?: Array<{type: string; text?: string; image_id?: string}> | null;
	cost_details: {
		input: string | null;
		output: string | null;
		reasoning: string | null;
		total: string | null;
	};
	usage_details: {
		input_tokens: number | null;
		output_tokens: number | null;
		reasoning_tokens: number | null;
		latency_ms: number | null;
		reasoning_latency_ms: number | null;
	};
	reasoning_details: {
		effort: string | null;
		budget_tokens: number | null;
	};
	request_settings: ChatMessageRequestSettings;
	tool_calls: Array<ChatMessageToolCall>;
	created_at: string;
	// Fork support
	parent_id: string | null;
	fork_index: number;
	sibling_count: number;
}

export interface ChatWithMessages {
	chat: Chat;
	messages: ChatMessage[];
}

export type StreamingAnimation = 'fade' | 'typewriter' | 'slide' | 'none';

export interface ThemeCssVars {
	theme: Record<string, string>;
	light: Record<string, string>;
	dark: Record<string, string>;
}

export interface UserPreferences {
	default_model_key: string | null;
	effective_default_model_key: string | null;
	default_provider_slug: string | null;
	default_tools: string[];
	favorite_model_keys: string[];
	streaming_animation: StreamingAnimation;
	use_remend: boolean;
	theme_css_vars: ThemeCssVars;
	custom_theme_urls: string[];
}

export interface GlobalConfig {
	default_theme: ThemeCssVars;
	default_model_key: string | null;
}

export interface FetchedTheme {
	name: string;
	preset: {cssVars: ThemeCssVars};
	url: string;
	error?: string;
	type: 'custom' | 'built-in';
}

export interface CreateWorkspaceRequest {
	name: string;
	icon?: string;
	color?: string;
	is_default?: boolean;
}

export interface UpdateWorkspaceRequest {
	name?: string;
	icon?: string;
	color?: string | null;
	sort_order?: number;
	is_default?: boolean;
}

export type WorkspaceDeleteAction = 'move' | 'archive' | 'delete';

export interface DeleteWorkspaceOptions {
	action: WorkspaceDeleteAction;
	target_workspace_id?: string;
}

export interface CreateChatRequest {
	workspace_id?: string;
	title?: string;
	model_id?: string;
}

export interface UpdateChatRequest {
	title?: string;
	workspace_id?: string | null;
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
	default_model_key?: string | null;
	default_provider_slug?: string | null;
	default_tools?: string[];
	favorite_model_keys?: string[];
	streaming_animation?: StreamingAnimation;
	use_remend?: boolean;
	reasoning_effort?: string | null;
	theme_css_vars?: ThemeCssVars;
	custom_theme_urls?: string[];
}

export interface ModelList {
	id: string;
	model_id: string;
	display_name: string;
	capabilities: string[];
	input_modalities: string[];
	output_modalities: string[];
	context_length: number | null;
	max_tokens: number | null;
	is_enabled: boolean;
	provider: {
		id: string;
		name: string;
		kind: string;
	};
	provider_name: string;
	icon: string | null;
	is_favorite: boolean;
	budget_blocked: boolean;
}

export interface ModelListAdmin {
	id: string;
	provider_id: string;
	model_id: string;
	display_name: string;
	capabilities: string[];
	input_modalities: string[];
	output_modalities: string[];
	context_length: number | null;
	max_tokens: number | null;
	is_enabled: boolean;
	created_at: string;
	updated_at: string;
	provider: {
		id: string;
		name: string;
		kind: string;
	};
	icon: string | null;
}

export interface ChatListParams {
	workspace_id?: string;
	include_archived?: boolean;
	limit?: number;
	offset?: number;
}
