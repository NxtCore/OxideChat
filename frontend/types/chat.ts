import type {Chat} from './generated/Chat';
import type {CreateMcpServerRequest as GeneratedCreateMcpServerRequest} from './generated/CreateMcpServerRequest';
import type {CreateWorkspaceRequest as GeneratedCreateWorkspaceRequest} from './generated/CreateWorkspaceRequest';
import type {ThemeCssVars} from './generated/ThemeCssVars';

export type {Chat} from './generated/Chat';
export type {Base} from './generated/Base';
export type {Role} from './generated/Role';
export type {UserPreferences} from './generated/UserPreferences';
export type {ProviderResponse} from './generated/ProviderResponse';
export type {ProviderOption} from './generated/ProviderOption';
export type {ProviderOptionsResponse} from './generated/ProviderOptionsResponse';
export type {ThemeCssVars} from './generated/ThemeCssVars';
export type {TeamSummary} from './generated/TeamSummary';
export type {TeamList} from './generated/TeamList';
export type {TeamRole} from './generated/TeamRole';
export type {TeamMember} from './generated/TeamMember';
export type {TeamDetailed} from './generated/TeamDetailed';
export type {ProviderTab} from './generated/ProviderTab';
export type {Workspace} from './generated/Workspace';
export type {McpServer} from './generated/McpServer';
export type {UpdateMcpServerRequest} from './generated/UpdateMcpServerRequest';
export type {CreateChatRequest} from './generated/CreateChatRequest';
export type {UpdateChatRequest} from './generated/UpdateChatRequest';
export type {UpdatePreferencesRequest} from './generated/UpdatePreferencesRequest';
export type {ModelList} from './generated/ModelList';
export type {ModelListAdmin} from './generated/ModelListAdmin';
export type {GlobalConfig} from './generated/GlobalConfig';
export type {DeleteWorkspaceOptions} from './generated/DeleteWorkspaceOptions';
export type {WorkspaceDeleteAction} from './generated/WorkspaceDeleteAction';
export type {UserListResponse} from './generated/UserListResponse';
export type {UserResponse} from './generated/UserResponse';
export type {SendMessageRequest} from './generated/SendMessageRequest';
export type {Page as PaginatedResponse} from './generated/Page';
export type {McpDiscoveryResult} from './generated/McpDiscoveryResult';
export type {UpdateWorkspaceRequest} from './generated/UpdateWorkspaceRequest';

export type CreateWorkspaceRequest = Omit<GeneratedCreateWorkspaceRequest, 'is_default'> & {is_default?: boolean};

export type CreateMcpServerRequest = Omit<GeneratedCreateMcpServerRequest, 'is_enabled'> & {is_enabled?: boolean};

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

export type ChatMessageContentPart =
	| {type: 'text'; text: string}
	| {type: 'reasoning'; text: string}
	| {type: 'image'; image_id: string}
	| {type: 'tool_call'; id: string; name: string; arguments: string}
	| {type: 'tool_result'; tool_call_id: string};

export interface ChatMessage {
	id: string;
	client_id?: string;
	role: 'user' | 'assistant' | 'system';
	content: string;
	reasoning_content: string | null;
	model_id: string | null;
	model_key: string | null;
	content_parts?: ChatMessageContentPart[] | null;
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

export interface FetchedTheme {
	name: string;
	preset: {cssVars: ThemeCssVars};
	url: string;
	error?: string;
	type: 'custom' | 'built-in';
}

export interface ChatListParams {
	workspace_id?: string;
	include_archived?: boolean;
	page?: number;
	size?: number;
}
