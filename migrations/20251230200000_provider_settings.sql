-- Provider settings page translations and permissions

-- Add provider admin permissions
INSERT INTO permissions (name, description) VALUES
    ('admin.providers.view', 'View AI provider configuration'),
    ('admin.providers.edit', 'Configure AI providers')
ON CONFLICT (name) DO NOTHING;

-- Assign provider permissions to admin role
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r, permissions p
WHERE r.name = 'admin' AND p.name LIKE 'admin.providers.%'
ON CONFLICT DO NOTHING;

-- Provider settings translations (English)
INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'settings.providers.title', 'BYOK AI Providers'),
    ('en', 'settings.providers.description', 'Bring your own API keys for enhanced AI capabilities'),
    ('en', 'settings.providers.edit', 'Configure'),
    ('en', 'settings.providers.configure', 'Configure'),
    ('en', 'settings.providers.configure_description', 'Enter your API credentials to enable this provider'),
    ('en', 'settings.providers.configured', 'Configured'),
    ('en', 'settings.providers.add_custom', 'Add Custom Provider'),
    ('en', 'settings.providers.name', 'Display Name'),
    ('en', 'settings.providers.api_key', 'API Key'),
    ('en', 'settings.providers.api_key_placeholder', 'Enter your API key'),
    ('en', 'settings.providers.api_key_hint', 'Your API key is encrypted and stored securely'),
    ('en', 'settings.providers.base_url', 'Base URL'),
    ('en', 'settings.providers.enabled', 'Enable Provider'),
    ('en', 'settings.providers.enabled_hint', 'Allow this provider to be used for AI requests'),
    ('en', 'settings.providers.save_success', 'Provider configuration saved successfully'),
    ('en', 'settings.providers.save_error', 'Failed to save provider configuration'),
    ('en', 'settings.providers.delete_success', 'Provider removed successfully'),
    ('en', 'settings.providers.delete_error', 'Failed to remove provider'),
    ('en', 'settings.providers.openrouter_description', 'Access a wide variety of models through OpenRouter'),
    ('en', 'settings.providers.openai_description', 'Access GPT-5.2, GPT-5.2-Codex and other OpenAI models'),
    ('en', 'settings.providers.anthropic_description', 'Access Opus 4.5, Sonnet 4.5 and other Anthropic models'),
    ('en', 'settings.providers.google_description', 'Access Gemini 3 family and other Google AI models'),
    ('en', 'settings.providers.ollama_description', 'Access local models through Ollama'),
    ('en', 'settings.providers.homl_description', 'Access local models through HoML (faster Ollama alternative)'),
    ('en', 'settings.providers.lmstudio_description', 'Access local models through LM Studio'),
    ('en', 'settings.providers.toggling_provider', 'Toggling provider'),
    ('en', 'settings.providers.toggling_provider_description', 'Please wait while the provider is being toggled'),
    ('en', 'settings.providers.toggling_provider_success', 'Provider successfully toggled'),
    ('en', 'common.cancel', 'Cancel'),  
    ('en', 'common.save', 'Save'),
    ('en', 'common.delete', 'Delete')
ON CONFLICT (language, key_path) DO NOTHING;

-- Provider settings translations (German)
INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('de', 'settings.providers.title', 'BYOK AI-Anbieter'),
    ('de', 'settings.providers.description', 'Verwenden Sie Ihre eigenen API-Schlüssel für erweiterte AI-Funktionen'),
    ('de', 'settings.providers.edit', 'Konfigurieren'),
    ('de', 'settings.providers.configure', 'Konfigurieren'),
    ('de', 'settings.providers.configure_description', 'Geben Sie Ihre API-Anmeldedaten ein, um diesen Anbieter zu aktivieren'),
    ('de', 'settings.providers.configured', 'Konfiguriert'),
    ('de', 'settings.providers.add_custom', 'Benutzerdefinierten Anbieter hinzufügen'),
    ('de', 'settings.providers.name', 'Anzeigename'),
    ('de', 'settings.providers.api_key', 'API-Schlüssel'),
    ('de', 'settings.providers.api_key_placeholder', 'Geben Sie Ihren API-Schlüssel ein'),
    ('de', 'settings.providers.api_key_hint', 'Ihr API-Schlüssel wird verschlüsselt und sicher gespeichert'),
    ('de', 'settings.providers.base_url', 'Basis-URL'),
    ('de', 'settings.providers.enabled', 'Anbieter aktivieren'),
    ('de', 'settings.providers.enabled_hint', 'Diesen Anbieter für AI-Anfragen verwenden'),
    ('de', 'settings.providers.save_success', 'Anbieterkonfiguration erfolgreich gespeichert'),
    ('de', 'settings.providers.save_error', 'Fehler beim Speichern der Anbieterkonfiguration'),
    ('de', 'settings.providers.delete_success', 'Anbieter erfolgreich entfernt'),
    ('de', 'settings.providers.delete_error', 'Fehler beim Entfernen des Anbieters'),
    ('de', 'settings.providers.openrouter_description', 'Greift auf eine Vielzahl von Modellen über OpenRouter zu'),
    ('de', 'settings.providers.openai_description', 'Greift auf GPT-5.2, GPT-5.2-Codex und andere OpenAI-Modelle zu'),
    ('de', 'settings.providers.anthropic_description', 'Greift auf Opus 4.5, Sonnet 4.5 und andere Anthropic-Modelle zu'),
    ('de', 'settings.providers.google_description', 'Greift auf Gemini 3 Familie und andere Google-Modelle zu'),
    ('de', 'settings.providers.ollama_description', 'Greift auf lokale Modelle über Ollama zu'),
    ('de', 'settings.providers.homl_description', 'Greift auf lokale Modelle über HoML (schnellere Alternative zu Ollama)'),
    ('de', 'settings.providers.lmstudio_description', 'Greift auf lokale Modelle über LM Studio zu'),
    ('de', 'settings.providers.toggling_provider', 'Anbieter umschalten'),
    ('de', 'settings.providers.toggling_provider_description', 'Bitte warten Sie, während der Anbieter umgeschaltet wird'),
    ('de', 'settings.providers.toggling_provider_success', 'Anbieter erfolgreich umgeschaltet'),
    ('de', 'common.cancel', 'Abbrechen'),
    ('de', 'common.save', 'Speichern'),
    ('de', 'common.delete', 'Löschen')
ON CONFLICT (language, key_path) DO NOTHING;
