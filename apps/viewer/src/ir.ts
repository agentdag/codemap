// Minimal TypeScript mirror of the IR, hand-written from docs/specs/ir.md.
// Not generated from Rust this stage. The viewer treats it generically — it does
// not assume any particular language or node/edge set beyond these unions.

export type NodeKind =
  | "File"
  | "Module"
  | "Function"
  | "Method"
  | "Class"
  | "Interface"
  | "TypeAlias"
  | "Variable";

export type EdgeKind = "contains" | "imports" | "calls";

export type Confidence = "resolved" | "heuristic" | "asserted";

export interface Range {
  startLine: number;
  startCol: number;
  endLine: number;
  endCol: number;
}

export interface Location {
  file: string;
  range: Range;
}

export interface CallSite {
  file: string;
  range: Range;
}

export interface IrNode {
  id: string;
  kind: NodeKind;
  name: string;
  qualifiedName: string;
  language: string;
  location: Location;
  contentHash: string;
  metadata: Record<string, unknown>;
}

export interface IrEdge {
  id: string;
  source: string;
  target: string;
  kind: EdgeKind;
  confidence: Confidence;
  sites: CallSite[];
  metadata: Record<string, unknown>;
}

export interface RootInfo {
  repoPath: string;
  analyzedAt: string;
  toolVersion: string;
}

export interface IrDocument {
  schemaVersion: string;
  root: RootInfo;
  nodes: IrNode[];
  edges: IrEdge[];
}
