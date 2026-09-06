import { useEffect, useMemo, useRef, useState } from "react";
import { useFormat, useT } from "../../lib/i18n";
import { useCatalogStore } from "../../stores/catalog";
import { useModelStore } from "../../stores/model";
import { useUiStore } from "../../stores/ui";
import styles from "./LoadRitual.module.css";

/** Labor order; the visible label and its subtitle come from the dictionary. */
const LABORS = ["weights", "memory", "context", "warmup"] as const;

/** Full-window "despertando o oráculo" overlay per "Abraxas Load Model.html".
 *  Mounts while a ritual-presented load is in flight; the labor checklist
 *  advances on a timer (the real load is one opaque await) and the last labor
 *  only completes when the backend confirms. On success it hands off to the
 *  chat; on failure it retreats and lets the Models view surface the error. */
export function LoadRitual() {
  const t = useT().ritual;
  const tShell = useT().onboarding;
  const f = useFormat();
  const status = useModelStore((s) => s.status);
  const loadingId = useModelStore((s) => s.loadingId);
  const loadedId = useModelStore((s) => s.loadedId);
  const presentation = useModelStore((s) => s.loadPresentation);
  const installed = useModelStore((s) => s.installed);
  const catalogModels = useCatalogStore((s) => s.models);
  const setView = useUiStore((s) => s.setView);

  const [visibleId, setVisibleId] = useState<string | null>(null);
  const [laborIdx, setLaborIdx] = useState(0);
  const [done, setDone] = useState(false);
  const [fading, setFading] = useState(false);
  const timers = useRef<number[]>([]);

  const clearTimers = () => {
    for (const t of timers.current) window.clearTimeout(t);
    timers.current = [];
  };

  // Enter: a ritual load began.
  useEffect(() => {
    if (status === "loading" && presentation === "ritual" && loadingId) {
      setVisibleId(loadingId);
      setLaborIdx(0);
      setDone(false);
      setFading(false);
    }
  }, [status, presentation, loadingId]);

  // Advance the labors while waiting; hold before the last until confirmed.
  useEffect(() => {
    if (!visibleId || done) return;
    if (status === "loading") {
      if (laborIdx >= LABORS.length - 1) return;
      const t = window.setTimeout(() => setLaborIdx((i) => i + 1), 1300);
      timers.current.push(t);
      return () => window.clearTimeout(t);
    }
    if (status === "loaded" && loadedId === visibleId) {
      // Confirmed: sweep the remaining labors, settle, hand off to the chat.
      setDone(true);
      setLaborIdx(LABORS.length);
      const t1 = window.setTimeout(() => setFading(true), 900);
      const t2 = window.setTimeout(() => {
        setVisibleId(null);
        setView("chat");
      }, 1400);
      timers.current.push(t1, t2);
      return;
    }
    // Load failed (status fell back to idle/error/loaded-something-else).
    setVisibleId(null);
  }, [status, loadedId, visibleId, laborIdx, done, setView]);

  useEffect(() => clearTimers, []);

  const entry = useMemo(
    () => catalogModels.find((m) => m.model.id === visibleId)?.model ?? null,
    [catalogModels, visibleId],
  );
  const row = installed.find((m) => m.id === visibleId) ?? null;

  if (!visibleId) return null;

  const asides: Record<(typeof LABORS)[number], string> = {
    weights: row ? `${f.gb(row.size_bytes, 1)} gb` : "—",
    memory: entry ? `${f.decimal(entry.min_ram_mb / 1024, 1)} gb · ${t.ram}` : t.ram,
    context: entry ? t.tokens(f.contextK(entry.context_length)) : "—",
    warmup: "—",
  };

  const activeKey = LABORS[Math.min(laborIdx, LABORS.length - 1)];

  return (
    <div className={`${styles.page} ${fading ? styles.fading : ""}`} role="dialog" aria-label={t.aria}>
      <span className={`${styles.corner} ${styles.cornerTl}`}>{tShell.wordmark}</span>
      <span className={`${styles.corner} ${styles.cornerBr}`}>
        <span className="pulse" />
        {tShell.offline}
      </span>

      <div className={styles.stage}>
        <div className={styles.markRow}>
          <span className={styles.markStep}>{t.step}</span>
          <span className={styles.markSep}>·</span>
          <span>{t.subtitle}</span>
        </div>

        <div className={styles.seal} role="img" aria-label={t.aria}>
          <svg viewBox="0 0 300 300" aria-hidden="true">
            <circle className={styles.ringOuter} cx="150" cy="150" r="138" />
            <circle className={styles.ringInner} cx="150" cy="150" r="110" />
            <g className={styles.ticks}>
              <line className={styles.cardinal} x1="150" y1="6" x2="150" y2="22" />
              <line x1="225" y1="26" x2="221" y2="38" />
              <line x1="274" y1="75" x2="262" y2="79" />
              <line className={styles.cardinal} x1="294" y1="150" x2="278" y2="150" />
              <line x1="274" y1="225" x2="262" y2="221" />
              <line x1="225" y1="274" x2="221" y2="262" />
              <line className={styles.cardinal} x1="150" y1="294" x2="150" y2="278" />
              <line x1="75" y1="274" x2="79" y2="262" />
              <line x1="26" y1="225" x2="38" y2="221" />
              <line className={styles.cardinal} x1="6" y1="150" x2="22" y2="150" />
              <line x1="26" y1="75" x2="38" y2="79" />
              <line x1="75" y1="26" x2="79" y2="38" />
            </g>
            <circle className={styles.arc} cx="150" cy="150" r="128" strokeDasharray="246 558" />
            <circle className={styles.arc2} cx="150" cy="150" r="96" strokeDasharray="100 503" />
          </svg>
          <div className={styles.center}>
            <svg className={styles.glyphC} viewBox="0 0 32 32" fill="none" aria-hidden="true">
              <circle cx="16" cy="16" r="13" stroke="#b89968" strokeWidth="0.9" fill="none" />
              <circle cx="16" cy="16" r="9.5" stroke="#7d2233" strokeWidth="0.7" fill="none" />
              <line x1="16" y1="1.5" x2="16" y2="30.5" stroke="#b89968" strokeWidth="0.9" />
              <line x1="11" y1="16" x2="21" y2="16" stroke="#7d2233" strokeWidth="0.7" />
              <circle cx="16" cy="11" r="1.4" fill="#b89968" />
            </svg>
            <span className={styles.sub}>{done ? t.ready : t.subtitles[activeKey]}</span>
          </div>
        </div>

        <h1 className={styles.verb}>
          <span>{t.verbLead}</span>
          <span className={styles.verbQuiet}>{t.verbQuiet}</span>
        </h1>

        <ol className={styles.labors} aria-live="polite">
          {LABORS.map((labor, i) => {
            const state = done || i < laborIdx ? "done" : i === laborIdx ? "active" : "idle";
            return (
              <li className={styles.labor} data-state={state} key={labor}>
                <span className={styles.mark} />
                <span>{t.labors[labor]}</span>
                <span className={styles.aside}>
                  {state === "done" ? t.doneLabor : asides[labor]}
                </span>
              </li>
            );
          })}
        </ol>

        <div className={styles.chosen}>
          <span>{t.loading}</span>
          <b>{entry?.name ?? visibleId}</b>
          <span className={styles.chosenMono}>
            {entry ? `${entry.params_b}B · ${entry.quantization.toUpperCase()}` : ""}
          </span>
        </div>
      </div>
    </div>
  );
}
