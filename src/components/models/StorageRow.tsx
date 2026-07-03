import { gb } from "../../lib/format";
import styles from "./StorageRow.module.css";

/** The storage row from the Download design: models dir, free-space meter
 *  (filled = disk already used, incoming = this model), and the
 *  disk-critical warning when what still needs to land doesn't fit with a
 *  1 GB breathing margin. Shared by the download spread and onboarding. */
export function StorageRow({
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

/** Same 1 GB margin the meter warns with — exported so flows can gate
 *  actions ("confirmar", "começar") on the identical rule. */
export function fitsOnDisk(sizeBytes: number, freeBytes: number | null): boolean {
  if (freeBytes === null) return true; // no reading — don't block on a guess
  return sizeBytes + 1e9 <= freeBytes;
}
