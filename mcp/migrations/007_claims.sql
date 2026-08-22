-- V2: claims layer — durable, evidence-backed memory semantics.
--
-- The graph `relations` table remains the canonical projection (unique
-- source/target/relation). A claim is the durable unit: one evidence-backed
-- statement about two entities, with modality, confidence, status, and
-- provenance. Graph edges are projections of claims; every claim keeps the
-- LLM-proposed labels alongside the resolved entity ids for auditability.

CREATE TABLE claims (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    note_id UUID NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    subject_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    proposed_subject_label TEXT NOT NULL,
    predicate TEXT NOT NULL,
    object_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    proposed_object_label TEXT NOT NULL,
    modality TEXT NOT NULL DEFAULT 'asserted'
        CHECK (modality IN ('asserted', 'negated', 'suggested', 'conditional')),
    confidence REAL NOT NULL DEFAULT 1 CHECK (confidence >= 0 AND confidence <= 1),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'contradicted', 'superseded', 'rejected', 'quarantined')),
    evidence_span TEXT,
    evidence_offset INTEGER,
    extraction_version TEXT NOT NULL DEFAULT '1',
    source TEXT NOT NULL DEFAULT 'user'
        CHECK (source IN ('user', 'file', 'agent', 'import')),
    source_document_id UUID REFERENCES documents(id) ON DELETE SET NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_claims_note ON claims (note_id);
CREATE INDEX idx_claims_subject ON claims (subject_id);
CREATE INDEX idx_claims_object ON claims (object_id);
CREATE INDEX idx_claims_status ON claims (status);

-- Claim -> relation projection. Relations keep the (source, target, relation)
-- unique projection used by graph traversal / UI.
CREATE TABLE claim_relations (
    claim_id UUID NOT NULL REFERENCES claims(id) ON DELETE CASCADE,
    relation_id UUID NOT NULL REFERENCES relations(id) ON DELETE CASCADE,
    PRIMARY KEY (claim_id, relation_id)
);

CREATE INDEX idx_claim_relations_relation ON claim_relations (relation_id);

-- Two claims that disagree. Both rows are preserved and marked
-- `contradicted`; this junction keeps the pair explicitly addressable.
CREATE TABLE claim_contradictions (
    claim_a UUID NOT NULL REFERENCES claims(id) ON DELETE CASCADE,
    claim_b UUID NOT NULL REFERENCES claims(id) ON DELETE CASCADE,
    detected_by TEXT NOT NULL DEFAULT 'deterministic',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (claim_a, claim_b),
    CONSTRAINT contradictions_canonical_order CHECK (claim_a < claim_b)
);