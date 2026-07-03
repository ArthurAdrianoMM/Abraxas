import { useEffect, useState } from "react";
import type { Conversation } from "../../lib/tauri/bindings";
import { useConversationsStore } from "../../stores/conversations";
import { useModelStore } from "../../stores/model";
import { useUiStore } from "../../stores/ui";
import { AbraxasGlyph } from "./AbraxasGlyph";
import styles from "./Sidebar.module.css";

type Group = { label: string; items: Conversation[] };

/** Buckets by updated_at into the design's três groups. */
function groupConversations(conversations: Conversation[]): Group[] {
  const now = new Date();
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const startOfWeek = startOfToday - 6 * 24 * 60 * 60 * 1000;

  const groups: Group[] = [
    { label: "— hoje", items: [] },
    { label: "— esta semana", items: [] },
    { label: "— mais antigas", items: [] },
  ];
  for (const c of conversations) {
    const t = Date.parse(c.updated_at);
    if (t >= startOfToday) groups[0].items.push(c);
    else if (t >= startOfWeek) groups[1].items.push(c);
    else groups[2].items.push(c);
  }
  return groups.filter((g) => g.items.length > 0);
}

function ConversationRow({ conversation }: { conversation: Conversation }) {
  const activeId = useConversationsStore((s) => s.activeId);
  const select = useConversationsStore((s) => s.select);
  const remove = useConversationsStore((s) => s.remove);
  const setView = useUiStore((s) => s.setView);
  const [confirming, setConfirming] = useState(false);

  if (confirming) {
    return (
      <div className={`conv ${styles.convConfirm}`}>
        <span className={styles.confirmLabel}>apagar esta conversa?</span>
        <span className={styles.confirmActions}>
          <button
            className={styles.confirmYes}
            onClick={(e) => {
              e.stopPropagation();
              void remove(conversation.id);
            }}
          >
            apagar
          </button>
          <button
            className={styles.confirmNo}
            onClick={(e) => {
              e.stopPropagation();
              setConfirming(false);
            }}
          >
            manter
          </button>
        </span>
      </div>
    );
  }

  return (
    <div
      className={`conv ${styles.convRow} ${conversation.id === activeId ? "active" : ""}`}
      onClick={() => {
        setView("chat");
        void select(conversation.id);
      }}
    >
      <span className={styles.convTitle}>{conversation.title}</span>
      <button
        className={styles.convDelete}
        title="apagar conversa"
        aria-label="apagar conversa"
        onClick={(e) => {
          e.stopPropagation();
          setConfirming(true);
        }}
      >
        <svg width="11" height="11" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <path
            d="M4 4l8 8M12 4l-8 8"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinecap="round"
          />
        </svg>
      </button>
    </div>
  );
}

function formatGb(bytes: number): string {
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

function ModelFooter() {
  const setView = useUiStore((s) => s.setView);
  const status = useModelStore((s) => s.status);
  const loadedId = useModelStore((s) => s.loadedId);
  const installed = useModelStore((s) => s.installed);

  const loaded = installed.find((m) => m.id === loadedId) ?? null;

  return (
    <>
      <div
        className={`model-row ${styles.modelRow}`}
        title="ateliê dos modelos"
        onClick={() => setView("models")}
      >
        {status === "loaded" && loaded ? (
          <>
            <span className="pulse"></span>
            {loaded.id}
          </>
        ) : (
          <span className={styles.modelIdle}>
            {status === "loading" ? "despertando…" : "nenhuma voz desperta"}
          </span>
        )}
      </div>
      <div className="model-meta">
        {status === "loaded" && loaded ? `${formatGb(loaded.size_bytes)} · local` : "— · —"}
      </div>
    </>
  );
}

export function Sidebar() {
  const view = useUiStore((s) => s.view);
  const setView = useUiStore((s) => s.setView);
  const conversations = useConversationsStore((s) => s.conversations);
  const conversationsLoaded = useConversationsStore((s) => s.loaded);
  const load = useConversationsStore((s) => s.load);
  const startNew = useConversationsStore((s) => s.startNew);

  useEffect(() => {
    if (!conversationsLoaded) void load();
  }, [conversationsLoaded, load]);

  const groups = groupConversations(conversations);

  return (
    <aside className="side">
      <div className="brand">
        <AbraxasGlyph />
        <div className={styles.brandText}>
          <span className="name">ABRAXAS</span>
          <span className="tag">v.0 · local</span>
        </div>
      </div>

      <button
        className="new-btn"
        onClick={() => {
          startNew();
          setView("chat");
        }}
      >
        + nova conversa
      </button>

      <nav className="conv-scroll" aria-label="Conversas">
        {groups.map((group) => (
          <div key={group.label}>
            <div className="group-label">{group.label}</div>
            {group.items.map((c) => (
              <ConversationRow key={c.id} conversation={c} />
            ))}
          </div>
        ))}
      </nav>

      <div className="footer">
        <ModelFooter />

        <nav className={styles.footerNav} aria-label="Atalhos">
          <button
            className={`${styles.footerLink} ${view === "models" ? styles.footerLinkActive : ""}`}
            onClick={() => setView("models")}
          >
            modelos
          </button>
          <button
            className={`${styles.footerLink} ${view === "settings" ? styles.footerLinkActive : ""}`}
            onClick={() => setView("settings")}
          >
            preferências
          </button>
        </nav>
      </div>
    </aside>
  );
}
