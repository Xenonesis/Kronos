import { memo, useEffect, useMemo, useState } from "react";
import { View, type ViewStyle } from "react-native";
import { useRetainedPanelActive } from "@/components/retained-panel";
import {
  SYNCED_LOADER_DOT_COUNT,
  getSyncedLoaderDotOpacity,
  getSyncedLoaderStep,
} from "@/components/synced-loader-state";

const GRID_COLUMNS = 2;
const DOT_KEYS = Array.from({ length: SYNCED_LOADER_DOT_COUNT }, (_, i) => `dot-${i}`);

// Global tick manager on Web - single global requestAnimationFrame loop
type StepListener = (step: number) => void;
const listeners = new Set<StepListener>();
let animationFrameId: number | null = null;
let currentStep = getSyncedLoaderStep(Date.now());

function tick() {
  const nextStep = getSyncedLoaderStep(Date.now());
  if (nextStep !== currentStep) {
    currentStep = nextStep;
    listeners.forEach((listener) => listener(nextStep));
  }
  if (listeners.size > 0) {
    animationFrameId = requestAnimationFrame(tick);
  } else {
    animationFrameId = null;
  }
}

function subscribe(listener: StepListener) {
  listeners.add(listener);
  listener(currentStep);
  if (animationFrameId === null) {
    animationFrameId = requestAnimationFrame(tick);
  }
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0 && animationFrameId !== null) {
      cancelAnimationFrame(animationFrameId);
      animationFrameId = null;
    }
  };
}

export const SyncedLoader = memo(function SyncedLoader({
  size = 10,
  color,
}: {
  size?: number;
  color: string;
}) {
  const active = useRetainedPanelActive();
  const [step, setStep] = useState(currentStep);

  useEffect(() => {
    if (!active) return;
    return subscribe(setStep);
  }, [active]);

  const gap = Math.max(1, Math.round(size * 0.12));
  const dotSize = Math.max(2, (size - gap * 2) / 3);
  const gridWidth = dotSize * 2 + gap;
  const gridHeight = dotSize * 3 + gap * 2;

  const gridStyle = useMemo(
    () => ({
      width: gridWidth,
      height: gridHeight,
      position: "relative" as const,
    }),
    [gridHeight, gridWidth],
  );

  return (
    <View style={gridStyle}>
      {DOT_KEYS.map((key, dotIndex) => {
        const column = dotIndex % GRID_COLUMNS;
        const row = Math.floor(dotIndex / GRID_COLUMNS);
        const left = column * (dotSize + gap);
        const top = row * (dotSize + gap);
        const opacity = getSyncedLoaderDotOpacity(step, dotIndex);

        return (
          <View
            key={key}
            style={[
              {
                width: dotSize,
                height: dotSize,
                borderRadius: dotSize / 2,
                backgroundColor: color,
                position: "absolute",
                left,
                top,
                opacity,
              },
              {
                transition: "opacity 120ms ease-in-out",
              } as unknown as ViewStyle,
            ]}
          />
        );
      })}
    </View>
  );
});
