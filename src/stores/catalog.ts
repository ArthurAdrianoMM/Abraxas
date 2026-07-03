import { create } from "zustand";
import {
  commands,
  type CatalogSource,
  type ClassifiedModel,
} from "../lib/tauri/bindings";
import { describeError, unwrap } from "../lib/tauri/result";

export type CatalogStatus = "idle" | "loading" | "ready" | "error";

interface CatalogState {
  status: CatalogStatus;
  models: ClassifiedModel[];
  /** "network" = fresh fetch; "cache" = offline fallback copy. */
  source: CatalogSource | null;
  fetchedAt: string | null;
  error: string | null;

  /** Fetch (or re-fetch) the classified catalog. Safe to call repeatedly;
   *  concurrent calls coalesce into the in-flight request. */
  refresh: () => Promise<void>;
}

let inFlight: Promise<void> | null = null;

export const useCatalogStore = create<CatalogState>((set) => ({
  status: "idle",
  models: [],
  source: null,
  fetchedAt: null,
  error: null,

  refresh: async () => {
    if (inFlight) return inFlight;
    inFlight = (async () => {
      set((prev) => ({ status: prev.models.length > 0 ? prev.status : "loading", error: null }));
      try {
        const resp = await unwrap(commands.fetchClassifiedCatalog());
        set({
          status: "ready",
          models: resp.models,
          source: resp.source,
          fetchedAt: resp.fetched_at,
          error: null,
        });
      } catch (e) {
        // Keep any previously-fetched models visible; the view decides
        // between "offline, no cache" and "showing stale copy".
        set({ status: "error", error: describeError(e) });
      } finally {
        inFlight = null;
      }
    })();
    return inFlight;
  },
}));
