/**
 * @vitest-environment jsdom
 */
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { TelemetryHud, calculateTps } from "./telemetry-hud";

afterEach(cleanup);

describe("TelemetryHud", () => {
  it("calculates TPS correctly given token count and elapsed milliseconds", () => {
    expect(calculateTps(120, 2000)).toBe(60);
    expect(calculateTps(0, 500)).toBe(0);
    expect(calculateTps(50, 0)).toBe(0);
  });

  it("renders tactical telemetry badges with TPS and TTFT", () => {
    render(<TelemetryHud tps={45.2} ttftMs={380} isStreaming={true} />);
    expect(screen.getByText(/45\.2 TPS/)).toBeTruthy();
    expect(screen.getByText(/380ms TTFT/)).toBeTruthy();
  });

  it("renders cost badge when costUsd is positive", () => {
    render(<TelemetryHud tps={10} ttftMs={100} costUsd={0.0123} isStreaming={true} />);
    expect(screen.getByText(/\$0\.0123/)).toBeTruthy();
  });

  it("renders cost badge when cost is positive even if idle", () => {
    render(<TelemetryHud costUsd={0.0123} isStreaming={false} />);
    expect(screen.getByText(/\$0\.0123/)).toBeTruthy();
  });

  it("renders null when idle, tps is 0, and ttftMs is not provided", () => {
    const { container } = render(<TelemetryHud isStreaming={false} tps={0} />);
    expect(container.firstChild).toBeNull();
  });
});
