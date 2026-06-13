import { useCallback, useEffect, useRef, useState } from "react";
import ReactFlow, {
  Background,
  Controls,
  MiniMap,
  type Edge,
  type Node,
} from "reactflow";
import type { IrDocument, IrNode } from "./ir";
import { toFlowEdges, toFlowNodes } from "./graph";
import type { NodePosition } from "./layout";
import { IrNodeView } from "./IrNodeView";
import { DetailsPanel } from "./DetailsPanel";
import { analyzeFolder, isTauri } from "./tauri";

const SAMPLES = [
  { label: "Go sample", file: "sample-go.json" },
  { label: "TypeScript sample", file: "sample-ts.json" },
];

// Above this many nodes, don't auto-layout/fit — a multi-thousand-node graph can
// hard-freeze the browser. Show counts + a "Render anyway" escape hatch instead.
const LARGE_GRAPH_THRESHOLD = 1500;

const nodeTypes = { ir: IrNodeView };

type Graph = { nodes: Node<IrNode>[]; edges: Edge[] };

export default function App() {
  const [doc, setDoc] = useState<IrDocument | null>(null);
  const [selected, setSelected] = useState<IrNode | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [layingOut, setLayingOut] = useState(false);
  const [confirmLarge, setConfirmLarge] = useState(false);
  const [graph, setGraph] = useState<Graph>({ nodes: [], edges: [] });
  // Bumped on every load so the canvas remounts and re-fits per document.
  const [loadId, setLoadId] = useState(0);
  const [native] = useState(isTauri);
  const workerRef = useRef<Worker | null>(null);

  const nodeCount = doc?.nodes.length ?? 0;
  const edgeCount = doc?.edges.length ?? 0;
  const empty = doc !== null && nodeCount === 0;
  const tooBig = nodeCount > LARGE_GRAPH_THRESHOLD && !confirmLarge;
  const renderingLarge = nodeCount > LARGE_GRAPH_THRESHOLD; // confirmed large

  // Long-lived layout worker (dagre off the main thread).
  useEffect(() => {
    const w = new Worker(new URL("./layout.worker.ts", import.meta.url), { type: "module" });
    workerRef.current = w;
    return () => {
      w.terminate();
      workerRef.current = null;
    };
  }, []);

  // A single pipeline for EVERY source (analyze, sample, JSON): guard large
  // graphs, otherwise lay out in the worker and render.
  const applyDoc = useCallback((next: IrDocument) => {
    setSelected(null);
    setError(null);
    setConfirmLarge(false);
    setGraph({ nodes: [], edges: [] });
    setDoc(next);
    setLoadId((n) => n + 1);
  }, []);

  useEffect(() => {
    if (!doc || empty || tooBig) {
      setLayingOut(false);
      return;
    }
    const worker = workerRef.current;
    const edges = toFlowEdges(doc);
    const baseNodes = toFlowNodes(doc);
    if (!worker) return;

    setLayingOut(true);
    let cancelled = false;
    const onMessage = (event: MessageEvent<NodePosition[]>) => {
      if (cancelled) return;
      const pos = new Map(event.data.map((p) => [p.id, p]));
      const nodes = baseNodes.map((n) => {
        const p = pos.get(n.id);
        return p ? { ...n, position: { x: p.x, y: p.y } } : n;
      });
      setGraph({ nodes, edges });
      setLayingOut(false);
    };
    worker.addEventListener("message", onMessage, { once: true });
    worker.postMessage({
      ids: baseNodes.map((n) => n.id),
      edges: edges.map((e) => ({ source: e.source, target: e.target })),
    });
    return () => {
      cancelled = true;
      worker.removeEventListener("message", onMessage);
    };
  }, [doc, empty, tooBig]);

  const loadSample = useCallback(
    (file: string) => {
      fetch(`${import.meta.env.BASE_URL}${file}`)
        .then((r) => {
          if (!r.ok) throw new Error(`${r.status} ${r.statusText}`);
          return r.json();
        })
        .then((d: IrDocument) => applyDoc(d))
        .catch((e) => setError(`Failed to load ${file}: ${String(e)}`));
    },
    [applyDoc],
  );

  useEffect(() => {
    loadSample(SAMPLES[0].file);
  }, [loadSample]);

  // Native: pick a folder and render a LIVE extract_repo run (not a sample).
  const openFolder = useCallback(async () => {
    setError(null);
    setAnalyzing(true);
    try {
      const live = await analyzeFolder();
      if (live) applyDoc(live);
    } catch (e) {
      setError(`Analyze failed: ${String(e)}`);
    } finally {
      setAnalyzing(false);
    }
  }, [applyDoc]);

  const onFile = (ev: React.ChangeEvent<HTMLInputElement>) => {
    const file = ev.target.files?.[0];
    ev.target.value = "";
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      try {
        applyDoc(JSON.parse(String(reader.result)) as IrDocument);
      } catch (e) {
        setError(`Invalid IR JSON: ${String(e)}`);
      }
    };
    reader.readAsText(file);
  };

  const status = analyzing
    ? "Analyzing…"
    : layingOut
      ? "Laying out…"
      : doc
        ? `${doc.root.repoPath} · ${nodeCount} nodes · ${edgeCount} edges`
        : "";

  return (
    <div className="app">
      <header className="toolbar">
        <strong>codemap viewer</strong>
        {native && (
          <button className="file-btn" onClick={openFolder} disabled={analyzing}>
            {analyzing ? "Analyzing…" : "Open folder…"}
          </button>
        )}
        <select
          aria-label="Bundled sample"
          defaultValue={SAMPLES[0].file}
          onChange={(e) => loadSample(e.target.value)}
        >
          {SAMPLES.map((s) => (
            <option key={s.file} value={s.file}>
              {s.label}
            </option>
          ))}
        </select>
        <label className="file-btn">
          Load IR JSON…
          <input type="file" accept="application/json,.json" onChange={onFile} hidden />
        </label>
        {status && <span className="muted">{status}</span>}
        {error && <span className="error">{error}</span>}
      </header>

      <div className="body">
        <div className="canvas">
          {empty ? (
            <div className="placeholder">No supported files found in this repository.</div>
          ) : tooBig ? (
            <div className="placeholder">
              <p>
                Large graph: <b>{nodeCount}</b> nodes · <b>{edgeCount}</b> edges.
              </p>
              <p className="muted">
                Auto-layout is skipped to keep the app responsive.
              </p>
              <button className="file-btn" onClick={() => setConfirmLarge(true)}>
                Render anyway
              </button>
            </div>
          ) : layingOut ? (
            <div className="placeholder">Laying out…</div>
          ) : (
            <ReactFlow
              key={loadId}
              nodes={graph.nodes}
              edges={graph.edges}
              nodeTypes={nodeTypes}
              fitView
              minZoom={0.02}
              onlyRenderVisibleElements
              nodesDraggable={!renderingLarge}
              onNodeClick={(_, node) => setSelected(node.data as IrNode)}
              onPaneClick={() => setSelected(null)}
            >
              <Background />
              <Controls />
              <MiniMap pannable zoomable />
            </ReactFlow>
          )}
        </div>
        <DetailsPanel node={selected} />
      </div>
    </div>
  );
}
