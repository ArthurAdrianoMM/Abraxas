import type { CommandError } from "./bindings";

type CommandResult<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: CommandError };

/** Unwraps a tauri-specta typed result, throwing the CommandError on failure. */
export async function unwrap<T>(result: Promise<CommandResult<T>>): Promise<T> {
  const r = await result;
  if (r.status === "error") {
    throw r.error;
  }
  return r.data;
}

export function describeError(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) {
    return String((e as { message: unknown }).message);
  }
  return String(e);
}
