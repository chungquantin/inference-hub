import type { NextConfig } from "next";

const nextConfig: NextConfig = {
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
