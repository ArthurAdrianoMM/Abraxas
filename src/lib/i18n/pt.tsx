/** Portuguese dictionary. Typed as `Dict` (the shape of `en`), so a key added
 *  to English and forgotten here fails the build instead of rendering blank.
 *
 *  This is the original copy the app was written in; English is the
 *  translation. Where the two diverge in register, this file is the one that
 *  sets the tone. */

import type { Dict } from ".";

export const pt: Dict = {
  onboarding: {
    wordmark: "abraxas · v.0",
    step: (n) => `passo ${n} / 04`,
    offline: "local · offline",

    welcome: {
      invocation:
        "Uma inteligência que vive na sua máquina, fala apenas com você, e não deve nada à nuvem.",
      begin: "começar · examinar a máquina",
      skip: "já conheço a casa — entrar direto →",
    },

    check: {
      rows: {
        ram: "memória ram",
        cpu: "processador",
        gpu: "placa gráfica",
        vram: "memória de vídeo",
        storage: "armazenamento",
      },
      titleLead: "Examinando",
      titleQuiet: "o instrumento.",
      scanning: (what) => `lendo ${what}`,
      scanningIdle: "verificando o que seu computador roda com folga",
      ledger: "— inventário do hardware",
      reading: "lendo",
      waiting: "aguardando",
      retry: "verificar novamente",
      skip: "pular e entrar no estúdio →",
      wait: "aguarde…",

      free: (free, total) => `${free} GB livres de ${total} GB`,
      cores: (brand, cores) => `${brand} · ${cores} núcleos`,
      unified: "unificada com a memória do sistema",
      dedicated: (gb) => `${gb} GB dedicados`,
      shared: "compartilhada com a memória",
      noGpu: "nenhuma gpu dedicada — usaremos o processador",
      diskUnreadable: "não foi possível ler o disco",
      interrupted: "leitura interrompida",

      verdicts: {
        optimal: {
          tier: "resultado · ótimo",
          main: "Seu computador está pronto para a IA local.",
          gloss: (
            <>
              Pode rodar modelos médios e grandes sem suar. Na próxima página, o baralho já vem com
              a <em>escolha recomendada</em> no centro.
            </>
          ),
          cta: "continuar",
        },
        medium: {
          tier: "resultado · adequado",
          main: "Seu computador roda melhor os modelos menores.",
          gloss: (
            <>
              Modelos pequenos e quantizados respondem bem. Os maiores podem ficar lentos — vamos
              sugerir uma boa primeira escolha.
            </>
          ),
          cta: "continuar",
        },
        low: {
          tier: "resultado · limitado",
          main: "Pode ficar lento, mas modelos leves devem funcionar.",
          gloss: (
            <>
              Use modelos quantizados de 1–3B. As respostas serão <em>devagar</em>, no espírito da
              escrita à pena.
            </>
          ),
          cta: "continuar mesmo assim",
        },
        error: {
          tier: "falha · não foi possível ler",
          main: "Não conseguimos completar o exame.",
          gloss: (
            <>
              Algumas leituras falharam. Feche outras aplicações pesadas e tente novamente — costuma
              resolver.
            </>
          ),
          cta: "continuar mesmo assim",
        },
      },
    },

    choose: {
      stepMeta: "a escolha do modelo",
      cached: " · catálogo salvo (offline)",
      loading: "consultando o compêndio de modelos",
      recommended: "recomendado para você",
      pips: ["sol", "lua", "sigilo", "colunas", "torre"],
      compat: {
        Recommended: "recomendado",
        Viable: "roda bem",
        Heavy: "pode ficar lento",
        NotSupported: "não recomendado",
      },
      cardAria: (name, compat) => `${name}, ${compat}`,
      ctx: "ctx",
      ram: "ram",
      diskWarn: (needs, free) =>
        `espaço crítico — este modelo pede ${needs} gb e o disco tem ${free} gb livres. escolha um menor ou libere espaço.`,
      back: "voltar ao exame",
      skip: "pular · entrar sem modelo →",
      previous: "anterior",
      next: "próximo",
      goTo: (name) => `ir para ${name}`,
      tooBig: "este modelo pede uma máquina maior",
      noRoom: "não cabe no disco com folga",
      confirm: "confirmar",

      offline: {
        badge: "i · compêndio inalcançável",
        title: "O compêndio não respondeu.",
        quiet: "sem rede não há catálogo — mas a casa continua de pé.",
        gloss: (reason) => (
          <>
            Não conseguimos baixar a lista de modelos — {reason}. O Abraxas funciona 100% offline{" "}
            <em>depois</em> que um modelo é instalado; este primeiro passo é o único que precisa de
            internet. Conecte-se e tente de novo, ou entre no estúdio e busque o modelo mais tarde
            no ateliê.
          </>
        ),
        reason: "a rede parece indisponível",
        retry: "tentar de novo",
        skip: "entrar no estúdio sem modelo →",
      },
      unsupported: {
        badge: "ii · máquina abaixo do compêndio",
        title: "Nenhum modelo do compêndio cabe nesta máquina.",
        quiet: "preferimos dizer isso agora a prometer o que não roda.",
        gloss: (
          <>
            Todos os modelos do catálogo pedem mais memória do que este computador tem hoje. Você
            ainda pode entrar no estúdio e conferir o compêndio no ateliê — modelos menores são
            adicionados com o tempo.
          </>
        ),
        enter: "entrar no estúdio mesmo assim",
        back: "voltar ao exame",
      },
    },

    download: {
      kicker: "a primeira voz da casa",
      badge: {
        confirm: "preparando",
        starting: "conectando",
        downloading: "descendo",
        verifying: "verificando",
        paused: "pausado",
        completed: "completo · sha-256 íntegro",
      },
      verb: {
        confirm: "um instante — abrindo o canal.",
        starting: "estabelecendo o canal com o repositório.",
        downloading: "o texto está atravessando a rede.",
        verifying: "conferindo o selo: sha-256 byte a byte.",
        paused: "o download está suspenso. retome quando quiser.",
        completed: "o modelo está em casa. despertando…",
      },
      intact: "íntegro",
      of: "de",
      throughput: "vazão",
      remaining: "restam",
      estimating: "estimando",
      underAMinute: "< 1 min",
      pausedShort: "pausado",
      chooseAnother: "escolher outro modelo",
      resume: "retomar",
      pause: "pausar",
      skip: "pular por agora — os bytes baixados ficam salvos →",

      loadFailed: {
        badge: "iii · o despertar falhou",
        title: "O modelo baixou, mas não despertou.",
        quiet: "o arquivo está íntegro no disco — nada se perdeu.",
        gloss: (
          <>
            O download terminou com o selo conferido, mas o carregamento na memória falhou —
            geralmente falta de RAM livre. Feche outras aplicações e tente de novo, ou entre no
            estúdio: o modelo fica instalado e pode ser desperto pelo ateliê.
          </>
        ),
        retry: "tentar despertar de novo",
        enter: "entrar no estúdio →",
      },
      checksumFailed: {
        badge: "vi · selo não confere",
        title: "O selo do arquivo não confere.",
        quiet: "não vou abrir um codex que possa ter sido tocado.",
        gloss: (
          <>
            Os bytes chegaram inteiros, mas a soma <em>sha-256</em> não bate com a publicada pelo
            autor do modelo. O arquivo foi descartado; preferimos não usar.
          </>
        ),
        redownload: "baixar de novo",
      },
      networkFailed: {
        badge: "ii · download interrompido",
        title: "O fio se cortou no meio.",
        quietProgress: (pct) => `${pct}% chegaram. retomamos.`,
        quietNone: "nada se perdeu. tentamos de novo.",
        gloss: (reason) => (
          <>
            A descida foi interrompida — {reason}. Os bytes baixados ficaram salvos; podemos
            continuar de onde paramos sem recomeçar.
          </>
        ),
        reason: "o servidor remoto parou de responder",
        progress: "progresso",
        resumeFrom: (pct) => `retomar de ${pct}%`,
        retry: "tentar de novo",
      },
      chooseOther: "escolher outro modelo",
      skipForNow: "pular por agora",
    },
  },

  storage: {
    critical: (needed) =>
      `espaço crítico — libere pelo menos ${needed} gb ou escolha um modelo menor.`,
    fits: (after) => `cabe com folga · restam ${after} gb depois`,
  },

  shell: {
    switcher: {
      aria: "trocar de modelo",
      kicker: "— qual voz desta vez",
      machine: "sua máq",
      empty: "nenhum codex instalado ainda.",
      loaded: "já carregada",
      switchHint: "trocar descarrega a atual · ",
      switchHintBold: "desperta em instantes",
      now: "agora",
      browseCatalog: "procurar no catálogo",
      remote: "remoto →",
      atelier: "ateliê dos modelos",
      installed: "instalados →",
    },
    awakening: "despertando",
  },


  ritual: {
    aria: "modelo carregando",
    step: "despertar",
    subtitle: "o oráculo sobe à memória",
    ready: "pronto",
    verbLead: "Despertando o oráculo,",
    verbQuiet: "um instante apenas.",
    doneLabor: "concluído",
    loading: "carregando",
    ram: "ram",
    tokens: (n) => `${n} tokens`,
    labors: {
      weights: "abrindo os pesos",
      memory: "alocando a memória",
      context: "semeando o contexto",
      warmup: "aquecendo o forno",
    },
    subtitles: {
      weights: "preparando",
      memory: "alocando",
      context: "semeando",
      warmup: "aquecendo",
    },
  },


  catalog: {
    kickerStep: "o compêndio",
    kickerSub: "catálogo remoto",
    h1Lead: "Os modelos do mundo,",
    h1Quiet: "ainda do outro lado do firmamento.",
    gloss: (
      <>
        Tudo que cabe nesta máquina aparece em <em>recomendado</em> ou <em>viável</em>. Os pesados
        rodam, mas vão arrastar; os não-suportados estão listados por integridade, não para serem
        baixados.
      </>
    ),
    machine: "sua máquina",
    cores: (n) => `${n} núcleos`,
    stale: "o firmamento está fora do alcance — mostrando a última cópia local do compêndio.",
    staleRetry: "tentar de novo",
    search: "procurar por nome, autor, quantização…",
    count: (n) => (n === 1 ? "modelo" : "modelos"),
    all: "todos",
    loading: "consultando o firmamento…",

    tiers: {
      Recommended: {
        name: "Recomendados",
        gloss: "cabem na sua máquina sem suar.",
        chip: "recomendados",
      },
      Viable: {
        name: "Viáveis",
        gloss: "rodam bem; talvez peçam paciência em respostas longas.",
        chip: "viáveis",
      },
      Heavy: {
        name: "Pesados",
        gloss: "rodam, com swap e paciência; mais para ocasiões.",
        chip: "pesados",
      },
      NotSupported: {
        name: "Não suportados",
        gloss: "precisam de máquina maior — listados para você saber que existem.",
        chip: "não suportados",
      },
    },

    row: {
      installed: "já instalado",
      descending: (pct) => `descendo · ${pct}%`,
      resume: "retomar download",
      outOfReach: "fora do alcance",
      busy: "um download por vez — outro modelo já está descendo",
      downloadAnyway: "baixar mesmo assim",
      download: "baixar",
    },

    unreachable: {
      badge: "v · não consegui ler o compêndio",
      title: "O firmamento está fora do alcance.",
      quiet: "e não há cópia local para mostrar.",
      gloss: (
        <>
          O catálogo remoto não respondeu e nenhuma cópia anterior foi guardada nesta máquina. Pode
          ser a rede, pode ser o servidor — confira a conexão e tente de novo.
        </>
      ),
      diagKey: "resposta",
      noConnection: "sem conexão",
      retry: "tentar de novo",
      back: "voltar ao ateliê",
    },
  },


  /** The Models view's download spread. Distinct from `onboarding.download`:
   *  the guided first run and this pane word the same phases differently. */
  downloadPane: {
    badge: {
      confirm: "confirmação",
      starting: "conectando",
      downloading: "descendo",
      verifying: "verificando",
      paused: "pausado",
      completed: "completo · sha-256 íntegro",
    },
    situation: {
      starting: "estabelecendo",
      downloading: "estável",
      verifying: "conferindo selo",
      paused: "pausado",
      completed: "íntegro",
    },
    verb: {
      confirm: "aguardando a sua palavra para começar.",
      starting: "estabelecendo o canal com o repositório.",
      downloading: "o texto está atravessando a rede.",
      verifying: "conferindo o selo: sha-256 byte a byte.",
      paused: "o download está suspenso. retome quando quiser.",
      completed: "o modelo está em casa.",
    },
    change: "trocar →",
    params: "parâmetros",
    size: "tamanho",
    time: "tempo",
    origin: "origem",
    estimatedOnStart: "estimado ao começar",
    estimating: "estimando",
    underAMinute: "< 1 min",
    pausedShort: "pausado",
    resumedAt: "retomado em",
    dismiss: "ok",
    toDescend: "a descer",
    intact: "íntegro",
    of: "de",
    throughput: "vazão",
    remaining: "restam",
    situationLabel: "situação",
    backToCatalog: "voltar ao compêndio",
    begin: "começar",
    pauseFirst: "pause antes de escolher outro modelo",
    returnsToCatalog: "devolve ao compêndio; os bytes baixados ficam salvos",
    chooseAnother: "escolher outro modelo",
    cancel: "cancelar",
    resume: "retomar",
    pause: "pausar",
    awaken: "despertar o modelo",

    checksumFailed: {
      badge: "vi · selo não confere",
      title: "O selo do arquivo não confere.",
      quiet: "não vou abrir um codex que possa ter sido tocado.",
      gloss: (
        <>
          Os bytes chegaram inteiros, mas a soma <em>sha-256</em> não bate com a publicada pelo
          autor do modelo. Pode ter sido corrupção no caminho — pode ter sido troca. O arquivo foi
          descartado; preferimos não usar.
        </>
      ),
      expected: "esperado",
      file: "arquivo",
      discarded: "descartado · não é exposto à conversa",
      redownload: "baixar de novo",
    },
    networkFailed: {
      badge: "ii · download interrompido",
      title: "O fio se cortou no meio.",
      quietProgress: (pct) => `${pct}% chegaram. retomamos.`,
      quietNone: "nada se perdeu. tentamos de novo.",
      gloss: (reason) => (
        <>
          A descida foi interrompida — {reason}. Os bytes baixados ficaram salvos; podemos
          continuar de onde paramos sem recomeçar.
        </>
      ),
      reason: "o servidor remoto parou de responder",
      progress: "progresso",
      link: "link",
      resumeFrom: (pct) => `retomar de ${pct}%`,
      retry: "tentar de novo",
      cancelDownload: "cancelar download",
    },
  },


  manager: {
    kickerStep: "o ateliê",
    kickerSub: "modelos instalados",
    h1Lead: "Os modelos da casa.",
    h1Quiet: "leves, médios, e os que pesam.",
    glossLead: "Cada modelo é um codex carregado do firmamento e guardado neste computador.",
    glossReady: (n) =>
      n === 1
        ? " Um está pronto para conversar; nada sai daqui sem você pedir."
        : ` ${n} estão prontos para conversar; nada sai daqui sem você pedir.`,
    glossEmpty: " A estante ainda está vazia — procure o compêndio para trazer o primeiro.",

    awakeTitle: "voz desperta",
    defaultTag: "padrão",
    defaultTagTitle: "acorda com o app",
    params: "parâmetros",
    quantization: "quantização",
    context: "contexto",
    installedAt: "instalado",
    dismiss: "ok",
    confirmRemove: "remover este codex?",
    remove: "remover",
    keep: "manter",
    awakeNow: "desperta agora",
    awaken: "despertar",
    makeDefaultTitle: "acordar esta voz ao abrir o app",
    isDefault: "já é padrão",
    makeDefault: "tornar padrão",
    openFolder: "abrir pasta",
    removeBlocked: "desperte outra voz antes de remover esta",
    removeLoaded: "esta voz está desperta — desperte outra antes de remover.",

    diskAria: "Espaço em disco",
    consecrated: "consagrados aos modelos",
    freeOnDisk: (gb) => ` · ${gb} GB livres no disco`,
    diskPct: (pct) => `${pct}% do disco`,
    codexCount: (n) => (n === 1 ? "codex" : "codices"),
    entriesHead: (n) =>
      n === 1 ? `— os codices · ${n} instalado` : `— os codices · ${n} instalados`,
    sizeCol: "tamanho",
    empty: "nenhum codex na estante ainda — o compêndio remoto tem o que baixar.",
    browseCatalog: "procurar no catálogo",
    remoteCatalog: "compêndio remoto",

    loadFailed: {
      badge: "i · não carregou",
      title: "O oráculo não acordou.",
      quiet: "o arquivo está aqui, mas recusou.",
      gloss: (which) => (
        <>
          O modelo <em>{which}</em> não pôde ser lido para a memória. O arquivo continua no disco;
          tente de novo ou desperte outra voz.
        </>
      ),
      chosen: "escolhido",
      diagKey: "causa provável",
      retry: "tentar de novo",
      dismiss: "dispensar",
    },
  },


  topbar: {
    backToStudio: "voltar ao estúdio",
    newConversation: "nova conversa",
    switchVoice: "trocar a voz",
    awakening: "despertando…",
    noVoice: "sem voz",
    orders: "ordens desta conversa",
    preferences: "preferências",
    offline: "offline",
    atelier: "ateliê dos modelos",
    localCopy: "cópia local",
    catalogue: "catálogo",
    synced: (when) => ` · sincronizado ${when}`,
    backToCatalog: "voltar ao compêndio",
    modelDownload: "download do modelo",
    models: "modelos",
  },

  sidebar: {
    groups: {
      today: "— hoje",
      week: "— esta semana",
      older: "— mais antigas",
    },
    confirmDelete: "apagar esta conversa?",
    delete: "apagar",
    keep: "manter",
    deleteConversation: "apagar conversa",
    tag: "v.0 · local",
    newConversation: "+ nova conversa",
    conversationsAria: "Conversas",
    shortcutsAria: "Atalhos",
    atelier: "ateliê dos modelos",
    awakening: "despertando…",
    noVoiceAwake: "nenhuma voz desperta",
    local: "local",
    models: "modelos",
    preferences: "preferências",
  },


  chat: {
    attach: "anexar fragmento",
    attachAria: "anexar",
    stop: "parar",
    send: "enviar",
    placeholderFirst: "diga a primeira palavra…",
    placeholder: "pergunte ao abraxas…",
    you: "você",
    assistant: "abraxas",
    typing: "modelo digitando",
    thinking: "— pensando devagar",
    dismiss: "dispensar",
    noVoice: "Nenhuma voz desperta — o Abraxas precisa de um modelo instalado para falar.",
    voiceFailed: (error) => `A voz não pôde despertar: ${error}`,
    goToAtelier: "ir ao ateliê dos modelos →",
    generationFailed: (error) => `o verbo falhou: ${error}`,

    empty: {
      kicker: "i · um silêncio sem palavras",
      titleLead: "Diga a primeira palavra.",
      titleQuiet: "o resto vem.",
      gloss:
        "Abraxas escuta em silêncio até você falar. Pergunte, peça uma leitura, traga um fragmento — ou comece por uma das passagens abaixo.",
      hintSend: "enviar",
      hintNewline: "quebrar linha",
      seeds: [
        {
          roman: "I",
          kind: "leitura",
          label: (
            <>
              o que Hesse quis dizer com <em>“abraxas”</em>?
            </>
          ),
          prompt:
            "Me explique, sem academicismo, o que significa Abraxas em Demian de Hesse.",
        },
        {
          roman: "II",
          kind: "escrita",
          label: <>reescreva como uma nota de margem</>,
          prompt: "Reescreva este parágrafo como se Borges o estivesse anotando na margem.",
        },
        {
          roman: "III",
          kind: "técnica",
          label: <>o que é quantização, em voz baixa</>,
          prompt:
            "Explique de forma honesta o que é quantização de modelos e por que importa para rodar localmente.",
        },
        {
          roman: "IV",
          kind: "conselho",
          label: <>uma rotina contemplativa para hoje</>,
          prompt: "Me dê uma rotina contemplativa de 20 minutos para esta noite.",
        },
      ],
    },

    orders: {
      aria: "ordens desta conversa",
      step: "ordens",
      kickerRest: " · desta conversa",
      close: "fechar",
      heading: "As ordens desta conversa.",
      turns: (n) => (n === 1 ? "1 turno" : `${n} turnos`),
      started: (when) => `iniciada ${when}`,
      sectionName: "i · a medida do verbo",
      sectionGloss: "parâmetros desta conversa. desmarque para herdar das preferências.",
      inherit: (hint) => `herdar do padrão (${hint})`,
      temperature: "temperatura",
      movement: "movimento",
      vsDefault: (value) => ` · padrão ${value}`,
      topP: "top-p",
      amplitude: "amplitude",
      maxTokens: "máximo de tokens",
      tokens: "tokens",
      seed: "semente",
      random: "aleatória",
      seedPlaceholder: "padrão · ex.: 365",
      resetAll: "restaurar tudo ao padrão",
      apply: "aplicar à conversa",
    },
  },


  settings: {
    kickerStep: "preferências",
    kickerSub: "as ordens da casa",
    h1Lead: "As preferências.",
    h1Quiet: "cinco capítulos breves.",

    fontSizes: { compacta: "compacta", comoda: "cômoda", ampla: "ampla" },
    backends: {
      metal: "gpu unificada",
      cuda: "gpu nvidia",
      vulkan: "gpu amd · intel",
      cpu: "processador",
    },

    appearance: {
      name: "A aparência da página",
      gloss: "A casa nasceu em pergaminho noturno; o que se ajusta é a medida da letra.",
      fontSize: "tamanho da letra",
      fontSizeDesc: "para leitura longa.",
      language: "idioma",
      languageDesc: "toda a interface, de uma vez.",
    },

    folder: {
      name: "A pasta dos modelos",
      gloss: "Onde os codices ficam guardados neste computador.",
      path: "caminho",
      pathDesc: "absoluto, no seu sistema.",
      openFolder: "abrir pasta",
      folderMissing: "a pasta ainda não existe — nada foi baixado até aqui.",
      count: (n) => (n === 1 ? "modelo" : "modelos"),
      available: (gb) => ` · disponível: ${gb} gb`,
      integrity: "conferir integridade",
      integrityDesc: "recalcular hashes dos arquivos baixados.",
      checking: "conferindo…",
      checkNow: "conferir agora",
      nothingToCheck: "nada na estante para conferir",
      never: "nunca conferido",
      last: (when, verdict) => `última: ${when} · ${verdict}`,
      intact: "íntegro",
      corrupt: (n) => (n === 1 ? `${n} corrompido` : `${n} corrompidos`),
    },

    generation: {
      name: "A medida do verbo",
      gloss:
        "Os parâmetros que entram em toda conversa nova. Cada conversa pode sobrescrever os seus.",
      temperature: "temperatura",
      temperatureDesc: "quão dilatado o modelo se permite ser.",
      movement: "movimento",
      boundLow: "0 · pedra",
      boundHigh: "2 · febre",
      topP: "top-p",
      topPDesc: "fração de probabilidade considerada por token.",
      amplitude: "amplitude",
      maxTokens: "máximo de tokens",
      maxTokensDesc: "o quanto pode dizer numa só resposta.",
      tokens: "tokens",
      seed: "semente",
      seedDesc: "para reproduzir uma mesma resposta; em branco, usa a semente padrão.",
      seedPlaceholder: "padrão · ex.: 365",
      resetToDefault: "voltar ao padrão",
    },

    instrument: {
      name: "O instrumento",
      gloss: "Se trocou de máquina ou ligou outro periférico, refaça o exame.",
      backend: "backend",
      backendDesc: "como o modelo é executado — escolhido pela casa.",
      examining: "examinando…",
      detected: (cores, gb) => `detectado · ${cores} núcleos · ${gb} gb de memória`,
      redo: "refazer o exame",
      redoDesc: "recalcula o que esta máquina aguenta.",
      examineAgain: "examinar de novo",
      cached: (when) => `exame guardado · ${when}`,
      examined: (when) => `examinado ${when}`,
    },

    about: {
      name: "A casa, em poucas linhas",
      gloss: "Versão, créditos, e as ações irreversíveis que se faz à meia-noite.",
      about: "sobre",
      aboutDesc: "o que está sendo usado.",
      version: "versão",
      runtime: "tempo de execução",
      license: "licença",
      licenseValue: "uso pessoal e contemplativo · MIT",
      quote: "“O pássaro luta para sair do ovo. O ovo é o mundo.”",

      clearConversations: "apagar conversas",
      clearConversationsDesc: "remove o histórico, mantém modelos. não há volta.",
      clearLabel: "apagar todo o histórico",
      clearConfirm: "todo o histórico, para sempre?",
      clearBusy: "apagando…",

      burn: "apagar tudo",
      burnDesc: "conversas, modelos, preferências. a casa vai esquecer.",
      burnLabel: "queimar tudo",
      burnConfirm: "conversas, modelos e preferências — tudo?",
      burnBusy: "queimando…",
      partialClear: (count, files) =>
        `tudo foi esquecido, mas ${count} arquivo(s) estavam em uso e ficaram no disco — feche e reabra o app para removê-los: ${files}`,
    },

    confirmYes: "sim, apagar",
    confirmNo: "manter",
  },

};
