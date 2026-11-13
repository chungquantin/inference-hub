import path from "node:path";
import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // A self-contained server (`.next/standalone`) for the container image. In this workspace the
  // trace root is the repository, so it includes the API client package the app compiles.
  output: "standalone",
  outputFileTracingRoot: path.join(import.meta.dirname, "../.."),
  // The API client is TypeScript source in the workspace; let Next compile it.
  transpilePackages: ["@inference-hub/api-client"],
  // The market pages now live under /analytics. Keep every old address working.
  async redirects() {
    return [
      { source: "/market/terminal", destination: "/analytics/metrics", permanent: true },
      { source: "/market/terminal/:metric", destination: "/analytics/metrics/:metric", permanent: true },
      { source: "/market", destination: "/analytics", permanent: true },
      { source: "/market/:path*", destination: "/analytics/:path*", permanent: true },
    ];
  },
};

export default nextConfig;
