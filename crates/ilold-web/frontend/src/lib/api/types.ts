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
  scenario: string;
  function: string;
  access: AccessLevel;
  step_index: number;
}

export interface SessionRemoveNode {
  type: "session_remove_node";
  scenario: string;
}

export interface SessionClear {
  type: "session_clear";
  scenario: string;
}

export interface SessionHighlight {
  type: "session_highlight";
  scenario: string;
  function: string;
}

// ── Scenario lifecycle events ───────────────────────────────────────────────

export interface ScenarioCreated {
  type: "scenario_created";
  name: string;
}

export interface ScenarioSwitched {
  type: "scenario_switched";
  from: string;
  to: string;
}

export interface ScenarioDeleted {
  type: "scenario_deleted";
  name: string;
}

export interface ScenarioForked {
  type: "scenario_forked";
  from: string;
  to: string;
  at_step: number;
}

/** Emitted after `LoadSession` replaces the entire ScenarioStore. The
 *  frontend store reacts by calling resync() to fetch the new snapshot. */
export interface ScenarioStoreReloaded {
  type: "scenario_store_reloaded";
  active: string;
}

// ── Scenario REST types ─────────────────────────────────────────────────────

export interface ScenarioInfo {
  name: string;
  active: boolean;
  step_count: number;
}

export interface SessionStepView {
  function: string;
  access: AccessLevel;
  step_index: number;
}

/** Where a scenario was forked from. `at_step` is the boundary: the
 *  scenario's steps [0..at_step) are the inherited prefix (a clone of the
 *  origin's steps at fork time), steps [at_step..] are its own. Null for
 *  `main` and for scenarios loaded from a pre-fork-origin save file. */
export interface ForkOrigin {
  scenario: string;
  at_step: number;
}

export interface ScenarioSnapshot {
  name: string;
  steps: SessionStepView[];
  forked_from: ForkOrigin | null;
}

// Backend serializes `ScenarioSnapshot` objects in insertion order (main
// first, then creation order) so the frontend canvas can anchor "main"
// consistently and render forks as branches from their recorded origin.
export interface AllScenariosResponse {
  active: string;
  scenarios: ScenarioSnapshot[];
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
  | ScenarioCreated
  | ScenarioSwitched
  | ScenarioDeleted
  | ScenarioForked
  | ScenarioStoreReloaded
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
  scenario_created: ScenarioCreated;
  scenario_switched: ScenarioSwitched;
  scenario_deleted: ScenarioDeleted;
  scenario_forked: ScenarioForked;
  scenario_store_reloaded: ScenarioStoreReloaded;
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
