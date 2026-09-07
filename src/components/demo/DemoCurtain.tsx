import { useEffect } from "react";
import { useUiStore } from "../../stores/ui";
import styles from "./DemoCurtain.module.css";

/**
 * The hold before a take. Only ever mounted by a build made with
 * `VITE_DEMO_MODE=1` (see `lib/demo.ts`): the app sits on the same blank the
 * real boot shows, indefinitely, so the screen recorder can start against a
 * still frame. A plain key press runs the first-run rule and the app enters on
 * camera, animations from frame one.
 *
 * Deliberately not click-to-start. Starting a window recording means clicking
 * the window — to pick it in the ⇧⌘5 picker, and again to give it focus — so a
 * click here fired the take before the recorder was rolling, every time. The
 * keyboard is the only trigger precisely because the mouse belongs to the
 * recorder during setup.
 */
export function DemoCurtain() {
  const startTake = useUiStore((s) => s.startTake);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Let chords through: ⌘⇧R is the reset hotkey, and a stray ⌘ shortcut
      // should not be what starts the take.
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === "Shift" || e.key === "Escape") return;
      e.preventDefault();
      void startTake();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [startTake]);

  return (
    <div className={styles.curtain}>
      <span className={styles.hint}>press any key to begin the take</span>
    </div>
  );
}
