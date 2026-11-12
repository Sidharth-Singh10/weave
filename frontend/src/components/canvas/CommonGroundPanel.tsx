"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ArrowCounterClockwise, Check, X } from "@phosphor-icons/react";
import { COMMON_GROUND_BASE, getCommonGroundDocId } from "@/lib/common-ground";

interface JournalActor {
  kind?: string;
  id?: string;
}

interface JournalOp {
  seq: number;
  op_id: string;
  turn_id: string;
  status: string;
  revision: number | null;
  undoes: string | null;
  undone_by: string | null;
  envelope: {
    type?: string;
    actor?: JournalActor;
    created_at?: string;
    target?: { id?: string };
    payload?: { op?: { op?: string } };
    approval?: string;
  };
  ack: {
    code?: string;
    message?: string;
    suggestion?: { approval_id?: string };
    inverse?: unknown[];
  } | null;
}

interface DocMeta {
  doc: { revision: number; pending: number; audit_seq: number };
  verification: { ok: boolean; broken_at?: number };
}

interface Turn {
  id: string;
  actor: JournalActor;
  createdAt: string;
  ops: JournalOp[];
  applied: number;
  rejected: number;
  undoes: string | null;
  undoneBy: string | null;
  canUndo: boolean;
}

const tokenCache = new Map<string, string>();

async function tokenFor(capId: string): Promise<string> {
  const cached = tokenCache.get(capId);
  if (cached) return cached;
  const response = await fetch(
    `${COMMON_GROUND_BASE}/api/capabilities/${encodeURIComponent(capId)}/token`,
  );
  if (!response.ok) throw new Error(`token ${capId}: HTTP ${response.status}`);
  const body = (await response.json()) as { token?: string };
  if (!body.token) throw new Error(`token ${capId}: missing`);
  tokenCache.set(capId, body.token);
  return body.token;
}

async function cgFetch<T>(path: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(`${COMMON_GROUND_BASE}${path}`, {
    ...init,
    headers: { "content-type": "application/json", ...(init.headers ?? {}) },
  });
  const text = await response.text();
  const body = text ? JSON.parse(text) : null;
  if (!response.ok) {
    throw new Error(body?.message ?? `${response.status} ${response.statusText}`);
  }
  return body as T;
}

export function CommonGroundPanel({ onClose }: { onClose: () => void }) {
  const [docId] = useState(() =>
    typeof window === "undefined" ? "" : getCommonGroundDocId(),
  );
  const [ops, setOps] = useState<JournalOp[]>([]);
  const [meta, setMeta] = useState<DocMeta | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const mounted = useRef(true);

  const load = useCallback(async () => {
    if (!docId) return;
    try {
      const [metaBody, opsBody] = await Promise.all([
        cgFetch<DocMeta>(`/api/docs/${encodeURIComponent(docId)}`),
        cgFetch<{ ops: JournalOp[] }>(
          `/api/docs/${encodeURIComponent(docId)}/ops?limit=100`,
        ),
      ]);
      if (!mounted.current) return;
      setMeta(metaBody);
      setOps(opsBody.ops);
      setError(null);
    } catch (cause) {
      if (mounted.current) setError((cause as Error).message);
    } finally {
      if (mounted.current) setLoading(false);
    }
  }, [docId]);

  useEffect(() => {
    mounted.current = true;
    const kickoff = window.setTimeout(() => void load(), 0);
    const timer = window.setInterval(() => void load(), 3000);
    return () => {
      mounted.current = false;
      window.clearTimeout(kickoff);
      window.clearInterval(timer);
    };
  }, [load]);

  const turns = useMemo<Turn[]>(() => {
    const byTurn = new Map<string, Turn>();
    for (const op of ops) {
      let turn = byTurn.get(op.turn_id);
      if (!turn) {
        turn = {
          id: op.turn_id,
          actor: op.envelope.actor ?? {},
          createdAt: op.envelope.created_at ?? "",
          ops: [],
          applied: 0,
          rejected: 0,
          undoes: op.undoes,
          undoneBy: op.undone_by,
          canUndo: false,
        };
        byTurn.set(op.turn_id, turn);
      }
      turn.ops.push(op);
      if (op.status === "applied") turn.applied += 1;
      else turn.rejected += 1;
    }
    for (const turn of byTurn.values()) {
      turn.ops.sort((a, b) => a.seq - b.seq);
      turn.canUndo = !turn.undoneBy && turn.applied > 0;
    }
    return [...byTurn.values()];
  }, [ops]);

  async function approve(op: JournalOp) {
    const approvalId = op.ack?.suggestion?.approval_id;
    if (!approvalId || busy) return;
    setBusy(`approve:${op.op_id}`);
    setError(null);
    try {
      const token = await tokenFor("cap_human");
      const result = await cgFetch<{ applied: boolean; acks?: Array<{ code?: string; message?: string }> }>(
        `/api/docs/${encodeURIComponent(docId)}/approvals/${encodeURIComponent(approvalId)}`,
        { method: "POST", body: "{}", headers: { "x-cg-capability": token } },
      );
      if (!result.applied) {
        const rejected = result.acks?.find((ack) => ack.code);
        setError(rejected?.message ?? "the approved op was not applied");
      }
      await load();
    } catch (cause) {
      setError((cause as Error).message);
    } finally {
      setBusy(null);
    }
  }

  async function undo(turn: Turn) {
    if (busy || !turn.canUndo) return;
    setBusy(`undo:${turn.id}`);
    setError(null);
    try {
      const token = await tokenFor("cap_undo");
      const result = await cgFetch<{ applied: boolean; acks?: Array<{ code?: string; message?: string }> }>(
        `/api/docs/${encodeURIComponent(docId)}/turns/${encodeURIComponent(turn.id)}/undo`,
        { method: "POST", body: "{}", headers: { "x-cg-capability": token } },
      );
      if (!result.applied) {
        const rejected = result.acks?.find((ack) => ack.code);
        setError(rejected?.message ?? "the turn was not undone");
      }
      await load();
    } catch (cause) {
      setError((cause as Error).message);
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="pointer-events-auto fixed right-4 top-16 z-50 flex max-h-[min(70vh,560px)] w-[380px] max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-2xl border border-line bg-surface/95 text-foreground shadow-float backdrop-blur-md">
      <div className="flex items-center gap-2 border-b border-line px-3 py-2">
        <span className="text-xs font-semibold">Common Ground</span>
        {meta && (
          <>
            <span className="rounded bg-surface-2 px-1.5 py-0.5 font-mono text-[10px] text-muted">
              r{meta.doc.revision}
            </span>
            <span
              className={`text-[10px] ${meta.verification.ok ? "text-green-400" : "text-red-400"}`}
            >
              {meta.verification.ok ? "audit verified" : "audit broken"}
            </span>
          </>
        )}
        <button
          onClick={onClose}
          aria-label="Close Common Ground panel"
          className="ml-auto rounded p-1 text-faint hover:text-foreground"
        >
          <X size={14} />
        </button>
      </div>
      <button
        type="button"
        title="Copy document id"
        onClick={() => void navigator.clipboard?.writeText(docId)}
        className="border-b border-line px-3 py-1.5 text-left font-mono text-[10px] text-faint hover:text-foreground"
      >
        {docId}
      </button>
      <div className="min-h-0 flex-1 overflow-y-auto px-3 py-2">
        {error && <p className="mb-2 text-[11px] text-red-400">{error}</p>}
        {loading && turns.length === 0 && (
          <p className="text-[11px] text-faint">Loading journal…</p>
        )}
        {!loading && turns.length === 0 && (
          <p className="text-[11px] leading-relaxed text-faint">
            No agent turns yet. The CG badge id is the document an agent targets.
          </p>
        )}
        <ul className="space-y-2">
          {turns.map((turn) => (
            <li key={turn.id} className="rounded-lg border border-line bg-background/50 p-2">
              <div className="flex items-center gap-2 text-[10px] text-faint">
                <span className="rounded border border-line px-1 py-0.5 uppercase">
                  {turn.actor.kind ?? "agent"}
                </span>
                <span className="truncate font-mono">{turn.actor.id ?? "unknown"}</span>
                <span className="ml-auto font-mono">{turn.id.slice(-10)}</span>
              </div>
              <div className="mt-1 text-[10px] text-muted">
                {turn.applied} applied
                {turn.rejected > 0 ? ` · ${turn.rejected} rejected` : ""}
                {turn.undoneBy ? " · undone" : ""}
                {turn.undoes ? ` · undo of ${turn.undoes.slice(-10)}` : ""}
              </div>
              <ul className="mt-1 space-y-1">
                {turn.ops.map((op) => {
                  const code = op.ack?.code;
                  const approvalId = op.ack?.suggestion?.approval_id;
                  const approved =
                    !!approvalId &&
                    ops.some(
                      (candidate) =>
                        candidate.status === "applied" &&
                        candidate.envelope.approval === approvalId,
                    );
                  return (
                    <li key={op.op_id} className="rounded bg-surface-2/60 px-2 py-1">
                      <div className="flex items-center gap-2">
                        <span
                          className={`size-1.5 shrink-0 rounded-full ${
                            op.status === "applied" ? "bg-green-400" : "bg-red-400"
                          }`}
                        />
                        <span className="truncate font-mono text-[10px] text-muted">
                          {op.envelope.type ?? "unknown"}
                        </span>
                        {op.revision != null && (
                          <span className="ml-auto font-mono text-[9px] text-faint">
                            r{op.revision}
                          </span>
                        )}
                      </div>
                      {code && (
                        <p className="mt-0.5 text-[10px] text-red-400">
                          {code}
                          {op.ack?.message ? ` — ${op.ack.message}` : ""}
                        </p>
                      )}
                      {code === "approval_required" && approvalId && !approved && (
                        <button
                          onClick={() => void approve(op)}
                          disabled={busy !== null}
                          className="mt-1 flex items-center gap-1 rounded border border-line px-1.5 py-0.5 text-[10px] text-muted hover:text-foreground disabled:opacity-50"
                        >
                          <Check size={11} /> Approve & apply
                        </button>
                      )}
                      {code === "approval_required" && approved && (
                        <p className="mt-1 text-[10px] text-green-400">approved</p>
                      )}
                    </li>
                  );
                })}
              </ul>
              <button
                onClick={() => void undo(turn)}
                disabled={!turn.canUndo || busy !== null}
                className="mt-1.5 flex items-center gap-1 rounded border border-line px-1.5 py-0.5 text-[10px] text-muted hover:text-foreground disabled:opacity-40"
              >
                <ArrowCounterClockwise size={11} />
                {busy === `undo:${turn.id}` ? "Undoing…" : "Undo turn"}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
