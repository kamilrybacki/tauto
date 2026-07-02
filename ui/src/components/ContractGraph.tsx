import { useMemo, useCallback } from 'react';
import {
  ReactFlow,
  type Node,
  type Edge,
  type NodeTypes,
  Background,
  Controls,
  Handle,
  Position,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import type { GraphResponse, GraphNodeData } from '../api/types';

type ContractNodeData = GraphNodeData & { selected: boolean; conflict: boolean } & Record<string, unknown>;

function ContractNode({ data }: { data: ContractNodeData }) {
  const r = data.requires_count as number;
  const e = data.ensures_count as number;
  return (
    <div
      className={`rf-node${data.conflict ? ' conflict' : ''}`}
      style={{ minWidth: 176, boxShadow: data.selected ? '0 0 0 2px #1f3a68' : 'none', cursor: 'pointer' }}
    >
      <Handle type="target" position={Position.Left} style={{ background: 'var(--ink-60)', width: 5, height: 5 }} />
      <div className="rf-node-title" style={{ fontWeight: 600 }}>{data.case}</div>
      <div className="rf-node-sub">
        {data.entity} · {data.operation}
      </div>
      <div className="rf-node-sub">
        {r} req · {e} ens{data.conflict ? ' · ⊥' : ''}
      </div>
      <Handle type="source" position={Position.Right} style={{ background: 'var(--ink-60)', width: 5, height: 5 }} />
    </div>
  );
}

const nodeTypes = { contract: ContractNode as NodeTypes['string'] } as NodeTypes;

function layoutNodes(rawNodes: GraphResponse['nodes']): Node<ContractNodeData>[] {
  const entityOrder = [...new Set(rawNodes.map((n) => n.data.entity))].sort();
  const entityX: Record<string, number> = {};
  entityOrder.forEach((e, i) => { entityX[e] = i * 320; });
  const sorted = [...rawNodes].sort((a, b) => {
    const ei = entityOrder.indexOf(a.data.entity) - entityOrder.indexOf(b.data.entity);
    if (ei !== 0) return ei;
    if (a.data.operation !== b.data.operation) return a.data.operation.localeCompare(b.data.operation);
    return a.data.case.localeCompare(b.data.case);
  });
  const entityRow: Record<string, number> = {};
  return sorted.map((n) => {
    const row = entityRow[n.data.entity] ?? 0;
    entityRow[n.data.entity] = row + 1;
    return {
      id: n.id,
      type: 'contract',
      position: { x: entityX[n.data.entity], y: row * 118 },
      data: { ...n.data, selected: false, conflict: false } as ContractNodeData,
    };
  });
}

interface ContractGraphProps {
  graph: GraphResponse;
  slug: string;
  selected: string | null;
  onSelect: (id: string) => void;
}

export default function ContractGraph({ graph, slug, selected, onSelect }: ContractGraphProps) {
  const conflictNodes = useMemo(() => {
    const s = new Set<string>();
    graph.edges.filter((e) => e.kind === 'conflict').forEach((e) => { s.add(e.source); s.add(e.target); });
    return s;
  }, [graph.edges]);

  const nodes = useMemo((): Node<ContractNodeData>[] =>
    layoutNodes(graph.nodes).map((n) => ({
      ...n,
      data: { ...n.data, selected: n.id === selected, conflict: conflictNodes.has(n.id) },
    })),
  [graph.nodes, selected, conflictNodes]);

  const edges = useMemo((): Edge[] =>
    graph.edges.map((e) => ({
      id: e.id,
      source: e.source,
      target: e.target,
      label: e.kind === 'conflict' ? '⊥' : undefined,
      style: {
        stroke: e.kind === 'conflict' ? '#8c2f22' : 'rgba(28,24,20,0.3)',
        strokeWidth: e.kind === 'conflict' ? 1.5 : 1,
        strokeDasharray: '5 4',
      },
      labelStyle: { fill: '#8c2f22', fontSize: 13, fontFamily: 'EB Garamond, serif' },
      labelBgStyle: { fill: '#faf7ef' },
    })),
  [graph.edges]);

  const onNodeClick = useCallback((_: React.MouseEvent, node: Node) => onSelect(node.id), [onSelect]);

  const ops = new Set(graph.nodes.map((n) => `${n.data.entity}/${n.data.operation}`)).size;
  const conflicts = graph.edges.filter((e) => e.kind === 'conflict').length;

  return (
    <figure className="figure">
      <div className="figure-frame graph-canvas">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodeClick={onNodeClick}
          fitView
          fitViewOptions={{ padding: 0.2 }}
          proOptions={{ hideAttribution: true }}
          colorMode="light"
          aria-label="Rule dependency graph"
        >
          <Background color="rgba(28,24,20,0.08)" gap={26} />
          <Controls showInteractive={false} />
        </ReactFlow>
      </div>
      <figcaption className="figcaption">
        <b>Figure 1.</b> Rule dependency graph for <em>{slug || 'default'}</em>: {graph.nodes.length} rules
        across {ops} operations{conflicts > 0 ? `, ${conflicts} candidate contradiction${conflicts !== 1 ? 's' : ''} (⊥)` : ''}.
      </figcaption>
    </figure>
  );
}
