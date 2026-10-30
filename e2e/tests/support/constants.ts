export const MOCK_LLM_URL = `http://127.0.0.1:${process.env.MOCK_LLM_PORT ?? 3103}`;

export const MOCK_LLM_KEY = 'mock-key';

export const MOCK_PRICING = {input: 10, output: 20};

const realKey = process.env.E2E_LLM_API_KEY || '';

export const LLM = realKey
	? {kind: process.env.E2E_LLM_KIND || 'OPENROUTER', baseUrl: process.env.E2E_LLM_BASE_URL || 'https://openrouter.ai/api', apiKey: realKey, model: process.env.E2E_LLM_MODEL || 'openai/gpt-4o-mini', isMock: false}
	: {kind: 'OPENAI_COMPAT', baseUrl: MOCK_LLM_URL, apiKey: MOCK_LLM_KEY, model: 'mock-chat', isMock: true};

export const ADMIN = {email: 'admin@e2e.test', username: 'e2eadmin', password: 'E2e-Admin-Passw0rd!'};

export const MEMBER = {email: 'member@e2e.test', username: 'e2emember', password: 'E2e-Member-Passw0rd!'};

export const AUTH_FILES = {admin: '.auth/admin.json', member: '.auth/member.json', seed: '.auth/seed.json'};
