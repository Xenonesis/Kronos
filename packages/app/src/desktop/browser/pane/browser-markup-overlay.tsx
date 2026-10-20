import React, { useCallback, useEffect, useRef, useState } from "react";
import { Platform, Pressable, StyleSheet, Text, TextInput, View } from "react-native";
import { Square, ArrowUpRight, Edit3, Trash2, Send, X } from "lucide-react";
import type {
  MarkupPoint,
  MarkupShape,
  MarkupShapeType,
  BrowserAnnotationPayload,
} from "./browser-markup-types";

export interface BrowserMarkupOverlayProps {
  active: boolean;
  onClose: () => void;
  onSendToAgent: (payload: BrowserAnnotationPayload) => void;
  url: string;
  title: string;
  testID?: string;
}

export function BrowserMarkupOverlay({
  active,
  onClose,
  onSendToAgent,
  url,
  title,
  testID = "browser-markup-overlay",
}: BrowserMarkupOverlayProps) {
  const [currentTool, setCurrentTool] = useState<MarkupShapeType>("rect");
  const [shapes, setShapes] = useState<MarkupShape[]>([]);
  const [currentShape, setCurrentShape] = useState<MarkupShape | null>(null);
  const [comment, setComment] = useState("");
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const isDrawingRef = useRef(false);
  const startPosRef = useRef<MarkupPoint>({ x: 0, y: 0 });

  // Redraw all shapes on canvas
  const redrawCanvas = useCallback(() => {
    if (Platform.OS !== "web" || !canvasRef.current) return;
    const canvas = canvasRef.current;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    ctx.clearRect(0, 0, canvas.width, canvas.height);

    const allShapes = currentShape ? [...shapes, currentShape] : shapes;
    for (const shape of allShapes) {
      ctx.strokeStyle = shape.color;
      ctx.lineWidth = shape.lineWidth;
      ctx.lineCap = "round";
      ctx.lineJoin = "round";

      if (shape.type === "rect" || shape.type === "crop") {
        if (
          shape.x !== undefined &&
          shape.y !== undefined &&
          shape.width !== undefined &&
          shape.height !== undefined
        ) {
          ctx.strokeRect(shape.x, shape.y, shape.width, shape.height);
          if (shape.type === "crop") {
            ctx.fillStyle = "rgba(16, 185, 129, 0.15)";
            ctx.fillRect(shape.x, shape.y, shape.width, shape.height);
          }
        }
      } else if (shape.type === "arrow") {
        if (
          shape.startX !== undefined &&
          shape.startY !== undefined &&
          shape.endX !== undefined &&
          shape.endY !== undefined
        ) {
          ctx.beginPath();
          ctx.moveTo(shape.startX, shape.startY);
          ctx.lineTo(shape.endX, shape.endY);
          ctx.stroke();

          // Arrowhead
          const angle = Math.atan2(shape.endY - shape.startY, shape.endX - shape.startX);
          const headLen = 12;
          ctx.beginPath();
          ctx.moveTo(shape.endX, shape.endY);
          ctx.lineTo(
            shape.endX - headLen * Math.cos(angle - Math.PI / 6),
            shape.endY - headLen * Math.sin(angle - Math.PI / 6),
          );
          ctx.moveTo(shape.endX, shape.endY);
          ctx.lineTo(
            shape.endX - headLen * Math.cos(angle + Math.PI / 6),
            shape.endY - headLen * Math.sin(angle + Math.PI / 6),
          );
          ctx.stroke();
        }
      } else if (shape.type === "freehand") {
        if (shape.points && shape.points.length > 1) {
          ctx.beginPath();
          ctx.moveTo(shape.points[0]!.x, shape.points[0]!.y);
          for (let i = 1; i < shape.points.length; i++) {
            ctx.lineTo(shape.points[i]!.x, shape.points[i]!.y);
          }
          ctx.stroke();
        }
      }
    }
  }, [shapes, currentShape]);

  useEffect(() => {
    redrawCanvas();
  }, [redrawCanvas]);

  // Sync canvas dimensions with parent viewport
  useEffect(() => {
    if (!active || Platform.OS !== "web" || !canvasRef.current) return;
    const canvas = canvasRef.current;
    const parent = canvas.parentElement;
    if (parent) {
      canvas.width = parent.clientWidth || 800;
      canvas.height = parent.clientHeight || 600;
      redrawCanvas();
    }
  }, [active, redrawCanvas]);

  const handleMouseDown = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (Platform.OS !== "web") return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    isDrawingRef.current = true;
    startPosRef.current = { x, y };

    const newShape: MarkupShape = {
      id: Math.random().toString(36).slice(2, 9),
      type: currentTool,
      color: currentTool === "crop" ? "#10b981" : "#ef4444",
      lineWidth: currentTool === "freehand" ? 3 : 2,
      points: currentTool === "freehand" ? [{ x, y }] : undefined,
      x,
      y,
      width: 0,
      height: 0,
      startX: x,
      startY: y,
      endX: x,
      endY: y,
    };
    setCurrentShape(newShape);
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!isDrawingRef.current || !currentShape || Platform.OS !== "web") return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const currentX = e.clientX - rect.left;
    const currentY = e.clientY - rect.top;

    if (currentTool === "rect" || currentTool === "crop") {
      const x = Math.min(startPosRef.current.x, currentX);
      const y = Math.min(startPosRef.current.y, currentY);
      const width = Math.abs(currentX - startPosRef.current.x);
      const height = Math.abs(currentY - startPosRef.current.y);
      setCurrentShape({ ...currentShape, x, y, width, height });
    } else if (currentTool === "arrow") {
      setCurrentShape({ ...currentShape, endX: currentX, endY: currentY });
    } else if (currentTool === "freehand") {
      const points = currentShape.points
        ? [...currentShape.points, { x: currentX, y: currentY }]
        : [{ x: currentX, y: currentY }];
      setCurrentShape({ ...currentShape, points });
    }
  };

  const handleMouseUp = () => {
    if (!isDrawingRef.current) return;
    isDrawingRef.current = false;
    if (currentShape) {
      setShapes((prev) => [...prev, currentShape]);
      setCurrentShape(null);
    }
  };

  const handleClear = () => {
    setShapes([]);
    setCurrentShape(null);
    setComment("");
    if (Platform.OS === "web" && canvasRef.current) {
      const ctx = canvasRef.current.getContext("2d");
      if (ctx) ctx.clearRect(0, 0, canvasRef.current.width, canvasRef.current.height);
    }
  };

  const handleSend = () => {
    const canvas = canvasRef.current;
    const viewport = {
      width: canvas?.width || 800,
      height: canvas?.height || 600,
    };

    let screenshotBase64: string | undefined = undefined;
    if (canvas) {
      try {
        screenshotBase64 = canvas.toDataURL("image/png");
      } catch {
        // Fallback if canvas is tainted
      }
    }

    onSendToAgent({
      url,
      title,
      timestamp: new Date().toISOString(),
      viewport,
      shapes,
      textComment: comment.trim() || undefined,
      screenshotBase64,
    });
    handleClear();
    onClose();
  };

  if (!active) return null;

  return (
    <View style={styles.overlay} testID={testID}>
      {/* Top Toolbar */}
      <View style={styles.toolbar}>
        <View style={styles.toolGroup}>
          <Pressable
            style={[styles.toolBtn, currentTool === "rect" && styles.toolBtnActive]}
            onPress={() => setCurrentTool("rect")}
            accessibilityLabel="Rectangle Tool"
          >
            <Square size={16} color={currentTool === "rect" ? "#fff" : "#94a3b8"} />
          </Pressable>

          <Pressable
            style={[styles.toolBtn, currentTool === "arrow" && styles.toolBtnActive]}
            onPress={() => setCurrentTool("arrow")}
            accessibilityLabel="Arrow Tool"
          >
            <ArrowUpRight size={16} color={currentTool === "arrow" ? "#fff" : "#94a3b8"} />
          </Pressable>

          <Pressable
            style={[styles.toolBtn, currentTool === "freehand" && styles.toolBtnActive]}
            onPress={() => setCurrentTool("freehand")}
            accessibilityLabel="Pen Tool"
          >
            <Edit3 size={16} color={currentTool === "freehand" ? "#fff" : "#94a3b8"} />
          </Pressable>

          <Pressable
            style={[styles.toolBtn, currentTool === "crop" && styles.toolBtnActive]}
            onPress={() => setCurrentTool("crop")}
            accessibilityLabel="Crop Area"
          >
            <Text style={[styles.cropLabel, currentTool === "crop" && styles.cropLabelActive]}>
              CROP
            </Text>
          </Pressable>

          <View style={styles.divider} />

          <Pressable style={styles.toolBtn} onPress={handleClear} accessibilityLabel="Clear All">
            <Trash2 size={16} color="#ef4444" />
          </Pressable>
        </View>

        {/* Comment Input */}
        <TextInput
          style={styles.commentInput}
          placeholder="Describe issue / change for agent..."
          placeholderTextColor="#64748b"
          value={comment}
          onChangeText={setComment}
        />

        <View style={styles.actionGroup}>
          <Pressable style={styles.sendBtn} onPress={handleSend}>
            <Send size={14} color="#fff" />
            <Text style={styles.sendBtnText}>Send to Agent</Text>
          </Pressable>

          <Pressable style={styles.closeBtn} onPress={onClose}>
            <X size={16} color="#94a3b8" />
          </Pressable>
        </View>
      </View>

      {/* HTML5 Canvas Surface */}
      {Platform.OS === "web" ? (
        <canvas
          ref={canvasRef as React.LegacyRef<HTMLCanvasElement>}
          style={{
            position: "absolute",
            top: 48,
            left: 0,
            width: "100%",
            height: "calc(100% - 48px)",
            cursor: "crosshair",
            touchAction: "none",
          }}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
        />
      ) : null}
    </View>
  );
}

const styles = StyleSheet.create({
  overlay: {
    position: "absolute",
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: "rgba(15, 23, 42, 0.45)",
    zIndex: 9999,
  },
  toolbar: {
    height: 48,
    backgroundColor: "#1e293b",
    borderBottomWidth: 1,
    borderBottomColor: "#334155",
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: 12,
    gap: 12,
    zIndex: 10000,
  },
  toolGroup: {
    flexDirection: "row",
    alignItems: "center",
    backgroundColor: "#0f172a",
    borderRadius: 6,
    padding: 3,
    gap: 4,
  },
  toolBtn: {
    padding: 6,
    borderRadius: 4,
    justifyContent: "center",
    alignItems: "center",
  },
  toolBtnActive: {
    backgroundColor: "#3b82f6",
  },
  cropLabel: {
    fontSize: 10,
    fontWeight: "700",
    color: "#94a3b8",
  },
  cropLabelActive: {
    color: "#fff",
  },
  divider: {
    width: 1,
    height: 16,
    backgroundColor: "#334155",
    marginHorizontal: 4,
  },
  commentInput: {
    flex: 1,
    height: 32,
    backgroundColor: "#0f172a",
    borderRadius: 6,
    paddingHorizontal: 10,
    color: "#f8fafc",
    fontSize: 12,
    borderWidth: 1,
    borderColor: "#334155",
  },
  actionGroup: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
  },
  sendBtn: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    backgroundColor: "#10b981",
    paddingHorizontal: 12,
    paddingVertical: 7,
    borderRadius: 6,
  },
  sendBtnText: {
    color: "#fff",
    fontSize: 12,
    fontWeight: "600",
  },
  closeBtn: {
    padding: 6,
    borderRadius: 4,
    backgroundColor: "#0f172a",
  },
});
