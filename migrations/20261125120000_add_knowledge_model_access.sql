-- Which providers, upstream providers and models may see a knowledge base's content, and which chats hold it.

-- An empty list means no restriction.
CREATE TABLE knowledge_base_model_targets (
    knowledge_base_id UUID NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    provider_id UUID REFERENCES providers(id) ON DELETE CASCADE,
    model_id UUID REFERENCES models(id) ON DELETE CASCADE,
    -- Slug of an upstream provider of a routing provider (OpenRouter, Cortecs).
    upstream_provider TEXT CHECK (upstream_provider IS NULL OR upstream_provider ~ '^[a-z0-9._/-]{1,64}$'),
    CHECK (num_nonnulls(provider_id, model_id, upstream_provider) = 1),
    UNIQUE NULLS NOT DISTINCT (knowledge_base_id, provider_id, model_id, upstream_provider)
);

-- Knowledge bases whose excerpts are in a chat's history; only grows, so every later request of the chat keeps their restrictions.
CREATE TABLE chat_knowledge_sources (
    chat_id UUID NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    knowledge_base_id UUID NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    PRIMARY KEY (chat_id, knowledge_base_id)
);

CREATE INDEX idx_chat_knowledge_sources_base ON chat_knowledge_sources(knowledge_base_id);

INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'chat.errors.knowledge_model_not_allowed', 'This chat contains knowledge that this model or provider may not see. Pick an allowed model or start a new chat.'),
    ('de', 'chat.errors.knowledge_model_not_allowed', 'Dieser Chat enthält Wissen, das dieses Modell oder dieser Anbieter nicht sehen darf. Wähle ein erlaubtes Modell oder starte einen neuen Chat.'),
    ('en', 'chat.composer.knowledge_not_allowed', 'Not allowed for the selected model'),
    ('de', 'chat.composer.knowledge_not_allowed', 'Für das gewählte Modell nicht erlaubt'),
    ('en', 'rag.error.embedding_model_not_allowed', 'The model access list must allow the knowledge base''s embedding model.'),
    ('de', 'rag.error.embedding_model_not_allowed', 'Die Modellfreigabe muss das Embedding-Modell der Wissensdatenbank erlauben.'),
    ('en', 'rag.error.invalid_model_access', 'Pick existing providers and models; upstream provider names are at most 64 lowercase letters, digits and . _ / -.'),
    ('de', 'rag.error.invalid_model_access', 'Wähle vorhandene Anbieter und Modelle; Namen von Upstream-Anbietern haben höchstens 64 Kleinbuchstaben, Ziffern und . _ / -.'),
    ('en', 'knowledge.editor.tabs.models', 'Model access'),
    ('de', 'knowledge.editor.tabs.models', 'Modellzugriff'),
    ('en', 'knowledge.models.title', 'Model access'),
    ('de', 'knowledge.models.title', 'Modellzugriff'),
    ('en', 'knowledge.models.description', 'Only these providers, upstream providers and models may see this knowledge base''s content: in searches, in every later turn of a chat that holds excerpts, in titles and in the image tool. Leave everything empty for no restriction.'),
    ('de', 'knowledge.models.description', 'Nur diese Anbieter, Upstream-Anbieter und Modelle dürfen den Inhalt dieser Wissensdatenbank sehen: bei der Suche, in jeder weiteren Runde eines Chats mit Auszügen, bei Titeln und im Bildwerkzeug. Leer lassen für keine Einschränkung.'),
    ('en', 'knowledge.models.unrestricted', 'No restriction: every model may see the content.'),
    ('de', 'knowledge.models.unrestricted', 'Keine Einschränkung: Jedes Modell darf den Inhalt sehen.'),
    ('en', 'knowledge.models.providers', 'Providers'),
    ('de', 'knowledge.models.providers', 'Anbieter'),
    ('en', 'knowledge.models.providers_hint', 'Every model of these providers. For OpenRouter and Cortecs, picked upstream providers narrow this further.'),
    ('de', 'knowledge.models.providers_hint', 'Jedes Modell dieser Anbieter. Bei OpenRouter und Cortecs schränken gewählte Upstream-Anbieter weiter ein.'),
    ('en', 'knowledge.models.upstream', 'Upstream providers'),
    ('de', 'knowledge.models.upstream', 'Upstream-Anbieter'),
    ('en', 'knowledge.models.upstream_hint', 'Requests through OpenRouter or Cortecs may only reach these providers.'),
    ('de', 'knowledge.models.upstream_hint', 'Anfragen über OpenRouter oder Cortecs dürfen nur diese Anbieter erreichen.'),
    ('en', 'knowledge.models.upstream_placeholder', 'Add a provider slug, e.g. anthropic'),
    ('de', 'knowledge.models.upstream_placeholder', 'Anbieter-Slug hinzufügen, z. B. anthropic'),
    ('en', 'knowledge.models.models', 'Models'),
    ('de', 'knowledge.models.models', 'Modelle'),
    ('en', 'knowledge.models.models_hint', 'Single models, whatever their provider.'),
    ('de', 'knowledge.models.models_hint', 'Einzelne Modelle, egal von welchem Anbieter.'),
    ('en', 'knowledge.models.search', 'Search models'),
    ('de', 'knowledge.models.search', 'Modelle suchen'),
    ('en', 'knowledge.models.save', 'Save model access'),
    ('de', 'knowledge.models.save', 'Modellzugriff speichern'),
    ('en', 'knowledge.models.saved', 'Model access saved'),
    ('de', 'knowledge.models.saved', 'Modellzugriff gespeichert'),
    ('en', 'knowledge.models.save_error', 'Model access could not be saved'),
    ('de', 'knowledge.models.save_error', 'Modellzugriff konnte nicht gespeichert werden'),
    ('en', 'knowledge.models.load_error', 'Model access could not be loaded'),
    ('de', 'knowledge.models.load_error', 'Modellzugriff konnte nicht geladen werden')
ON CONFLICT (language, key_path) DO NOTHING;
