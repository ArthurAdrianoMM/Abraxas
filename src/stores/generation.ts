import { create } from "zustand";
import { commands, events, type ChatMessage } from "../lib/tauri/bindings";
import { describeError, unwrap } from "../lib/tauri/result";
import { useConversationsStore } from "./conversations";
import { useModelStore } from "./model";

export type GenerationStatus = "idle" | "starting" | "streaming";

interface GenerationState {
  status: GenerationStatus;
  /** id returned by start_generation; null until the backend confirms. */
  generationId: string | null;
  /** Conversation the in-flight generation belongs to. */
  conversationId: string | null;
  /** Tokens accumulated so far for the live turn. */
  streamText: string;
  error: string | null;

  send: (text: string) => Promise<void>;
  stop: () => Promise<void>;
  dismissError: () => void;
}

/** Persist whatever streamed so far as the assistant turn, then reset. */
async function finalize(persist: boolean) {
  const { streamText, conversationId } = useGenerationStore.getState();
  if (persist && streamText.trim() && conversationId) {
    try {
      await useConversationsStore
        .getState()
        .appendMessage(conversationId, "assistant", streamText);
    } catch (e) {
      useGenerationStore.setState({ error: describeError(e) });
    }
  }
  useGenerationStore.setState({
    status: "idle",
    generationId: null,
    conversationId: null,
    streamText: "",
  });
}

let listening = false;
function ensureListener() {
  if (listening) return;
  listening = true;
  // Single-flight on the backend, so every generation-event belongs to the
  // one in-flight run — no per-id demux needed beyond a sanity check.
  void events.generationEvent.listen(({ payload }) => {
    const s = useGenerationStore.getState();
    if (s.status === "idle") return;
    switch (payload.type) {
      case "started":
        useGenerationStore.setState({
          status: "streaming",
          generationId: payload.generation_id,
        });
        break;
      case "token":
        useGenerationStore.setState((prev) => ({
          streamText: prev.streamText + payload.text,
        }));
        break;
      case "end":
        void finalize(true);
        break;
      case "cancelled":
        // Keep whatever streamed before the stop — matches "stop" semantics
        // where the partial reply stays in the conversation.
        void finalize(true);
        break;
      case "failed":
        useGenerationStore.setState({ error: payload.message });
        void finalize(true);
        break;
    }
  });
}

export const useGenerationStore = create<GenerationState>((set, get) => ({
  status: "idle",
  generationId: null,
  conversationId: null,
  streamText: "",
  error: null,

  send: async (text) => {
    const content = text.trim();
    if (!content || get().status !== "idle") return;
    ensureListener();
    set({ error: null });

    const conversations = useConversationsStore.getState();
    const loadedModelId = useModelStore.getState().loadedId;
    try {
      const conversationId =
        conversations.activeId ??
        (await conversations.createForFirstMessage(content, loadedModelId)).id;

      await conversations.appendMessage(conversationId, "user", content);

      const history: ChatMessage[] = useConversationsStore
        .getState()
        .messages.map((m) => ({ role: m.role, content: m.content }));

      set({ status: "starting", conversationId, streamText: "" });
      const generationId = await unwrap(
        commands.startGeneration(history, null, conversationId),
      );
      // The "started" event usually lands first; this is the fallback.
      if (get().status === "starting") {
        set({ status: "streaming", generationId });
      }
    } catch (e) {
      set({
        status: "idle",
        generationId: null,
        conversationId: null,
        streamText: "",
        error: describeError(e),
      });
    }
  },

  stop: async () => {
    const { generationId } = get();
    if (!generationId) return;
    try {
      await unwrap(commands.cancelGeneration(generationId));
    } catch (e) {
      set({ error: describeError(e) });
    }
  },

  dismissError: () => set({ error: null }),
}));
