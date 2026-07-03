import { create } from "zustand";
import {
  commands,
  type ChatRole,
  type Conversation,
  type ConversationGenerationParams,
  type StoredMessage,
} from "../lib/tauri/bindings";
import { unwrap } from "../lib/tauri/result";

/** Longest title derived from the first message, per the design mock. */
const TITLE_MAX = 44;

export function titleFromFirstMessage(text: string): string {
  const line = text.trim().replace(/\s+/g, " ");
  return line.length > TITLE_MAX ? `${line.slice(0, TITLE_MAX)}…` : line;
}

interface ConversationsState {
  conversations: Conversation[];
  /** null = the "new conversation" empty state. */
  activeId: string | null;
  /** Messages of the active conversation, oldest first. */
  messages: StoredMessage[];
  loaded: boolean;

  load: () => Promise<void>;
  select: (id: string) => Promise<void>;
  startNew: () => void;
  /** Creates a conversation titled after the first message and makes it active. */
  createForFirstMessage: (text: string, modelId: string | null) => Promise<Conversation>;
  remove: (id: string) => Promise<void>;
  appendMessage: (conversationId: string, role: ChatRole, content: string) => Promise<void>;
  updateParams: (id: string, params: ConversationGenerationParams) => Promise<void>;
  /** Re-sorts the sidebar after updated_at changes without a full refetch. */
  touch: (id: string) => void;
}

export const useConversationsStore = create<ConversationsState>((set, get) => ({
  conversations: [],
  activeId: null,
  messages: [],
  loaded: false,

  load: async () => {
    const conversations = await unwrap(commands.listConversations());
    set({ conversations, loaded: true });
  },

  select: async (id) => {
    set({ activeId: id, messages: [] });
    const messages = await unwrap(commands.listMessages(id));
    // Guard against a rapid re-selection while the fetch was in flight.
    if (get().activeId === id) set({ messages });
  },

  startNew: () => set({ activeId: null, messages: [] }),

  createForFirstMessage: async (text, modelId) => {
    const conversation = await unwrap(
      commands.createConversation(titleFromFirstMessage(text), modelId),
    );
    set((s) => ({
      conversations: [conversation, ...s.conversations],
      activeId: conversation.id,
      messages: [],
    }));
    return conversation;
  },

  remove: async (id) => {
    await unwrap(commands.deleteConversation(id));
    set((s) => ({
      conversations: s.conversations.filter((c) => c.id !== id),
      ...(s.activeId === id ? { activeId: null, messages: [] } : {}),
    }));
  },

  appendMessage: async (conversationId, role, content) => {
    const stored = await unwrap(commands.appendMessage(conversationId, role, content));
    set((s) =>
      s.activeId === conversationId ? { messages: [...s.messages, stored] } : {},
    );
    get().touch(conversationId);
  },

  updateParams: async (id, params) => {
    const updated = await unwrap(commands.updateConversationGenerationParams(id, params));
    set((s) => ({
      conversations: s.conversations.map((c) => (c.id === id ? updated : c)),
    }));
  },

  touch: (id) => {
    const now = new Date().toISOString();
    set((s) => {
      const target = s.conversations.find((c) => c.id === id);
      if (!target) return {};
      const rest = s.conversations.filter((c) => c.id !== id);
      return { conversations: [{ ...target, updated_at: now }, ...rest] };
    });
  },
}));
