import { create } from "zustand";
import {
  commands,
  events,
  type ClassifiedModel,
  type RemoteGguf,
  type ResolvedSource,
} from "../lib/tauri/bindings";
import { describeError, unwrap } from "../lib/tauri/result";
import { useModelStore } from "./model";

/** One download at a time is the backend contract, so the store models a
 *  single ephemeral download session. Nothing here is persisted — resume
 *  works because the backend keeps the `.part` file on disk. */
export type DownloadPhase =
  | "confirm" // target chosen, waiting for the user to begin
  | "starting" // start command accepted, no bytes yet
  | "downloading"
  | "verifying" // SHA256 pass after the last byte (catalog models only)
  | "paused" // cancelled on the backend; `.part` retained, resumable
  | "completed"
  | "failed";

/** What is being downloaded, flattened for display. A catalog entry brings
 *  its metadata along; a user link only has what the listing told us. */
export interface DownloadTarget {
  kind: "catalog" | "url";
  name: string;
  /** Mono secondary line: "publisher · 7b · q4_k_m", or the file name. */
  subtitle: string;
  /** 0 = unknown until the server answers. */
  sizeBytes: number;
  url: string;
  /** Published checksum; null for user links (never verified). */
  sha256: string | null;
  /** The pane the user came from — where "back" and "cancel" return to. */
  origin: "catalog" | "import";
  /** Full catalog entry when `kind === "catalog"`. */
  entry: ClassifiedModel | null;
  /** `start_url_download` payload when `kind === "url"`. */
  request: { url: string; filename: string; source_url: string; expected_size: number | null } | null;
}

interface DownloadSession {
  /** Backend id the `DownloadEvent`s are keyed by. For a user link it is
   *  assigned by the backend and adopted from the first event / the command
   *  result, so it is `null` while the request is in flight. */
  modelId: string | null;
  target: DownloadTarget;
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
  /** Enter the confirm step for a file from a pasted link. */
  beginUrl: (file: RemoteGguf, source: ResolvedSource) => void;
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

/** Does `modelId` belong to the current session? A url session whose id is
 *  still pending adopts the first `url:` event it sees — the backend emits
 *  `Started` before the start command has even returned. */
function claim(session: DownloadSession, modelId: string): boolean {
  if (session.modelId === modelId) return true;
  if (session.modelId === null && session.target.kind === "url" && modelId.startsWith("url:")) {
    patchSession({ modelId });
    return true;
  }
  return false;
}

let listening = false;
function ensureListener() {
  if (listening) return;
  listening = true;
  void events.downloadEvent.listen(({ payload }) => {
    const session = useDownloadsStore.getState().session;
    if (!session || !claim(session, payload.model_id)) return;
    switch (payload.type) {
      case "started":
        if (payload.total_bytes > 0) patchSession({ totalBytes: payload.total_bytes });
        break;
      case "progress": {
        const speed = sampleSpeed(payload.downloaded_bytes);
        patchSession({
          phase: "downloading",
          downloadedBytes: payload.downloaded_bytes,
          // 0 means the server hasn't said yet — keep whatever we knew.
          ...(payload.total_bytes > 0 ? { totalBytes: payload.total_bytes } : {}),
          ...(speed !== undefined ? { speedBps: speed } : {}),
          // A resumed session's first progress lands at the `.part` offset —
          // well past the first chunk of a fresh download.
          ...(session.phase === "starting" &&
          session.resumedFrom == null &&
          payload.downloaded_bytes > Math.max(64e6, payload.total_bytes * 0.01)
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
      case "completed": {
        // An unknown-size download only learns its final size here.
        const total = session.totalBytes > 0 ? session.totalBytes : session.downloadedBytes;
        patchSession({ phase: "completed", downloadedBytes: total, totalBytes: total });
        void useModelStore.getState().refreshInstalled();
        break;
      }
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

function freshSession(modelId: string | null, target: DownloadTarget): DownloadSession {
  return {
    modelId,
    target,
    phase: "confirm",
    downloadedBytes: 0,
    totalBytes: target.sizeBytes,
    hashedBytes: 0,
    speedBps: null,
    resumedFrom: null,
    errorKind: null,
    errorMessage: null,
  };
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
    const m = entry.model;
    set({
      session: freshSession(m.id, {
        kind: "catalog",
        name: m.name,
        subtitle: `${m.publisher} · ${m.params_b}b · ${m.quantization.toLowerCase()}`,
        sizeBytes: m.size_bytes,
        url: m.url,
        sha256: m.sha256,
        origin: "catalog",
        entry,
        request: null,
      }),
    });
  },

  beginUrl: (file, source) => {
    const current = get().session;
    if (current && current.target.url === file.url && current.phase !== "completed") {
      return;
    }
    lastSample = null;
    set({
      session: freshSession(null, {
        kind: "url",
        name: file.filename.replace(/\.gguf$/i, ""),
        subtitle: source.repo ?? source.source_url.replace(/^https?:\/\//, ""),
        sizeBytes: file.size_bytes ?? 0,
        url: file.url,
        sha256: null,
        origin: "import",
        entry: null,
        request: {
          url: file.url,
          filename: file.filename,
          source_url: source.source_url,
          expected_size: file.size_bytes,
        },
      }),
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
      if (session.target.kind === "url" && session.target.request) {
        const modelId = await unwrap(commands.startUrlDownload(session.target.request));
        patchSession({ modelId });
      } else if (session.modelId) {
        await unwrap(commands.startModelDownload(session.modelId));
      }
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
    if (!session?.modelId) return;
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
