import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: 'export',
  trailingSlash: true,
  // Published images use /__OXI_BASE_PATH__; the backend resolves it at startup.
  // Other builds can still select a fixed prefix (empty means the origin root).
  basePath: process.env.NEXT_PUBLIC_BASE_PATH || '',
  experimental: {
    optimizePackageImports: ['lucide-react'],
  },
};

export default nextConfig;
