// Types matching the Rust API responses

export interface ProjectSummary {
  files: number;
  contracts: ContractSummary[];
}

export interface ContractSummary {
  name: string;
  kind: string;
  functions: number;
  state_vars: number;
  inherits: string[];
}

export interface ContractDetail {
  name: string;
  kind: string;
  inherits: string[];
  functions: FunctionSummary[];
  state_vars: StateVarSummary[];
  inherited_functions?: FunctionSummary[];
  inherited_state_vars?: StateVarSummary[];
}

export interface FunctionSummary {
  name: string;
  kind: string;
  visibility: string;
  mutability: string;
  params: { name: string; type_name: string }[];
  path_count: number;
  happy_paths: number;
  revert_paths: number;
  modifiers: string[];
}

export interface StateVarSummary {
  name: string;
  type_name: string;
  visibility: string;
  is_constant: boolean;
  is_immutable: boolean;
}

export interface CytoscapeGraph {
  nodes: { data: CytoscapeNodeData }[];
  edges: { data: CytoscapeEdgeData }[];
}

export interface CytoscapeNodeData {
  id: string;
  label: string;
  node_type: string;
  contract: string;
  is_external: boolean;
  statements?: string[];
}

export interface CytoscapeEdgeData {
  id: string;
  source: string;
  target: string;
  kind: string;
  call_count: number;
}

// API client

export interface SearchSuggestions {
  functions: string[];
  state_vars: string[];
  events: string[];
  external_calls: string[];
  categories: { label: string; items: string[] }[];
}

export interface ProjectMap {
  kind: string;
  contracts: MapContract[];
  programs: MapProgram[];
  relationships: MapRelationship[];
}

export interface MapProgram {
  name: string;
  program_id: string;
  instructions: MapInstruction[];
  account_types: { name: string }[];
}

export interface MapInstruction {
  name: string;
  args_count: number;
  accounts_count: number;
  has_pdas: boolean;
}

export type AccountKind = 'program' | 'system' | 'sysvar' | 'pda' | 'other';

export interface ArgView {
  name: string;
  ty: string;
}

export interface FieldView {
  name: string;
  ty: string;
}

export type SeedView =
  | { kind: 'const'; value_hex: string; value_utf8?: string }
  | { kind: 'arg'; name: string; ty: string }
  | { kind: 'account'; path: string };

export interface PdaView {
  seeds: SeedView[];
  program?: string;
  bump_arg?: string;
}

export interface IxAccountView {
  path: string;
  name: string;
  kind: AccountKind;
  writable: boolean;
  signer: boolean;
  optional: boolean;
  address?: string;
  pda?: PdaView;
  relations: string[];
}

export interface IxView {
  name: string;
  discriminator_hex: string;
  args: ArgView[];
  accounts: IxAccountView[];
  returns?: string;
}

export interface AccountView {
  name: string;
  discriminator_hex: string;
  fields: FieldView[];
}

export interface CouplingPair {
  a: string;
  b: string;
  shared_writable: string[];
}

export interface ProgramView {
  name: string;
  program_id: string;
  instructions: IxView[];
  accounts: AccountView[];
  state_coupling?: CouplingPair[];
  admin_gated?: string[];
  system_accounts?: string[];
}

export interface CuStats {
  min: number;
  max: number;
  avg: number;
  samples: number;
}

export interface CpiEdge {
  from_ix: string;
  to_program: string;
  depth: number;
  samples: number;
}

export interface RuntimeOverlay {
  program: string;
  scenario: string;
  calls_per_ix: Record<string, number>;
  failed_per_ix: Record<string, number>;
  cu_stats_per_ix: Record<string, CuStats>;
  cpi_edges: CpiEdge[];
}

export interface MapContract {
  name: string;
  kind: string;
  inherits: string[];
  functions: MapFunction[];
  state_vars: { name: string; type_name: string; is_constant: boolean }[];
}

export interface MapFunction {
  name: string;
  visibility: string;
  mutability: string;
  path_count: number;
  happy_paths: number;
  revert_paths: number;
  has_external_calls: boolean;
}

export interface MapRelationship {
  from_contract: string;
  from_function: string;
  to_contract: string;
  to_function: string;
  kind: string;
}

const BASE = '';  // same origin in production, proxied in dev

export async function getProjectMap(): Promise<ProjectMap> {
  const res = await fetch(`${BASE}/api/project/map`);
  return res.json();
}

export async function getProject(): Promise<ProjectSummary> {
  const res = await fetch(`${BASE}/api/project`);
  return res.json();
}

export async function getContract(name: string): Promise<ContractDetail> {
  const res = await fetch(`${BASE}/api/contract/${name}`);
  if (!res.ok) throw new Error(`Contract ${name} not found`);
  return res.json();
}

export async function getProgramView(name: string): Promise<ProgramView> {
  const res = await fetch(`${BASE}/api/program/${encodeURIComponent(name)}/view`);
  if (!res.ok) throw new Error(`Program ${name} not found`);
  return res.json();
}

export async function getProgramOverlay(
  name: string,
  scenario?: string,
): Promise<RuntimeOverlay> {
  const qs = scenario ? `?scenario=${encodeURIComponent(scenario)}` : '';
  const res = await fetch(
    `${BASE}/api/program/${encodeURIComponent(name)}/overlay${qs}`,
  );
  if (!res.ok) throw new Error(`Overlay for ${name} not found`);
  return res.json();
}

/** Per-scenario pubkey -> user-name map (e.g. "Bxk7..." -> "alice"). Used by
 *  the canvas to render authority labels instead of raw base58 pubkeys. */
export async function getUserLabels(
  scenario: string,
): Promise<Record<string, string>> {
  const res = await fetch(
    `${BASE}/api/users/${encodeURIComponent(scenario)}/labels`,
  );
  if (!res.ok) throw new Error(`User labels for ${scenario} not found`);
  return res.json();
}

export async function getCallGraph(contractName: string): Promise<CytoscapeGraph> {
  const res = await fetch(`${BASE}/api/contract/${contractName}/callgraph`);
  return res.json();
}

export async function getCfg(contractName: string, funcName: string): Promise<CytoscapeGraph> {
  const res = await fetch(`${BASE}/api/contract/${contractName}/${funcName}/cfg`);
  return res.json();
}

export async function getPaths(contractName: string, funcName: string) {
  const res = await fetch(`${BASE}/api/contract/${contractName}/${funcName}/paths`);
  return res.json();
}

export interface FunctionSourceResponse {
  file_path: string;
  source: string;
  span: {
    file_index: number;
    start_line: number;
    start_col: number;
    end_line: number;
    end_col: number;
  };
}

export async function getFunctionSource(
  contractName: string,
  funcName: string,
): Promise<FunctionSourceResponse> {
  const res = await fetch(`${BASE}/api/contract/${contractName}/${funcName}/source`);
  if (!res.ok) throw new Error(`Source for ${funcName} not found`);
  return res.json();
}

// Solana counterpart — same response shape as `getFunctionSource` so the
// `FunctionSourcePanel` component is agnostic to backend.
export async function getInstructionSource(
  programName: string,
  ixName: string,
): Promise<FunctionSourceResponse> {
  const res = await fetch(
    `${BASE}/api/program/${encodeURIComponent(programName)}/${encodeURIComponent(ixName)}/source`,
  );
  if (!res.ok) throw new Error(`Source for ${ixName} not found`);
  return res.json();
}

export interface SequenceAnalysis {
  functions: {
    name: string;
    preconditions: string[];
    state_writes: string[];
    state_reads: string[];
    external_calls: string[];
    events: string[];
    can_revert: boolean;
    always_reverts: boolean;
    read_only: boolean;
  }[];
  transitions: {
    from: string;
    to: string;
    shared_state: string[];
    conditions_affected: string[];
    has_external_in_from: boolean;
    has_external_in_to: boolean;
  }[];
}

export async function getSequenceAnalysis(contractName: string): Promise<SequenceAnalysis> {
  const res = await fetch(`${BASE}/api/contract/${contractName}/analysis`);
  return res.json();
}

export async function getSearchSuggestions(contractName: string): Promise<SearchSuggestions> {
  const res = await fetch(`${BASE}/api/contract/${contractName}/suggestions`);
  return res.json();
}

export async function getSequences(contractName: string, depth?: number) {
  const url = depth
    ? `${BASE}/api/contract/${contractName}/sequences?depth=${depth}`
    : `${BASE}/api/contract/${contractName}/sequences`;
  const res = await fetch(url);
  return res.json();
}
