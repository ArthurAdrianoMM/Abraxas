import { useUiStore } from "../../stores/ui";
import { AbraxasGlyph } from "./AbraxasGlyph";
import styles from "./Sidebar.module.css";

// Placeholder conversation groups, mirroring the design mock. Fase 5.5 UI
// wiring replaces these with the persisted conversation list.
const PLACEHOLDER_GROUPS: { label: string; items: string[] }[] = [
  {
    label: "— hoje",
    items: ["o pássaro e o ovo", "privacidade em modelos locais", "gnose e linguagem"],
  },
  {
    label: "— esta semana",
    items: [
      "notas sobre Hesse",
      "como funciona quantização",
      "alquimia interior, leituras",
      "o silêncio dos pitagóricos",
    ],
  },
  {
    label: "— mais antigas",
    items: [
      "cabala e combinatória",
      "rascunho de ensaio: o duplo",
      "Jung, Eranos, sincronicidade",
    ],
  },
];

export function Sidebar() {
  const view = useUiStore((s) => s.view);
  const setView = useUiStore((s) => s.setView);

  return (
    <aside className="side">
      <div className="brand">
        <AbraxasGlyph />
        <div className={styles.brandText}>
          <span className="name">ABRAXAS</span>
          <span className="tag">v.0 · local</span>
        </div>
      </div>

      <button className="new-btn" onClick={() => setView("chat")}>
        + nova conversa
      </button>

      <nav className="conv-scroll" aria-label="Conversas">
        {PLACEHOLDER_GROUPS.map((group) => (
          <div key={group.label}>
            <div className="group-label">{group.label}</div>
            {group.items.map((title, i) => (
              <div
                key={title}
                className={
                  view === "chat" && group.label === "— hoje" && i === 0
                    ? "conv active"
                    : "conv"
                }
                onClick={() => setView("chat")}
              >
                {title}
              </div>
            ))}
          </div>
        ))}
      </nav>

      <div className="footer">
        <div
          className={`model-row ${styles.modelRow}`}
          title="ateliê dos modelos"
          onClick={() => setView("models")}
        >
          <span className="pulse"></span>llama-3.1-8b
        </div>
        <div className="model-meta">7.3 GB · ram 12/16</div>

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
