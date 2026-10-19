import Svg, { Polygon, G } from "react-native-svg";
import { useUnistyles } from "react-native-unistyles";

interface PaseoLogoProps {
  size?: number;
  color?: string;
}

export function PaseoLogo({ size = 64, color }: PaseoLogoProps) {
  const { theme } = useUnistyles();
  const fill = color ?? theme.colors.foreground;

  return (
    <Svg width={size} height={size} viewBox="0 0 512 512" fill="none">
      <G id="kronosEmblem">
        {/* Spine */}
        <Polygon points="126,90 190,90 190,400 126,400" fill={fill} />
        <Polygon points="190,90 210,110 210,380 190,400" fill="#334155" />
        <Polygon points="126,90 114,102 114,388 126,400" fill="#1e293b" />

        {/* Upper Chrono Blade */}
        <Polygon points="214,236 348,90 388,124 266,260" fill={fill} />
        <Polygon points="348,90 388,124 374,138 340,108" fill="#1e293b" />
        <Polygon points="214,236 266,260 252,274 204,248" fill="#334155" />

        {/* Lower Chrono Blade */}
        <Polygon points="246,236 384,374 346,406 208,268" fill={fill} opacity={0.88} />
        <Polygon points="384,374 346,406 332,396 364,368" fill="#1e293b" />
        <Polygon points="208,268 346,406 334,416 196,280" fill="#334155" />

        {/* Core Quantum Nexus */}
        <Polygon points="210,218 238,246 210,274 186,246" fill="#00f0ff" />
        <Polygon points="210,228 226,246 210,264 196,246" fill="#ffffff" />
      </G>
    </Svg>
  );
}
