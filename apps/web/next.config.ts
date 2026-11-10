import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // The API client is TypeScript source in the workspace; let Next compile it.
  transpilePackages: ["@inference-hub/api-client"],
};

export default nextConfig;
