import { useEffect, useState } from "react";
import { eta, gb, mbps, size } from "../../lib/format";
import { useDiskStore } from "../../stores/disk";
import { useDownloadsStore } from "../../stores/downloads";
import { useModelStore } from "../../stores/model";
import { useUiStore } from "../../stores/ui";
import { ErrorAction, ErrorCard, ErrorLink } from "./ErrorCard";
import { SealDial } from "./SealDial";
import styles from "./DownloadPane.module.css";

const BADGE: Record<string, string> = {
  confirm: "confirmação",
  starting: "conectando",
  downloading: "descendo",
  verifying: "verificando",
  paused: "pausado",
  completed: "completo · sha-256 íntegro",
};

const SITUATION: Record<string, string> = {
  starting: "estabelecendo",
  downloading: "estável",
  verifying: "conferindo selo",
  paused: "pausado",
  completed: "íntegro",
};

export function DownloadPane() {
  const session = useDownloadsStore((s) => s.session);
  const start = useDownloadsStore((s) => s.start);
  const pause = useDownloadsStore((s) => s.pause);
  const reset = useDownloadsStore((s) => s.reset);
  const load = useModelStore((s) => s.load);
  const setModelsPane = useUiStore((s) => s.setModelsPane);

  const usage = useDiskStore((s) => s.usage);
  const refreshDisk = useDiskStore((s) => s.refresh);
  const [recoveryDismissed, setRecoveryDismissed] = useState(false);

  const phaseForEffects = session?.phase;
  useEffect(() => {
    void refreshDisk();
    // Free space shrinks while bytes land — keep the meter and the
    // disk-critical warning honest during an active download.
    if (phaseForEffects !== "downloading") return;
    const timer = setInterval(() => void refreshDisk(), 5000);
    return () => clearInterval(timer);
  }, [refreshDisk, phaseForEffects]);

  // Nothing to show — the user landed here without picking a model.
  useEffect(() => {
    if (!session) setModelsPane("catalog");
  }, [session, setModelsPane]);
  if (!session) return null;

  const { entry, phase, downloadedBytes, totalBytes, hashedBytes, speedBps, resumedFrom } = session;
  const model = entry.model;

  const backToCatalog = () => {
    reset();
    setModelsPane("catalog");
  };
  const abandon = () => {
    // Cancel on the backend if still moving; the `.part` stays for a resume.
    if (phase === "downloading" || phase === "starting") void pause();
    backToCatalog();
  };

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
          : phase === "starting"
            ? "estimando"
            : "—";

  const origin = model.url.replace(/^https?:\/\//, "");

  const verbLine: Record<string, string> = {
    confirm: "aguardando a sua palavra para começar.",
    starting: "estabelecendo o canal com o repositório.",
    downloading: "o texto está atravessando a rede.",
    verifying: "conferindo o selo: sha-256 byte a byte.",
    paused: "o download está suspenso. retome quando quiser.",
    completed: "o modelo está em casa.",
  };

  return (
    <section className={styles.page}>
      {/* ============== LEFT — LEDGER ============== */}
      <div className={styles.leafL}>
        <div className={styles.ledgerStack}>
          <h1 className={styles.verb}>
            <em>{model.name}</em>
          </h1>

          <div className={styles.chosen}>
            <span className={styles.chosenWho}>
              <span className={styles.chosenName}>{model.name}</span>
              <span className={styles.chosenMono}>
                {model.publisher} · {model.params_b}b · {model.quantization.toLowerCase()}
              </span>
            </span>
            {phase === "confirm" && (
              <button className={styles.reverseLink} onClick={backToCatalog}>
                trocar →
              </button>
            )}
          </div>

          <div className={styles.entries}>
            <div className={styles.entry}>
              <span className={styles.entryLabel}>parâmetros</span>
              <span className={styles.entryValue}>
                {model.params_b} B · {model.quantization}
              </span>
            </div>
            <div className={styles.entry}>
              <span className={styles.entryLabel}>tamanho</span>
              <span className={styles.entryValue}>
                <b>{gb(model.size_bytes)} GB</b>
              </span>
            </div>
            <div className={styles.entry}>
              <span className={styles.entryLabel}>tempo</span>
              <span className={styles.entryValue}>
                {phase === "downloading" && speedBps
                  ? `${etaText} · ${mbps(speedBps)} mb/s`
                  : phase === "confirm"
                    ? "estimado ao começar"
                    : etaText}
              </span>
            </div>
            <div className={styles.entry}>
              <span className={styles.entryLabel}>origem</span>
              <span className={`${styles.entryValue} ${styles.entryMono}`}>{origin}</span>
            </div>
          </div>

          <StorageRow
            modelsDir={usage?.models_dir ?? null}
            freeBytes={usage && usage.total_bytes > 0 ? usage.free_bytes : null}
            totalBytes={usage && usage.total_bytes > 0 ? usage.total_bytes : null}
            remainingBytes={phase === "confirm" ? model.size_bytes : remaining}
          />
        </div>
      </div>

      {/* ============== RIGHT — THE WORK ============== */}
      <div className={styles.leafR}>
        {resumedFrom != null && resumedFrom > 0 && !recoveryDismissed && phase !== "completed" && (
          <div className={styles.recovery} role="status" aria-live="polite">
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path
                d="M3 8a5 5 0 1 0 1.5-3.6M3 4v3h3"
                stroke="currentColor"
                strokeWidth="1.3"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
            retomado em{" "}
            <b>
              {gb(resumedFrom, 1)} GB / {gb(totalBytes)} GB
            </b>
            <button className={styles.recoveryDismiss} onClick={() => setRecoveryDismissed(true)}>
              ok
            </button>
          </div>
        )}

        {phase === "failed" ? (
          <DownloadFailure />
        ) : (
          <div className={styles.work}>
            <div className={styles.stateCaption} data-state={phase}>
              <span className={styles.stateBadge}>
                <span className={styles.stateGlyph} />
                {BADGE[phase]}
              </span>
              <span className={styles.verbLine}>{verbLine[phase]}</span>
            </div>

            {phase === "confirm" ? (
              <SealDial progress={0} state="confirm" armed>
                <div className="pct">
                  {gb(model.size_bytes)}
                  <span className="sym"> gb</span>
                </div>
                <div className="below">
                  <span>a descer</span>
                </div>
              </SealDial>
            ) : phase === "completed" ? (
              <SealDial progress={100} state="complete" showCheck>
                <div className="below" style={{ marginTop: 62 }}>
                  <b>{gb(totalBytes)} gb</b> · íntegro
                </div>
              </SealDial>
            ) : (
              <SealDial
                progress={pct}
                state={
                  phase === "starting"
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

            {phase !== "confirm" && phase !== "completed" && (
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
                <div className={styles.metric}>
                  <span className={styles.metricK}>situação</span>
                  <span className={styles.metricV}>{SITUATION[phase] ?? "—"}</span>
                </div>
              </div>
            )}

            {phase === "confirm" && (
              <div className={styles.actionsRow}>
                <button className={styles.btn} onClick={backToCatalog}>
                  <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                    <path
                      d="M13 8H3M7 4 3 8l4 4"
                      stroke="currentColor"
                      strokeWidth="1.4"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                  <span>voltar ao compêndio</span>
                </button>
                <button className={`${styles.btn} ${styles.btnPrimary}`} onClick={() => void start()}>
                  <span>começar</span>
                  <span className={styles.btnArrow} aria-hidden="true">
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
            )}

            {(phase === "starting" ||
              phase === "downloading" ||
              phase === "verifying" ||
              phase === "paused") && (
              <div className={styles.actionsRow}>
                <button
                  className={styles.btn}
                  disabled={phase !== "paused"}
                  title={
                    phase !== "paused"
                      ? "pause antes de escolher outro modelo"
                      : "devolve ao compêndio; os bytes baixados ficam salvos"
                  }
                  onClick={backToCatalog}
                >
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
                  className={`${styles.btn} ${styles.btnWarn}`}
                  disabled={phase === "verifying"}
                  onClick={abandon}
                >
                  cancelar
                </button>
                <button
                  className={`${styles.btn} ${styles.btnPrimary}`}
                  disabled={phase === "verifying" || phase === "starting"}
                  onClick={() => (phase === "paused" ? void start() : void pause())}
                >
                  <span>{phase === "paused" ? "retomar" : "pausar"}</span>
                </button>
              </div>
            )}

            {phase === "completed" && (
              <div className={styles.actionsRow}>
                <button className={styles.ghostLink} onClick={backToCatalog}>
                  voltar ao compêndio
                </button>
                <button
                  className={`${styles.btn} ${styles.btnPrimary}`}
                  onClick={() => {
                    const id = model.id;
                    reset();
                    setModelsPane("manager");
                    void load(id, "ritual");
                  }}
                >
                  <span>despertar o modelo</span>
                  <span className={styles.btnArrow} aria-hidden="true">
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
            )}
          </div>
        )}
      </div>
    </section>
  );
}

/** The storage row: models dir, free-space meter (filled = disk already
 *  used, incoming = this model), and the disk-critical warning when what
 *  still needs to land doesn't fit with a 1 GB breathing margin. */
function StorageRow({
  modelsDir,
  freeBytes,
  totalBytes,
  remainingBytes,
}: {
  modelsDir: string | null;
  freeBytes: number | null;
  totalBytes: number | null;
  /** Bytes still to land on disk (full size before start, shrinking after). */
  remainingBytes: number;
}) {
  const MARGIN_BYTES = 1e9;
  const hasDisk = freeBytes !== null && totalBytes !== null;
  const warn = hasDisk && remainingBytes + MARGIN_BYTES > freeBytes;
  const usedPct = hasDisk ? ((totalBytes - freeBytes) / totalBytes) * 100 : 0;
  // Incoming band = what still needs to land; already-landed bytes are
  // counted inside `used` by the periodic disk refresh.
  const incomingPct = hasDisk ? Math.min(100 - usedPct, (remainingBytes / totalBytes) * 100) : 0;
  const afterBytes = hasDisk ? freeBytes - remainingBytes : 0;

  return (
    <div className={styles.storage} data-warn={warn}>
      <div className={styles.storageTop}>
        <span className={styles.storagePath}>{modelsDir ?? "…"}</span>
        <span className={styles.storageRhs}>
          {hasDisk
            ? `${gb(totalBytes - freeBytes, 0)} / ${gb(totalBytes, 0)} gb`
            : `+${gb(remainingBytes)} gb`}
        </span>
      </div>
      {hasDisk && (
        <>
          <div className={styles.storageMeter} aria-hidden="true">
            <div className={styles.storageFilled} style={{ width: `${usedPct}%` }} />
            <div
              className={styles.storageIncoming}
              style={{ left: `${usedPct}%`, width: `${incomingPct}%` }}
            />
          </div>
          {warn ? (
            <span className={styles.storageWarnLine}>
              espaço crítico — libere pelo menos{" "}
              {gb(Math.max(0, remainingBytes + MARGIN_BYTES - freeBytes), 1)} gb ou escolha um
              modelo menor.
            </span>
          ) : (
            <div className={styles.storageFoot}>
              <span className={styles.storageVerdictOk}>
                cabe com folga · restam {gb(afterBytes, 0)} gb depois
              </span>
            </div>
          )}
        </>
      )}
    </div>
  );
}

/** Failure card variants inside the download spread (Error States II / VI). */
function DownloadFailure() {
  const session = useDownloadsStore((s) => s.session);
  const start = useDownloadsStore((s) => s.start);
  const reset = useDownloadsStore((s) => s.reset);
  const setModelsPane = useUiStore((s) => s.setModelsPane);

  if (!session) return null;
  const { entry, errorKind, errorMessage, downloadedBytes, totalBytes } = session;
  const pct = totalBytes > 0 ? Math.round((downloadedBytes / totalBytes) * 100) : 0;

  const discard = () => {
    reset();
    setModelsPane("catalog");
  };

  if (errorKind === "ChecksumMismatch") {
    return (
      <ErrorCard
        badge="vi · selo não confere"
        code="err.integrity.sha256"
        title="O selo do arquivo não confere."
        quiet="não vou abrir um codex que possa ter sido tocado."
        gloss={
          <>
            Os bytes chegaram inteiros, mas a soma <em>sha-256</em> não bate com a publicada pelo
            autor do modelo. Pode ter sido corrupção no caminho — pode ter sido troca. O arquivo
            foi descartado; preferimos não usar.
          </>
        }
        diag={[
          { k: "esperado", v: entry.model.sha256.slice(0, 24) + "…" },
          { k: "arquivo", v: "descartado · não é exposto à conversa", italic: true },
        ]}
        actions={
          <>
            <ErrorAction onClick={() => void start()}>baixar de novo</ErrorAction>
            <ErrorLink onClick={discard}>voltar ao compêndio</ErrorLink>
          </>
        }
      />
    );
  }

  return (
    <ErrorCard
      badge="ii · download interrompido"
      code="err.download.network"
      title="O fio se cortou no meio."
      quiet={pct > 0 ? `${pct}% chegaram. retomamos.` : "nada se perdeu. tentamos de novo."}
      gloss={
        <>
          A descida foi interrompida — {errorMessage ?? "o servidor remoto parou de responder"}. Os
          bytes baixados ficaram salvos; podemos continuar de onde paramos sem recomeçar.
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
        { k: "link", v: entry.model.url.replace(/^https?:\/\//, "") },
      ]}
      actions={
        <>
          <ErrorAction onClick={() => void start()}>
            {pct > 0 ? `retomar de ${pct}%` : "tentar de novo"}
          </ErrorAction>
          <ErrorLink onClick={discard}>cancelar download</ErrorLink>
        </>
      }
      pulse
    />
  );
}
