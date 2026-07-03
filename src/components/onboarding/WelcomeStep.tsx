import { useEffect } from "react";
import fibonacci from "../../assets/fibonacci.gif";
import styles from "./WelcomeStep.module.css";

/** First-open invocation per "Abraxas Welcome.html": spiral figure, mark +
 *  wordmark, one CTA into the exame, and the quiet skip underneath. */
export function WelcomeStep({ onBegin, onSkip }: { onBegin: () => void; onSkip: () => void }) {
  // Keyboard-first, like the chat: Enter/Space proceed.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // A focused button keeps its native activation (e.g. the skip link).
      if (e.target instanceof HTMLElement && e.target.closest("button")) return;
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        onBegin();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onBegin]);

  return (
    <div className={styles.welcome}>
      <div className={styles.figure} aria-hidden="true">
        <img src={fibonacci} alt="" />
      </div>

      <div className={styles.stage}>
        <div className={styles.mark}>
          <svg className={styles.glyph} viewBox="0 0 32 32" fill="none" aria-hidden="true">
            <circle cx="16" cy="16" r="13" stroke="#b89968" strokeWidth="0.7" fill="none" />
            <circle cx="16" cy="16" r="9.5" stroke="#7d2233" strokeWidth="0.55" fill="none" />
            <line x1="16" y1="1.5" x2="16" y2="30.5" stroke="#b89968" strokeWidth="0.7" />
            <line x1="11" y1="16" x2="21" y2="16" stroke="#7d2233" strokeWidth="0.55" />
            <circle cx="16" cy="11" r="1.2" fill="#b89968" />
          </svg>
          <span className={styles.wordmark}>ABRAXAS</span>
        </div>

        <p className={styles.invocation}>
          Uma inteligência que vive na sua máquina, fala apenas com você, e não
          deve nada à nuvem.
        </p>

        <span className={styles.rule} aria-hidden="true" />

        <button className={styles.enterBtn} onClick={onBegin}>
          <span>começar · examinar a máquina</span>
          <span className={styles.arrow} aria-hidden="true">
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
              <path
                d="M3 8h10M9 4l4 4-4 4"
                stroke="currentColor"
                strokeWidth="1.4"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </span>
        </button>

        <button className={styles.altLink} onClick={onSkip}>
          já conheço a casa — entrar direto →
        </button>
      </div>
    </div>
  );
}
