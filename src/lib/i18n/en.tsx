/** English dictionary — the source of truth for the shape of every other
 *  locale. `pt.tsx` is typed as `Dict`, so adding a key here and forgetting it
 *  there is a compile error rather than a string that silently falls back.
 *
 *  Entries are plain strings when the copy is fixed, functions when it
 *  interpolates, and JSX when the design emphasises a word mid-sentence.
 *  Units (gb, mb, min) live in `lib/format`, not here — they read the same in
 *  both locales. */

export const en = {
  onboarding: {
    wordmark: "abraxas · v.0",
    step: (n: string) => `step ${n} / 04`,
    offline: "local · offline",

    welcome: {
      invocation:
        "An intelligence that lives on your machine, speaks only to you, and owes nothing to the cloud.",
      begin: "begin · examine the machine",
      skip: "i already know the house — go straight in →",
    },

    check: {
      rows: {
        ram: "system memory",
        cpu: "processor",
        gpu: "graphics card",
        vram: "video memory",
        storage: "storage",
      },
      titleLead: "Examining",
      titleQuiet: "the instrument.",
      scanning: (what: string) => `reading ${what}`,
      scanningIdle: "checking what your computer can run comfortably",
      ledger: "— hardware inventory",
      reading: "reading",
      waiting: "waiting",
      retry: "examine again",
      skip: "skip and enter the studio →",
      wait: "one moment…",

      free: (free: string, total: string) => `${free} GB free of ${total} GB`,
      cores: (brand: string, cores: number) => `${brand} · ${cores} cores`,
      unified: "unified with system memory",
      dedicated: (gb: string) => `${gb} GB dedicated`,
      shared: "shared with system memory",
      noGpu: "no dedicated gpu — we'll use the processor",
      diskUnreadable: "could not read the disk",
      interrupted: "reading interrupted",

      verdicts: {
        optimal: {
          tier: "verdict · excellent",
          main: "Your computer is ready for local AI.",
          gloss: (
            <>
              It runs medium and large models without breaking a sweat. On the next page the hand
              is already dealt, with the <em>recommended choice</em> at its centre.
            </>
          ),
          cta: "continue",
        },
        medium: {
          tier: "verdict · adequate",
          main: "Your computer is happiest with the smaller models.",
          gloss: (
            <>
              Small, quantised models answer well. The larger ones may drag — we'll suggest a good
              first choice.
            </>
          ),
          cta: "continue",
        },
        low: {
          tier: "verdict · limited",
          main: "It may be slow, but light models should work.",
          gloss: (
            <>
              Use quantised 1–3B models. The answers will come <em>slowly</em>, in the spirit of
              writing by quill.
            </>
          ),
          cta: "continue anyway",
        },
        error: {
          tier: "failure · could not read",
          main: "We couldn't finish the examination.",
          gloss: (
            <>
              Some readings failed. Close other heavy applications and try again — that usually
              settles it.
            </>
          ),
          cta: "continue anyway",
        },
      },
    },

    choose: {
      stepMeta: "the choice of model",
      cached: " · saved catalogue (offline)",
      loading: "consulting the compendium of models",
      recommended: "recommended for you",
      pips: ["sun", "moon", "sigil", "columns", "tower"],
      compat: {
        Recommended: "recommended",
        Viable: "runs well",
        Heavy: "may be slow",
        NotSupported: "not recommended",
      },
      cardAria: (name: string, compat: string) => `${name}, ${compat}`,
      ctx: "ctx",
      ram: "ram",
      diskWarn: (needs: string, free: string) =>
        `critical space — this model asks for ${needs} gb and the disk has ${free} gb free. choose a smaller one or free up space.`,
      back: "back to the examination",
      skip: "skip · enter without a model →",
      previous: "previous",
      next: "next",
      goTo: (name: string) => `go to ${name}`,
      tooBig: "this model asks for a larger machine",
      noRoom: "won't fit on disk with room to spare",
      confirm: "confirm",

      offline: {
        badge: "i · compendium unreachable",
        title: "The compendium did not answer.",
        quiet: "no network, no catalogue — but the house still stands.",
        gloss: (reason: string) => (
          <>
            We couldn't download the list of models — {reason}. Abraxas works fully offline{" "}
            <em>after</em> a model is installed; this first step is the only one that needs the
            internet. Reconnect and try again, or enter the studio and fetch a model later from the
            atelier.
          </>
        ),
        reason: "the network appears to be unavailable",
        retry: "try again",
        skip: "enter the studio without a model →",
      },
      unsupported: {
        badge: "ii · machine below the compendium",
        title: "No model in the compendium fits this machine.",
        quiet: "we'd rather say so now than promise what won't run.",
        gloss: (
          <>
            Every model in the catalogue asks for more memory than this computer currently has. You
            can still enter the studio and browse the compendium in the atelier — smaller models are
            added over time.
          </>
        ),
        enter: "enter the studio anyway",
        back: "back to the examination",
      },
    },

    download: {
      kicker: "the first voice of the house",
      badge: {
        confirm: "preparing",
        starting: "connecting",
        downloading: "descending",
        verifying: "verifying",
        paused: "paused",
        completed: "complete · sha-256 intact",
      },
      verb: {
        confirm: "one moment — opening the channel.",
        starting: "establishing the channel with the repository.",
        downloading: "the text is crossing the network.",
        verifying: "checking the seal: sha-256, byte by byte.",
        paused: "the download is suspended. resume whenever you like.",
        completed: "the model is home. awakening…",
      },
      intact: "intact",
      of: "of",
      throughput: "throughput",
      remaining: "remaining",
      estimating: "estimating",
      underAMinute: "< 1 min",
      pausedShort: "paused",
      chooseAnother: "choose another model",
      resume: "resume",
      pause: "pause",
      skip: "skip for now — the downloaded bytes are kept →",

      loadFailed: {
        badge: "iii · the awakening failed",
        title: "The model downloaded, but did not awaken.",
        quiet: "the file is intact on disk — nothing was lost.",
        gloss: (
          <>
            The download finished with its seal verified, but loading it into memory failed —
            usually a shortage of free RAM. Close other applications and try again, or enter the
            studio: the model stays installed and can be awakened from the atelier.
          </>
        ),
        retry: "try to awaken again",
        enter: "enter the studio →",
      },
      checksumFailed: {
        badge: "vi · seal does not match",
        title: "The file's seal does not match.",
        quiet: "i won't open a codex that may have been touched.",
        gloss: (
          <>
            The bytes arrived whole, but the <em>sha-256</em> sum does not match the one published
            by the model's author. The file was discarded; we'd rather not use it.
          </>
        ),
        redownload: "download again",
      },
      networkFailed: {
        badge: "ii · download interrupted",
        title: "The thread snapped midway.",
        quietProgress: (pct: number) => `${pct}% arrived. we resume.`,
        quietNone: "nothing was lost. we try again.",
        gloss: (reason: string) => (
          <>
            The descent was interrupted — {reason}. The downloaded bytes were saved; we can carry on
            from where we stopped instead of starting over.
          </>
        ),
        reason: "the remote server stopped responding",
        progress: "progress",
        resumeFrom: (pct: number) => `resume from ${pct}%`,
        retry: "try again",
      },
      chooseOther: "choose another model",
      skipForNow: "skip for now",
    },
  },

  storage: {
    critical: (needed: string) =>
      `critical space — free up at least ${needed} gb or choose a smaller model.`,
    fits: (after: string) => `fits with room to spare · ${after} gb left afterwards`,
  },

  shell: {
    switcher: {
      aria: "switch model",
      kicker: "— which voice this time",
      machine: "your machine",
      empty: "no codex installed yet.",
      loaded: "already loaded",
      switchHint: "switching unloads the current one · ",
      switchHintBold: "awakens in moments",
      now: "now",
      browseCatalog: "browse the catalogue",
      remote: "remote →",
      atelier: "atelier of models",
      installed: "installed →",
    },
    awakening: "awakening",
  },


  ritual: {
    aria: "model loading",
    step: "awaken",
    subtitle: "the oracle rises into memory",
    ready: "ready",
    verbLead: "Awakening the oracle,",
    verbQuiet: "just a moment.",
    doneLabor: "done",
    loading: "loading",
    ram: "ram",
    tokens: (n: string) => `${n} tokens`,
    labors: {
      weights: "opening the weights",
      memory: "allocating memory",
      context: "seeding the context",
      warmup: "heating the furnace",
    },
    subtitles: {
      weights: "preparing",
      memory: "allocating",
      context: "seeding",
      warmup: "heating",
    },
  },


  catalog: {
    kickerStep: "the compendium",
    kickerSub: "remote catalogue",
    h1Lead: "The models of the world,",
    h1Quiet: "still on the far side of the firmament.",
    gloss: (
      <>
        Everything that fits this machine appears under <em>recommended</em> or <em>viable</em>.
        The heavy ones run, but they will drag; the unsupported are listed for completeness, not
        to be downloaded.
      </>
    ),
    machine: "your machine",
    cores: (n: number) => `${n} cores`,
    stale: "the firmament is out of reach — showing the last local copy of the compendium.",
    staleRetry: "try again",
    search: "search by name, publisher, quantisation…",
    /** Explicit `string` return: without it TS narrows to the literal union
     *  and no other locale can satisfy the type. */
    count: (n: number): string => (n === 1 ? "model" : "models"),
    all: "all",
    loading: "consulting the firmament…",

    tiers: {
      Recommended: {
        name: "Recommended",
        gloss: "they fit your machine without breaking a sweat.",
        chip: "recommended",
      },
      Viable: {
        name: "Viable",
        gloss: "they run well; long answers may ask for patience.",
        chip: "viable",
      },
      Heavy: {
        name: "Heavy",
        gloss: "they run, with swap and patience; more for special occasions.",
        chip: "heavy",
      },
      NotSupported: {
        name: "Unsupported",
        gloss: "they need a larger machine — listed so you know they exist.",
        chip: "unsupported",
      },
    },

    row: {
      installed: "already installed",
      descending: (pct: string) => `descending · ${pct}%`,
      resume: "resume download",
      outOfReach: "out of reach",
      busy: "one download at a time — another model is already descending",
      downloadAnyway: "download anyway",
      download: "download",
    },

    unreachable: {
      badge: "v · could not read the compendium",
      title: "The firmament is out of reach.",
      quiet: "and there is no local copy to show.",
      gloss: (
        <>
          The remote catalogue did not answer and no earlier copy was kept on this machine. It may
          be the network, it may be the server — check the connection and try again.
        </>
      ),
      diagKey: "response",
      noConnection: "no connection",
      retry: "try again",
      back: "back to the atelier",
    },
  },


  /** The Models view's download spread. Distinct from `onboarding.download`:
   *  the guided first run and this pane word the same phases differently. */
  downloadPane: {
    badge: {
      confirm: "confirmation",
      starting: "connecting",
      downloading: "descending",
      verifying: "verifying",
      paused: "paused",
      completed: "complete · sha-256 intact",
    },
    situation: {
      starting: "establishing",
      downloading: "steady",
      verifying: "checking seal",
      paused: "paused",
      completed: "intact",
    },
    verb: {
      confirm: "awaiting your word to begin.",
      starting: "establishing the channel with the repository.",
      downloading: "the text is crossing the network.",
      verifying: "checking the seal: sha-256, byte by byte.",
      paused: "the download is suspended. resume whenever you like.",
      completed: "the model is home.",
    },
    change: "change →",
    params: "parameters",
    size: "size",
    time: "time",
    origin: "origin",
    estimatedOnStart: "estimated once started",
    estimating: "estimating",
    underAMinute: "< 1 min",
    pausedShort: "paused",
    resumedAt: "resumed at",
    dismiss: "ok",
    toDescend: "to descend",
    intact: "intact",
    of: "of",
    throughput: "throughput",
    remaining: "remaining",
    situationLabel: "situation",
    backToCatalog: "back to the compendium",
    begin: "begin",
    pauseFirst: "pause before choosing another model",
    returnsToCatalog: "returns to the compendium; downloaded bytes are kept",
    chooseAnother: "choose another model",
    cancel: "cancel",
    resume: "resume",
    pause: "pause",
    awaken: "awaken the model",
    completedUnverified: "complete · no seal to check",
    backToImport: "back to your models",
    unknownSize: "size unknown",

    invalidGguf: {
      badge: "vii · not a model",
      title: "What arrived is not a model.",
      quiet: "the link answered, but not with a gguf.",
      gloss: (reason: string) => (
        <>
          The bytes came down whole, but the file does not open as a GGUF — {reason}. The link may
          point at a web page, a pointer file, or a different format. The file was discarded.
        </>
      ),
      link: "link",
      retry: "try again",
      back: "choose another file",
    },

    checksumFailed: {
      badge: "vi · seal does not match",
      title: "The file's seal does not match.",
      quiet: "i won't open a codex that may have been touched.",
      gloss: (
        <>
          The bytes arrived whole, but the <em>sha-256</em> sum does not match the one published by
          the model's author. It may have been corruption in transit — it may have been a swap. The
          file was discarded; we'd rather not use it.
        </>
      ),
      expected: "expected",
      file: "file",
      discarded: "discarded · never exposed to the conversation",
      redownload: "download again",
    },
    networkFailed: {
      badge: "ii · download interrupted",
      title: "The thread snapped midway.",
      quietProgress: (pct: number) => `${pct}% arrived. we resume.`,
      quietNone: "nothing was lost. we try again.",
      gloss: (reason: string) => (
        <>
          The descent was interrupted — {reason}. The downloaded bytes were saved; we can carry on
          from where we stopped instead of starting over.
        </>
      ),
      reason: "the remote server stopped responding",
      progress: "progress",
      link: "link",
      resumeFrom: (pct: number) => `resume from ${pct}%`,
      retry: "try again",
      cancelDownload: "cancel download",
    },
  },


  importPane: {
    kickerStep: "your own",
    kickerSub: "models from outside the compendium",
    h1Lead: "Bring your own model.",
    h1Quiet: "a file you already have, or a link you trust.",
    gloss:
      "Any GGUF works: a file already on this computer, a Hugging Face repository, or a direct link. Nothing is verified against a published checksum — you vouch for what you bring.",

    local: {
      title: "From this computer",
      desc: "The file is used where it is — nothing is copied. Removing it from the shelf later leaves the file untouched.",
      pick: "choose a .gguf file",
      importing: "reading the file…",
      imported: (name: string) => `${name} is on the shelf.`,
      importedNoTemplate: (name: string) =>
        `${name} is on the shelf — choose its chat format before waking it.`,
      awaken: "awaken it",
      seeShelf: "see the shelf",
      failed: "the file could not be brought in",
    },

    link: {
      title: "From a link",
      desc: "Paste a Hugging Face repository, a file inside one, or a direct link to a .gguf.",
      placeholder: "huggingface.co/owner/model  ·  owner/model  ·  https://…/model.gguf",
      resolve: "look",
      resolving: "looking…",
      filesHead: (n: number) => (n === 1 ? "1 file" : `${n} files`),
      from: (repo: string) => `in ${repo}`,
      sizeUnknown: "size unknown",
      split: "split file · not supported",
      download: "download",
      installed: "already installed",
      busy: "one download at a time — another model is already descending",
      failed: "the link did not resolve",
      hint: "Repositories with several quantizations list them all; the lighter ones come first.",
    },
  },
  manager: {
    kickerStep: "the atelier",
    kickerSub: "installed models",
    h1Lead: "The models of the house.",
    h1Quiet: "light, middling, and the ones that weigh.",
    glossLead: "Each model is a codex pulled from the firmament and kept on this computer.",
    glossReady: (n: number) =>
      n === 1
        ? " One is ready to talk; nothing leaves here unless you ask."
        : ` ${n} are ready to talk; nothing leaves here unless you ask.`,
    glossEmpty: " The shelf is still bare — browse the compendium to bring the first one.",

    awakeTitle: "voice awake",
    defaultTag: "default",
    defaultTagTitle: "wakes with the app",
    params: "parameters",
    quantization: "quantisation",
    context: "context",
    installedAt: "installed",
    dismiss: "ok",
    confirmRemove: "remove this codex?",
    remove: "remove",
    keep: "keep",
    awakeNow: "awake now",
    awaken: "awaken",
    makeDefaultTitle: "wake this voice when the app opens",
    isDefault: "already default",
    makeDefault: "make default",
    openFolder: "open folder",
    removeBlocked: "awaken another voice before removing this one",
    removeLoaded: "this voice is awake — awaken another before removing it.",

    diskAria: "Disk space",
    consecrated: "consecrated to models",
    freeOnDisk: (gb: string) => ` · ${gb} GB free on disk`,
    diskPct: (pct: string) => `${pct}% of the disk`,
    codexCount: (n: number) => (n === 1 ? "codex" : "codices"),
    entriesHead: (n: number) =>
      n === 1 ? `— the codices · ${n} installed` : `— the codices · ${n} installed`,
    sizeCol: "size",
    empty: "no codex on the shelf yet — the remote compendium has plenty to download.",
    browseCatalog: "browse the catalogue",
    remoteCatalog: "remote compendium",
    bringYourOwn: "bring your own model",
    bringYourOwnSub: "from disk or a link",
    sourceLocal: "from your computer",
    sourceUrl: "from a link",
    unverified: "unverified",
    unverifiedTitle: "no published checksum to check this file against",
    templateLabel: "chat format",
    templateEmbedded: "as written in the file",
    templateChoose: "choose…",
    templateMissing:
      "this file does not say how it talks — choose a chat format before waking it.",
    templateFamilies: {
      Llama3: "Llama 3",
      Llama2: "Llama 2",
      ChatML: "ChatML",
      Mistral: "Mistral",
      Gemma: "Gemma",
      Gemma4: "Gemma 4",
      Qwen: "Qwen",
      Qwen3: "Qwen 3",
      Phi3: "Phi-3",
      DeepSeek: "DeepSeek",
      CommandR: "Command R",
      GLM4: "GLM-4",
    },
    confirmForget: "forget this codex? the file stays where it is.",
    forget: "forget",

    loadFailed: {
      badge: "i · did not load",
      title: "The oracle did not wake.",
      quiet: "the file is here, but it refused.",
      gloss: (which: string) => (
        <>
          The model <em>{which}</em> could not be read into memory. The file is still on disk; try
          again, or awaken another voice.
        </>
      ),
      chosen: "chosen",
      diagKey: "likely cause",
      retry: "try again",
      dismiss: "dismiss",
    },
  },


  topbar: {
    backToStudio: "back to the studio",
    newConversation: "new conversation",
    switchVoice: "switch the voice",
    awakening: "awakening…",
    noVoice: "no voice",
    orders: "orders for this conversation",
    preferences: "preferences",
    offline: "offline",
    atelier: "atelier of models",
    localCopy: "local copy",
    catalogue: "catalogue",
    synced: (when: string) => ` · synced ${when}`,
    backToCatalog: "back to the compendium",
    modelDownload: "model download",
    models: "models",
    importPane: "bring your own",
    backToImport: "back to your models",
  },

  sidebar: {
    groups: {
      today: "— today",
      week: "— this week",
      older: "— older",
    },
    confirmDelete: "delete this conversation?",
    delete: "delete",
    keep: "keep",
    deleteConversation: "delete conversation",
    tag: "v.0 · local",
    newConversation: "+ new conversation",
    conversationsAria: "Conversations",
    shortcutsAria: "Shortcuts",
    atelier: "atelier of models",
    awakening: "awakening…",
    noVoiceAwake: "no voice awake",
    local: "local",
    models: "models",
    preferences: "preferences",
  },


  chat: {
    attach: "attach a fragment",
    attachAria: "attach",
    stop: "stop",
    send: "send",
    placeholderFirst: "say the first word…",
    placeholder: "ask abraxas…",
    you: "you",
    assistant: "abraxas",
    typing: "model typing",
    thinking: "— thinking slowly",
    dismiss: "dismiss",
    noVoice: "No voice awake — Abraxas needs an installed model to speak.",
    voiceFailed: (error: string) => `The voice could not wake: ${error}`,
    goToAtelier: "go to the atelier of models →",
    generationFailed: (error: string) => `the verb failed: ${error}`,

    empty: {
      kicker: "i · a silence without words",
      titleLead: "Say the first word.",
      titleQuiet: "the rest will come.",
      gloss:
        "Abraxas listens in silence until you speak. Ask, request a reading, bring a fragment — or begin with one of the passages below.",
      hintSend: "send",
      hintNewline: "line break",
      seeds: [
        {
          roman: "I",
          kind: "reading",
          label: (
            <>
              what did Hesse mean by <em>“abraxas”</em>?
            </>
          ),
          prompt:
            "Explain to me, without academic jargon, what Abraxas means in Hesse's Demian.",
        },
        {
          roman: "II",
          kind: "writing",
          label: <>rewrite this as a marginal note</>,
          prompt: "Rewrite this paragraph as if Borges were annotating it in the margin.",
        },
        {
          roman: "III",
          kind: "technical",
          label: <>what quantisation is, quietly</>,
          prompt:
            "Explain honestly what model quantisation is and why it matters for running locally.",
        },
        {
          roman: "IV",
          kind: "counsel",
          label: <>a contemplative routine for today</>,
          prompt: "Give me a 20-minute contemplative routine for tonight.",
        },
      ],
    },

    orders: {
      aria: "orders for this conversation",
      step: "orders",
      kickerRest: " · for this conversation",
      close: "close",
      heading: "The orders of this conversation.",
      turns: (n: number) => (n === 1 ? "1 turn" : `${n} turns`),
      started: (when: string) => `started ${when}`,
      sectionName: "i · the measure of the verb",
      sectionGloss: "parameters for this conversation. uncheck to inherit from preferences.",
      inherit: (hint: string) => `inherit from default (${hint})`,
      temperature: "temperature",
      movement: "movement",
      vsDefault: (value: string) => ` · default ${value}`,
      topP: "top-p",
      amplitude: "amplitude",
      maxTokens: "maximum tokens",
      tokens: "tokens",
      seed: "seed",
      random: "random",
      seedPlaceholder: "default · e.g. 365",
      resetAll: "restore everything to default",
      apply: "apply to conversation",
    },
  },


  settings: {
    kickerStep: "preferences",
    kickerSub: "the orders of the house",
    h1Lead: "The preferences.",
    h1Quiet: "five brief chapters.",

    fontSizes: { compacta: "compact", comoda: "comfortable", ampla: "ample" },
    backends: {
      metal: "unified gpu",
      cuda: "nvidia gpu",
      vulkan: "amd · intel gpu",
      cpu: "processor",
    },

    appearance: {
      name: "The appearance of the page",
      gloss: "The house was born on nocturnal parchment; what adjusts is the measure of the letter.",
      fontSize: "letter size",
      fontSizeDesc: "for long reading.",
      language: "language",
      languageDesc: "the whole interface, at once.",
    },

    folder: {
      name: "The models folder",
      gloss: "Where the codices are kept on this computer.",
      path: "path",
      pathDesc: "absolute, on your system.",
      openFolder: "open folder",
      folderMissing: "the folder does not exist yet — nothing has been downloaded so far.",
      count: (n: number): string => (n === 1 ? "model" : "models"),
      available: (gb: string) => ` · available: ${gb} gb`,
      integrity: "check integrity",
      integrityDesc: "recompute hashes of the downloaded files — models you brought yourself have no seal and are skipped.",
      checking: "checking…",
      checkNow: "check now",
      nothingToCheck: "nothing on the shelf to check",
      never: "never checked",
      last: (when: string, verdict: string) => `last: ${when} · ${verdict}`,
      intact: "intact",
      corrupt: (n: number) => (n === 1 ? `${n} corrupt` : `${n} corrupt`),
    },

    generation: {
      name: "The measure of the verb",
      gloss:
        "The parameters applied to every new conversation. Each conversation may override its own.",
      temperature: "temperature",
      temperatureDesc: "how dilated the model allows itself to be.",
      movement: "movement",
      boundLow: "0 · stone",
      boundHigh: "2 · fever",
      topP: "top-p",
      topPDesc: "fraction of probability considered per token.",
      amplitude: "amplitude",
      maxTokens: "maximum tokens",
      maxTokensDesc: "how much it may say in a single answer.",
      tokens: "tokens",
      seed: "seed",
      seedDesc: "to reproduce the same answer; blank uses the default seed.",
      seedPlaceholder: "default · e.g. 365",
      resetToDefault: "back to default",
    },

    instrument: {
      name: "The instrument",
      gloss: "If you changed machine or plugged in another device, redo the examination.",
      backend: "backend",
      backendDesc: "how the model is run — chosen by the house.",
      examining: "examining…",
      detected: (cores: number, gb: string) => `detected · ${cores} cores · ${gb} gb of memory`,
      redo: "redo the examination",
      redoDesc: "recomputes what this machine can handle.",
      examineAgain: "examine again",
      cached: (when: string) => `cached examination · ${when}`,
      examined: (when: string) => `examined ${when}`,
    },

    about: {
      name: "The house, in a few lines",
      gloss: "Version, credits, and the irreversible acts one performs at midnight.",
      about: "about",
      aboutDesc: "what is being used.",
      version: "version",
      runtime: "runtime",
      license: "license",
      licenseValue: "personal and contemplative use · MIT",
      quote: "“The bird fights its way out of the egg. The egg is the world.”",

      clearConversations: "delete conversations",
      clearConversationsDesc: "removes the history, keeps the models. there is no going back.",
      clearLabel: "delete all history",
      clearConfirm: "all the history, forever?",
      clearBusy: "deleting…",

      burn: "delete everything",
      burnDesc: "conversations, models, preferences. the house will forget.",
      burnLabel: "burn it all",
      burnConfirm: "conversations, models and preferences — everything?",
      burnBusy: "burning…",
      partialClear: (count: number, files: string) =>
        `everything was forgotten, but ${count} file(s) were in use and stayed on disk — close and reopen the app to remove them: ${files}`,
    },

    confirmYes: "yes, delete",
    confirmNo: "keep",
  },

};
