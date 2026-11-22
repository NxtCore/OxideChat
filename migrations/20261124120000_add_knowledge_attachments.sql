-- Knowledge bases attached to chats and presets; the knowledge tool searches them for users who may.

CREATE TABLE chat_knowledge_bases (
    chat_id UUID NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    knowledge_base_id UUID NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    PRIMARY KEY (chat_id, knowledge_base_id)
);

CREATE TABLE preset_knowledge_bases (
    preset_id UUID NOT NULL REFERENCES model_presets(id) ON DELETE CASCADE,
    knowledge_base_id UUID NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    PRIMARY KEY (preset_id, knowledge_base_id)
);

CREATE INDEX idx_chat_knowledge_bases_base ON chat_knowledge_bases(knowledge_base_id);
CREATE INDEX idx_preset_knowledge_bases_base ON preset_knowledge_bases(knowledge_base_id);

INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'chat.composer.knowledge', 'Knowledge'),
    ('de', 'chat.composer.knowledge', 'Wissen'),
    ('en', 'chat.composer.knowledge_chip', 'Knowledge: {count}'),
    ('de', 'chat.composer.knowledge_chip', 'Wissen: {count}'),
    ('en', 'presets.editor.tabs.knowledge', 'Knowledge'),
    ('de', 'presets.editor.tabs.knowledge', 'Wissen'),
    ('en', 'presets.editor.knowledge.title', 'Knowledge'),
    ('de', 'presets.editor.knowledge.title', 'Wissen'),
    ('en', 'presets.editor.knowledge.description', 'Chats with this preset may search these knowledge bases with the knowledge tool.'),
    ('de', 'presets.editor.knowledge.description', 'Chats mit dieser Vorlage dürfen diese Wissensdatenbanken mit dem Wissenswerkzeug durchsuchen.'),
    ('en', 'presets.editor.knowledge.empty', 'No knowledge base is shared with you.'),
    ('de', 'presets.editor.knowledge.empty', 'Mit dir ist keine Wissensdatenbank geteilt.'),
    ('en', 'presets.editor.knowledge.note', 'Attaching a knowledge base gives nobody access: each person searches only the knowledge bases shared with them.'),
    ('de', 'presets.editor.knowledge.note', 'Das Anhängen gibt niemandem Zugriff: Jede Person durchsucht nur die Wissensdatenbanken, die mit ihr geteilt sind.')
ON CONFLICT (language, key_path) DO NOTHING;
