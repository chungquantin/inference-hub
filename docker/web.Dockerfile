# The Next.js web app, as a standalone server.
#
#   docker build -f docker/web.Dockerfile -t inference-hub-web .
#   docker run -e API_URL=http://api:4000 -p 3000:3000 inference-hub-web
#
# The build needs no API: pages render on request. The server reads API_URL (and
# ADMIN_TOKEN for /admin) at run time, and listens on PORT.

ARG NODE_VERSION=22

FROM node:${NODE_VERSION}-bookworm-slim AS base
ENV PNPM_HOME=/pnpm PATH=/pnpm:$PATH NEXT_TELEMETRY_DISABLED=1
RUN corepack enable
WORKDIR /repo

# --- Dependencies: only the manifests, so a source change keeps this layer ----
FROM base AS deps
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/web/package.json apps/web/
COPY packages/api-client/package.json packages/api-client/
RUN --mount=type=cache,id=pnpm,target=/pnpm/store \
    pnpm install --frozen-lockfile --filter web... --store-dir /pnpm/store

# --- Build -------------------------------------------------------------------
FROM deps AS build
COPY apps/web apps/web
COPY packages/api-client packages/api-client
RUN pnpm --filter web build

# --- Runtime -----------------------------------------------------------------
FROM node:${NODE_VERSION}-bookworm-slim AS runtime
ENV NODE_ENV=production NEXT_TELEMETRY_DISABLED=1 HOSTNAME=:: PORT=3000
RUN useradd --system --uid 10001 --home-dir /app --shell /usr/sbin/nologin app
WORKDIR /app
COPY --from=build --chown=root:root /repo/apps/web/.next/standalone ./
COPY --from=build --chown=root:root /repo/apps/web/.next/static apps/web/.next/static
COPY --from=build --chown=root:root /repo/apps/web/public apps/web/public
# Next's data cache (API responses) lives here.
RUN mkdir -p apps/web/.next/cache && chown app apps/web/.next/cache
USER app
EXPOSE 3000
CMD ["node", "apps/web/server.js"]
