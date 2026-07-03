import { create } from "zustand";
import { commands, events, type ClassifiedModel } from "../lib/tauri/bindings";
import { describeError, unwrap } from "../lib/tauri/result";
import { useModelStore } from "./model";

/** One download at a time is the backend contract, so the store models a
 *  single ephemeral download session. Nothing here is persisted — resume
 *  works because the backend keeps the `.part` file on disk. */
export type DownloadPhase =
  | "confirm" // target chosen, waiting for the user to begin
  | "starting" // start_model_download accepted, no bytes yet
  | "downloading"
  | "verifying" // SHA256 pass after the last byte
  | "paused" // cancelled on the backend; `.part` retained, resumable
  | "completed"
  | "failed";

interface DownloadSession {
  modelId: string;
  /** Catalog snapshot taken when the flow was entered, so the download
   *  screen renders without depending on catalog store state. */
  entry: ClassifiedModel;
  phase: DownloadPhase;
  downloadedBytes: number;
  totalBytes: number;
  hashedBytes: number;
  /** Rolling throughput estimate (bytes/s); null until measurable. */
  speedBps: number | null;
  /** Byte position a resumed session picked up from, for the recovery note. */
  resumedFrom: number | null;
  errorKind: string | null;
  errorMessage: string | null;
}

interface DownloadsState {
  session: DownloadSession | null;

  /** Enter the confirm step for a catalog entry. */
  begin: (entry: ClassifiedModel) => void;
  /** Kick off (or resume) the backend download for the current session. */
  start: () => Promise<void>;
  /** Cancel on the backend; the `.part` stays for a later resume. */
  pause: () => Promise<void>;
  /** Drop the session (leaves any `.part` on disk for a future resume). */
  reset: () => void;
}

// Throughput sampling for the live speed metric — module-level because it is
// bookkeeping for the listener, not renderable state.
let lastSample: { t: number; bytes: number } | null = null;

/** Returns the new estimate, `null` for "not measurable yet", or `undefined`
 *  for "sampled too recently — keep the previous reading". */
function sampleSpeed(bytes: number): number | null | undefined {
  const now = performance.now();
  if (!lastSample) {
    lastSample = { t: now, bytes };
    return null;
  }
  const dt = (now - lastSample.t) / 1000;
  if (dt < 0.5) return undefined;
  const inst = (bytes - lastSample.bytes) / dt;
  lastSample = { t: now, bytes };
  const prev = useDownloadsStore.getState().session?.speedBps ?? null;
  // EMA so the readout doesn't jitter.
  return prev == null ? inst : prev * 0.7 + inst * 0.3;
}

function patchSession(patch: Partial<DownloadSession>) {
  useDownloadsStore.setState((prev) =>
    prev.session ? { session: { ...prev.session, ...patch } } : prev,
  );
}

let listening = false;
function ensureListener() {
  if (listening) return;
  listening = true;
  void events.downloadEvent.listen(({ payload }) => {
    const session = useDownloadsStore.getState().session;
    if (!session || payload.model_id !== session.modelId) return;
    switch (payload.type) {
      case "started":
        patchSession({ totalBytes: payload.total_bytes });
        break;
      case "progress": {
        const speed = sampleSpeed(payload.downloaded_bytes);
        patchSession({
          phase: "downloading",
          downloadedBytes: payload.downloaded_bytes,
          totalBytes: payload.total_bytes,
          ...(speed !== undefined ? { speedBps: speed } : {}),
          // First progress of a resumed session lands well past zero.
          ...(session.phase === "starting" && payload.downloaded_bytes > 0 && session.resumedFrom == null
            ? { resumedFrom: payload.downloaded_bytes }
            : {}),
        });
        break;
      }
      case "verifying":
        patchSession({
          phase: "verifying",
          hashedBytes: payload.hashed_bytes,
          totalBytes: payload.total_bytes,
          speedBps: null,
        });
        break;
      case "completed":
        patchSession({ phase: "completed", downloadedBytes: session.totalBytes });
        void useModelStore.getState().refreshInstalled();
        break;
      case "cancelled":
        patchSession({ phase: "paused", speedBps: null });
        break;
      case "failed":
        patchSession({
          phase: "failed",
          speedBps: null,
          errorKind: payload.kind,
          errorMessage: payload.message,
        });
        break;
    }
  });
}

export const useDownloadsStore = create<DownloadsState>((set, get) => ({
  session: null,

  begin: (entry) => {
    const current = get().session;
    // Re-entering the flow for the model already in progress just refocuses it.
    if (current && current.modelId === entry.model.id && current.phase !== "completed") {
      return;
    }
    lastSample = null;
    set({
      session: {
        modelId: entry.model.id,
        entry,
        phase: "confirm",
        downloadedBytes: 0,
        totalBytes: entry.model.size_bytes,
        hashedBytes: 0,
        speedBps: null,
        resumedFrom: null,
        errorKind: null,
        errorMessage: null,
      },
    });
  },

  start: async () => {
    const session = get().session;
    if (!session) return;
    ensureListener();
    lastSample = null;
    patchSession({
      phase: "starting",
      errorKind: null,
      errorMessage: null,
      speedBps: null,
      resumedFrom: null,
    });
    try {
      await unwrap(commands.startModelDownload(session.modelId));
    } catch (e) {
      patchSession({
        phase: "failed",
        errorKind: (e as { kind?: string })?.kind ?? "Download",
        errorMessage: describeError(e),
      });
    }
  },

  pause: async () => {
    const session = get().session;
    if (!session) return;
    try {
      await unwrap(commands.cancelModelDownload(session.modelId));
      // The backend emits Cancelled once the task observes the flag; the
      // listener flips the phase then. Nothing to do here.
    } catch (e) {
      patchSession({ errorKind: "Download", errorMessage: describeError(e) });
    }
  },

  reset: () => {
    lastSample = null;
    set({ session: null });
  },
}));
