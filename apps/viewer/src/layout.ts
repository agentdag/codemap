// Hierarchical top-down auto-layout via dagre. Run once when a document loads.

import dagre from "@dagrejs/dagre";
import type { Edge, Node } from "reactflow";
import { NODE_H, NODE_W } from "./graph";

export function layout<T>(nodes: Node<T>[], edges: Edge[]): Node<T>[] {
  const g = new dagre.graphlib.Graph();
  g.setGraph({ rankdir: "TB", nodesep: 44, ranksep: 80, marginx: 24, marginy: 24 });
  g.setDefaultEdgeLabel(() => ({}));

  const ids = new Set(nodes.map((n) => n.id));
  for (const n of nodes) {
    g.setNode(n.id, { width: NODE_W, height: NODE_H });
  }
  for (const e of edges) {
    // Only edges between present nodes constrain the layout.
    if (ids.has(e.source) && ids.has(e.target)) {
      g.setEdge(e.source, e.target);
    }
  }

  dagre.layout(g);

  return nodes.map((n) => {
    const p = g.node(n.id);
    // dagre gives node centers; React Flow positions are top-left.
    return { ...n, position: { x: p.x - NODE_W / 2, y: p.y - NODE_H / 2 } };
  });
}
