/// <reference lib="webworker" />
// Runs the dagre layout off the main thread. Receives node ids + edges, posts
// back computed positions. Reuses layoutPositions (the renderer is not forked).

import { layoutPositions, type LayoutEdge } from "./layout";

export interface LayoutRequest {
  ids: string[];
  edges: LayoutEdge[];
}

const ctx = self as unknown as DedicatedWorkerGlobalScope;

ctx.onmessage = (event: MessageEvent<LayoutRequest>) => {
  const { ids, edges } = event.data;
  ctx.postMessage(layoutPositions(ids, edges));
};
