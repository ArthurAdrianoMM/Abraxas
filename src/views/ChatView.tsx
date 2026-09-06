import { useEffect, useState } from "react";
import { useT } from "../lib/i18n";
import { Composer } from "../components/chat/Composer";
import { EmptyState } from "../components/chat/EmptyState";
import { OrdersDrawer } from "../components/chat/OrdersDrawer";
import { Thread } from "../components/chat/Thread";
import { useConversationsStore } from "../stores/conversations";
import { useGenerationStore } from "../stores/generation";
import { useModelStore } from "../stores/model";
import { useUiStore } from "../stores/ui";
import styles from "./ChatView.module.css";

/** Inline notice for the degraded chat (no model installed / load failed). */
function ModelNotice() {
  const t = useT().chat;
  const status = useModelStore((s) => s.status);
  const error = useModelStore((s) => s.error);
  const setView = useUiStore((s) => s.setView);

  if (status !== "none-installed" && status !== "error" && status !== "idle") return null;

  return (
    <div className={styles.modelNotice}>
      <span>
        {status === "error" ? t.voiceFailed(String(error)) : t.noVoice}
      </span>
      <button className={styles.modelNoticeLink} onClick={() => setView("models")}>
        {t.goToAtelier}
      </button>
    </div>
  );
}

function GenerationError() {
  const t = useT().chat;
  const error = useGenerationStore((s) => s.error);
  const dismiss = useGenerationStore((s) => s.dismissError);
  if (!error) return null;
  return (
    <div className={styles.genError}>
      <span>{t.generationFailed(error)}</span>
      <button className={styles.genErrorDismiss} onClick={dismiss} aria-label={t.dismiss}>
        ✕
      </button>
    </div>
  );
}

export function ChatView() {
  const tChat = useT().chat;
  const activeId = useConversationsStore((s) => s.activeId);
  const messages = useConversationsStore((s) => s.messages);
  const loadConversations = useConversationsStore((s) => s.load);
  const conversationsLoaded = useConversationsStore((s) => s.loaded);
  const conversations = useConversationsStore((s) => s.conversations);

  const status = useGenerationStore((s) => s.status);
  const send = useGenerationStore((s) => s.send);
  const stop = useGenerationStore((s) => s.stop);

  const modelStatus = useModelStore((s) => s.status);
  const initModel = useModelStore((s) => s.init);

  const ordersOpen = useUiStore((s) => s.ordersOpen);

  const [draft, setDraft] = useState("");

  useEffect(() => {
    if (!conversationsLoaded) void loadConversations();
    void initModel();
  }, [conversationsLoaded, loadConversations, initModel]);

  const generating = status !== "idle";
  const degraded =
    modelStatus === "none-installed" || modelStatus === "error" || modelStatus === "idle";
  const showEmpty = activeId === null && !generating;

  const handleSend = () => {
    const text = draft.trim();
    if (!text || generating || degraded) return;
    setDraft("");
    void send(text);
  };

  const activeConversation = conversations.find((c) => c.id === activeId) ?? null;

  return (
    <>
      {showEmpty ? (
        <EmptyState onSeed={(prompt) => setDraft(prompt)} />
      ) : (
        <Thread messages={messages} />
      )}

      <GenerationError />
      <ModelNotice />

      <Composer
        value={draft}
        placeholder={showEmpty ? tChat.placeholderFirst : tChat.placeholder}
        generating={generating}
        disabled={degraded}
        onChange={setDraft}
        onSend={handleSend}
        onStop={() => void stop()}
      />

      {ordersOpen && activeConversation && <OrdersDrawer conversation={activeConversation} />}
    </>
  );
}
