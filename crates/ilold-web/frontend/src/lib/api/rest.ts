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
}

export interface CytoscapeEdgeData {
  id: string;
  source: string;
  target: string;
  kind: string;
  call_count: number;
}

// API client

const BASE = '';  // same origin in production, proxied in dev

export async function getProject(): Promise<ProjectSummary> {
  const res = await fetch(`${BASE}/api/project`);
  return res.json();
}

export async function getContract(name: string): Promise<ContractDetail> {
  const res = await fetch(`${BASE}/api/contract/${name}`);
  if (!res.ok) throw new Error(`Contract ${name} not found`);
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

export async function getSequences(contractName: string, depth?: number) {
  const url = depth
    ? `${BASE}/api/contract/${contractName}/sequences?depth=${depth}`
    : `${BASE}/api/contract/${contractName}/sequences`;
  const res = await fetch(url);
  return res.json();
}
