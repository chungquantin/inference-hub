/**
 * Railway project for Inference Hub's backend: the Rust API and worker, built
 * from the repository's Dockerfile, and Postgres with pgvector on a volume.
 * The web app is deployed to Vercel and is not part of this project.
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
      DATABASE_URL: databaseUrl,
      RUN_MIGRATIONS: "true",
      DATABASE_MAX_CONNECTIONS: "10",
      // The web app's origin, such as https://your-domain; set per environment.
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
      // The worker has no public port; Railway's healthcheck uses PORT.
      PORT: "9090",
      WORKER_METRICS_PORT: "9090",
      GITHUB_TOKEN: preserve(),
      LIQUID_BASE_URL: "https://router.inference.ai.exchange",
      LIQUID_API_KEY: preserve(),
      MAX_ENRICH_USD_PER_DAY: prod ? "1.00" : "0.10",
    },
  });

  return project("inference-hub", {
    resources: [group("Backend", [postgres, api, worker]), pgdata],
  });
});
