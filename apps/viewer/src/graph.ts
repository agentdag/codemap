// Pure mapping: IR document -> React Flow nodes/edges + per-kind visual styling.
// Nothing here is language- or fixture-specific.

import { MarkerType, type Edge, type Node } from "reactflow";
import type { EdgeKind, IrDocument, IrNode, NodeKind } from "./ir";

// Re-exported for existing importers; defined in dims.ts (worker-safe).
export { NODE_W, NODE_H } from "./dims";

// Distinct color per node kind.
const KIND_COLOR: Record<string, string> = {
  Module: "#6366f1",
  File: "#0ea5e9",
  Class: "#f59e0b",
  Interface: "#10b981",
  Function: "#8b5cf6",
  Method: "#ec4899",
  TypeAlias: "#14b8a6",
  Variable: "#64748b",
};

export function kindColor(kind: NodeKind | string): string {
  return KIND_COLOR[kind] ?? "#64748b";
}

// Distinct shape per node kind, via corner rounding (Module = pill).
export function kindRadius(kind: NodeKind | string): number {
  switch (kind) {
    case "Module":
      return 26;
    case "File":
      return 3;
    case "TypeAlias":
      return 16;
    case "Function":
    case "Method":
      return 11;
    default:
      return 6;
  }
}

const EDGE_COLOR: Record<EdgeKind, string> = {
  contains: "#cbd5e1", // subtle
  imports: "#2563eb", // distinct
  calls: "#db2777", // arrowed accent
};

export function toFlowNodes(doc: IrDocument): Node<IrNode>[] {
  return doc.nodes.map((n) => ({
    id: n.id,
    type: "ir",
    position: { x: 0, y: 0 },
    data: n,
  }));
}

export function toFlowEdges(doc: IrDocument): Edge[] {
  return doc.edges.map((e) => {
    const color = EDGE_COLOR[e.kind] ?? "#94a3b8";
    // The cardinal rule, made visible: heuristic edges are dashed, resolved solid.
    const dashed = e.confidence === "heuristic";
    const arrowed = e.kind !== "contains";
    return {
      id: e.id,
      source: e.source,
      target: e.target,
      type: "smoothstep",
      style: {
        stroke: color,
        strokeWidth: e.kind === "contains" ? 1.5 : 2,
        strokeDasharray: dashed ? "6 4" : undefined,
      },
      markerEnd: arrowed
        ? { type: MarkerType.ArrowClosed, color, width: 16, height: 16 }
        : undefined,
      data: { kind: e.kind, confidence: e.confidence },
    };
  });
}
