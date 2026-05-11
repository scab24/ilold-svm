import { getProgramOverlay, type CpiEdge, type CuStats, type RuntimeOverlay } from '$lib/api/rest';
import type { SessionOverlayUpdate } from '$lib/api/types';

let program = $state<string>('');
let scenario = $state<string>('');
let callsPerIx = $state<Record<string, number>>({});
let failedPerIx = $state<Record<string, number>>({});
let cuStatsPerIx = $state<Record<string, CuStats>>({});
let cpiEdges = $state<CpiEdge[]>([]);
let initialized = $state<boolean>(false);

export function getCallsPerIx(): Record<string, number> {
  return callsPerIx;
}

export function getFailedPerIx(): Record<string, number> {
  return failedPerIx;
}

export function getCuStatsPerIx(): Record<string, CuStats> {
  return cuStatsPerIx;
}

export function getCpiEdges(): CpiEdge[] {
  return cpiEdges;
}

export function getOverlayProgram(): string {
  return program;
}

export function getOverlayScenario(): string {
  return scenario;
}

export function clearOverlay(): void {
  program = '';
  scenario = '';
  callsPerIx = {};
  failedPerIx = {};
  cuStatsPerIx = {};
  cpiEdges = [];
  initialized = false;
}

function applySnapshot(overlay: RuntimeOverlay): void {
  program = overlay.program;
  scenario = overlay.scenario;
  callsPerIx = { ...overlay.calls_per_ix };
  failedPerIx = { ...overlay.failed_per_ix };
  cuStatsPerIx = { ...overlay.cu_stats_per_ix };
  cpiEdges = overlay.cpi_edges.map((e) => ({ ...e }));
  initialized = true;
}

export async function loadInitialOverlay(name: string, scenarioName?: string): Promise<void> {
  try {
    const overlay = await getProgramOverlay(name, scenarioName);
    applySnapshot(overlay);
  } catch (err) {
    console.warn('runtimeOverlay loadInitial failed:', err);
    clearOverlay();
    program = name;
    if (scenarioName) scenario = scenarioName;
    initialized = true;
  }
}

function recomputeStats(prev: CuStats | undefined, sample: number): CuStats {
  if (!prev) {
    return { min: sample, max: sample, avg: sample, samples: 1 };
  }
  const samples = prev.samples + 1;
  const sum = prev.avg * prev.samples + sample;
  return {
    min: Math.min(prev.min, sample),
    max: Math.max(prev.max, sample),
    avg: Math.round(sum / samples),
    samples,
  };
}

export function applyOverlayUpdate(patch: SessionOverlayUpdate): void {
  if (!initialized) return;
  if (scenario && patch.scenario !== scenario) return;

  const ix = patch.ix_name;

  if (patch.calls_added > 0) {
    callsPerIx = {
      ...callsPerIx,
      [ix]: (callsPerIx[ix] ?? 0) + patch.calls_added,
    };
  }

  if (patch.failed_added > 0) {
    failedPerIx = {
      ...failedPerIx,
      [ix]: (failedPerIx[ix] ?? 0) + patch.failed_added,
    };
  }

  if (typeof patch.cu === 'number' && patch.calls_added > 0) {
    cuStatsPerIx = {
      ...cuStatsPerIx,
      [ix]: recomputeStats(cuStatsPerIx[ix], patch.cu),
    };
  }

  if (patch.cpi_targets_added.length > 0) {
    const map = new Map<string, CpiEdge>();
    for (const e of cpiEdges) {
      map.set(`${e.from_ix}|${e.to_program}|${e.depth}`, { ...e });
    }
    for (const to of patch.cpi_targets_added) {
      const key = `${ix}|${to}|1`;
      const prev = map.get(key);
      if (prev) {
        prev.samples += 1;
        map.set(key, prev);
      } else {
        map.set(key, { from_ix: ix, to_program: to, depth: 1, samples: 1 });
      }
    }
    cpiEdges = Array.from(map.values()).sort((a, b) => {
      if (a.from_ix !== b.from_ix) return a.from_ix.localeCompare(b.from_ix);
      if (a.to_program !== b.to_program) return a.to_program.localeCompare(b.to_program);
      return a.depth - b.depth;
    });
  }
}
