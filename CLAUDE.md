# CLAUDE.md

Contexto do projeto para consulta durante o desenvolvimento.

---

## 1. Visão geral

### 1.1. O que é

Um aplicativo desktop cross-platform (Windows, macOS, Linux) que permite ao usuário baixar e conversar com modelos de linguagem (LLMs) rodando 100% localmente em sua própria máquina. Sem servidor centralizado, sem envio de dados pra nuvem, sem API keys externas.

### 1.2. Intenção e filosofia

- **Privacidade por design**: tudo roda offline depois do primeiro setup. Nenhuma conversa sai da máquina do usuário.
- **Autonomia**: o usuário escolhe modelos sem censura corporativa excessiva (modelos abliterados, Dolphin, Hermes, etc. são cidadãos de primeira classe no catálogo).
- **Zero fricção**: usuário casual não-técnico clica no instalador, abre o app, escolhe um modelo recomendado pelo hardware dele, e conversa. Em nenhum momento ele precisa saber o que é CUDA, quantização, Python, Rust ou terminal.
- **Feito direito, não improvisado**: o projeto prioriza arquitetura sólida e decisões técnicas profundas em vez de atalhos. Projeto pessoal sem fins comerciais cujo propósito é servir como item forte de portfólio e oportunidade de aprendizado profundo em Rust.

### 1.3. O que não é

- Não é um wrapper de Ollama/LM Studio. A engine de inferência é embutida no app, não depende de serviços externos.
- Não é um framework de agentes / RAG / tools. Escopo inicial: chat simples com modelo local.
- Não é multi-usuário. É um app desktop pessoal, single-user.
- Não tem fins comerciais. Sem telemetria invasiva, sem paywall, sem auth. Licença open source (MIT ou Apache-2.0).

### 1.4. Usuário-alvo

Pessoa não-técnica que ouviu falar de "IA local" e quer testar sem complicação. Ela não sabe o que é quantização ou backend de inferência. Ela sabe clicar em "baixar" e digitar num chat. Toda decisão de UX e default é tomada com essa persona em mente.

---

## 2. Stack técnico

### 2.1. Core

- **Tauri v2** (estável) como framework de app desktop. Não v1.
- **Rust** no backend/core do app.
- **React + TypeScript + Vite** no frontend.
- **SQLite** via `sqlx` como persistência local.

### 2.2. Rust crates principais

- `tauri` (v2) — framework
- `tauri-specta` ou `specta` — geração de tipos TS a partir de commands Rust
- `tokio` — runtime async
- `serde` + `serde_json` — serialização
- `sqlx` — SQLite async + migrations via `sqlx-cli`
- `tracing` + `tracing-subscriber` — logging estruturado
- `reqwest` — HTTP client (download de modelos e catálogo)
- `sysinfo` — detecção de RAM/CPU
- `raw-cpuid` — detecção de features de CPU (AVX2, AVX-512)
- `nvml-wrapper` — detecção de NVIDIA GPUs (opcional/condicional)
- `ash` ou `vulkano` — enumeração de devices Vulkan
- `llama-cpp-2` — bindings seguros pro llama.cpp
- `sha2` — checksum de modelos baixados
- `thiserror` — tipos de erro ergonômicos

### 2.3. Frontend libs

- `react` + `react-dom`
- `@tauri-apps/api` — bridge com Rust
- `zustand` — state management
- `@tanstack/react-query` — cache/fetch de dados vindos do Rust
- `tailwindcss` + `shadcn/ui` — styling e componentes
- `react-markdown` + `remark-gfm` — render de markdown nas mensagens

### 2.4. Backends de inferência

Estratégia deliberadamente enxuta pra reduzir matriz de build:

| OS | Backend primário | Fallback |
|----|------------------|----------|
| macOS | Metal (sempre) | — |
| Windows | Vulkan | CPU |
| Linux | Vulkan | CPU |
| Qualquer com NVIDIA | CUDA (variante opcional de performance) | volta pra Vulkan |

Vulkan foi escolhido como primário no Windows/Linux porque cobre NVIDIA, AMD e Intel com uma só variante de build, fica 5-15% abaixo de CUDA em performance (aceitável), e reduz dramaticamente complexidade de distribuição vs. manter CUDA + ROCm + SYCL separados. CUDA é tratado como variante premium opcional pra usuário NVIDIA que quer os 10-15% extras.

### 2.5. Build e distribuição

- **GitHub Actions** com matrix `[windows-latest, macos-latest, ubuntu-latest]` pra CI desde o dia 1.
- **Tauri Bundler** gera `.dmg` (macOS), `.msi` + `.exe` (Windows), `.AppImage` + `.deb` (Linux).
- Sem assinatura de código paga no MVP. Instaladores vão exigir bypass manual de Gatekeeper/SmartScreen na primeira execução.

---

## 3. Decisões arquiteturais e justificativas

### 3.1. Por que Tauri + Rust, não Electron + Node nem Electron + Python sidecar

Considerados e rejeitados:

- **Django sidecar**: over-engineering. Django é framework web multi-usuário com ORM e admin. Zero disso se aplica a um app desktop local.
- **FastAPI sidecar com PyInstaller**: viável, mas adiciona duas linguagens, dois runtimes, IPC entre processos, e uma segunda fronteira de erros pra debugar.
- **Electron + Node + node-llama-cpp**: opção mais pragmática. Rejeitada porque o dev priorizou profundidade técnica e aprendizado de Rust sobre velocidade de entrega. Escolha consciente.
- **Tauri + Rust (escolhido)**: single-runtime, bundle pequeno (~15MB vs ~150MB de Electron), performance superior, aprendizado valioso, alinhado com intenção de "feito direito".

### 3.2. Por que Vulkan em vez de matriz CUDA+ROCm+SYCL

Ver tabela em 2.4. Trade-off de ~10% de performance por uma fração do custo de engenharia. Usuário casual não percebe diferença; dev economiza meses.

### 3.3. Por que um único modelo carregado por vez

Manter múltiplos modelos carregados simultaneamente leva a OOM em máquina de usuário casual. Trocar de modelo = descarregar anterior + carregar novo. Simplifica gerenciamento de memória em ordem de magnitude.

### 3.4. Por que catálogo remoto em JSON, não hardcoded

Catálogo hospedado em GitHub Pages permite adicionar/remover/atualizar modelos sem fazer release do app. Trivial de implementar com `reqwest` + `serde`. Cache local do último catálogo bem-sucedido pra funcionar offline.

### 3.5. Por que SQLite com migrations desde o dia 1

Schema vai mudar durante o desenvolvimento. Sem migrations (via `sqlx` migrate ou `refinery`), cada mudança destrói dados de teste e exige intervenção manual. O custo de configurar no início é baixo.

### 3.6. Por que abstração de backend de inferência (trait `InferenceBackend`)

Mesmo com só uma implementação hoje (`llama-cpp-2`), a abstração permite trocar por `mistral.rs`, `candle`, ou outra engine no futuro sem reescrever tudo. É desacoplamento, não premature abstraction — a trait é pequena e bem definida.

---

## 4. Estrutura do projeto

```
app/
├── src/                      # React frontend (TypeScript)
│   ├── components/           # Componentes reutilizáveis (shadcn/ui-based)
│   ├── routes/               # Telas principais (onboarding, chat, settings)
│   ├── stores/               # Zustand stores (global state)
│   ├── hooks/                # React hooks customizados
│   ├── lib/
│   │   ├── tauri/            # Wrappers tipados pros commands Rust
│   │   └── utils.ts
│   └── main.tsx              # Entry point React
├── src-tauri/                # Rust (convenção Tauri)
│   ├── src/
│   │   ├── main.rs           # Entry point, setup do app Tauri
│   │   ├── lib.rs            # Root do crate
│   │   ├── commands/         # Tauri commands (API exposta ao frontend)
│   │   │   ├── mod.rs
│   │   │   ├── hardware.rs
│   │   │   ├── models.rs
│   │   │   ├── chat.rs
│   │   │   └── settings.rs
│   │   ├── inference/        # Camada de inferência
│   │   │   ├── mod.rs
│   │   │   ├── backend.rs    # trait InferenceBackend
│   │   │   ├── llama_cpp.rs  # impl concreta
│   │   │   └── manager.rs    # gerencia ciclo de vida do modelo carregado
│   │   ├── hardware/         # Detecção de hardware
│   │   │   ├── mod.rs
│   │   │   ├── system.rs     # CPU/RAM via sysinfo
│   │   │   ├── gpu.rs        # detecção Metal/CUDA/Vulkan
│   │   │   └── selector.rs   # lógica de escolha de backend
│   │   ├── models/           # Gerenciamento de modelos
│   │   │   ├── mod.rs
│   │   │   ├── catalog.rs    # fetch + parse do catálogo remoto
│   │   │   ├── download.rs   # download com resume + checksum
│   │   │   └── registry.rs   # modelos instalados no disco
│   │   ├── chat/             # Lógica de conversa
│   │   │   ├── mod.rs
│   │   │   ├── templates.rs  # chat templates por família de modelo
│   │   │   ├── context.rs    # gerenciamento de janela de contexto
│   │   │   └── generation.rs # params de geração (temperature, etc.)
│   │   ├── db/               # Persistência
│   │   │   ├── mod.rs
│   │   │   ├── conversations.rs
│   │   │   ├── messages.rs
│   │   │   └── settings.rs
│   │   ├── events/           # Definições de eventos emitidos pro frontend
│   │   │   └── mod.rs
│   │   └── error.rs          # Tipos de erro do app (thiserror)
│   ├── migrations/           # sqlx migrations
│   ├── Cargo.toml
│   └── tauri.conf.json
├── model-catalog/            # JSON do catálogo (publicado separadamente em GitHub Pages)
│   └── catalog.json
├── docs/
│   └── decisions/            # Architecture Decision Records (ADRs)
├── .github/workflows/        # CI/CD
├── CLAUDE.md                 # Este arquivo
└── README.md
```

---

## 5. Roadmap de features

Organizado em 6 fases. Dentro de uma fase, features são razoavelmente independentes; entre fases, há dependências que devem ser respeitadas.

### Fase 0 — Preparação (não-código)

Aprendizado de fundamentos de Rust e primeiro contato com Tauri.

- 0.1. Fundamentos mínimos de Rust (Rustlings ou primeiros capítulos do Rust Book): ownership, borrowing, Result/Option, traits, structs, enums, async básico.
- 0.2. "Hello Tauri": tutorial oficial do Tauri v2 até um app que abre uma janela React e chama um command Rust.
- 0.3. Setup do repo no GitHub com `.gitignore`, README inicial, licença.

### Fase 1 — Fundação técnica

- 1.1. Monorepo estruturado conforme seção 4.
- 1.2. CI/CD multi-OS (GitHub Actions, matrix de 3 OS rodando `cargo check`, `cargo clippy`, `cargo test`, build do frontend).
- 1.3. Sistema de logging (`tracing` com logs escrevendo em arquivo no diretório de dados do usuário).
- 1.4. Setup de SQLite com `sqlx` + `sqlx-cli` pra migrations + primeira migration vazia.
- 1.5. Contrato inicial de commands tipados via `tauri-specta` (tipos TS gerados automaticamente a partir do Rust).

**Critério de pronto**: app abre nos 3 OS, tem log funcional, tem DB criado no diretório correto por OS, React chama command Rust tipado e retorna dado correto.

### Fase 2 — Detecção de hardware

- 2.1. Detecção de SO, CPU (cores, features AVX2/AVX-512) e RAM via `sysinfo` + `raw-cpuid`.
- 2.2. Detecção de GPU: Metal assumido em macOS via `cfg!(target_os)`; NVIDIA via `nvml-wrapper`; Vulkan via `ash`/`vulkano` enumerando devices físicos. Retorna enum `GpuBackend { Metal, Cuda { vram_mb }, Vulkan { vendor }, None }`.
- 2.3. Lógica pura de seleção de backend com ordem de prioridade: Metal > CUDA (se NVIDIA + driver recente) > Vulkan > CPU. Testes unitários cobrindo combinações.
- 2.4. Cache de detecção em config local com hash de fingerprint de hardware pra re-detectar automaticamente se mudar.

**Critério de pronto**: command `detect_hardware()` retorna estrutura serializável com backend escolhido e justificativa; segunda execução é instantânea via cache.

### Fase 3 — Motor de inferência

- 3.1. Integração com `llama-cpp-2`: carregar um modelo hardcoded pequeno (ex. TinyLlama 1.1B Q4) e gerar tokens via teste de linha de comando.
- 3.2. Trait `InferenceBackend` com métodos `load_model`, `unload`, `generate_stream` + impl concreta usando `llama-cpp-2`.
- 3.3. Gerenciador de ciclo de vida do modelo: um modelo por vez, descarrega anterior antes de carregar novo, gerencia memória via `Arc<Mutex<Option<Model>>>` ou padrão similar.
- 3.4. Build cross-platform com features condicionais de Cargo: `metal` em macOS, `vulkan` em Windows/Linux, `cuda` opcional.
- 3.5. Streaming de tokens via Tauri Events (Rust emite cada token, React escuta e renderiza) + cancelamento.

**Critério de pronto**: de uma tela temporária no frontend, usuário dispara geração e vê tokens aparecendo conforme são gerados, com possibilidade de cancelar.

### Fase 4 — Gerenciador de modelos

- 4.1. Catálogo remoto em JSON hospedado em GitHub Pages, contendo URL do Hugging Face, SHA256, tamanho, RAM mínima, descrição. Rust baixa e parseia via `reqwest` + `serde`.
- 4.2. Filtragem por compatibilidade com hardware detectado: cada modelo marcado como `Recommended`, `Viable`, `Heavy` ou `NotSupported` baseado em RAM/VRAM disponível.
- 4.3. Download com resume via range requests HTTP. Escreve em arquivo temporário (`.gguf.part`), renomeia ao terminar. Emite progresso via eventos.
- 4.4. Verificação de integridade: SHA256 calculado pós-download, comparado com o do catálogo. Arquivo deletado se não bater.
- 4.5. Gerenciamento de modelos instalados: listar, ver tamanhos, deletar.

**Critério de pronto**: usuário escolhe modelo no frontend, baixa com progresso visível, arquivo é verificado e fica pronto pra carregar. Download sobrevive a queda de conexão e fechamento do app.

### Fase 5 — Chat completo

- 5.1. Chat templates por família de modelo (Llama 3, ChatML, Mistral, etc.). Cada entrada do catálogo indica qual template usar.
- 5.2. Persistência de conversas e mensagens no SQLite via `sqlx`. Tabelas `conversations` e `messages`. Commands: criar/listar/deletar conversa, listar mensagens.
- 5.3. Gerenciamento de janela de contexto: quando conversa excede o limite do modelo, truncar mensagens antigas mantendo system prompt.
- 5.4. Parâmetros de geração configuráveis por conversa: temperature, top_p, max_tokens, seed.
- 5.5. UI completa do chat: lista lateral de conversas, área de mensagens, input, indicador de "gerando", botão stop, markdown rendering com `react-markdown`.

**Critério de pronto**: conversas persistem entre sessões, trocar de modelo funciona sem quebrar conversas antigas, UI fluida com respostas bem formatadas.

### Fase 6 — Polish e release

- 6.1. Onboarding da primeira execução: boas-vindas, detecção de hardware animada, sugestão de modelo recomendado, download guiado, primeira conversa.
- 6.2. Tela de settings: tema, diretório de modelos, parâmetros default, re-detectar hardware, limpar dados.
- 6.3. Estados de erro com mensagens claras e ações sugeridas pro usuário.
- 6.4. Instaladores por OS via Tauri Bundler + GitHub Releases automático por tag.
- 6.5. README visual com screenshots/GIF + ADRs + instruções de build + roadmap público.
- 6.6. Release v0.1.0.

**Critério de pronto**: pessoa não-técnica baixa do GitHub Releases, instala, e consegue do zero ao primeiro token sem instrução externa.

---

## 6. Fluxo do usuário

### 6.1. Primeira execução

1. Usuário abre o app instalado.
2. App detecta hardware em background (rápido, < 2s). Tela de boas-vindas mostra animação/progresso.
3. App mostra catálogo de modelos filtrado/ordenado por compatibilidade. Modelo recomendado destacado.
4. Usuário escolhe modelo. App mostra resumo ("vamos baixar X GB, pode demorar Y minutos").
5. Download inicia com progresso visível e estimativa de tempo.
6. Ao terminar, modelo é verificado (checksum) e carregado na memória.
7. Usuário cai na tela de chat, pronto pra conversar.

### 6.2. Execuções subsequentes

1. App abre na tela de chat, última conversa aberta.
2. Modelo da última sessão é recarregado automaticamente (configurável).
3. Usuário pode trocar modelo, criar nova conversa, ajustar settings.

---

## 7. Fora de escopo do MVP

Essas coisas não fazem parte do MVP:

- Sistema de plugins / extensões
- RAG (Retrieval-Augmented Generation)
- Agentes / tool use / function calling
- Suporte a múltiplos modelos carregados simultaneamente
- Voice I/O (speech-to-text, text-to-speech)
- Geração de imagens ou outros modelos além de LLM de texto
- Sync entre dispositivos / conta de usuário
- Compartilhamento de conversas
- Servidor HTTP local expondo API compatível com OpenAI
- Assinatura de código paga
- Telemetria / analytics
- Auto-update (pode entrar pós-v0.1)

---

## 8. Referências úteis

Fontes oficiais e atualizadas pra consulta durante o desenvolvimento:

- Tauri v2: https://v2.tauri.app/
- `llama-cpp-2` crate: https://docs.rs/llama-cpp-2 (+ repositório no GitHub pros exemplos)
- `sqlx`: https://docs.rs/sqlx
- `tauri-specta`: https://github.com/specta-rs/tauri-specta
- The Rust Book: https://doc.rust-lang.org/book/
- Hugging Face (catálogo de modelos GGUF): https://huggingface.co/models?library=gguf
- shadcn/ui: https://ui.shadcn.com/
- TanStack Query: https://tanstack.com/query/latest
- Zustand: https://zustand-demo.pmnd.rs/
- `tracing`: https://docs.rs/tracing
