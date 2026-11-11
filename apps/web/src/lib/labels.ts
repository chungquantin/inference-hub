import type { OrgKind, ProductType } from "@inference-hub/api-client";

export const ORG_KIND_LABELS: Record<OrgKind, string> = {
  model_provider: "Model provider",
  inference_provider: "Inference provider",
  llm_gateway: "LLM gateway",
  gpu_cloud: "GPU cloud",
  hyperscaler: "Hyperscaler",
  hardware: "Hardware",
  open_source: "Open source",
  decentralized: "Decentralized compute",
  llmops: "LLMOps",
  agent_tool: "Agent tool",
  investor: "Investor",
  other: "Other",
};

export const PRODUCT_TYPE_LABELS: Record<ProductType, string> = {
  model: "Model",
  inference_api: "Inference API",
  gateway: "Gateway",
  gpu_instance: "GPU instance",
  oss_project: "Open-source project",
  hardware: "Hardware",
  platform: "Platform",
  other: "Other",
};

export function isOrgKind(value: string | undefined): value is OrgKind {
  return value !== undefined && Object.hasOwn(ORG_KIND_LABELS, value);
}

export function isProductType(value: string | undefined): value is ProductType {
  return value !== undefined && Object.hasOwn(PRODUCT_TYPE_LABELS, value);
}
