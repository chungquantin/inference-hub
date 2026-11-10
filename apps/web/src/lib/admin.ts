import "server-only";

import { createHmac, timingSafeEqual } from "node:crypto";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { cache } from "react";
import { createApiClient } from "@inference-hub/api-client";

/**
 * Admin session for the editorial pages.
 *
 * The web server holds ADMIN_TOKEN, the same token the Rust API checks. An
 * editor proves they know it once; the session cookie then holds an HMAC of the
 * token (never the token), so a leaked cookie stops working when the token is
 * rotated. Every admin page and Server Action calls `requireAdmin()` itself:
 * rendering a page is not an authorization check for the actions on it.
 */

const COOKIE = "ih_admin";
const MAX_AGE_SECONDS = 12 * 60 * 60;

function adminToken(): string | null {
  const token = process.env.ADMIN_TOKEN?.trim();
  return token && token.length >= 32 ? token : null;
}

export function adminEnabled(): boolean {
  return adminToken() !== null;
}

function sessionValue(token: string, issuedAt: number): string {
  const mac = createHmac("sha256", token).update(`ih-admin-session:${issuedAt}`).digest("base64url");
  return `${issuedAt}.${mac}`;
}

function equal(a: string, b: string): boolean {
  const ab = Buffer.from(a);
  const bb = Buffer.from(b);
  return ab.length === bb.length && timingSafeEqual(ab, bb);
}

/** Checks a submitted token and starts a session. Returns false when it is wrong. */
export async function startSession(submitted: string): Promise<boolean> {
  const token = adminToken();
  if (!token || !equal(submitted.trim(), token)) return false;
  const store = await cookies();
  store.set(COOKIE, sessionValue(token, Date.now()), {
    httpOnly: true,
    secure: process.env.NODE_ENV === "production",
    sameSite: "strict",
    path: "/admin",
    maxAge: MAX_AGE_SECONDS,
  });
  return true;
}

export async function endSession(): Promise<void> {
  (await cookies()).delete({ name: COOKIE, path: "/admin" });
}

/** Whether the request carries a valid, unexpired admin session. */
export const hasSession = cache(async (): Promise<boolean> => {
  const token = adminToken();
  const value = (await cookies()).get(COOKIE)?.value;
  if (!token || !value) return false;
  const issuedAt = Number(value.split(".", 1)[0]);
  if (!Number.isSafeInteger(issuedAt)) return false;
  if (Date.now() - issuedAt > MAX_AGE_SECONDS * 1000) return false;
  return equal(value, sessionValue(token, issuedAt));
});

/** Redirects to the login page unless the request has an admin session. */
export async function requireAdmin(): Promise<void> {
  if (!(await hasSession())) redirect("/admin/login");
}

/** API client that sends the admin token. Only call after `requireAdmin()`. */
export function adminApi() {
  const token = adminToken();
  if (!token) throw new Error("ADMIN_TOKEN is not configured");
  return createApiClient({
    baseUrl: process.env.API_URL ?? "http://127.0.0.1:4000",
    headers: { Authorization: `Bearer ${token}` },
  });
}
