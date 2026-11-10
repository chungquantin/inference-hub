import type { Metadata } from "next";
import { notFound, redirect } from "next/navigation";
import { adminEnabled, hasSession } from "@/lib/admin";
import { LoginForm } from "./login-form";

export const metadata: Metadata = { title: "Admin sign in", robots: { index: false } };

export default async function LoginPage() {
  if (!adminEnabled()) notFound();
  if (await hasSession()) redirect("/admin");
  return (
    <div className="flex flex-col gap-6">
      <h1 className="text-2xl font-semibold">Admin</h1>
      <LoginForm />
    </div>
  );
}
