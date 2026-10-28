# weave-mcp

A personal **knowledge / memory MCP server** for AI assistants (Hermes, Claude
Desktop, Codex, …). Any MCP client can connect over stdio and use the tools to
store notes, files, and a knowledge graph — then retrieve exactly what it needs
on demand, so the agent's own context stays small.

It is generic (works with any MCP agent), and reuses the shared `weave-core`
LLM client + extraction logic used by `weave-api`.

## What it does

```
Agent ──MCP stdio──▶ weave-mcp ──▶ weave-core (LLM extraction)
                                  ──▶ PostgreSQL weave_mcp (notes, entities,
                                       relations, documents, pgvector)
                                  ──▶ disk blob store (files)
```

- **Notes/memories** — store text, get an LLM **summary** stored alongside the
  full text (the token-saving mechanism: `recall_memory` returns compact
  context; `get_note` fetches full text on demand).
- **Knowledge graph** — each note is extracted into entities + relations,
  resolved against the persisted graph (no duplicate concepts), with
  provenance linking notes → nodes/edges. A note that already states its graph
  (a Markdown outline of `- [[Target]] — relation` or a JSON `{nodes, edges}`
  document) is parsed deterministically instead — no model call, no tokens —
  and its edges keep the source line as evidence.
- **Claims** — the durable memory unit: every extracted statement is
  validated (labels, predicates, self-loops), resolved deterministically
  (exact → alias → create), given an evidence span from the source note, a
  modality (asserted/negated/suggested/conditional) and confidence.
  Unsupported claims are quarantined; contradicting claims (same triple,
  opposing modality) are linked and both marked `contradicted` — nothing is
  silently overwritten.
- **Selective deep verification** — high-risk claims (ambiguous entity
  resolution, contradiction with a high-confidence claim, uncertain or
  comparative language, unsupported evidence) are reviewed by a narrow
  structured verifier LLM call (accept/reject/quarantine, optional modality
  correction, canonical-entity resolution). Normal notes pay nothing;
  verifier decisions are recorded on the claim (`WEAVE_MCP_VERIFIER` toggles).
- **Files** — documents, PDFs, images, audio are stored as blobs on disk with
  metadata; text is extracted for text-ish files and ingested.
- **Retrieval (GraphRAG)** — hybrid: vector (pgvector) + full-text (Postgres
  FTS) + graph neighborhood expansion, merged with reciprocal-rank fusion into
  a compact context block.

## Tools

| Tool | Purpose |
|---|---|
| `remember(text, kind?, tags?)` | Store a note/memory; extract + persist entities/relations; returns added items + summary |
| `add_file(path, description?)` | Store a file on disk, extract text, ingest into the graph |
| `list_notes(limit?, tag?, kind?)` | Browse note summaries, newest first |
| `get_note(id)` | Full note content + the entities/relations it created |
| `delete_note(id)` | Delete a note |
| `search(query, limit?)` | Full-text notes + keyword entities |
| `get_node(label)` | An entity and the relations touching it |
| `get_related(label, depth?)` | BFS subgraph around an entity (up to depth 3) |
| `explain_node(label)` | One entity's degree and every relation touching it, both directions |
| `path_between(from, to, max_depth?)` | Shortest path between two entities, hop by hop, with a rendered chain |
| `get_claim(id)` | One evidence-backed claim + its source note + contradictions |
| `list_claims(entity_label, status?, limit?)` | Claims about an entity, filtered by status |
| `reindex_embeddings(limit?)` | Re-embed rows whose vectors are missing or from an older model |
| `forget_entity(label)` | Permanently forget an entity + derived memory (audited, hard delete) |
| `prune_notes(days, limit?)` | Audited retention cleanup of old notes |
| `correct_claim(id, predicate?, object_label?)` | Supersede a claim and commit the corrected version |
| `memory_stats` | Counts, claim statuses, verifier stats, embedding coverage |
| `recall_memory(query, top_k?, include_contradicted?)` | **Flagship**: hybrid retrieval → compact context block (summaries + subgraph + claims) |

### Stable tool contract

The tools above are the long-term public contract for the agent memory
service. Internal schema and retrieval internals may evolve, but these tool
names/semantics are stable. Writes are idempotent (identical content returns
the existing note), every significant mutation is written to `audit_log`
(actor, action, before/after), and long documents are chunked into `chunk`
notes linked to the source document.

Lookups never guess. A label that does not resolve exactly returns the
near-match candidates instead of an answer, and a write that creates a label
resembling an existing entity reports it in `possible_duplicates` rather than
silently forking the concept into two.

The server also sends connection instructions to every client: consult Weave
before answering questions about prior context, start from `recall_memory`,
choose from the candidates a lookup offers instead of inventing a label, and
surface `possible_duplicates`. Clients hand that text to their model, so
graph-first behaviour does not depend on a user installing an agent file.

## Setup

Requirements: Rust (edition 2024), Docker (PostgreSQL with the pgvector
extension — the repo's `docker compose` already uses `pgvector/pgvector:pg16`).

```bash
docker compose up -d                 # postgres (pgvector) + redis
cd mcp
cp .env.example .env                 # optional; defaults work with docker compose
cargo run                            # weave-mcp, ready on stdio
```

The `weave_mcp` database is created automatically on first run.

### Real embeddings

The containerized build compiles with the `embedding` feature: local ONNX
embeddings (`BAAI/bge-small-en-v1.5`, 384-dim) via fastembed, downloaded once
into `HF_HOME` (`/data/hf` in the container) and cached in the data volume.
If the model cannot load, the server falls back to a deterministic stub
embedder and disables the semantic retrieval layer (lexical + graph still
work).

- **Semantic retrieval** powers write-time grounding and `recall_memory`:
  candidate union = lexical (mention/keyword) + semantic similarity + 1-hop
  graph expansion, with an explainable score and per-candidate reasons.
- Every vector is stamped with the producing model (`embedding_model`), so
  `reindex_embeddings` can rebuild stale indexes safely.

Local dev builds without the feature use the stub embedder:
`cargo run` (stub) vs `cargo run --features embedding` (local ONNX).

### LLM provider

Extraction/summarization use the same OpenAI-compatible provider as
`weave-api` (`OPENCODE_BASE_URL`, `OPENCODE_MODEL`, `OPENCODE_API_KEY`). When
the key is unset the deterministic mock extractor is used (great for tests).

## Connecting an agent

stdio transport — no server process to keep running; the agent launches it.
For Claude Desktop:

```json
{
  "mcpServers": {
    "weave": {
      "command": "/path/to/weave/mcp/target/debug/weave-mcp",
      "env": {
        "WEAVE_MCP_DATABASE_URL": "postgres://weave:weave@localhost:5432/weave_mcp",
        "WEAVE_MCP_DATA_DIR": "/path/to/weave/mcp/data"
      }
    }
  }
}
```

## Database

Schema lives in `mcp/migrations/` (applied at startup):

- `notes` — content, LLM `summary`, kind, tags, importance, source, FTS
  `tsvector`, `embedding vector(384)` (pgvector, HNSW index)
- `entities` — normalized-unique labels, kind, aliases, description, embedding
- `relations` — (source, target, relation) unique, weight
- `claims` — durable evidence-backed statements: endpoints + proposed labels,
  predicate, modality, confidence, status (active/contradicted/superseded/
  rejected/quarantined), evidence span, extraction version, source
- `claim_relations` / `claim_contradictions` — claim → relation projection and
  contradiction pairs
- `audit_log` — durable record of significant memory mutations (actor, action,
  before/after)
- `documents` — file metadata + `storage_key` into the disk blob store
- `note_entities` / `note_relations` — provenance junctions
- `006_embeddings` — requires the `vector` extension (pgvector image)

## Tests

```bash
DATABASE_URL=postgres://weave:weave@localhost:5432/weave cargo test -p weave-mcp
```

Covers the ingestion pipeline (with provenance + dedup), and an end-to-end MCP
protocol test that spawns the binary and drives `remember`, `get_note`,
`search`, `get_node`, `get_related`, `recall_memory`, and `delete_note`.

### Retrieval evaluation

`tests/retrieval_eval.rs` measures recall@k and MRR@k of the ranking formula
over `tests/fixtures/retrieval_eval.json`. It needs no database, network, or
embedder — each case supplies the candidate features the scorer sees — so it
runs in a plain `cargo test`.

```bash
cargo test -p weave-mcp --test retrieval_eval -- --nocapture
```

The run prints the ranking and per-case metrics. Change a weight or the hub
penalty without adding cases to justify it, and the aggregate drops below the
corpus thresholds and the test fails. When retrieval is found to be wrong in
practice, add the failing case to the fixture first: the corpus is the record
of every failure mode already fixed.

Known weaknesses the baseline encodes (visible as sub-1.0 per-case recall):
over-connected generic concepts can outrank specific content when they are
mentioned in the query, and a strong keyword match can beat a semantically
closer one.
