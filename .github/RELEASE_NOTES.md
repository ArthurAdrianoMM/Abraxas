## Instalação

| Sistema | Arquivo |
|---|---|
| Windows 10/11 | `.exe` (recomendado) ou `.msi` |
| macOS (Apple Silicon) | `.dmg` |
| Linux | `.AppImage` (qualquer distro) ou `.deb` (Debian/Ubuntu) |

### Primeira execução

Os instaladores não são assinados digitalmente — assinatura de código é um custo
anual que este projeto pessoal não paga. O sistema vai avisar que o
desenvolvedor é desconhecido. É esperado:

- **Windows** — na tela azul do SmartScreen: *Mais informações* → *Executar assim mesmo*.
- **macOS** — clique com o botão direito no app → *Abrir* → *Abrir*. Ou: *Ajustes do Sistema* → *Privacidade e Segurança* → *Abrir assim mesmo*.
- **Linux** — dê permissão de execução ao AppImage: `chmod +x Abraxas_*.AppImage`.

### O que vem dentro

Um único instalador por sistema, com todos os backends de inferência daquele
sistema embutidos. O app detecta seu hardware na primeira execução e escolhe
sozinho entre Metal, CUDA, Vulkan ou CPU — não há nada pra configurar.

Tudo roda na sua máquina. Nenhuma conversa sai do seu computador.
