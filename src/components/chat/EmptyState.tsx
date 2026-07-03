import styles from "./EmptyState.module.css";

const SEEDS = [
  {
    roman: "I",
    kind: "leitura",
    label: (
      <>
        o que Hesse quis dizer com <em>“abraxas”</em>?
      </>
    ),
    prompt: "Me explique, sem academicismo, o que significa Abraxas em Demian de Hesse.",
  },
  {
    roman: "II",
    kind: "escrita",
    label: <>reescreva como uma nota de margem</>,
    prompt: "Reescreva este parágrafo como se Borges o estivesse anotando na margem.",
  },
  {
    roman: "III",
    kind: "técnica",
    label: <>o que é quantização, em voz baixa</>,
    prompt:
      "Explique de forma honesta o que é quantização de modelos e por que importa para rodar localmente.",
  },
  {
    roman: "IV",
    kind: "conselho",
    label: <>uma rotina contemplativa para hoje</>,
    prompt: "Me dê uma rotina contemplativa de 20 minutos para esta noite.",
  },
];

/** The "new conversation" invocation — a still page before the first word. */
export function EmptyState({ onSeed }: { onSeed: (prompt: string) => void }) {
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

        <span className={styles.kicker}>i · um silêncio sem palavras</span>

        <h1 className={styles.title}>
          <span>Diga a primeira palavra.</span>
          <span className={styles.quiet}>o resto vem.</span>
        </h1>

        <span className={styles.rule} aria-hidden="true"></span>

        <p className={styles.gloss}>
          Abraxas escuta em silêncio até você falar. Pergunte, peça uma leitura, traga um
          fragmento — ou comece por uma das passagens abaixo.
        </p>

        <div className={styles.seeds}>
          {SEEDS.map((seed) => (
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
          <kbd>↵</kbd> enviar · <kbd>shift</kbd>+<kbd>↵</kbd> quebrar linha
        </span>
      </div>
    </section>
  );
}
