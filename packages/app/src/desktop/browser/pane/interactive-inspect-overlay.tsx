import React, { useEffect, useRef, useState } from "react";
import type {
  BrowserGrabAccessibility,
  BrowserGrabComputedStyles,
  BrowserGrabPayload,
  BrowserGrabTarget,
} from "./browser-grab-payload";

export type {
  BrowserGrabAccessibility,
  BrowserGrabComputedStyles,
  BrowserGrabPayload,
  BrowserGrabTarget,
} from "./browser-grab-payload";

export interface InspectTargetInfo extends BrowserGrabTarget {
  textSnippet: string;
  payload?: BrowserGrabPayload;
}

export interface InteractiveInspectOverlayProps {
  active: boolean;
  iframeRef: React.RefObject<HTMLIFrameElement | null>;
  onSelectElement: (target: InspectTargetInfo) => void;
  onCancel: () => void;
}

interface FiberDebugSource {
  fileName?: string;
  lineNumber?: number;
  columnNumber?: number;
}

interface FiberLike {
  tag?: number;
  type?: unknown;
  elementType?: unknown;
  return?: FiberLike | null;
  _debugSource?: FiberDebugSource | null;
  _debugOwner?: FiberLike | null;
}

const IGNORED_COMPONENTS: Record<string, true> = {
  Fragment: true,
  Suspense: true,
  StrictMode: true,
  Profiler: true,
  Provider: true,
  Consumer: true,
  ContextConsumer: true,
  ContextProvider: true,
  Router: true,
  BrowserRouter: true,
  MemoryRouter: true,
  HashRouter: true,
  Routes: true,
  Route: true,
  Outlet: true,
  Navigate: true,
};

function getReactFiberFromElement(element: Element): FiberLike | null {
  if (typeof element !== "object" || element === null) return null;
  const elementObj = element as unknown as Record<string, unknown>;
  const keys = Object.keys(elementObj);
  for (const key of keys) {
    if (key.startsWith("__reactFiber$") || key.startsWith("__reactInternalInstance$")) {
      const fiber = elementObj[key];
      if (typeof fiber === "object" && fiber !== null) {
        return fiber as unknown as FiberLike;
      }
    }
  }
  return null;
}

function getComponentNameFromType(type: unknown): string | null {
  if (!type) return null;
  if (typeof type === "string") return type;
  if (typeof type === "function") {
    const fn = type as { displayName?: unknown; name?: unknown };
    if (typeof fn.displayName === "string" && fn.displayName) return fn.displayName;
    if (typeof fn.name === "string" && fn.name) return fn.name;
    return null;
  }
  if (typeof type === "object") {
    const obj = type as Record<string, unknown>;
    if (typeof obj.displayName === "string" && obj.displayName) return obj.displayName;
    if (typeof obj.name === "string" && obj.name) return obj.name;
    if (typeof obj.render === "object" && obj.render !== null) {
      const render = obj.render as { displayName?: unknown; name?: unknown };
      if (typeof render.displayName === "string" && render.displayName) return render.displayName;
      if (typeof render.name === "string" && render.name) return render.name;
    }
    if (typeof obj.type === "object" && obj.type !== null) {
      return getComponentNameFromType(obj.type);
    }
  }
  return null;
}

function isIgnoredComponentName(name: string): boolean {
  if (!name) return true;
  if (IGNORED_COMPONENTS[name]) return true;
  if (name.includes(".Provider") || name.includes(".Consumer")) return true;
  if (name.startsWith("Context.") || name.startsWith("React.")) return true;
  if (/^[a-z][a-z0-9-]*$/.test(name)) return true;
  return false;
}
export function extractReactFiberInfo(element: Element): { component?: string; source?: string } {
  const info: { component?: string; source?: string } = {};
  try {
    const rootFiber = getReactFiberFromElement(element);
    if (!rootFiber) return info;

    let curr: FiberLike | null = rootFiber;
    let depth = 0;
    const maxDepth = 50;

    while (curr && depth < maxDepth) {
      if (!info.source && curr._debugSource) {
        const { fileName, lineNumber } = curr._debugSource;
        if (fileName) {
          info.source = lineNumber ? `${fileName}:${lineNumber}` : fileName;
        }
      }

      if (!info.component) {
        const candidateName =
          getComponentNameFromType(curr.type) || getComponentNameFromType(curr.elementType);
        if (candidateName && !isIgnoredComponentName(candidateName)) {
          info.component = candidateName;
        }
      }

      if (info.component && info.source) {
        break;
      }

      curr = curr.return || null;
      depth++;
    }
  } catch {
    // Graceful fallback for cross-origin or security restrictions
  }
  return info;
}

function getBoxShorthand(style: CSSStyleDeclaration, prop: "padding" | "margin"): string {
  const direct = style.getPropertyValue(prop);
  if (direct) return direct;
  const top = style.getPropertyValue(`${prop}-top`);
  const right = style.getPropertyValue(`${prop}-right`);
  const bottom = style.getPropertyValue(`${prop}-bottom`);
  const left = style.getPropertyValue(`${prop}-left`);
  if (!top && !right && !bottom && !left) return "";
  if (top === bottom && right === left) {
    return top === right ? top : `${top} ${right}`;
  }
  return `${top} ${right} ${bottom} ${left}`;
}

function getBorderShorthand(style: CSSStyleDeclaration): string {
  const direct = style.getPropertyValue("border");
  if (direct) return direct;
  const width = style.getPropertyValue("border-top-width");
  const styleVal = style.getPropertyValue("border-top-style");
  const color = style.getPropertyValue("border-top-color");
  if (width || styleVal || color) {
    return `${width} ${styleVal} ${color}`.trim();
  }
  return "";
}

export function extractComputedStyles(element: Element): BrowserGrabComputedStyles | undefined {
  try {
    const win = element.ownerDocument?.defaultView || window;
    if (!win || typeof win.getComputedStyle !== "function") {
      return undefined;
    }
    const style = win.getComputedStyle(element);
    return {
      display: style.getPropertyValue("display") || style.display || "",
      position: style.getPropertyValue("position") || style.position || "",
      color: style.getPropertyValue("color") || style.color || "",
      backgroundColor: style.getPropertyValue("background-color") || style.backgroundColor || "",
      fontSize: style.getPropertyValue("font-size") || style.fontSize || "",
      fontWeight: style.getPropertyValue("font-weight") || style.fontWeight || "",
      padding: getBoxShorthand(style, "padding"),
      margin: getBoxShorthand(style, "margin"),
      border: getBorderShorthand(style),
    };
  } catch {
    return undefined;
  }
}

export function extractAccessibility(element: Element): BrowserGrabAccessibility | undefined {
  try {
    const roleAttr = element.getAttribute("role");
    let role = roleAttr || undefined;
    if (!role) {
      const tag = element.tagName.toLowerCase();
      if (tag === "button") role = "button";
      else if (tag === "a" && element.hasAttribute("href")) role = "link";
      else if (tag === "input") {
        const type = element.getAttribute("type") || "text";
        role = type === "checkbox" ? "checkbox" : type === "radio" ? "radio" : "textbox";
      } else if (tag === "select") role = "combobox";
      else if (tag === "textarea") role = "textbox";
      else if (/^h[1-6]$/.test(tag)) role = "heading";
      else if (tag === "nav") role = "navigation";
      else if (tag === "main") role = "main";
    }

    const ariaLabel = element.getAttribute("aria-label") || undefined;

    let accessibleName: string | undefined = ariaLabel;
    if (!accessibleName && element.hasAttribute("aria-labelledby")) {
      const id = element.getAttribute("aria-labelledby");
      if (id && element.ownerDocument) {
        const labelEl = element.ownerDocument.getElementById(id);
        if (labelEl && labelEl.textContent) {
          accessibleName = labelEl.textContent.trim();
        }
      }
    }
    if (!accessibleName && element.tagName.toLowerCase() === "img") {
      accessibleName = element.getAttribute("alt") || undefined;
    }
    if (!accessibleName && element.hasAttribute("title")) {
      accessibleName = element.getAttribute("title") || undefined;
    }
    if (!accessibleName && (role === "button" || role === "link" || role === "heading")) {
      const text = (element.textContent || "").trim();
      if (text) {
        accessibleName = text.slice(0, 100);
      }
    }

    if (!role && !ariaLabel && !accessibleName) {
      return undefined;
    }

    return {
      role,
      ariaLabel,
      accessibleName,
    };
  } catch {
    return undefined;
  }
}

function getElementStepSelector(el: Element): string {
  const tag = el.tagName.toLowerCase();
  if (el.id) {
    return `${tag}#${el.id}`;
  }
  if (typeof el.className === "string" && el.className.trim()) {
    const classes = el.className
      .trim()
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((c) => `.${c}`)
      .join("");
    if (classes) {
      return `${tag}${classes}`;
    }
  }
  const parent = el.parentElement;
  if (parent) {
    const sameTagSiblings = Array.from(parent.children).filter(
      (child) => child.tagName.toLowerCase() === tag,
    );
    if (sameTagSiblings.length > 1) {
      const index = sameTagSiblings.indexOf(el) + 1;
      return `${tag}:nth-of-type(${index})`;
    }
  }
  return tag;
}

export function generateHierarchicalSelector(element: Element, maxDepth = 4): string {
  const steps: string[] = [];
  let curr: Element | null = element;

  while (curr && curr.nodeType === 1) {
    const tag = curr.tagName.toLowerCase();
    if (tag === "body" || tag === "html") {
      break;
    }

    const step = getElementStepSelector(curr);
    steps.unshift(step);

    if (curr.id) {
      break;
    }

    if (steps.length >= maxDepth) {
      break;
    }

    curr = curr.parentElement;
  }

  return steps.join(" > ") || element.tagName.toLowerCase();
}

export function extractNearbyContext(element: Element): string | undefined {
  try {
    const parts: string[] = [];
    const prev = element.previousElementSibling;
    if (prev) {
      const prevText = (prev.textContent || "").trim().replace(/\s+/g, " ");
      if (prevText) {
        const tag = prev.tagName.toLowerCase();
        parts.push(`Previous (<${tag}>): "${prevText.slice(0, 60)}"`);
      }
    }
    const next = element.nextElementSibling;
    if (next) {
      const nextText = (next.textContent || "").trim().replace(/\s+/g, " ");
      if (nextText) {
        const tag = next.tagName.toLowerCase();
        parts.push(`Next (<${tag}>): "${nextText.slice(0, 60)}"`);
      }
    }
    if (parts.length === 0 && element.parentElement) {
      const parent = element.parentElement;
      const parentTag = parent.tagName.toLowerCase();
      if (parentTag !== "body" && parentTag !== "html") {
        const parentText = (parent.textContent || "").trim().replace(/\s+/g, " ");
        if (parentText) {
          parts.push(`Parent (<${parentTag}>): "${parentText.slice(0, 80)}"`);
        }
      }
    }
    return parts.length > 0 ? parts.join(" | ") : undefined;
  } catch {
    return undefined;
  }
}

export function extractHtmlSnippet(element: Element, maxLength = 1000): string | undefined {
  try {
    const html = element.outerHTML;
    if (!html) return undefined;
    if (html.length <= maxLength) {
      return html;
    }
    return `${html.slice(0, maxLength)}...`;
  } catch {
    return undefined;
  }
}

export function buildInspectTargetInfo(
  element: Element,
  iframe?: HTMLIFrameElement | null,
): InspectTargetInfo {
  const elRect = element.getBoundingClientRect();
  const tag = element.tagName.toLowerCase();
  const id = element.id || undefined;
  const className = typeof element.className === "string" ? element.className : undefined;
  const selector = generateHierarchicalSelector(element);
  const textContent = (element.textContent || "").trim();
  const fiberInfo = extractReactFiberInfo(element);
  const computedStyles = extractComputedStyles(element);
  const accessibility = extractAccessibility(element);
  const nearbyContext = extractNearbyContext(element);
  const html = extractHtmlSnippet(element);

  const target: BrowserGrabTarget = {
    tag,
    id,
    className,
    selector,
    rect: {
      left: elRect.left,
      top: elRect.top,
      width: elRect.width,
      height: elRect.height,
    },
    textContent,
    component: fiberInfo.component,
    source: fiberInfo.source,
    nearbyContext,
    computedStyles,
    accessibility,
    html,
  };

  const url = iframe?.contentWindow?.location?.href || iframe?.src || "";
  const title = iframe?.contentDocument?.title || undefined;

  const payload: BrowserGrabPayload = {
    url,
    title,
    target,
    timestamp: Date.now(),
  };

  return {
    ...target,
    textSnippet: textContent.slice(0, 150),
    payload,
  };
}

export function InteractiveInspectOverlay({
  active,
  iframeRef,
  onSelectElement,
  onCancel,
}: InteractiveInspectOverlayProps) {
  const [highlightRect, setHighlightRect] = useState<{
    left: number;
    top: number;
    width: number;
    height: number;
    label: string;
    targetInfo: InspectTargetInfo;
  } | null>(null);

  const hoveredTargetRef = useRef<InspectTargetInfo | null>(null);

  useEffect(() => {
    if (!active) {
      setHighlightRect(null);
      hoveredTargetRef.current = null;
    }
  }, [active]);

  if (!active) return null;

  return (
    <div
      style={{
        position: "absolute",
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        zIndex: 99999,
        cursor: "crosshair",
        pointerEvents: "auto",
      }}
      onMouseMove={(e) => {
        const iframe = iframeRef.current;
        if (!iframe) return;
        try {
          const doc = iframe.contentDocument || iframe.contentWindow?.document;
          if (!doc) return;

          const iframeRect = iframe.getBoundingClientRect();
          const relX = e.clientX - iframeRect.left;
          const relY = e.clientY - iframeRect.top;

          const target = doc.elementFromPoint(relX, relY);
          if (!target || target === doc.documentElement || target === doc.body) {
            setHighlightRect(null);
            hoveredTargetRef.current = null;
            return;
          }

          const targetInfo = buildInspectTargetInfo(target, iframe);
          const elRect = targetInfo.rect;
          const tag = targetInfo.tag;
          const id = targetInfo.id ? `#${targetInfo.id}` : "";
          const cls = targetInfo.className
            ? `.${targetInfo.className.trim().split(/\s+/).slice(0, 2).join(".")}`
            : "";
          const compPrefix = targetInfo.component ? `<${targetInfo.component}> ` : "";
          const label = `${compPrefix}${tag}${id}${cls} (${Math.round(elRect.width)}×${Math.round(elRect.height)})`;

          hoveredTargetRef.current = targetInfo;
          setHighlightRect({
            left: elRect.left,
            top: elRect.top,
            width: elRect.width,
            height: elRect.height,
            label,
            targetInfo,
          });
        } catch {
          // If cross-origin, ignore
        }
      }}
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();

        const iframe = iframeRef.current;
        let selected = hoveredTargetRef.current || highlightRect?.targetInfo;

        if (!selected && iframe) {
          try {
            const doc = iframe.contentDocument || iframe.contentWindow?.document;
            if (doc) {
              const iframeRect = iframe.getBoundingClientRect();
              const relX = e.clientX - iframeRect.left;
              const relY = e.clientY - iframeRect.top;
              const target = doc.elementFromPoint(relX, relY);
              if (target && target !== doc.documentElement && target !== doc.body) {
                selected = buildInspectTargetInfo(target, iframe);
              }
            }
          } catch {
            // cross-origin
          }
        }

        if (selected) {
          onSelectElement(selected);
        } else {
          onCancel();
        }
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        onCancel();
      }}
    >
      {highlightRect ? (
        <div
          style={{
            position: "absolute",
            left: highlightRect.left,
            top: highlightRect.top,
            width: highlightRect.width,
            height: highlightRect.height,
            border: "2px solid #22c55e",
            backgroundColor: "rgba(34, 197, 94, 0.2)",
            boxShadow: "0 0 0 1px rgba(0,0,0,0.4), inset 0 0 0 1px rgba(255,255,255,0.4)",
            pointerEvents: "none",
            transition: "all 0.05s ease-out",
            borderRadius: "2px",
          }}
        >
          <div
            style={{
              position: "absolute",
              bottom: "100%",
              left: 0,
              marginBottom: 4,
              backgroundColor: "#15803d",
              color: "#ffffff",
              fontSize: "11px",
              fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
              fontWeight: 600,
              padding: "2px 6px",
              borderRadius: "4px",
              whiteSpace: "nowrap",
              boxShadow: "0 2px 4px rgba(0,0,0,0.3)",
            }}
          >
            {highlightRect.label}
          </div>
        </div>
      ) : null}
    </div>
  );
}
