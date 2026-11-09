import React, { useEffect, useMemo } from "react";
import { View } from "react-native";
import Animated, {
  cancelAnimation,
  Easing,
  useAnimatedStyle,
  useSharedValue,
  withRepeat,
  withTiming,
} from "react-native-reanimated";
import MaskedView from "@react-native-masked-view/masked-view";
import Svg, { Defs, LinearGradient as SvgLinearGradient, Rect, Stop } from "react-native-svg";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { PaseoLogo } from "@/components/icons/paseo-logo";
import { isNative, isWeb } from "@/constants/platform";
import type { Theme } from "@/styles/theme";

export const LOGO_SIZE = 96;
export const SHIMMER_PEAK_WIDTH = 120;
export const SHIMMER_DURATION_MS = 1800;

const WEB_SPLASH_SHIMMER_KEYFRAME_ID = "paseo-splash-shimmer-keyframes";
const WEB_SPLASH_SHIMMER_ANIMATION_NAME = "paseo-splash-shimmer";

const WEB_SPLASH_SHIMMER_KEYFRAME_CSS = `
  @keyframes ${WEB_SPLASH_SHIMMER_ANIMATION_NAME} {
    0% {
      background-position: -${LOGO_SIZE + SHIMMER_PEAK_WIDTH}px 0;
    }
    100% {
      background-position: ${LOGO_SIZE + SHIMMER_PEAK_WIDTH}px 0;
    }
  }
`;

let webSplashShimmerRegistered = false;

function ensureWebSplashShimmerKeyframes() {
  if (isNative) {
    return;
  }
  if (webSplashShimmerRegistered) {
    return;
  }
  const existing = document.getElementById(WEB_SPLASH_SHIMMER_KEYFRAME_ID);
  if (existing) {
    webSplashShimmerRegistered = true;
    return;
  }
  const styleElement = document.createElement("style");
  styleElement.id = WEB_SPLASH_SHIMMER_KEYFRAME_ID;
  styleElement.textContent = WEB_SPLASH_SHIMMER_KEYFRAME_CSS;
  document.head.appendChild(styleElement);
  webSplashShimmerRegistered = true;
}

export function WebLogoShimmer({ color }: { color: string }) {
  useEffect(() => {
    ensureWebSplashShimmerKeyframes();
  }, []);

  const shimmerStyle = useMemo(
    () => ({
      width: LOGO_SIZE,
      height: LOGO_SIZE,
      WebkitMaskImage: `url("data:image/svg+xml,${encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' width='${LOGO_SIZE}' height='${LOGO_SIZE}' viewBox='0 0 512 512'>
        <g fill='black'>
          <polygon points='126,90 190,90 190,400 126,400'/>
          <polygon points='190,90 210,110 210,380 190,400'/>
          <polygon points='126,90 114,102 114,388 126,400'/>
          <polygon points='214,236 348,90 388,124 266,260'/>
          <polygon points='348,90 388,124 374,138 340,108'/>
          <polygon points='214,236 266,260 252,274 204,248'/>
          <polygon points='246,236 384,374 346,406 208,268' opacity='0.88'/>
          <polygon points='384,374 346,406 332,396 364,368'/>
          <polygon points='208,268 346,406 334,416 196,280'/>
          <polygon points='210,218 238,246 210,274 186,246'/>
          <polygon points='210,228 226,246 210,264 196,246'/>
        </g>
      </svg>`)}")`,
      WebkitMaskSize: "contain",
      WebkitMaskRepeat: "no-repeat",
      WebkitMaskPosition: "center",
      background: `linear-gradient(90deg, ${color} 0%, ${color}88 40%, ${color}FF 50%, ${color}88 60%, ${color} 100%)`,
      backgroundSize: `${LOGO_SIZE + SHIMMER_PEAK_WIDTH * 2}px ${LOGO_SIZE}px`,
      animationName: WEB_SPLASH_SHIMMER_ANIMATION_NAME,
      animationDuration: `${SHIMMER_DURATION_MS}ms`,
      animationTimingFunction: "linear",
      animationIterationCount: "infinite",
    }),
    [color],
  );

  return <View style={shimmerStyle as never} />;
}

export function NativeLogoShimmer({ color }: { color: string }) {
  const shimmerTranslateX = useSharedValue(-SHIMMER_PEAK_WIDTH);

  useEffect(() => {
    shimmerTranslateX.value = -SHIMMER_PEAK_WIDTH;
    shimmerTranslateX.value = withRepeat(
      withTiming(LOGO_SIZE + SHIMMER_PEAK_WIDTH, {
        duration: SHIMMER_DURATION_MS,
        easing: Easing.linear,
      }),
      -1,
      false,
    );
    return () => {
      cancelAnimation(shimmerTranslateX);
    };
  }, [shimmerTranslateX]);

  const peakStyle = useAnimatedStyle(() => ({
    transform: [{ translateX: shimmerTranslateX.value }],
  }));

  const trackStyle = useMemo(
    () => [styles.nativeShimmerTrack, { width: LOGO_SIZE, height: LOGO_SIZE }],
    [],
  );

  const peakCombinedStyle = useMemo(
    () => [styles.nativeShimmerPeak, peakStyle, { width: SHIMMER_PEAK_WIDTH, height: LOGO_SIZE }],
    [peakStyle],
  );

  const maskElement = useMemo(
    () => (
      <View style={styles.shimmerMask}>
        <PaseoLogo size={LOGO_SIZE} color="#000000" />
      </View>
    ),
    [],
  );

  return (
    <MaskedView style={trackStyle} maskElement={maskElement}>
      <View style={trackStyle}>
        <View style={styles.nativeShimmerBase}>
          <PaseoLogo size={LOGO_SIZE} color={color} />
        </View>
        <Animated.View style={peakCombinedStyle}>
          <Svg width="100%" height="100%" preserveAspectRatio="none">
            <Defs>
              <SvgLinearGradient id="splashShimmer" x1="0" y1="0" x2="1" y2="0">
                <Stop offset="0" stopColor="#FFFFFF" stopOpacity="0" />
                <Stop offset="0.5" stopColor="#FFFFFF" stopOpacity="0.4" />
                <Stop offset="1" stopColor="#FFFFFF" stopOpacity="0" />
              </SvgLinearGradient>
            </Defs>
            <Rect x="0" y="0" width="100%" height="100%" fill="url(#splashShimmer)" />
          </Svg>
        </Animated.View>
      </View>
    </MaskedView>
  );
}

function BaseLogoShimmer({ color }: { color: string }) {
  if (isWeb) {
    return <WebLogoShimmer color={color} />;
  }

  return <NativeLogoShimmer color={color} />;
}

export const LogoShimmer = withUnistyles(BaseLogoShimmer, (theme: Theme) => ({
  color: theme.colors.foreground,
}));

const styles = StyleSheet.create(() => ({
  nativeShimmerTrack: {
    position: "relative",
    overflow: "hidden",
  },
  nativeShimmerBase: {
    ...StyleSheet.absoluteFillObject,
  },
  nativeShimmerPeak: {
    position: "absolute",
    top: 0,
    bottom: 0,
    left: 0,
  },
  shimmerMask: {
    width: LOGO_SIZE,
    height: LOGO_SIZE,
    backgroundColor: "transparent",
  },
}));
