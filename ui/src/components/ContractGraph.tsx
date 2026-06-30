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

// xyflow requires NodeData to extend Record<string, unknown>
type ContractNodeData = GraphNodeData & { selected: boolean } & Record<string, unknown>;

function ContractNode({ data }: { data: ContractNodeData }) {
  return (
    <div
      style={{
        padding: '8px 12px',
        border: `2px solid ${data.selected ? '#6366f1' : '#374151'}`,
        borderRadius: 8,
        background: data.selected ? '#1e1b4b' : '#1f2937',
        color: '#f9fafb',
        minWidth: 190,
        cursor: 'pointer',
        transition: 'border-color 0.15s, background 0.15s',
      }}
    >
      <Handle type="target" position={Position.Left} style={{ background: '#4b5563' }} />
      <div style={{ fontSize: 10, color: '#9ca3af', textTransform: 'uppercase', letterSpacing: '0.06em' }}>
        {data.entity}
      </div>
      <div style={{ fontSize: 13, fontWeight: 600, color: '#e5e7eb', marginTop: 2 }}>
        {data.operation}
      </div>
      <div style={{ fontSize: 11, color: '#d1d5db', marginTop: 1 }}>
        {data.case}
      </div>
      {((data.requires_count as number) > 0 || (data.ensures_count as number) > 0) && (
        <div style={{ marginTop: 5, fontSize: 10, color: '#6b7280', display: 'flex', gap: 6 }}>
          {(data.requires_count as number) > 0 && (
            <span style={{ color: '#fbbf24' }}>{data.requires_count as number} req</span>
          )}
          {(data.ensures_count as number) > 0 && (
            <span style={{ color: '#34d399' }}>{data.ensures_count as number} ens</span>
          )}
        </div>
      )}
      <Handle type="source" position={Position.Right} style={{ background: '#4b5563' }} />
    </div>
  );
}

// Cast needed: xyflow's NodeTypes expects ComponentType<NodeProps> which uses generic Record
const nodeTypes = {
  contract: ContractNode as NodeTypes['string'],
} as NodeTypes;

function layoutNodes(rawNodes: GraphResponse['nodes']): Node<ContractNodeData>[] {
  const entityOrder = [...new Set(rawNodes.map(n => n.data.entity))].sort();
  const entityX: Record<string, number> = {};
  entityOrder.forEach((e, i) => { entityX[e] = i * 330; });

  const sorted = [...rawNodes].sort((a, b) => {
    const ei = entityOrder.indexOf(a.data.entity) - entityOrder.indexOf(b.data.entity);
    if (ei !== 0) return ei;
    if (a.data.operation !== b.data.operation) return a.data.operation.localeCompare(b.data.operation);
    return a.data.case.localeCompare(b.data.case);
  });

  const entityRow: Record<string, number> = {};
  return sorted.map(n => {
    const row = entityRow[n.data.entity] ?? 0;
    entityRow[n.data.entity] = row + 1;
    return {
      id: n.id,
      type: 'contract',
      position: { x: entityX[n.data.entity], y: row * 130 },
      data: { ...n.data, selected: false } as ContractNodeData,
    };
  });
}

interface ContractGraphProps {
  graph: GraphResponse;
  selected: string | null;
  onSelect: (id: string) => void;
}

export default function ContractGraph({ graph, selected, onSelect }: ContractGraphProps) {
  const nodes = useMemo((): Node<ContractNodeData>[] => {
    const laid = layoutNodes(graph.nodes);
    return laid.map(n => ({
      ...n,
      data: { ...n.data, selected: n.id === selected },
    }));
  }, [graph.nodes, selected]);

  const edges = useMemo((): Edge[] =>
    graph.edges.map(e => ({
      id: e.id,
      source: e.source,
      target: e.target,
      label: e.label,
      style: {
        stroke: e.kind === 'conflict' ? '#ef4444' : '#6b7280',
        strokeWidth: e.kind === 'conflict' ? 2 : 1,
        strokeDasharray: e.kind === 'same_op' ? '6 4' : undefined,
      },
      labelStyle: { fill: '#9ca3af', fontSize: 10 },
      labelBgStyle: { fill: 'transparent' },
    })),
  [graph.edges]);

  const onNodeClick = useCallback((_: React.MouseEvent, node: Node) => {
    onSelect(node.id);
  }, [onSelect]);

  return (
    <div style={{ width: '100%', height: '100%', background: '#111827', position: 'relative' }}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodeClick={onNodeClick}
        fitView
        fitViewOptions={{ padding: 0.2 }}
        attributionPosition="bottom-left"
        colorMode="dark"
      >
        <Background color="#1f2937" gap={24} />
        <Controls showInteractive={false} />
      </ReactFlow>
      <div className="legend">
        <div className="legend-item">
          <div
            className="legend-line"
            style={{ background: '#6b7280', backgroundImage: 'repeating-linear-gradient(to right, #6b7280, #6b7280 6px, transparent 6px, transparent 10px)' }}
          />
          <span>same operation</span>
        </div>
        <div className="legend-item">
          <div className="legend-line" style={{ background: '#ef4444', height: 2 }} />
          <span>conflict candidate</span>
        </div>
      </div>
    </div>
  );
}
