import { useCallback, useEffect, useState } from "react";
import type { ClassifiedModel } from "../../lib/tauri/bindings";
import { useDownloadsStore } from "../../stores/downloads";
import { useSettingsStore } from "../../stores/settings";
import { useUiStore } from "../../stores/ui";
import { LoadRitual } from "../models/LoadRitual";
import { CheckStep } from "./CheckStep";
import { ChooseStep } from "./ChooseStep";
import { DownloadStep } from "./DownloadStep";
import { WelcomeStep } from "./WelcomeStep";
import styles from "./OnboardingFlow.module.css";

type Step = "welcome" | "check" | "choose" | "download";

const STEP_NUMBER: Record<Step, string> = {
  welcome: "01",
  check: "02",
  choose: "03",
  download: "04",
};

/**
 * First-run ceremony (Fase 6.1): welcome → exame da máquina → escolha do
 * modelo → download guiado → despertar → estúdio. Every step has an escape
 * hatch to the shell; nothing here can trap the user.
 */
export function OnboardingFlow() {
  const [step, setStep] = useState<Step>("welcome");
  const completeOnboarding = useUiStore((s) => s.completeOnboarding);
  const initSettings = useSettingsStore((s) => s.init);
  const beginDownload = useDownloadsStore((s) => s.begin);
  const resetDownload = useDownloadsStore((s) => s.reset);

  // Settings are needed to stamp default_model_id and the completion flag.
  useEffect(() => {
    void initSettings();
  }, [initSettings]);

  const skip = useCallback(() => {
    resetDownload();
    completeOnboarding();
  }, [resetDownload, completeOnboarding]);

  const chooseModel = useCallback(
    (entry: ClassifiedModel) => {
      beginDownload(entry);
      setStep("download");
    },
    [beginDownload],
  );

  return (
    <div className={styles.flow}>
      <span className={`${styles.corner} ${styles.cornerTl}`}>abraxas · v.0</span>
      <span className={`${styles.corner} ${styles.cornerBl}`}>
        passo {STEP_NUMBER[step]} / 04
      </span>
      <span className={`${styles.corner} ${styles.cornerBr}`}>
        <span className="pulse" />
        local · offline
      </span>

      <div className={styles.stepHost} key={step}>
        {step === "welcome" && (
          <WelcomeStep onBegin={() => setStep("check")} onSkip={skip} />
        )}
        {step === "check" && (
          <CheckStep onContinue={() => setStep("choose")} onSkip={skip} />
        )}
        {step === "choose" && (
          <ChooseStep
            onBack={() => setStep("check")}
            onChoose={chooseModel}
            onSkip={skip}
          />
        )}
        {step === "download" && (
          <DownloadStep onChooseAnother={() => setStep("choose")} onSkip={skip} />
        )}
      </div>

      {/* The "despertando o oráculo" overlay mounts here so the ritual can
          run before the shell exists; it self-hides while idle. */}
      <LoadRitual />
    </div>
  );
}
