import { useEffect, useMemo, useState } from "react";
import type { ClassifiedModel, CompatibilityTier } from "../../lib/tauri/bindings";
import { contextK, gb } from "../../lib/format";
import { useCatalogStore } from "../../stores/catalog";
import { useDiskStore } from "../../stores/disk";
import { ErrorAction, ErrorCard, ErrorLink } from "../models/ErrorCard";
import { fitsOnDisk } from "../models/StorageRow";
import styles from "./ChooseStep.module.css";

const COMPAT: Record<CompatibilityTier, { key: string; label: string }> = {
  Recommended: { key: "recommended", label: "recomendado" },
  Viable: { key: "works", label: "roda bem" },
  Heavy: { key: "slow", label: "pode ficar lento" },
  NotSupported: { key: "not", label: "não recomendado" },
};

const PIPS = ["sol", "lua", "sigilo", "colunas", "torre"];

/** Hermetic card emblems from the design, cycled by card index. */
function Emblem({ index }: { index: number }) {
  const which = index % 5;
  return (
    <svg viewBox="0 0 100 100" aria-hidden="true">
      <g fill="none" stroke="#b89968" strokeWidth="0.9">
        {which === 0 && (
          <>
            <circle cx="50" cy="50" r="14" />
            <circle cx="50" cy="50" r="22" opacity="0.4" />
            <g strokeLinecap="round">
              <line x1="50" y1="14" x2="50" y2="22" />
              <line x1="50" y1="78" x2="50" y2="86" />
              <line x1="14" y1="50" x2="22" y2="50" />
              <line x1="78" y1="50" x2="86" y2="50" />
              <line x1="25" y1="25" x2="31" y2="31" />
              <line x1="69" y1="69" x2="75" y2="75" />
              <line x1="75" y1="25" x2="69" y2="31" />
              <line x1="31" y1="69" x2="25" y2="75" />
            </g>
            <circle cx="50" cy="50" r="3" fill="#b89968" stroke="none" />
          </>
        )}
        {which === 1 && (
          <>
            <path d="M62 22 a30 30 0 1 0 0 56 a22 22 0 1 1 0 -56 z" />
            <circle cx="38" cy="38" r="1.6" fill="#b89968" stroke="none" />
            <circle cx="42" cy="58" r="1.2" fill="#b89968" stroke="none" opacity=".7" />
            <circle cx="32" cy="50" r="1" fill="#b89968" stroke="none" opacity=".5" />
          </>
        )}
        {which === 2 && (
          <>
            <polygon points="50,16 86,80 14,80" />
            <polygon points="50,28 76,74 24,74" opacity=".45" />
            <ellipse cx="50" cy="60" rx="14" ry="7" />
            <circle cx="50" cy="60" r="3.2" fill="#b89968" stroke="none" />
            <line x1="50" y1="16" x2="50" y2="28" strokeLinecap="round" opacity=".55" />
          </>
        )}
        {which === 3 && (
          <>
            <line x1="20" y1="22" x2="80" y2="22" />
            <line x1="22" y1="22" x2="22" y2="78" />
            <line x1="78" y1="22" x2="78" y2="78" />
            <line x1="20" y1="78" x2="80" y2="78" />
            <rect x="34" y="32" width="32" height="36" opacity=".3" />
            <line x1="36" y1="32" x2="36" y2="68" opacity=".5" />
            <line x1="50" y1="32" x2="50" y2="68" opacity=".5" />
            <line x1="64" y1="32" x2="64" y2="68" opacity=".5" />
            <circle cx="50" cy="50" r="4" fill="#b89968" stroke="none" opacity=".6" />
          </>
        )}
        {which === 4 && (
          <>
            <path d="M30 80 L30 38 L50 22 L70 38 L70 80 Z" />
            <line x1="50" y1="22" x2="50" y2="80" opacity=".4" />
            <line x1="30" y1="50" x2="70" y2="50" opacity=".4" />
            <line x1="30" y1="64" x2="70" y2="64" opacity=".4" />
            <line x1="40" y1="38" x2="40" y2="50" opacity=".5" />
            <line x1="60" y1="38" x2="60" y2="50" opacity=".5" />
            <circle cx="50" cy="14" r="2.5" fill="#b89968" stroke="none" />
            <line x1="50" y1="16" x2="50" y2="22" />
          </>
        )}
      </g>
    </svg>
  );
}

function romanFor(i: number): string {
  const romans = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII"];
  return romans[i] ?? String(i + 1);
}

function posClass(delta: number): string {
  if (delta === 0) return styles.pos0;
  if (delta === -1) return styles.posL1;
  if (delta === 1) return styles.posR1;
  if (delta === -2) return styles.posL2;
  if (delta === 2) return styles.posR2;
  return delta < 0 ? styles.posHiddenL : styles.posHiddenR;
}

/** Model Recommendation per "Abraxas Model Recommendation.html": a hand of
 *  cards fanned on the table, the backend-recommended one centered and
 *  gilded. Tiers come classified from `fetch_classified_catalog` — nothing
 *  is re-derived here. */
export function ChooseStep({
  onBack,
  onChoose,
  onSkip,
}: {
  onBack: () => void;
  onChoose: (entry: ClassifiedModel) => void;
  onSkip: () => void;
}) {
  const status = useCatalogStore((s) => s.status);
  const models = useCatalogStore((s) => s.models);
  const source = useCatalogStore((s) => s.source);
  const error = useCatalogStore((s) => s.error);
  const refresh = useCatalogStore((s) => s.refresh);
  const usage = useDiskStore((s) => s.usage);
  const refreshDisk = useDiskStore((s) => s.refresh);

  useEffect(() => {
    void refresh();
    void refreshDisk();
  }, [refresh, refreshDisk]);

  const recommendedIdx = useMemo(() => {
    const i = models.findIndex((m) => m.tier === "Recommended");
    return i === -1 ? 0 : i;
  }, [models]);

  const [activeIdx, setActiveIdx] = useState<number | null>(null);
  const idx = activeIdx ?? recommendedIdx;
  const setActive = (i: number) => setActiveIdx(Math.max(0, Math.min(models.length - 1, i)));

  // Keyboard: arrows browse the hand; Enter on a card confirms/centers it.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        setActiveIdx((cur) => Math.max(0, (cur ?? recommendedIdx) - 1));
      }
      if (e.key === "ArrowRight") {
        e.preventDefault();
        setActiveIdx((cur) => Math.min(models.length - 1, (cur ?? recommendedIdx) + 1));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [models.length, recommendedIdx]);

  const freeBytes = usage && usage.total_bytes > 0 ? usage.free_bytes : null;

  /* -------- degraded states -------- */

  if (status === "error" && models.length === 0) {
    return (
      <div className={styles.page}>
        <div className={styles.centerCard}>
          <ErrorCard
            badge="i · compêndio inalcançável"
            code="err.catalog.network"
            title="O compêndio não respondeu."
            quiet="sem rede não há catálogo — mas a casa continua de pé."
            gloss={
              <>
                Não conseguimos baixar a lista de modelos — {error ?? "a rede parece indisponível"}
                . O Abraxas funciona 100% offline <em>depois</em> que um modelo é instalado; este
                primeiro passo é o único que precisa de internet. Conecte-se e tente de novo, ou
                entre no estúdio e busque o modelo mais tarde no ateliê.
              </>
            }
            actions={
              <>
                <ErrorAction onClick={() => void refresh()}>tentar de novo</ErrorAction>
                <ErrorLink onClick={onSkip}>entrar no estúdio sem modelo →</ErrorLink>
              </>
            }
            pulse
          />
        </div>
      </div>
    );
  }

  if (status === "loading" || (status === "idle" && models.length === 0)) {
    return (
      <div className={styles.page}>
        <div className={styles.loading}>
          <span className={styles.whisperBar} />
          consultando o compêndio de modelos
        </div>
      </div>
    );
  }

  const allUnsupported = models.length > 0 && models.every((m) => m.tier === "NotSupported");
  if (allUnsupported) {
    return (
      <div className={styles.page}>
        <div className={styles.centerCard}>
          <ErrorCard
            tier="warn"
            badge="ii · máquina abaixo do compêndio"
            code="err.compat.none"
            title="Nenhum modelo do compêndio cabe nesta máquina."
            quiet="preferimos dizer isso agora a prometer o que não roda."
            gloss={
              <>
                Todos os modelos do catálogo pedem mais memória do que este computador tem hoje.
                Você ainda pode entrar no estúdio e conferir o compêndio no ateliê — modelos
                menores são adicionados com o tempo.
              </>
            }
            actions={
              <>
                <ErrorAction onClick={onSkip}>entrar no estúdio mesmo assim</ErrorAction>
                <ErrorLink onClick={onBack}>voltar ao exame</ErrorLink>
              </>
            }
          />
        </div>
      </div>
    );
  }

  const active = models[idx];
  const activeFits = active ? fitsOnDisk(active.model.size_bytes, freeBytes) : true;
  const activeBlocked = active ? active.tier === "NotSupported" || !activeFits : true;

  return (
    <div className={styles.page}>
      <span className={styles.stepMeta}>
        passo 03 · <b>a escolha do modelo</b>
        {source === "cache" && <span className={styles.cacheNote}> · catálogo salvo (offline)</span>}
      </span>

      {/* ============== TABLE ============== */}
      <main className={styles.table}>
        <div className={styles.deck}>
          {models.map((m, i) => {
            const compat = COMPAT[m.tier];
            const isRec = m.tier === "Recommended" && i === recommendedIdx;
            return (
              <article
                key={m.model.id}
                className={`${styles.card} ${posClass(i - idx)}`}
                data-compat={compat.key}
                data-recommended={isRec}
                role="button"
                tabIndex={0}
                aria-label={`${m.model.name}, ${compat.label}`}
                onClick={() => i !== idx && setActive(i)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    if (i === idx) {
                      if (!activeBlocked) onChoose(m);
                    } else setActive(i);
                  }
                }}
              >
                {isRec ? (
                  <span className={styles.recBanner}>
                    <span className={styles.recLine} />
                    recomendado para você
                    <span className={styles.recLine} />
                  </span>
                ) : (
                  <span className={styles.compat}>
                    <span className={styles.compatBullet} />
                    {compat.label}
                  </span>
                )}

                {!isRec && (
                  <>
                    <span className={`${styles.index} ${styles.indexTl}`}>
                      <span className={styles.indexGlyph}>{romanFor(i)}</span>
                      <span className={styles.indexPip}>{PIPS[i % 5]}</span>
                    </span>
                    <span className={`${styles.index} ${styles.indexBr}`}>
                      <span className={styles.indexGlyph}>{romanFor(i)}</span>
                      <span className={styles.indexPip}>{PIPS[i % 5]}</span>
                    </span>
                  </>
                )}

                <span className={styles.emblem}>
                  <Emblem index={i} />
                </span>

                <div className={styles.content}>
                  <h3 className={styles.cardName}>{m.model.name}</h3>
                  <span className={styles.cardMono}>
                    {gb(m.model.size_bytes)} gb · {m.model.params_b}b ·{" "}
                    {m.model.quantization.toLowerCase()}
                  </span>
                  <span className={styles.cardRule} />
                  <p className={styles.rationale}>{m.model.description}</p>
                  <div className={styles.tags}>
                    <span className={styles.tag}>
                      ctx · <b>{contextK(m.model.context_length)}</b>
                    </span>
                    <span className={styles.tag}>
                      ram · <b>{Math.round(m.model.min_ram_mb / 1024)} gb</b>
                    </span>
                  </div>
                </div>
              </article>
            );
          })}
        </div>
      </main>

      {/* disk verdict for the centered card, only when it matters */}
      {active && !activeFits && (
        <div className={styles.diskWarn}>
          espaço crítico — este modelo pede {gb(active.model.size_bytes)} gb e o disco tem{" "}
          {freeBytes !== null ? gb(freeBytes, 0) : "?"} gb livres. escolha um menor ou libere
          espaço.
        </div>
      )}

      {/* ============== FOOTER ============== */}
      <footer className={styles.foot}>
        <div className={styles.footLeft}>
          <button className={styles.ghostLink} onClick={onBack}>
            <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path
                d="M13 8H3M7 4 3 8l4 4"
                stroke="currentColor"
                strokeWidth="1.4"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
            voltar ao exame
          </button>
          <button className={styles.skipLink} onClick={onSkip}>
            pular · entrar sem modelo →
          </button>
        </div>

        <div className={styles.navWrap}>
          <button
            className={styles.navBtn}
            disabled={idx === 0}
            onClick={() => setActive(idx - 1)}
            aria-label="anterior"
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
              <path
                d="M10 4 6 8l4 4"
                stroke="currentColor"
                strokeWidth="1.4"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </button>
          <span className={styles.counter}>
            <b>{idx + 1}</b> / {models.length}
          </span>
          <button
            className={styles.navBtn}
            disabled={idx === models.length - 1}
            onClick={() => setActive(idx + 1)}
            aria-label="próximo"
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
              <path
                d="M6 4l4 4-4 4"
                stroke="currentColor"
                strokeWidth="1.4"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </button>
        </div>

        <div className={styles.footRight}>
          <div className={styles.dots}>
            {models.map((m, i) => (
              <button
                key={m.model.id}
                className={styles.dotBtn}
                data-active={i === idx}
                onClick={() => setActive(i)}
                aria-label={`ir para ${m.model.name}`}
              >
                <span className={styles.dotMark} />
              </button>
            ))}
          </div>
          <button
            className={styles.useBtn}
            disabled={activeBlocked}
            title={
              active?.tier === "NotSupported"
                ? "este modelo pede uma máquina maior"
                : !activeFits
                  ? "não cabe no disco com folga"
                  : undefined
            }
            onClick={() => active && !activeBlocked && onChoose(active)}
          >
            <span>confirmar</span>
            <span className={styles.useArrow} aria-hidden="true">
              <svg width="13" height="13" viewBox="0 0 16 16" fill="none">
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
        </div>
      </footer>
    </div>
  );
}
