// Hierarchical top-down auto-layout via dagre. Pure and reactflow-free so it can
// run inside a Web Worker (see layout.worker.ts) — the heavy dagre pass stays off
// the main thread. The renderer applies the returned positions to its nodes.

import dagre from "@dagrejs/dagre";
import { NODE_H, NODE_W } from "./dims";

export interface LayoutEdge {
  source: string;
  target: string;
}

export interface NodePosition {
  id: string;
  x: number;
  y: number;
}

/** Compute top-left positions for `ids` given the `edges` between them. */
export function layoutPositions(ids: string[], edges: LayoutEdge[]): NodePosition[] {
  const g = new dagre.graphlib.Graph();
  g.setGraph({ rankdir: "TB", nodesep: 44, ranksep: 80, marginx: 24, marginy: 24 });
  g.setDefaultEdgeLabel(() => ({}));

  const present = new Set(ids);
  for (const id of ids) {
    g.setNode(id, { width: NODE_W, height: NODE_H });
  }
  for (const e of edges) {
    if (present.has(e.source) && present.has(e.target)) {
      g.setEdge(e.source, e.target);
    }
  }

  dagre.layout(g);

  return ids.map((id) => {
    const p = g.node(id);
    // dagre gives node centers; React Flow positions are top-left.
    return { id, x: p.x - NODE_W / 2, y: p.y - NODE_H / 2 };
  });
}
