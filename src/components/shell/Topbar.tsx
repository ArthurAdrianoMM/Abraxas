import { useConversationsStore } from "../../stores/conversations";
import { useModelStore } from "../../stores/model";
import { useUiStore, type View } from "../../stores/ui";
import styles from "./Topbar.module.css";

function BackLink({ onClick }: { onClick: () => void }) {
  return (
    <button className={styles.backLink} onClick={onClick}>
      <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <path
          d="M13 8H3M7 4L3 8l4 4"
          stroke="currentColor"
          strokeWidth="1.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
      voltar ao estúdio
    </button>
  );
}

function Meta({ children }: { children: React.ReactNode }) {
  return <span className="meta">{children}</span>;
}

function ChatTopbar() {
  const setView = useUiStore((s) => s.setView);
  const setOrdersOpen = useUiStore((s) => s.setOrdersOpen);
  const activeId = useConversationsStore((s) => s.activeId);
  const conversations = useConversationsStore((s) => s.conversations);
  const modelStatus = useModelStore((s) => s.status);
  const loadedId = useModelStore((s) => s.loadedId);

  const active = conversations.find((c) => c.id === activeId) ?? null;

  return (
    <header className={`topbar ${styles.chatTopbar}`}>
      {active ? (
        <span className={`title ${styles.title}`}>{active.title}</span>
      ) : (
        <span className={`title ${styles.title}`}>
          <em style={{ opacity: 0.6 }}>nova conversa</em>
        </span>
      )}

      {/* model pill — the switcher popover lands with the Models phase; opens the atelier */}
      <button
        className={styles.modelPill}
        title="trocar a voz"
        onClick={() => setView("models")}
      >
        <span className="pulse"></span>
        <span className={styles.modelPillName}>
          {modelStatus === "loaded" && loadedId ? loadedId : "sem voz"}
        </span>
        <svg className={styles.modelPillChev} width="11" height="11" viewBox="0 0 16 16" fill="none">
          <path
            d="M4 6l4 4 4-4"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </button>

      <button
        className={styles.toolBtn}
        title="ordens desta conversa"
        aria-haspopup="dialog"
        disabled={!active}
        style={!active ? { opacity: 0.35, cursor: "default" } : undefined}
        onClick={() => active && setOrdersOpen(true)}
      >
        <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <line x1="2" y1="4.5" x2="14" y2="4.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
          <line x1="2" y1="11.5" x2="14" y2="11.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
          <circle cx="10.5" cy="4.5" r="1.8" fill="var(--bg-2)" stroke="currentColor" strokeWidth="1.2" />
          <circle cx="5.5" cy="11.5" r="1.8" fill="var(--bg-2)" stroke="currentColor" strokeWidth="1.2" />
        </svg>
      </button>

      <button className={styles.toolBtn} title="preferências" onClick={() => setView("settings")}>
        <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <circle cx="8" cy="8" r="2.6" stroke="currentColor" strokeWidth="1.2" />
          <line x1="8" y1="1.5" x2="8" y2="3.6" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
          <line x1="8" y1="12.4" x2="8" y2="14.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
          <line x1="1.5" y1="8" x2="3.6" y2="8" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
          <line x1="12.4" y1="8" x2="14.5" y2="8" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
        </svg>
      </button>

      <Meta>
        <span>
          <span className="dot"></span>offline
        </span>
      </Meta>
    </header>
  );
}

function ModelsTopbar() {
  const setView = useUiStore((s) => s.setView);
  return (
    <header className="topbar">
      <BackLink onClick={() => setView("chat")} />
      <Meta>
        <span>
          <span className="dot"></span>offline
        </span>
        <span>modelos</span>
      </Meta>
    </header>
  );
}

function SettingsTopbar() {
  const setView = useUiStore((s) => s.setView);
  return (
    <header className="topbar">
      <BackLink onClick={() => setView("chat")} />
      <Meta>
        <span>
          <span className="dot"></span>preferências
        </span>
      </Meta>
    </header>
  );
}

const TOPBARS: Record<View, () => React.ReactNode> = {
  chat: ChatTopbar,
  models: ModelsTopbar,
  settings: SettingsTopbar,
};

export function Topbar() {
  const view = useUiStore((s) => s.view);
  const Bar = TOPBARS[view];
  return <Bar />;
}
