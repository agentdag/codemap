// Custom React Flow node: colored/shaped by IR node kind, labelled by name.

import { Handle, Position, type NodeProps } from "reactflow";
import type { IrNode } from "./ir";
import { kindColor, kindRadius, NODE_H, NODE_W } from "./graph";

export function IrNodeView({ data, selected }: NodeProps<IrNode>) {
  const color = kindColor(data.kind);
  return (
    <div
      style={{
        width: NODE_W,
        height: NODE_H,
        boxSizing: "border-box",
        background: "#ffffff",
        color: "#0f172a",
        borderRadius: kindRadius(data.kind),
        border: `1px solid ${selected ? "#0f172a" : "#e2e8f0"}`,
        borderLeft: `6px solid ${color}`,
        boxShadow: selected ? "0 0 0 2px rgba(15,23,42,0.25)" : "0 1px 2px rgba(0,0,0,0.08)",
        padding: "6px 12px",
        display: "flex",
        flexDirection: "column",
        justifyContent: "center",
        gap: 2,
        fontFamily: "system-ui, sans-serif",
        overflow: "hidden",
      }}
    >
      <Handle type="target" position={Position.Top} style={{ opacity: 0 }} />
      <div
        style={{
          fontSize: 10,
          fontWeight: 700,
          letterSpacing: 0.5,
          textTransform: "uppercase",
          color,
        }}
      >
        {data.kind}
      </div>
      <div
        style={{
          fontSize: 13,
          fontWeight: 600,
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
        title={data.qualifiedName || data.name}
      >
        {data.name || "(anonymous)"}
      </div>
      <Handle type="source" position={Position.Bottom} style={{ opacity: 0 }} />
    </div>
  );
}
