export type ArtifactType = "html" | "svg" | "react" | "markdown";

export interface Artifact {
  id: string;
  title: string;
  type: ArtifactType;
  content: string;
  createdAt: number;
}

export type ViewportMode = "desktop" | "tablet" | "mobile";

export type CanvasBackground = "checkerboard" | "light" | "dark";

export type InspectMode = "preview" | "code" | "split";

export interface HtmlArtifactFrameProps {
  artifact: Artifact;
  defaultViewport?: ViewportMode;
  defaultBackground?: CanvasBackground;
  defaultInspectMode?: InspectMode;
  onClose?: () => void;
  style?: React.CSSProperties;
}

export interface ArtifactInspectorModalProps {
  visible?: boolean;
  artifact?: Artifact | null;
  artifacts?: Artifact[];
  onClose?: () => void;
}

export interface ArtifactSnippetChipProps {
  artifact?: Artifact;
  artifactId?: string;
  title?: string;
  type?: ArtifactType;
  content?: string;
  onClick?: () => void;
  compact?: boolean;
  style?: React.CSSProperties;
}
