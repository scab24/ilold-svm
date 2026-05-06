import type { Node, Edge } from '@xyflow/svelte';
import type { ProgramDetail } from '$lib/api/rest';
import type {
  AccountNodeData,
  GraphNodeData,
  InstructionNodeData,
} from '$lib/stores/graph.svelte';

const INSTRUCTION_X_STEP = 220;
const ACCOUNT_X_STEP = 180;
const INSTRUCTION_Y = 320;
const ACCOUNT_Y = 0;

export function composeProgramGraph(program: ProgramDetail): {
  nodes: Node<GraphNodeData>[];
  edges: Edge[];
} {
  const accountIds = new Map<string, string>();
  const accountNodes: Node<GraphNodeData>[] = program.account_types.map(
    (a, i) => {
      const id = `account:${a.name}`;
      accountIds.set(a.name, id);
      const data: AccountNodeData = {
        _type: 'account',
        label: a.name,
        programName: program.name,
        fields: extractFieldList(a),
      };
      return {
        id,
        type: 'account',
        position: { x: i * ACCOUNT_X_STEP, y: ACCOUNT_Y },
        data,
      };
    },
  );

  const instructionNodes: Node<GraphNodeData>[] = program.instructions.map(
    (ix, i) => {
      const id = `ix:${ix.name}`;
      const signers = (ix.accounts ?? [])
        .filter((a: any) => a.signer)
        .map((a: any) => a.name);
      const hasPdas = (ix.accounts ?? []).some((a: any) => a.pda != null);
      const data: InstructionNodeData = {
        _type: 'instruction',
        label: ix.name,
        programName: program.name,
        programId: program.program_id,
        argsCount: (ix.args ?? []).length,
        accountsCount: (ix.accounts ?? []).length,
        hasPdas,
        signers,
      };
      return {
        id,
        type: 'instruction',
        position: { x: i * INSTRUCTION_X_STEP, y: INSTRUCTION_Y },
        data,
      };
    },
  );

  const edges: Edge[] = [];
  for (const ix of program.instructions) {
    const ixId = `ix:${ix.name}`;
    const seen = new Set<string>();
    for (const acc of ix.accounts ?? []) {
      const accId = accountIds.get(acc.name);
      if (!accId) continue;
      const key = `${ixId}->${accId}`;
      if (seen.has(key)) continue;
      seen.add(key);
      edges.push({
        id: `e:${key}`,
        source: ixId,
        sourceHandle: 't',
        target: accId,
        targetHandle: 'b',
        animated: false,
      });
    }
  }

  return { nodes: [...accountNodes, ...instructionNodes], edges };
}

function extractFieldList(a: any): { name: string; type: string }[] {
  const ty = a?.layout?.ty;
  const fields = ty?.kind === 'Struct' ? ty.fields ?? ty?.Struct?.fields : null;
  if (!Array.isArray(fields)) return [];
  return fields.map((f: any) => ({
    name: f?.name ?? '?',
    type: typeof f?.ty === 'string' ? f.ty : JSON.stringify(f?.ty ?? '?'),
  }));
}
