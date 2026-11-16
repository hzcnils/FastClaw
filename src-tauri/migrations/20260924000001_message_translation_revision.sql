-- Keep translation results tied to a particular message body and request.
ALTER TABLE messages ADD COLUMN content_revision INTEGER NOT NULL DEFAULT 0;
ALTER TABLE messages ADD COLUMN translation_request_id TEXT;

-- Covers editor, stream finalization and other SQL writers, including A -> B -> A.
-- Unchanged content and metadata-only updates do not invalidate a translation.
CREATE TRIGGER messages_invalidate_translation
AFTER UPDATE OF content, conversation_id ON messages
WHEN NEW.content IS NOT OLD.content OR NEW.conversation_id IS NOT OLD.conversation_id
BEGIN
    UPDATE messages
    SET content_revision = OLD.content_revision + 1,
        translation_request_id = NULL,
        translation = NULL,
        translation_lang = NULL
    WHERE id = NEW.id;
END;
