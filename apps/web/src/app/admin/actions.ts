"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { adminApi, endSession, requireAdmin, startSession } from "@/lib/admin";

export type LoginState = { error?: string } | undefined;

export async function login(_state: LoginState, formData: FormData): Promise<LoginState> {
  const token = String(formData.get("token") ?? "");
  if (!(await startSession(token))) {
    return { error: "That token is not valid." };
  }
  redirect("/admin");
}

export async function logout(): Promise<void> {
  await endSession();
  redirect("/admin/login");
}

const STATUSES = ["approved", "rejected", "pending"] as const;
type Status = (typeof STATUSES)[number];
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** Sets the review status of the checked items. */
export async function review(formData: FormData): Promise<void> {
  await requireAdmin();
  const status = String(formData.get("status")) as Status;
  if (!STATUSES.includes(status)) throw new Error("Invalid status");
  const ids = formData
    .getAll("id")
    .map(String)
    .filter((id) => UUID.test(id))
    .slice(0, 200);
  if (ids.length === 0) return;

  const { error } = await adminApi().POST("/v1/admin/news/review", { body: { ids, status } });
  if (error) throw new Error(`Review failed: ${error.message}`);

  revalidatePath("/admin");
  revalidatePath("/news");
  revalidatePath("/");
}

/** Replaces the organizations of one item from a comma-separated list of slugs. */
export async function setOrganizations(formData: FormData): Promise<void> {
  await requireAdmin();
  const id = String(formData.get("id"));
  if (!UUID.test(id)) throw new Error("Invalid id");
  const organizations = String(formData.get("organizations") ?? "")
    .split(",")
    .map((s) => s.trim().toLowerCase())
    .filter(Boolean);

  const { error } = await adminApi().PUT("/v1/admin/news/{id}/organizations", {
    params: { path: { id } },
    body: { organizations },
  });
  if (error) throw new Error(error.message);
  revalidatePath("/admin");
}
