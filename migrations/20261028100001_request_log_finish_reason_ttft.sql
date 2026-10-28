ALTER TABLE gateway_request_logs
    ADD COLUMN IF NOT EXISTS finish_reason TEXT
        CONSTRAINT gateway_request_logs_finish_reason_check CHECK (finish_reason IN ('stop', 'length', 'tool_calls', 'content_filter')),
    ADD COLUMN IF NOT EXISTS time_to_first_token_ms INTEGER;

INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'developer.logs.ttft', 'First token'),
    ('en', 'developer.logs.finish_reason', 'Finish reason'),
    ('en', 'developer.logs.finish_stop', 'Stop'),
    ('en', 'developer.logs.finish_length', 'Length limit'),
    ('en', 'developer.logs.finish_tool_calls', 'Tool calls'),
    ('en', 'developer.logs.finish_content_filter', 'Content filter'),
    ('de', 'developer.logs.ttft', 'Erstes Token'),
    ('de', 'developer.logs.finish_reason', 'Abschlussgrund'),
    ('de', 'developer.logs.finish_stop', 'Stopp'),
    ('de', 'developer.logs.finish_length', 'Längenlimit'),
    ('de', 'developer.logs.finish_tool_calls', 'Tool-Aufrufe'),
    ('de', 'developer.logs.finish_content_filter', 'Inhaltsfilter')
ON CONFLICT (language, key_path) DO NOTHING;
