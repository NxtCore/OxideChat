import type {Tool as GeneratedTool} from './generated/Tool';

export type {ToolFunction} from './generated/ToolFunction';

export type Tool = GeneratedTool & {source_config?: any};
