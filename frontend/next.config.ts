import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: 'export',
  experimental: {
    optimizePackageImports: ['lucide-react'],
  },
};

export default nextConfig;
