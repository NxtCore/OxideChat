-- Gateway catalog: persists a gateway's full model catalog separately from runnable `models`.
--
-- A gateway (V1: OpenRouter) exposes more models than the configured API key can actually
-- run. `gateway_catalog_models` stores every public catalog model — including those the key
-- cannot use (`USER_UNAVAILABLE`) — and optionally links to a runnable `models` row via
-- `local_model_id`. `gateway_model_provider_options` stores per-endpoint provider options for
-- a catalog model, fetched lazily.

DO $$ BEGIN
    CREATE TYPE gateway_availability AS ENUM (
        'AVAILABLE',
        'USER_UNAVAILABLE'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS gateway_catalog_models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_id UUID NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    source_gateway VARCHAR(50) NOT NULL,            -- e.g. 'openrouter'
    gateway_model_id TEXT NOT NULL,                 -- bare 'author/slug'
    local_model_id UUID REFERENCES models(id) ON DELETE SET NULL,
    display_name TEXT NOT NULL DEFAULT '',
    availability_state gateway_availability NOT NULL DEFAULT 'AVAILABLE',
    reason TEXT,
    raw JSONB NOT NULL DEFAULT '{}',
    fetched_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (provider_id, source_gateway, gateway_model_id)
);

CREATE TABLE IF NOT EXISTS gateway_model_provider_options (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    catalog_model_id UUID NOT NULL REFERENCES gateway_catalog_models(id) ON DELETE CASCADE,
    provider_slug TEXT,                             -- endpoint tag, e.g. 'openai'
    provider_name TEXT,
    endpoint_name TEXT,
    status DOUBLE PRECISION,                        -- OpenRouter health status (0 = healthy)
    quantization TEXT,
    context_length INTEGER,
    max_completion_tokens INTEGER,
    latency DOUBLE PRECISION,                       -- p50 latency (seconds)
    throughput DOUBLE PRECISION,                    -- p50 throughput (tokens/sec)
    uptime DOUBLE PRECISION,                        -- uptime over last 30m (percent)
    price_input DOUBLE PRECISION,                   -- USD per million input tokens
    price_output DOUBLE PRECISION,                  -- USD per million output tokens
    raw JSONB NOT NULL DEFAULT '{}',
    fetched_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_gateway_catalog_models_provider ON gateway_catalog_models(provider_id);
CREATE INDEX IF NOT EXISTS idx_gateway_catalog_models_local ON gateway_catalog_models(local_model_id);
CREATE INDEX IF NOT EXISTS idx_gateway_catalog_models_availability ON gateway_catalog_models(availability_state);
CREATE INDEX IF NOT EXISTS idx_gateway_provider_options_catalog ON gateway_model_provider_options(catalog_model_id);

-- UI translations for the catalog tab and the model provider-options table.
INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'settings.providers.tab_settings', 'Settings'),
    ('en', 'settings.providers.tab_catalog', 'Catalog'),
    ('en', 'settings.providers.catalog_search', 'Search catalog models...'),
    ('en', 'settings.providers.catalog_empty', 'No catalog models found'),
    ('en', 'settings.providers.catalog_disabled_key', 'Disabled for this key'),
    ('en', 'settings.providers.catalog_available', 'Available'),
    ('en', 'settings.models.editor.providers', 'Providers'),
    ('en', 'settings.models.editor.providers_desc', 'Provider endpoints that can serve this model, with status and pricing.'),
    ('en', 'settings.models.editor.providers_refresh', 'Refresh'),
    ('en', 'settings.models.editor.providers_user_unavailable', 'This model is disabled for the configured API key.'),
    ('en', 'settings.models.editor.providers_empty', 'No provider options available'),
    ('en', 'settings.models.editor.providers_load_error', 'Failed to load provider options'),
    ('en', 'settings.models.editor.provider_col_provider', 'Provider'),
    ('en', 'settings.models.editor.provider_col_status', 'Status'),
    ('en', 'settings.models.editor.provider_col_quant', 'Quantization'),
    ('en', 'settings.models.editor.provider_col_context', 'Context'),
    ('en', 'settings.models.editor.provider_col_price', 'Price (in/out per 1M)'),
    ('en', 'settings.models.editor.provider_col_uptime', 'Uptime'),
    ('en', 'settings.models.editor.row_available', 'Available'),
    ('en', 'settings.models.editor.row_unavailable', 'Unavailable'),
    ('en', 'settings.models.editor.row_user_unavailable', 'Disabled for this key'),
    ('en', 'settings.models.editor.row_unknown', 'Unknown'),
    ('de', 'settings.providers.tab_settings', 'Einstellungen'),
    ('de', 'settings.providers.tab_catalog', 'Katalog'),
    ('de', 'settings.providers.catalog_search', 'Katalogmodelle suchen...'),
    ('de', 'settings.providers.catalog_empty', 'Keine Katalogmodelle gefunden'),
    ('de', 'settings.providers.catalog_disabled_key', 'Für diesen Schlüssel deaktiviert'),
    ('de', 'settings.providers.catalog_available', 'Verfügbar'),
    ('de', 'settings.models.editor.providers', 'Anbieter'),
    ('de', 'settings.models.editor.providers_desc', 'Anbieter-Endpunkte, die dieses Modell bereitstellen können, mit Status und Preisen.'),
    ('de', 'settings.models.editor.providers_refresh', 'Aktualisieren'),
    ('de', 'settings.models.editor.providers_user_unavailable', 'Dieses Modell ist für den konfigurierten API-Schlüssel deaktiviert.'),
    ('de', 'settings.models.editor.providers_empty', 'Keine Anbieteroptionen verfügbar'),
    ('de', 'settings.models.editor.providers_load_error', 'Anbieteroptionen konnten nicht geladen werden'),
    ('de', 'settings.models.editor.provider_col_provider', 'Anbieter'),
    ('de', 'settings.models.editor.provider_col_status', 'Status'),
    ('de', 'settings.models.editor.provider_col_quant', 'Quantisierung'),
    ('de', 'settings.models.editor.provider_col_context', 'Kontext'),
    ('de', 'settings.models.editor.provider_col_price', 'Preis (Ein/Aus pro 1M)'),
    ('de', 'settings.models.editor.provider_col_uptime', 'Verfügbarkeit'),
    ('de', 'settings.models.editor.row_available', 'Verfügbar'),
    ('de', 'settings.models.editor.row_unavailable', 'Nicht verfügbar'),
    ('de', 'settings.models.editor.row_user_unavailable', 'Für diesen Schlüssel deaktiviert'),
    ('de', 'settings.models.editor.row_unknown', 'Unbekannt')
ON CONFLICT (language, key_path) DO NOTHING;
