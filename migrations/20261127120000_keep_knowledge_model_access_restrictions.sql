-- A deleted provider or model no longer lifts a knowledge base's model access: its target row stays with every column
-- NULL, which allows nothing, until the list is saved again.

ALTER TABLE knowledge_base_model_targets
    DROP CONSTRAINT knowledge_base_model_targets_check,
    DROP CONSTRAINT knowledge_base_model_targets_knowledge_base_id_provider_id__key,
    DROP CONSTRAINT knowledge_base_model_targets_model_id_fkey,
    DROP CONSTRAINT knowledge_base_model_targets_provider_id_fkey,
    ADD CONSTRAINT knowledge_base_model_targets_check CHECK (num_nonnulls(provider_id, model_id, upstream_provider) <= 1),
    ADD CONSTRAINT knowledge_base_model_targets_model_id_fkey FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE SET NULL,
    ADD CONSTRAINT knowledge_base_model_targets_provider_id_fkey FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE SET NULL;

CREATE UNIQUE INDEX idx_knowledge_base_model_targets_provider ON knowledge_base_model_targets(knowledge_base_id, provider_id) WHERE provider_id IS NOT NULL;
CREATE UNIQUE INDEX idx_knowledge_base_model_targets_model ON knowledge_base_model_targets(knowledge_base_id, model_id) WHERE model_id IS NOT NULL;
CREATE UNIQUE INDEX idx_knowledge_base_model_targets_upstream ON knowledge_base_model_targets(knowledge_base_id, upstream_provider) WHERE upstream_provider IS NOT NULL;

INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'rag.error.invalid_description', 'Descriptions have at most 2000 characters.'),
    ('de', 'rag.error.invalid_description', 'Beschreibungen haben höchstens 2000 Zeichen.'),
    ('en', 'rag.error.upstream_needs_provider', 'Upstream providers only narrow OpenRouter or Cortecs: also pick that provider under Providers.'),
    ('de', 'rag.error.upstream_needs_provider', 'Upstream-Anbieter schränken nur OpenRouter oder Cortecs ein: wähle diesen Anbieter auch unter Anbieter.'),
    ('en', 'knowledge.models.removed_targets', 'A provider or model of this list was deleted, so no model may see the content until the list is saved again.'),
    ('de', 'knowledge.models.removed_targets', 'Ein Anbieter oder Modell dieser Liste wurde gelöscht, daher darf kein Modell den Inhalt sehen, bis die Liste erneut gespeichert wird.'),
    ('en', 'knowledge.models.admins_only', 'Only knowledge admins can change the model access.'),
    ('de', 'knowledge.models.admins_only', 'Nur Wissens-Admins können den Modellzugriff ändern.'),
    ('en', 'presets.editor.knowledge.not_shared', 'A knowledge base that is not shared with you'),
    ('de', 'presets.editor.knowledge.not_shared', 'Eine Wissensdatenbank, die nicht mit dir geteilt ist'),
    ('en', 'presets.editor.knowledge.remove', 'Remove'),
    ('de', 'presets.editor.knowledge.remove', 'Entfernen')
ON CONFLICT (language, key_path) DO NOTHING;

UPDATE i18n_translations SET value = 'Requests through these routing providers may only reach these upstream providers. Pick OpenRouter or Cortecs under Providers as well.'
WHERE language = 'en' AND key_path = 'knowledge.models.upstream_hint';
UPDATE i18n_translations SET value = 'Anfragen über diese Routing-Anbieter dürfen nur diese Upstream-Anbieter erreichen. Wähle OpenRouter oder Cortecs auch unter Anbieter.'
WHERE language = 'de' AND key_path = 'knowledge.models.upstream_hint';
