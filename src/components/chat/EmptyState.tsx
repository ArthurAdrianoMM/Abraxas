import { useT } from "../../lib/i18n";
import styles from "./EmptyState.module.css";

/** The "new conversation" invocation — a still page before the first word. */
export function EmptyState({ onSeed }: { onSeed: (prompt: string) => void }) {
  const t = useT().chat.empty;
  return (
    <section className={`thread ${styles.empty}`}>
      <div className={styles.invocation}>
        <div className={styles.glyphWrap} aria-hidden="true">
          <svg width="48" height="48" viewBox="0 0 32 32" fill="none">
            <circle cx="16" cy="16" r="13" stroke="#b89968" strokeWidth="0.7" fill="none" />
            <circle cx="16" cy="16" r="9.5" stroke="#7d2233" strokeWidth="0.55" fill="none" />
            <line x1="16" y1="1.5" x2="16" y2="30.5" stroke="#b89968" strokeWidth="0.7" />
            <line x1="11" y1="16" x2="21" y2="16" stroke="#7d2233" strokeWidth="0.55" />
            <circle cx="16" cy="11" r="1.2" fill="#b89968" />
          </svg>
        </div>

        <span className={styles.kicker}>{t.kicker}</span>

        <h1 className={styles.title}>
          <span>{t.titleLead}</span>
          <span className={styles.quiet}>{t.titleQuiet}</span>
        </h1>

        <span className={styles.rule} aria-hidden="true"></span>

        <p className={styles.gloss}>{t.gloss}</p>

        <div className={styles.seeds}>
          {t.seeds.map((seed) => (
            <button key={seed.roman} className={styles.seed} onClick={() => onSeed(seed.prompt)}>
              <span className={styles.roman}>{seed.roman}</span>
              <span className={styles.text}>
                <b>{seed.kind}</b>
                {seed.label}
              </span>
            </button>
          ))}
        </div>

        <span className={styles.belowHint}>
          <kbd>↵</kbd> {t.hintSend} · <kbd>shift</kbd>+<kbd>↵</kbd> {t.hintNewline}
        </span>
      </div>
    </section>
  );
}
