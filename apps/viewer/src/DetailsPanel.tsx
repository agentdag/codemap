// Right-hand panel: details for the selected node, or a legend when none is.

import type { IrNode, NodeKind } from "./ir";
import { kindColor } from "./graph";

const KINDS: NodeKind[] = [
  "Module",
  "File",
  "Class",
  "Interface",
  "Function",
  "Method",
  "TypeAlias",
  "Variable",
];

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ marginBottom: 10 }}>
      <div style={{ fontSize: 11, textTransform: "uppercase", color: "#64748b" }}>{label}</div>
      <div style={{ fontSize: 13, wordBreak: "break-all", fontFamily: "ui-monospace, monospace" }}>
        {value || "—"}
      </div>
    </div>
  );
}

function Legend() {
  return (
    <div>
      <h3 style={{ margin: "0 0 8px" }}>Legend</h3>
      <p style={{ fontSize: 12, color: "#64748b", marginTop: 0 }}>
        Click a node to inspect it.
      </p>
      <div style={{ display: "grid", gap: 6, marginBottom: 16 }}>
        {KINDS.map((k) => (
          <div key={k} style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span
              style={{
                width: 14,
                height: 14,
                borderRadius: k === "Module" ? 7 : 3,
                background: kindColor(k),
                display: "inline-block",
              }}
            />
            <span style={{ fontSize: 13 }}>{k}</span>
          </div>
        ))}
      </div>
      <h4 style={{ margin: "0 0 6px" }}>Edges</h4>
      <ul style={{ fontSize: 12, color: "#475569", paddingLeft: 18, margin: 0 }}>
        <li>
          <span style={{ color: "#94a3b8" }}>contains</span> — subtle, no arrow
        </li>
        <li>
          <span style={{ color: "#2563eb" }}>imports</span> — blue, arrowed
        </li>
        <li>
          <span style={{ color: "#db2777" }}>calls</span> — pink, arrowed
        </li>
        <li>
          <b>dashed</b> = heuristic · <b>solid</b> = resolved
        </li>
      </ul>
    </div>
  );
}

export function DetailsPanel({ node }: { node: IrNode | null }) {
  if (!node) {
    return (
      <aside className="panel">
        <Legend />
      </aside>
    );
  }
  const { range } = node.location;
  const loc = `${node.location.file}  ${range.startLine}:${range.startCol}–${range.endLine}:${range.endCol}`;
  return (
    <aside className="panel">
      <h3 style={{ margin: "0 0 12px", color: kindColor(node.kind) }}>{node.name || "(anonymous)"}</h3>
      <Row label="kind" value={node.kind} />
      <Row label="id" value={node.id} />
      <Row label="qualifiedName" value={node.qualifiedName} />
      <Row label="language" value={node.language} />
      <Row label="location" value={loc} />
    </aside>
  );
}
