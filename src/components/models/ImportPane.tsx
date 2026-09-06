import { useState } from "react";
import {
  commands,
  type CompatibilityTier,
  type InstalledModel,
  type RemoteGguf,
  type ResolvedSource,
} from "../../lib/tauri/bindings";
import { describeError, unwrap } from "../../lib/tauri/result";
import { useFormat, useT } from "../../lib/i18n";
import { useDownloadsStore } from "../../stores/downloads";
import { displayNameOf, useModelStore } from "../../stores/model";
import { useUiStore } from "../../stores/ui";
import styles from "./ImportPane.module.css";

/** Styling key for the compatibility dot — same palette as the catalog. */
const TIER_KEY: Record<CompatibilityTier, string> = {
  Recommended: "rec",
  Viable: "via",
  Heavy: "hea",
  NotSupported: "not",
};

/** "Bring your own model": a GGUF already on disk (referenced in place) or
 *  one behind a pasted link (Hugging Face repo/file or a direct .gguf URL).
 *  Neither is checksum-verified — the user vouches for what they bring. */
export function ImportPane() {
  const t = useT().importPane;
  return (
    <section className={styles.column}>
      <div className={styles.inner}>
        <div className={styles.head}>
          <div className={styles.kicker}>
            <span className={styles.kickerStep}>{t.kickerStep}</span>
            <span className={styles.kickerSep}>·</span>
            <span>{t.kickerSub}</span>
          </div>
          <h1 className={styles.h1}>
            <span>{t.h1Lead}</span> <span className={styles.h1Quiet}>{t.h1Quiet}</span>
          </h1>
          <p className={styles.gloss}>{t.gloss}</p>
        </div>

        <LocalSection />
        <LinkSection />
      </div>
    </section>
  );
}

function LocalSection() {
  const t = useT().importPane.local;
  const refreshInstalled = useModelStore((s) => s.refreshInstalled);
  const load = useModelStore((s) => s.load);
  const setModelsPane = useUiStore((s) => s.setModelsPane);
  const [busy, setBusy] = useState(false);
  const [imported, setImported] = useState<InstalledModel | null>(null);
  const [error, setError] = useState<string | null>(null);

  const pick = async () => {
    setError(null);
    setImported(null);
    setBusy(true);
    try {
      const path = await unwrap(commands.pickGgufFile());
      if (!path) return;
      const row = await unwrap(commands.importLocalModel(path, null));
      await refreshInstalled();
      setImported(row);
    } catch (e) {
      setError(describeError(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={styles.section}>
      <div className={styles.sectionHead}>
        <span className={styles.roman}>I</span>
        <div>
          <h2 className={styles.sectionTitle}>{t.title}</h2>
          <p className={styles.sectionDesc}>{t.desc}</p>
        </div>
      </div>

      <div className={styles.actionsRow}>
        <button className={`${styles.btn} ${styles.btnPrimary}`} disabled={busy} onClick={() => void pick()}>
          <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path
              d="M2 5.5A1.5 1.5 0 0 1 3.5 4h3l1.5 1.5h4.5A1.5 1.5 0 0 1 14 7v5.5A1.5 1.5 0 0 1 12.5 14h-9A1.5 1.5 0 0 1 2 12.5z"
              stroke="currentColor"
              strokeWidth="1.3"
              strokeLinejoin="round"
            />
          </svg>
          <span>{busy ? t.importing : t.pick}</span>
        </button>
      </div>

      {imported && (
        <div className={styles.notice} role="status">
          <span>
            {imported.chat_template
              ? t.imported(displayNameOf(imported))
              : t.importedNoTemplate(displayNameOf(imported))}
          </span>
          <span className={styles.noticeLinks}>
            {imported.chat_template && (
              <button
                className={styles.link}
                onClick={() => {
                  const id = imported.id;
                  setModelsPane("manager");
                  void load(id, "ritual");
                }}
              >
                {t.awaken}
              </button>
            )}
            <button className={styles.link} onClick={() => setModelsPane("manager")}>
              {t.seeShelf}
            </button>
          </span>
        </div>
      )}

      {error && (
        <div className={`${styles.notice} ${styles.noticeBad}`} role="alert">
          <span>
            <b>{t.failed}</b> — {error}
          </span>
          <button className={styles.link} onClick={() => setError(null)}>
            ok
          </button>
        </div>
      )}
    </div>
  );
}

function LinkSection() {
  const t = useT().importPane.link;
  const tTiers = useT().catalog.tiers;
  const f = useFormat();
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [resolved, setResolved] = useState<ResolvedSource | null>(null);
  const [error, setError] = useState<string | null>(null);
  const installed = useModelStore((s) => s.installed);
  const session = useDownloadsStore((s) => s.session);
  const beginUrl = useDownloadsStore((s) => s.beginUrl);
  const setModelsPane = useUiStore((s) => s.setModelsPane);

  const activePhases = ["starting", "downloading", "verifying"];
  const otherBusy = session != null && activePhases.includes(session.phase);

  const resolve = async () => {
    const value = input.trim();
    if (!value || busy) return;
    setError(null);
    setResolved(null);
    setBusy(true);
    try {
      setResolved(await unwrap(commands.resolveModelSource(value)));
    } catch (e) {
      setError(describeError(e));
    } finally {
      setBusy(false);
    }
  };

  const download = (file: RemoteGguf) => {
    if (!resolved) return;
    beginUrl(file, resolved);
    setModelsPane("download");
  };

  return (
    <div className={styles.section}>
      <div className={styles.sectionHead}>
        <span className={styles.roman}>II</span>
        <div>
          <h2 className={styles.sectionTitle}>{t.title}</h2>
          <p className={styles.sectionDesc}>{t.desc}</p>
        </div>
      </div>

      <form
        className={styles.search}
        onSubmit={(e) => {
          e.preventDefault();
          void resolve();
        }}
      >
        <span className={styles.searchIco} aria-hidden="true">
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
            <path
              d="M6.5 9.5 9.5 6.5M7 4.5l1-1a2.5 2.5 0 0 1 3.5 3.5l-1 1M9 11.5l-1 1A2.5 2.5 0 0 1 4.5 9l1-1"
              stroke="currentColor"
              strokeWidth="1.3"
              strokeLinecap="round"
            />
          </svg>
        </span>
        <input
          type="text"
          placeholder={t.placeholder}
          autoComplete="off"
          spellCheck={false}
          value={input}
          onChange={(e) => setInput(e.target.value)}
        />
        <button type="submit" className={styles.searchBtn} disabled={busy || !input.trim()}>
          {busy ? t.resolving : t.resolve}
        </button>
      </form>

      {error && (
        <div className={`${styles.notice} ${styles.noticeBad}`} role="alert">
          <span>
            <b>{t.failed}</b> — {error}
          </span>
          <button className={styles.link} onClick={() => setError(null)}>
            ok
          </button>
        </div>
      )}

      {resolved && (
        <div className={styles.files}>
          <div className={styles.filesHead}>
            <span>
              {t.filesHead(resolved.files.length)}
              {resolved.repo ? ` · ${t.from(resolved.repo)}` : ""}
            </span>
            <span className={styles.filesHint}>{t.hint}</span>
          </div>
          {resolved.files.map((file) => {
            // Url downloads land in the models dir under their file name.
            const already = installed.some((m) => m.source === "url" && m.filename === file.filename);
            const isThis = session?.target.url === file.url && session.phase !== "completed";
            return (
              <article
                key={file.url}
                className={styles.file}
                data-tier={file.tier ? TIER_KEY[file.tier] : undefined}
                data-split={file.split}
              >
                <span className={styles.compat} />
                <div className={styles.fileBody}>
                  <span className={styles.fileName}>{file.filename}</span>
                  <span className={styles.fileMeta}>
                    {file.split
                      ? t.split
                      : file.tier
                        ? tTiers[file.tier].chip
                        : t.sizeUnknown}
                  </span>
                </div>
                <span className={styles.size}>
                  {file.size_bytes != null ? (
                    <>
                      {f.gb(file.size_bytes, 1)}
                      <span className={styles.sizeSym}>gb</span>
                    </>
                  ) : (
                    "—"
                  )}
                </span>
                {already ? (
                  <button className={styles.act} disabled>
                    {t.installed}
                  </button>
                ) : isThis ? (
                  <button className={styles.act} onClick={() => setModelsPane("download")}>
                    {t.download}
                  </button>
                ) : (
                  <button
                    className={styles.act}
                    disabled={file.split || otherBusy}
                    title={otherBusy ? t.busy : undefined}
                    onClick={() => download(file)}
                  >
                    {t.download}
                    <svg className={styles.actArrow} viewBox="0 0 16 16" fill="none" aria-hidden="true">
                      <path
                        d="M8 3v9M4 8l4 4 4-4"
                        stroke="currentColor"
                        strokeWidth="1.4"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      />
                    </svg>
                  </button>
                )}
              </article>
            );
          })}
        </div>
      )}
    </div>
  );
}
