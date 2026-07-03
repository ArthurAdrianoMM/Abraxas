import { useEffect, useRef, useState } from "react";
import { eta, gb, mbps, size } from "../../lib/format";
import { useDiskStore } from "../../stores/disk";
import { useDownloadsStore } from "../../stores/downloads";
import { useModelStore } from "../../stores/model";
import { useSettingsStore } from "../../stores/settings";
import { useUiStore } from "../../stores/ui";
import { ErrorAction, ErrorCard, ErrorLink } from "../models/ErrorCard";
import { SealDial } from "../models/SealDial";
import { StorageRow } from "../models/StorageRow";
import styles from "./DownloadStep.module.css";

const BADGE: Record<string, string> = {
  confirm: "preparando",
  starting: "conectando",
  downloading: "descendo",
  verifying: "verificando",
  paused: "pausado",
  completed: "completo · sha-256 íntegro",
};

const VERB_LINE: Record<string, string> = {
  confirm: "um instante — abrindo o canal.",
  starting: "estabelecendo o canal com o repositório.",
  downloading: "o texto está atravessando a rede.",
  verifying: "conferindo o selo: sha-256 byte a byte.",
  paused: "o download está suspenso. retome quando quiser.",
  completed: "o modelo está em casa. despertando…",
};

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
            badge="iii · o despertar falhou"
            code="err.model.load"
            title="O modelo baixou, mas não despertou."
            quiet="o arquivo está íntegro no disco — nada se perdeu."
            gloss={
              <>
                O download terminou com o selo conferido, mas o carregamento na memória falhou —
                geralmente falta de RAM livre. Feche outras aplicações e tente de novo, ou entre
                no estúdio: o modelo fica instalado e pode ser desperto pelo ateliê.
              </>
            }
            actions={
              <>
                <ErrorAction
                  onClick={() => {
                    setLoadFailed(false);
                    awakeningRef.current = false;
                  }}
                >
                  tentar despertar de novo
                </ErrorAction>
                <ErrorLink onClick={onSkip}>entrar no estúdio →</ErrorLink>
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
              badge="vi · selo não confere"
              code="err.integrity.sha256"
              title="O selo do arquivo não confere."
              quiet="não vou abrir um codex que possa ter sido tocado."
              gloss={
                <>
                  Os bytes chegaram inteiros, mas a soma <em>sha-256</em> não bate com a publicada
                  pelo autor do modelo. O arquivo foi descartado; preferimos não usar.
                </>
              }
              actions={
                <>
                  <ErrorAction onClick={() => void start()}>baixar de novo</ErrorAction>
                  <ErrorLink onClick={chooseAnother}>escolher outro modelo</ErrorLink>
                  <ErrorLink onClick={skipForNow}>pular por agora</ErrorLink>
                </>
              }
            />
          ) : (
            <ErrorCard
              badge="ii · download interrompido"
              code="err.download.network"
              title="O fio se cortou no meio."
              quiet={pct > 0 ? `${pct}% chegaram. retomamos.` : "nada se perdeu. tentamos de novo."}
              gloss={
                <>
                  A descida foi interrompida —{" "}
                  {session.errorMessage ?? "o servidor remoto parou de responder"}. Os bytes
                  baixados ficaram salvos; podemos continuar de onde paramos sem recomeçar.
                </>
              }
              diag={[
                {
                  k: "progresso",
                  v: (
                    <>
                      <b>
                        {gb(downloadedBytes)} / {gb(totalBytes)}
                      </b>{" "}
                      GB · {pct}%
                    </>
                  ),
                },
              ]}
              actions={
                <>
                  <ErrorAction onClick={() => void start()}>
                    {pct > 0 ? `retomar de ${pct}%` : "tentar de novo"}
                  </ErrorAction>
                  <ErrorLink onClick={chooseAnother}>escolher outro modelo</ErrorLink>
                  <ErrorLink onClick={skipForNow}>pular por agora</ErrorLink>
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
        ? eta(remaining / speedBps)
        : "estimando"
      : phase === "verifying"
        ? "< 1 min"
        : phase === "paused"
          ? "pausado"
          : "estimando";

  return (
    <div className={styles.page}>
      <div className={styles.stage}>
        <div className={styles.chosen}>
          <span className={styles.chosenKicker}>a primeira voz da casa</span>
          <h1 className={styles.chosenName}>
            <em>{model.name}</em>
          </h1>
          <span className={styles.chosenMono}>
            {model.publisher} · {model.params_b}b · {model.quantization.toLowerCase()} ·{" "}
            {gb(model.size_bytes)} gb
          </span>
        </div>

        <div className={styles.stateCaption} data-state={phase}>
          <span className={styles.stateBadge}>
            <span className={styles.stateGlyph} />
            {BADGE[phase ?? "confirm"]}
          </span>
          <span className={styles.verbLine}>{VERB_LINE[phase ?? "confirm"]}</span>
        </div>

        {phase === "completed" ? (
          <SealDial progress={100} state="complete" showCheck>
            <div className="below" style={{ marginTop: 62 }}>
              <b>{gb(totalBytes)} gb</b> · íntegro
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
                <b>{size(phase === "verifying" ? hashedBytes : downloadedBytes)}</b> de{" "}
                <b>{gb(totalBytes)} gb</b>
              </span>
            </div>
          </SealDial>
        )}

        {phase !== "completed" && (
          <div className={styles.metrics}>
            <div className={`${styles.metric} ${phase !== "downloading" ? styles.metricDim : ""}`}>
              <span className={styles.metricK}>vazão</span>
              <span className={styles.metricV}>
                {phase === "downloading" && speedBps ? mbps(speedBps) : "—"}{" "}
                <span className={styles.metricSub}>mb/s</span>
              </span>
            </div>
            <div className={styles.metric}>
              <span className={styles.metricK}>restam</span>
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
                <span>escolher outro modelo</span>
              </button>
              <button
                className={`${styles.btn} ${styles.btnPrimary}`}
                disabled={phase === "verifying" || phase === "starting" || phase === "confirm"}
                onClick={() => (phase === "paused" ? void start() : void pause())}
              >
                <span>{phase === "paused" ? "retomar" : "pausar"}</span>
              </button>
            </div>
            <button className={styles.skipLink} onClick={skipForNow}>
              pular por agora — os bytes baixados ficam salvos →
            </button>
          </>
        )}
      </div>
    </div>
  );
}
