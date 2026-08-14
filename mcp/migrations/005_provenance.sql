-- Provenance: which note created which entity/relation, and documents that
-- produced notes.

CREATE TABLE note_entities (
    note_id UUID NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    PRIMARY KEY (note_id, entity_id)
);

CREATE INDEX idx_note_entities_entity ON note_entities (entity_id);

CREATE TABLE note_relations (
    note_id UUID NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    relation_id UUID NOT NULL REFERENCES relations(id) ON DELETE CASCADE,
    PRIMARY KEY (note_id, relation_id)
);

CREATE INDEX idx_note_relations_relation ON note_relations (relation_id);

ALTER TABLE notes
    ADD COLUMN source_document_id UUID REFERENCES documents(id) ON DELETE SET NULL;
