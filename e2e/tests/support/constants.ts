export const MOCK_LLM_URL = `http://127.0.0.1:${process.env.MOCK_LLM_PORT ?? 3103}`;

export const MOCK_USAGE = {input: 1000, output: 2000};

export const MOCK_PRICING = {input: 10, output: 20};

export const MOCK_COST_PER_REQUEST = (MOCK_USAGE.input * MOCK_PRICING.input + MOCK_USAGE.output * MOCK_PRICING.output) / 1_000_000;

export const ADMIN = {email: 'admin@e2e.test', username: 'e2eadmin', password: 'E2e-Admin-Passw0rd!'};

export const MEMBER = {email: 'member@e2e.test', username: 'e2emember', password: 'E2e-Member-Passw0rd!'};

export const AUTH_FILES = {admin: '.auth/admin.json', member: '.auth/member.json', seed: '.auth/seed.json'};
