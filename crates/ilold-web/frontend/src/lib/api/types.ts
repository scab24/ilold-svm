// ── AccessLevel ─────────────────────────────────────────────────────────────
// Rust `AccessLevel` uses default serde (no #[serde(tag)]):
//   Unit variants  → JSON strings: "Public", "Internal"
//   Struct variants → externally-tagged: { "Restricted": { "role": "onlyOwner" } }
export type AccessLevel =
  | "Public"
  | "Internal"
  | { Restricted: { role: string } }
  | { Special: { kind: string } };

// ── ServerMessage variants (internally tagged: #[serde(tag = "type")]) ──────

export interface SessionAddNode {
  type: "session_add_node";
  function: string;
  access: AccessLevel;
  step_index: number;
}

export interface SessionRemoveNode {
  type: "session_remove_node";
}

export interface SessionClear {
  type: "session_clear";
}

export interface SessionHighlight {
  type: "session_highlight";
  function: string;
}

export interface SearchResult {
  type: "search_result";
  contract: string;
  function: string;
  path_id: number;
  terminal: string;
  matches: Array<{ field: string; value: string }>;
  depth: number;
}

export interface SearchComplete {
  type: "search_complete";
  total: number;
}

export interface SearchError {
  type: "error";
  message: string;
  disconnected?: boolean;
}

// ── Discriminated union ─────────────────────────────────────────────────────

export type ServerMessage =
  | SessionAddNode
  | SessionRemoveNode
  | SessionClear
  | SessionHighlight
  | SearchResult
  | SearchComplete
  | SearchError;

// ── Connection events (synthetic, frontend-only) ────────────────────────────

export interface ConnectionEvent {
  state: "connected" | "disconnected";
}

export type ConnectionState = "connected" | "connecting" | "disconnected";

// ── Topic map (topic name → message type for type-safe subscribe<T>()) ──────

export interface TopicMap {
  search_result: SearchResult;
  search_complete: SearchComplete;
  error: SearchError;
  session_add_node: SessionAddNode;
  session_remove_node: SessionRemoveNode;
  session_clear: SessionClear;
  session_highlight: SessionHighlight;
  connection: ConnectionEvent;
}

// ── Session step (used by session store) ────────────────────────────────────

export interface SessionStep {
  function: string;
  access: AccessLevel;
  step_index: number;
}

// ── REST re-sync types ──────────────────────────────────────────────────────
// CommandResult::SessionView uses externally-tagged serde (no #[serde(tag)] on enum).
// Wire format: { "SessionView": { "contract": "Vault", "steps": ["swap"], "findings_count": 0 } }

export interface SessionViewPayload {
  contract: string;
  steps: string[];
  findings_count: number;
}

export interface SessionViewResponse {
  SessionView: SessionViewPayload;
}

// ── Search navigate payload (frontend-only, camelCase) ──────────────────────

export interface SearchNavigatePayload {
  contract: string;
  func: string;
  pathId: number;
}

// ── Callback interfaces ─────────────────────────────────────────────────────

export interface SearchCallbacks {
  onResult: (result: SearchResult) => void;
  onComplete: (total: number) => void;
  onError: (message: string) => void;
}

export interface SearchOptions {
  contract?: string;
  function?: string;
}

export interface SessionEventCallbacks {
  onAddNode?: (msg: SessionAddNode) => void;
  onRemoveNode?: (msg: SessionRemoveNode) => void;
  onClear?: (msg: SessionClear) => void;
  onHighlight?: (msg: SessionHighlight) => void;
}
