import { useEffect, useRef, useState } from "react";
import { useFormat, useT } from "../../lib/i18n";
import { useDiskStore } from "../../stores/disk";
import { useDownloadsStore } from "../../stores/downloads";
import { useModelStore } from "../../stores/model";
import { useSettingsStore } from "../../stores/settings";
import { useUiStore } from "../../stores/ui";
import { ErrorAction, ErrorCard, ErrorLink } from "../models/ErrorCard";
import { SealDial } from "../models/SealDial";
import { StorageRow } from "../models/StorageRow";
import styles from "./DownloadStep.module.css";

/** Guided first download (passo 04): starts by itself, shows the seal dial
 *  with live progress, and on a verified finish awakens the model through
 *  the LoadRitual before handing the user to the studio. */
export function DownloadStep({
  onChooseAnother,
  onSkip,
}: {
  onChooseAnother: () => void;
  onSkip: () => void;
}) {
  const t = useT().onboarding.download;
  const f = useFormat();
  const session = useDownloadsStore((s) => s.session);
  const start = useDownloadsStore((s) => s.start);
  const pause = useDownloadsStore((s) => s.pause);
  const reset = useDownloadsStore((s) => s.reset);
  const load = useModelStore((s) => s.load);
  const completeOnboarding = useUiStore((s) => s.completeOnboarding);

  const usage = useDiskStore((s) => s.usage);
  const refreshDisk = useDiskStore((s) => s.refresh);

  const [loadFailed, setLoadFailed] = useState(false);
  const awakeningRef = useRef(false);
  const handoffTimer = useRef<number | null>(null);

  const phase = session?.phase;

  // The guided flow begins on its own — the user already confirmed by
  // choosing the card.
  useEffect(() => {
    if (phase === "confirm") void start();
  }, [phase, start]);

  useEffect(() => {
    void refreshDisk();
    if (phase !== "downloading") return;
    const timer = setInterval(() => void refreshDisk(), 5000);
    return () => clearInterval(timer);
  }, [refreshDisk, phase]);

  // Verified download → stamp as default, wake it, hand off to the studio.
  useEffect(() => {
    if (phase !== "completed" || awakeningRef.current) return;
    const modelId = session?.modelId;
    if (!modelId) return;
    awakeningRef.current = true;
    void (async () => {
      await useSettingsStore.getState().save({ default_model_id: modelId });
      const ok = await load(modelId, "ritual");
      if (ok) {
        // Let the ritual settle its "pronto" beat before the shell appears.
        handoffTimer.current = window.setTimeout(() => {
          reset();
          completeOnboarding();
        }, 1500);
      } else {
        awakeningRef.current = false;
        setLoadFailed(true);
      }
    })();
  }, [phase, session?.modelId, load, reset, completeOnboarding]);

  useEffect(
    () => () => {
      if (handoffTimer.current !== null) window.clearTimeout(handoffTimer.current);
    },
    [],
  );

  // No session — the user landed here without choosing. Send them back.
  useEffect(() => {
    if (!session) onChooseAnother();
  }, [session, onChooseAnother]);
  if (!session) return null;

  const { entry, downloadedBytes, totalBytes, hashedBytes, speedBps } = session;
  const model = entry.model;

  const chooseAnother = () => {
    if (phase === "downloading" || phase === "starting") void pause();
    reset();
    onChooseAnother();
  };
  const skipForNow = () => {
    // The `.part` stays on disk; the ateliê can resume it any time.
    if (phase === "downloading" || phase === "starting") void pause();
    onSkip();
  };

  if (loadFailed) {
    return (
      <div className={styles.page}>
        <div className={styles.centerCard}>
          <ErrorCard
            badge={t.loadFailed.badge}
            code="err.model.load"
            title={t.loadFailed.title}
            quiet={t.loadFailed.quiet}
            gloss={t.loadFailed.gloss}
            actions={
              <>
                <ErrorAction
                  onClick={() => {
                    setLoadFailed(false);
                    awakeningRef.current = false;
                  }}
                >
                  {t.loadFailed.retry}
                </ErrorAction>
                <ErrorLink onClick={onSkip}>{t.loadFailed.enter}</ErrorLink>
              </>
            }
          />
        </div>
      </div>
    );
  }

  if (phase === "failed") {
    const pct = totalBytes > 0 ? Math.round((downloadedBytes / totalBytes) * 100) : 0;
    const isChecksum = session.errorKind === "ChecksumMismatch";
    return (
      <div className={styles.page}>
        <div className={styles.centerCard}>
          {isChecksum ? (
            <ErrorCard
              badge={t.checksumFailed.badge}
              code="err.integrity.sha256"
              title={t.checksumFailed.title}
              quiet={t.checksumFailed.quiet}
              gloss={t.checksumFailed.gloss}
              actions={
                <>
                  <ErrorAction onClick={() => void start()}>
                    {t.checksumFailed.redownload}
                  </ErrorAction>
                  <ErrorLink onClick={chooseAnother}>{t.chooseOther}</ErrorLink>
                  <ErrorLink onClick={skipForNow}>{t.skipForNow}</ErrorLink>
                </>
              }
            />
          ) : (
            <ErrorCard
              badge={t.networkFailed.badge}
              code="err.download.network"
              title={t.networkFailed.title}
              quiet={pct > 0 ? t.networkFailed.quietProgress(pct) : t.networkFailed.quietNone}
              gloss={t.networkFailed.gloss(session.errorMessage ?? t.networkFailed.reason)}
              diag={[
                {
                  k: t.networkFailed.progress,
                  v: (
                    <>
                      <b>
                        {f.gb(downloadedBytes)} / {f.gb(totalBytes)}
                      </b>{" "}
                      GB · {pct}%
                    </>
                  ),
                },
              ]}
              actions={
                <>
                  <ErrorAction onClick={() => void start()}>
                    {pct > 0 ? t.networkFailed.resumeFrom(pct) : t.networkFailed.retry}
                  </ErrorAction>
                  <ErrorLink onClick={chooseAnother}>{t.chooseOther}</ErrorLink>
                  <ErrorLink onClick={skipForNow}>{t.skipForNow}</ErrorLink>
                </>
              }
              pulse
            />
          )}
        </div>
      </div>
    );
  }

  const pct =
    phase === "verifying"
      ? totalBytes > 0
        ? (hashedBytes / totalBytes) * 100
        : 0
      : totalBytes > 0
        ? (downloadedBytes / totalBytes) * 100
        : 0;

  const remaining = Math.max(0, totalBytes - downloadedBytes);
  const etaText =
    phase === "downloading"
      ? speedBps && speedBps > 0
        ? f.eta(remaining / speedBps)
        : t.estimating
      : phase === "verifying"
        ? t.underAMinute
        : phase === "paused"
          ? t.pausedShort
          : t.estimating;

  return (
    <div className={styles.page}>
      <div className={styles.stage}>
        <div className={styles.chosen}>
          <span className={styles.chosenKicker}>{t.kicker}</span>
          <h1 className={styles.chosenName}>
            <em>{model.name}</em>
          </h1>
          <span className={styles.chosenMono}>
            {model.publisher} · {model.params_b}b · {model.quantization.toLowerCase()} ·{" "}
            {f.gb(model.size_bytes)} gb
          </span>
        </div>

        <div className={styles.stateCaption} data-state={phase}>
          <span className={styles.stateBadge}>
            <span className={styles.stateGlyph} />
            {t.badge[phase ?? "confirm"]}
          </span>
          <span className={styles.verbLine}>{t.verb[phase ?? "confirm"]}</span>
        </div>

        {phase === "completed" ? (
          <SealDial progress={100} state="complete" showCheck>
            <div className="below" style={{ marginTop: 62 }}>
              <b>{f.gb(totalBytes)} gb</b> · {t.intact}
            </div>
          </SealDial>
        ) : (
          <SealDial
            progress={pct}
            state={
              phase === "confirm" || phase === "starting"
                ? "connecting"
                : (phase as "downloading" | "verifying" | "paused")
            }
          >
            <div className="pct">
              {Math.max(0, Math.min(100, pct)).toFixed(0)}
              <span className="sym"> %</span>
            </div>
            <div className="below">
              <span>
                <b>{f.size(phase === "verifying" ? hashedBytes : downloadedBytes)}</b> {t.of}{" "}
                <b>{f.gb(totalBytes)} gb</b>
              </span>
            </div>
          </SealDial>
        )}

        {phase !== "completed" && (
          <div className={styles.metrics}>
            <div className={`${styles.metric} ${phase !== "downloading" ? styles.metricDim : ""}`}>
              <span className={styles.metricK}>{t.throughput}</span>
              <span className={styles.metricV}>
                {phase === "downloading" && speedBps ? f.mbps(speedBps) : "—"}{" "}
                <span className={styles.metricSub}>mb/s</span>
              </span>
            </div>
            <div className={styles.metric}>
              <span className={styles.metricK}>{t.remaining}</span>
              <span className={styles.metricV}>{etaText}</span>
            </div>
          </div>
        )}

        <div className={styles.storageSlot}>
          <StorageRow
            modelsDir={usage?.models_dir ?? null}
            freeBytes={usage && usage.total_bytes > 0 ? usage.free_bytes : null}
            totalBytes={usage && usage.total_bytes > 0 ? usage.total_bytes : null}
            remainingBytes={phase === "completed" ? 0 : remaining}
          />
        </div>

        {phase !== "completed" && (
          <>
            <div className={styles.actionsRow}>
              <button className={styles.btn} onClick={chooseAnother}>
                <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                  <path
                    d="M13 8H3M7 4 3 8l4 4"
                    stroke="currentColor"
                    strokeWidth="1.4"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </svg>
                <span>{t.chooseAnother}</span>
              </button>
              <button
                className={`${styles.btn} ${styles.btnPrimary}`}
                disabled={phase === "verifying" || phase === "starting" || phase === "confirm"}
                onClick={() => (phase === "paused" ? void start() : void pause())}
              >
                <span>{phase === "paused" ? t.resume : t.pause}</span>
              </button>
            </div>
            <button className={styles.skipLink} onClick={skipForNow}>
              {t.skip}
            </button>
          </>
        )}
      </div>
    </div>
  );
}
