INSERT INTO i18n_translations (language, key_path, value) VALUES
    ('en', 'errors.rate_limited', 'Too many requests. Please try again later.'),
    ('en', 'errors.too_many_requests', 'Too many requests. Please try again later.'),
    ('en', 'gateway.errors.rate_limited', 'Too many authentication attempts. Please retry later.'),
    ('de', 'errors.rate_limited', 'Zu viele Anfragen. Bitte versuchen Sie es später erneut.'),
    ('de', 'errors.too_many_requests', 'Zu viele Anfragen. Bitte versuchen Sie es später erneut.'),
    ('de', 'gateway.errors.rate_limited', 'Zu viele Authentifizierungsversuche. Bitte versuchen Sie es später erneut.')
ON CONFLICT (language, key_path) DO NOTHING;
