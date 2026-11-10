import type { OrgKind } from "@inference-hub/api-client";

export const ORG_KIND_LABELS: Record<OrgKind, string> = {
  model_provider: "Model provider",
  inference_provider: "Inference provider",
  llm_gateway: "LLM gateway",
  gpu_cloud: "GPU cloud",
  hyperscaler: "Hyperscaler",
  hardware: "Hardware",
  open_source: "Open source",
  investor: "Investor",
  other: "Other",
};
