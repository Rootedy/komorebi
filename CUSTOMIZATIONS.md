# Alterações Customizadas neste Fork

Este fork (baseado em `LGUG2Z/komorebi` **v0.1.41**, tag `v0.1.41`) contém
alterações comportamentais específicas para uso pessoal. Nenhuma delas é
configurável por `komorebi.json` — foram mudanças diretas no código.

> Compatível com o cenário de uso: 2 monitores (retrato 1080×1920 à esquerda +
> paisagem 1920×1080 à direita), stack de abas no monitor retrato e
> `window_container_behaviour: Append` nesse monitor.

Commit único com todas as alterações até o momento: `c07b674`.

---

## 1. Drag sobre outra janela troca de lugar (em vez de empilhar)

**Arquivo:** `komorebi/src/process_event.rs`

**O que foi alterado:** quando uma janela é arrastada com o mouse e solta sobre
outra janela **no mesmo monitor**, o workspace com
`window_container_behaviour: Append` chamava `move_window_to_container()`,
que empilha a janela arrastada **como aba dentro da janela de baixo do cursor**.
Agora chama `swap_containers()`, igual ao comportamento dos workspaces `Create`.

**Motivo:** no monitor retrato usamos `Append` para que janelas novas virem abas
no stack da área de baixo. Porém isso apresentava um efeito colateral indesejado:
arrastar uma janela sobre outra empilhava em vez de trocar de lugar. O drag
passa pelo mesmo código que decide `Create` vs `Append`, e não havia chave de
configuração separada. Com o patch, janelas **novas** continuam empilhando como
abas (fluxo `Manage`, intocado) e o **arraste** troca de lugar, que é a
expectativa natural de desktop.

## 2. Mover uma aba empilhada para outro monitor leva só a janela focada

**Arquivo:** `komorebi/src/window_manager.rs` (`transfer_window`)

**O que foi alterado:** `transfer_window` (caminho do arraste de mouse entre
monitores) sempre chamava `transfer_container()`, transferindo o **container
inteiro** — todas as abas da pilha viajavam juntas. Agora, quando a janela
focada está num stack com mais de uma aba, ela é dividida (`split`) em um
container próprio antes da transferência, de modo que **somente a aba focada**
atravessa o monitor. Se o split não for possível (container de janela única ou
container de origem não é o focado), o comportamento original de transferir o
container inteiro é mantido como fallback.

**Motivo:** arrastar uma aba para o outro monitor e ver a pilha inteira
desaparecer do monitor original era surpreendente e destrutivo para a sessão
de trabalho. O usuário também usava (via `whkd`) comandos que moviam o
container inteiro, escondendo o problema; o patch corrige o caminho de
arraste, que é o principal ponto de dor.

Observações:
- comandos de teclado `komorebic send/move-to-monitor` continuam sendo comandos
  de **container** por design no upstream e continuam movendo a pilha inteira;
- os patches 2 e 7 se complementam: o comando interno voltado a ações de janela
  única (item 7) reaproveita o split do `transfer_window`.

## 3. Menu de contexto no stackbar

**Arquivo:** `komorebi/src/stackbar_manager/stackbar.rs`

**O que foi alterado:** a aba do stackbar agora aceita **clique direito**
(`WM_RBUTTONUP`), abre um menu nativo Win32 (`CreatePopupMenu` /
`TrackPopupMenuEx` com `TPM_RETURNCMD`) com as opções:

| Item | Ação |
|---|---|
| `Fechar` | fecha a aba clicada via `WindowsApi::close_window()` (nativo, sem depender do foco atual do WM) |
| `Remover da stack` | foca a aba clicada (`focus_tab`) e envia `SocketMessage::UnstackWindow` ao socket após 300 ms (aguardando eventos de foco processarem) |
| `Mover para o outro monitor` | foca a aba clicada e envia `SocketMessage::SendFocusedWindowToNextMonitor` após 300 ms |

A lógica comum de localizar a aba sob o cursor e a de focar a aba
(reposicionar + restaurar + focar, ocultando as demais) foi extraída para os
helpers `tab_index_at()` e `focus_tab()`, reutilizados tanto no clique esquerdo
(`WM_LBUTTONDOWN`, comportamento original preservado) quanto no direito.

**Motivo:** o stackbar é desenhado internamente pelo komorebi e não oferece
nenhuma interação além do clique esquerdo para trocar de aba. As ações mais
comuns sobre uma aba (fechar, tirar da pilha, mandar para o outro monitor)
ficam acessíveis pelo mouse, sem memorizar atalhos de teclado.

## 4. Ícone da aplicação nas abas do stackbar

**Arquivo:** `komorebi/src/stackbar_manager/stackbar.rs`

**O que foi alterado:** cada aba agora desenha o ícone da aplicação (16×16) à
esquerda do texto, via `DrawIconEx`. O ícone é recuperado com
`SendMessageTimeoutW(WM_GETICON)` (small e depois big) com fallback para o
ícone da classe da janela (`GetClassLongPtrW(GCLP_HICON)`). O texto da aba
passou de centrado para alinhado à esquerda, com recuo para depois do ícone.

**Motivo:** identificação visual rápida das abas da pilha sem depender do
título. `SendMessageTimeoutW` com timeout de 100 ms evita travar a renderização
do stackbar caso alguma janela esteja sem resposta.

## 5. Novo comando `SendFocusedWindowToNextMonitor`

**Arquivos:** `komorebi/src/core/mod.rs` (nova variante de `SocketMessage`),
`komorebi/src/window_manager.rs` (`send_focused_window_to_next_monitor`),
`komorebi/src/process_command.rs` (dispatch)

**O que foi alterado:** adicionada uma mensagem de socket que envia **apenas a
janela focada** para o próximo monitor, num comando único e atômico:

1. captura a janela focada do container focado;
2. usa o `transfer_window` já patcheado (item 2), que desempilha a janela do
   stack e a transfere sozinha para o monitor seguinte
   (`(idx + 1) % total`, com wrap);
3. atualiza os dois monitores (evita tiles fantasmas, espelhando o caminho do
   arraste entre monitores) e devolve o foco à janela movida.

**Motivo:** não havia comando de janela (não container) para mover entre
monitores — todos os comandos existentes de monitor (`send/move-to-monitor`,
`cycle-*`) operam no container inteiro. Isso permitiu criar a opção de menu
`Mover para o outro monitor` sem corrida de comandos separados (o foco da aba
clicada era necessário antes de desempilhar, o que não é garantido com
mensagens sequenciais independentes). O destino "próximo monitor" com wrap dá
o mesmo resultado com 2 monitores em qualquer direção.

## 6. Mouse nunca se move automaticamente para o centro das janelas

**Arquivos:** `komorebi/src/lib.rs` (`DEFAULT_MOUSE_FOLLOWS_FOCUS`),
`komorebi/src/window_manager.rs` (default no `WindowManager::new` e caminho de
foco do desktop em `update_focused_workspace`)

**O que foi alterado:** três pontos onde o cursor era recentrado:

1. `WindowManager::new` inicia `mouse_follows_focus: false` em vez de `true`;
2. `DEFAULT_MOUSE_FOLLOWS_FOCUS` (usada como fallback quando a chave está
   ausente no `komorebi.json`) passa de `true` para `false` — este era o
   ponto que **permitia a flag voltar a ligar sozinha** a cada start, pois o
   carregador de configuração estática sobrescreve o valor do construtor
   (`wm.mouse_follows_focus = value.mouse_follows_focus
   .unwrap_or(DEFAULT_MOUSE_FOLLOWS_FOCUS)`);
3. o caminho de foco do **desktop** (quando a última janela de um workspace é
   fechada, por exemplo) recentrava o cursor no centro do monitor
   **incondicionalmente** — o recentro foi removido.

**Motivo:** o usuário considera o movimento automático do cursor um
comportamento hostil (foco por teclado desloca a mão do mouse). Percebeu-se que
as tentativas anteriores de desligar via `komorebic mouse-follows-focus
disable` não sobreviviam a reinícios: o flag não é persistido no `state.json`,
e o carregador de config restaurava `true` a cada start. Com os três pontos,
não existe mais caminho de código que desloque o cursor no processo de foco.

## 7. Memória de geometria por janela (minimizar/restaurar)

**Arquivos:** `komorebi/src/state.rs` (`PersistedWindowGeometry`, campo
`last_window_geometry`), `komorebi/src/window_manager.rs`
(`save_window_geometry_on_cloak`, `apply_saved_window_geometry`),
`komorebi/src/process_event.rs` (hooks em `Cloak`/`Uncloak`/`Unmanage`),
`komorebi/src/static_config.rs` (inicialização do campo).

**O que foi alterado:** janelas tiled que são minimizadas lembram o retângulo
(posição + tamanho) que tinham no momento do minimize, e ao serem restauradas
têm esse retângulo reaplicado — em qualquer monitor.

1. **Captura (`Cloak`):** ao minimizar (que o Windows reporta como
   `ObjectCloaked`), o retângulo é lido do `latest_layout` do workspace (o rect
   do SO já está em -32000 nesse momento) e gravado em
   `last_window_geometry[hwnd]`, junto com `exe` e `title` da janela. Janelas
   flutuantes, maximizadas ou em monocle são ignoradas.
2. **Reaplicação (`Uncloak`):** ao restaurar, depois do tratamento normal do
   uncloak (reconciliação/re-tile), o retângulo salvo é reaplicado com
   `Window::set_position`. Um hwnd reutilizado por acaso é detectado pela
   comparação do nome do processo (`exe`); se não bater, a entrada é descartada.
3. **Prune (`Unmanage`):** janela fechada esquece a geometria memorizada.
4. **Persistência:** o mapa vai dentro do `State` (serializado no
   `%TEMP%\komorebi.state.json` com `komorebic stop`) e volta no `apply_state`,
   então a memória sobrevive a reinícios do próprio komorebi. A anotação
   `#[serde(default)]` mantém compatibilidade com states antigos.

**Verificação empírica:** logs de traçado (`remembering geometry…` /
`reapplied remembered geometry…`) confirmaram captura e reaplicação; o
`state.json` passou a conter o mapa `last_window_geometry` após `komorebic
stop`. (Observação: ao restaurar via Uncloak o komorebi às vezes re-tila a
janela de volta para a stack de abas no monitor retrato — se comportamento
indesejado aparecer no uso real, restringir a reaplicação para o caso em que o
rect atual difere do salvo.)

**Motivo:** janelas minimizadas “esqueciam” tamanho/posição quando o layout era
recomputado enquanto estavam ocultas; o usuário quer que cada janela volte
exatamente de onde saiu, nos dois monitores.

## Notas adicionais que não viraram patch

- `komorebic` não foi reconstruído: o binário oficial do `komorebic.exe` é
  totalmente compatível, pois os novos `SocketMessage` são enviados apenas
  internamente pelo próprio komorebi (item 5) e pelo stackbar.
- Atalho `alt+shift+s` no `~/.config/whkdrc` (`unstack ; cycle-send-to-monitor
  next`) é **unreliable** para janelas em stack, pois usa comandos
  container-based e o foco pós-unstack fica no stack antigo — preferir o item
  de menu `Mover para o outro monitor` (item 3/5).

## Rebuild e atualização futura

```powershell
cd C:\Users\roote\code\komorebi
git fetch upstream
git merge upstream/master      # resolve conflitos nos arquivos patcheados, se houver
cargo build --release -p komorebi
```

Instalar (necessário, pois o binário vive em `Program Files`):

1. `komorebic stop`
2. copiar `target\release\komorebi.exe` para `C:\Program Files\komorebi\bin\komorebi.exe`
   (elevado; o original fica em `komorebi.exe.bak`)
3. `komorebic start --config "$env:USERPROFILE\komorebi.json"`

## Rollback

```
komorebic stop
Copy-Item "C:\Program Files\komorebi\bin\komorebi.exe.bak" "C:\Program Files\komorebi\bin\komorebi.exe" -Force
komorebic start
```
