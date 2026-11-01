import type {CreateKeyPayload as GeneratedCreateKeyPayload} from './generated/CreateKeyPayload';
import type {DeveloperKeyRule as GeneratedDeveloperKeyRule} from './generated/DeveloperKeyRule';
import type {SetKeyBudgetPayload as GeneratedSetKeyBudgetPayload} from './generated/SetKeyBudgetPayload';

export type {DeveloperModel} from './generated/DeveloperModel';
export type {DeveloperTeam} from './generated/DeveloperTeam';
export type {DeveloperKey} from './generated/DeveloperKey';
export type {DeveloperManagedTeam} from './generated/DeveloperManagedTeam';
export type {DeveloperTeamKey} from './generated/DeveloperTeamKey';
export type {DeveloperTeamKeys} from './generated/DeveloperTeamKeys';
export type {DeveloperKeyWithToken} from './generated/DeveloperKeyWithToken';
export type {DeveloperKeyRotation} from './generated/DeveloperKeyRotation';
export type {DeveloperRequestLog} from './generated/DeveloperRequestLog';
export type {DeveloperKeyBudget} from './generated/DeveloperKeyBudget';

export type DeveloperKeyRule = Omit<GeneratedDeveloperKeyRule, 'id'> & {id?: string};

export type CreateKeyPayload = Omit<GeneratedCreateKeyPayload, 'service_account'> & {service_account?: boolean};

export type SetKeyBudgetPayload = Omit<GeneratedSetKeyBudgetPayload, 'amount'> & {amount: number};

export type KeyStatus = 'active' | 'disabled' | 'expired' | 'revoked';

export const KEY_SCOPES = ['inference:read', 'inference:write'] as const;

export type RequestLogStatus = 'success' | 'error';

export const REQUEST_LOG_ERROR_CATEGORIES = ['upstream', 'interrupted', 'budget', 'rate_limit', 'auth', 'model_access', 'internal'] as const;

export type RequestLogErrorCategory = (typeof REQUEST_LOG_ERROR_CATEGORIES)[number];

export interface RequestLogFilters {
	scope: 'team_service' | null;
	team_id: string | null;
	api_key_id: string | null;
	status: RequestLogStatus | null;
	error_category: RequestLogErrorCategory | null;
	model: string | null;
	provider: string | null;
	from: string | null;
	to: string | null;
}

export type BudgetInterval = 'daily' | 'weekly' | 'monthly' | 'total';
export type BudgetOnExceed = 'block' | 'warn';
