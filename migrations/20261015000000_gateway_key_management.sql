ALTER TABLE gateway_api_keys ADD COLUMN IF NOT EXISTS created_by UUID REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE gateway_api_keys ADD COLUMN IF NOT EXISTS revoked_by UUID REFERENCES users(id) ON DELETE SET NULL;

UPDATE gateway_api_keys SET created_by = user_id WHERE created_by IS NULL AND user_id IS NOT NULL;

INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'developer.errors.key_not_found', 'API key not found'),
    ('en', 'developer.errors.key_revoked', 'This API key has been revoked'),
    ('en', 'developer.errors.team_forbidden', 'You are not a member of this team'),
    ('en', 'developer.errors.name_invalid', 'The API key name must be between 1 and 100 characters'),
    ('en', 'developer.errors.scopes_invalid', 'Scopes must be a non-empty list of supported scopes'),
    ('en', 'developer.errors.expiry_invalid', 'The expiry must be in the future'),
    ('en', 'developer.errors.concurrency_invalid', 'The concurrent request limit must be greater than zero'),
    ('en', 'developer.errors.overlap_invalid', 'The rotation overlap must be between 0 seconds and 7 days'),
    ('en', 'developer.errors.rule_invalid', 'The rate limit rule is invalid'),
    ('en', 'developer.errors.rule_duplicate', 'Only one rate limit rule is allowed per route class, metric and window'),
    ('de', 'developer.errors.key_not_found', 'API-Schlüssel nicht gefunden'),
    ('de', 'developer.errors.key_revoked', 'Dieser API-Schlüssel wurde widerrufen'),
    ('de', 'developer.errors.team_forbidden', 'Du bist kein Mitglied dieses Teams'),
    ('de', 'developer.errors.name_invalid', 'Der Name des API-Schlüssels muss zwischen 1 und 100 Zeichen lang sein'),
    ('de', 'developer.errors.scopes_invalid', 'Die Scopes müssen eine nicht leere Liste unterstützter Scopes sein'),
    ('de', 'developer.errors.expiry_invalid', 'Das Ablaufdatum muss in der Zukunft liegen'),
    ('de', 'developer.errors.concurrency_invalid', 'Das Limit gleichzeitiger Anfragen muss größer als null sein'),
    ('de', 'developer.errors.overlap_invalid', 'Die Rotationsüberlappung muss zwischen 0 Sekunden und 7 Tagen liegen'),
    ('de', 'developer.errors.rule_invalid', 'Die Ratenlimit-Regel ist ungültig'),
    ('de', 'developer.errors.rule_duplicate', 'Pro Routenklasse, Metrik und Zeitfenster ist nur eine Ratenlimit-Regel erlaubt')
ON CONFLICT (language, key_path) DO NOTHING;
