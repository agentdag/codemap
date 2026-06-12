import { useCallback, useEffect, useMemo, useState } from "react";
import ReactFlow, {
  Background,
  Controls,
  MiniMap,
  type Edge,
  type Node,
} from "reactflow";
import type { IrDocument, IrNode } from "./ir";
import { toFlowEdges, toFlowNodes } from "./graph";
import { layout } from "./layout";
import { IrNodeView } from "./IrNodeView";
import { DetailsPanel } from "./DetailsPanel";

const SAMPLES = [
  { label: "Go sample", file: "sample-go.json" },
  { label: "TypeScript sample", file: "sample-ts.json" },
];

const nodeTypes = { ir: IrNodeView };

export default function App() {
  const [doc, setDoc] = useState<IrDocument | null>(null);
  const [selected, setSelected] = useState<IrNode | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadSample = useCallback((file: string) => {
    setError(null);
    fetch(`${import.meta.env.BASE_URL}${file}`)
      .then((r) => {
        if (!r.ok) throw new Error(`${r.status} ${r.statusText}`);
        return r.json();
      })
      .then((d: IrDocument) => {
        setDoc(d);
        setSelected(null);
      })
      .catch((e) => setError(`Failed to load ${file}: ${String(e)}`));
  }, []);

  useEffect(() => {
    loadSample(SAMPLES[0].file);
  }, [loadSample]);

  const onFile = (ev: React.ChangeEvent<HTMLInputElement>) => {
    const file = ev.target.files?.[0];
    ev.target.value = "";
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      try {
        const parsed = JSON.parse(String(reader.result)) as IrDocument;
        setDoc(parsed);
        setSelected(null);
        setError(null);
      } catch (e) {
        setError(`Invalid IR JSON: ${String(e)}`);
      }
    };
    reader.readAsText(file);
  };

  // Layout runs once per loaded document (memoized on doc identity).
  const { nodes, edges } = useMemo<{ nodes: Node<IrNode>[]; edges: Edge[] }>(() => {
    if (!doc) return { nodes: [], edges: [] };
    const e = toFlowEdges(doc);
    const n = layout(toFlowNodes(doc), e);
    return { nodes: n, edges: e };
  }, [doc]);

  return (
    <div className="app">
      <header className="toolbar">
        <strong>codemap viewer</strong>
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
        {doc && (
          <span className="muted">
            {doc.root.repoPath} · {doc.nodes.length} nodes · {doc.edges.length} edges
          </span>
        )}
        {error && <span className="error">{error}</span>}
      </header>

      <div className="body">
        <div className="canvas">
          <ReactFlow
            nodes={nodes}
            edges={edges}
            nodeTypes={nodeTypes}
            fitView
            minZoom={0.05}
            onNodeClick={(_, node) => setSelected(node.data as IrNode)}
            onPaneClick={() => setSelected(null)}
          >
            <Background />
            <Controls />
            <MiniMap pannable zoomable />
          </ReactFlow>
        </div>
        <DetailsPanel node={selected} />
      </div>
    </div>
  );
}
