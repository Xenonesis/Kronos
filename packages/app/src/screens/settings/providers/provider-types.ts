import type { buildProviderDefinitions } from "@/utils/provider-definitions";
import type { useProvidersSnapshot } from "@/hooks/use-providers-snapshot";

export type ProviderDefinition = ReturnType<typeof buildProviderDefinitions>[number];
export type ProviderEntry = NonNullable<ReturnType<typeof useProvidersSnapshot>["entries"]>[number];

export type StatusTone = "success" | "warning" | "danger" | "muted" | "loading";

export interface ProviderStatus {
  tone: StatusTone;
  label: string;
  modelCount: number | null;
}
