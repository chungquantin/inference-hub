/**
 * Railway project for Inference Hub: the Rust API and worker, built from the
 * repository's Dockerfile, Postgres with pgvector on a volume, and the Next.js
 * web app, built from docker/web.Dockerfile. Only the web service is public;
 * it reaches the API over the private network.
 *
 *   railway link                 # once, pick the project and environment
 *   railway config plan          # preview
 *   railway config apply         # apply after review
 *
 * Secrets (ADMIN_TOKEN, POSTGRES_PASSWORD, LIQUID_API_KEY, GITHUB_TOKEN) are
 * set in the Railway dashboard and kept here with preserve(). See DEPLOY.md.
 */
import { defineRailway, github, group, image, preserve, project, service, volume } from "railway/iac";

const REPO = "chungquantin/inference-hub";

export default defineRailway((ctx) => {
  const prod = ctx.environment === "production";

  const pgdata = volume("pgdata", { sizeMB: 1024 });

  const postgres = service("postgres", {
    source: image("pgvector/pgvector:pg17"),
    volumeMounts: { "/var/lib/postgresql/data": pgdata },
    env: {
      POSTGRES_USER: "inference_hub",
      POSTGRES_DB: "inference_hub",
      POSTGRES_PASSWORD: preserve(),
      // A subdirectory, because the volume's root holds lost+found.
      PGDATA: "/var/lib/postgresql/data/pgdata",
    },
  });

  // Both backend services connect over the private network.
  const databaseUrl =
    "postgres://inference_hub:${{postgres.POSTGRES_PASSWORD}}@${{postgres.RAILWAY_PRIVATE_DOMAIN}}:5432/inference_hub";

  const backendBuild = { builder: "DOCKERFILE" as const, dockerfilePath: "Dockerfile" };

  const api = service("api", {
    source: github(REPO, { branch: "main" }),
    build: {
      ...backendBuild,
      watchPatterns: ["crates/**", "migrations/**", "Cargo.*", "Dockerfile", "docker/**"],
    },
    healthcheck: "/health",
    healthcheckTimeout: 120,
    deploy: { restartPolicyType: "ON_FAILURE", restartPolicyMaxRetries: 10 },
    env: {
      IH_SERVICE: "api",
      // Railway's private network is IPv6 on older environments; "::" also takes IPv4.
      API_HOST: "::",
      PORT: "4000",
      DATABASE_URL: databaseUrl,
      RUN_MIGRATIONS: "true",
      DATABASE_MAX_CONNECTIONS: "10",
      // The browser never calls the API (the web server does, over the private network), so
      // this is only for other origins you allow; leave empty otherwise.
      CORS_ORIGINS: preserve(),
      ADMIN_TOKEN: preserve(),
      // Optional networks for the admin API. Railway's edge sets X-Real-IP.
      ADMIN_ALLOWED_IPS: preserve(),
      CLIENT_IP_HEADER: "X-Real-IP",
    },
  });

  const worker = service("worker", {
    source: github(REPO, { branch: "main" }),
    build: {
      ...backendBuild,
      watchPatterns: ["crates/**", "migrations/**", "data/**", "Cargo.*", "Dockerfile", "docker/**"],
    },
    healthcheck: "/healthz",
    healthcheckTimeout: 120,
    deploy: { restartPolicyType: "ALWAYS" },
    env: {
      IH_SERVICE: "worker",
      DATABASE_URL: databaseUrl,
      DATABASE_MAX_CONNECTIONS: "8",
      WORKER_CONCURRENCY: "4",
      // Import the curated data (data/) on every start, so a deploy that adds organizations and
      // adapters together never leaves the adapters failing. Idempotent.
      RUN_SEED: "true",
      // The worker has no public port; Railway's healthcheck uses PORT.
      PORT: "9090",
      WORKER_METRICS_PORT: "9090",
      GITHUB_TOKEN: preserve(),
      LIQUID_BASE_URL: "https://router.inference.ai.exchange",
      LIQUID_API_KEY: preserve(),
      MAX_ENRICH_USD_PER_DAY: prod ? "1.00" : "0.10",
    },
  });

  const web = service("web", {
    source: github(REPO, { branch: "main" }),
    build: {
      builder: "DOCKERFILE",
      dockerfilePath: "docker/web.Dockerfile",
      // Logos live in apps/web/public, so a data change that adds logos redeploys the web app too.
      watchPatterns: ["apps/web/**", "packages/api-client/**", "package.json", "pnpm-*.yaml", "docker/web.Dockerfile"],
    },
    healthcheck: "/api/health",
    healthcheckTimeout: 120,
    deploy: { restartPolicyType: "ON_FAILURE", restartPolicyMaxRetries: 10 },
    env: {
      PORT: "3000",
      // Server to server over the private network; the API is not public.
      API_URL: "http://${{api.RAILWAY_PRIVATE_DOMAIN}}:4000",
      // The site's public origin, for canonical URLs and the sitemap.
      SITE_URL: preserve(),
      // The same value as the API's; unlocks /admin.
      ADMIN_TOKEN: preserve(),
    },
  });

  return project("inference-hub", {
    resources: [group("Backend", [postgres, api, worker]), group("Web", [web]), pgdata],
  });
});
