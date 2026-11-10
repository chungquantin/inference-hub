"use client";

import { useActionState } from "react";
import { buttonClass } from "@/components/ui/primitives";
import { login } from "../actions";

export function LoginForm() {
  const [state, action, pending] = useActionState(login, undefined);
  return (
    <form action={action} className="flex max-w-sm flex-col gap-3">
      <label htmlFor="token" className="text-sm font-medium">
        Admin token
      </label>
      <input
        id="token"
        name="token"
        type="password"
        autoComplete="current-password"
        required
        className="h-10 rounded-[6px] bg-surface px-3 text-sm text-ink shadow-elev-1"
      />
      {state?.error && (
        <p role="alert" className="text-sm text-red">
          {state.error}
        </p>
      )}
      <button
        type="submit"
        disabled={pending}
        className={buttonClass("primary")}
      >
        {pending ? "Checking…" : "Sign in"}
      </button>
    </form>
  );
}
