# memory-work-editor — Planejamento

Editor de texto simples em Rust, focado em **memória de trabalho**: textos voláteis, com auto-save em cache, sem precisar salvar manualmente.

---

## 1. Visão geral

- **Nome:** `memory-work-editor`
- **Linguagem:** Rust (edição 2021)
- **GUI:** [`egui`](https://github.com/emilk/egui) via `eframe` (multiplataforma, modo imediato, simples de empacotar)
- **Paradigma:** sempre baseado em **abas**. Não existe "abrir/salvar arquivo" no fluxo normal — cada aba é um buffer respaldado por um arquivo de cache.

---

## 2. Requisitos funcionais

### 2.1. Abas
- Sempre existe pelo menos 1 aba aberta (ao iniciar sem cache, cria uma vazia).
- Ações: **nova aba**, **fechar aba**, **renomear título da aba** (opcional, default = nome do arquivo de cache).
- Navegação por clique e atalhos:
  - `Ctrl+T` → nova aba
  - `Ctrl+W` → fechar aba atual
  - `Ctrl+Tab` / `Ctrl+Shift+Tab` → próxima/anterior

### 2.2. Auto-save
- Auto-save em **intervalo fixo de 2 segundos**, apenas para abas marcadas como `dirty`.
- Save é atômico: escrever em `tab-<hash>-<data>.txt.tmp` → `rename` para o nome final.
- Nenhum diálogo de "salvar" exposto ao usuário.

### 2.3. Nomenclatura de arquivos de cache
- Formato: `tab-<hash6>-<YYYYMMDD-HHMMSS>.txt`
  - `<hash6>` — 6 primeiros caracteres de um hash aleatório (ex: `blake3` de um UUID v4, ou base32 de 4 bytes randômicos).
  - `<YYYYMMDD-HHMMSS>` — data/hora de criação da aba (timezone local).
- Exemplo: `tab-a3f9c1-20260528-091500.txt`

### 2.4. Ciclo de vida do arquivo de cache
| Evento | Ação |
|---|---|
| Criar aba | Cria arquivo vazio em `.memory-work-cache/` |
| Editar conteúdo | Marca aba como `dirty`; flush a cada 2s |
| Fechar aba | **Remove o arquivo** do disco imediatamente |
| Fechar app | **Mantém** todos os arquivos; restaura no próximo `start` |
| Crash / kill | Cache fica intacto → restaurado normalmente |

### 2.5. Diretório de cache
- Localização: `./.memory-work-cache/` (oculto, relativo ao `cwd` da aplicação no momento da execução).
- Criado on-demand se não existir.
- Não é versionado (sugerir entrada `.gitignore` ao usuário no README).

### 2.6. Restauração de sessão
- No start, varrer `.memory-work-cache/*.txt`.
- Cada arquivo vira uma aba; ordenar por timestamp do nome (mais antigo primeiro).
- Se o diretório estiver vazio/ausente, abrir uma única aba nova.

---

## 3. Requisitos não-funcionais

- **Performance:** abrir/digitar deve ser instantâneo para arquivos < 1 MB.
- **Segurança/UX:** auto-save **não deve travar a UI** — escrita em thread separada via canal (`std::sync::mpsc` ou `crossbeam`).
- **Multiplataforma:** Linux (alvo principal), Windows e macOS (gratuitos via egui).
- **Sem dependências de rede.**

---

## 4. Arquitetura

```
src/
├── main.rs              # bootstrap eframe, App::new()
├── app.rs               # struct EditorApp (impl eframe::App): estado global, loop UI
├── tab.rs               # struct Tab { id, title, content, file_path, dirty, created_at }
├── cache/
│   ├── mod.rs           # API pública: scan(), create_tab_file(), delete(), write()
│   ├── naming.rs        # geração de hash + timestamp
│   └── writer.rs        # worker thread de escrita atômica
├── ui/
│   ├── mod.rs
│   ├── tab_bar.rs       # barra de abas (egui)
│   └── editor.rs        # área de texto (TextEdit::multiline)
└── shortcuts.rs         # mapeamento de atalhos
```

### Fluxo de dados
1. `EditorApp` mantém `Vec<Tab>` + `active_tab: usize`.
2. UI renderiza a aba ativa; mudanças no `TextEdit` mutam `Tab.content` e setam `dirty = true`.
3. Timer (cada `update()` do egui verifica `Instant::now() - last_flush`) → quando ≥ 2s, envia snapshot `(file_path, content)` para canal do writer thread.
4. Writer thread escreve atomicamente e zera flag `dirty` via `Arc<AtomicBool>` por aba.
5. Ao fechar aba: remove do `Vec`, manda comando `Delete(path)` para o writer.

---

## 5. Dependências (Cargo.toml previsto)

```toml
[package]
name = "memory-work-editor"
version = "0.1.0"
edition = "2021"

[dependencies]
eframe = "0.28"            # egui + framework de janela
egui = "0.28"
chrono = "0.4"             # timestamps formatados
rand = "0.8"               # bytes aleatórios para o hash
data-encoding = "2"        # base32 para o hash legível
anyhow = "1"               # erros ergonômicos
log = "0.4"
env_logger = "0.11"
```

---

## 6. Roadmap de implementação (fases / todos)

| # | Fase | Entrega |
|---|---|---|
| 1 | Setup do projeto | `cargo new`, dependências, "hello egui" rodando |
| 2 | Estrutura `Tab` + UI mínima | 1 aba estática editável, sem persistência |
| 3 | Cache: criação/escrita | Criar `.memory-work-cache/`, gerar nome, escrever síncrono |
| 4 | Tab bar funcional | Múltiplas abas, criar/fechar/trocar |
| 5 | Auto-save em background | Worker thread + intervalo de 2s |
| 6 | Restauração de sessão | Scan ao iniciar, popular abas |
| 7 | Deleção ao fechar aba | Remover arquivo do cache |
| 8 | Atalhos de teclado | Ctrl+T, Ctrl+W, Ctrl+Tab |
| 9 | Polimento UX | Indicador "salvando", título com `*` quando dirty |
| 10 | README + .gitignore | Documentar uso e convenções |

---

## 7. Pontos abertos / decisões futuras

- Suporte a busca dentro da aba (`Ctrl+F`)?
- Tema claro/escuro? (egui já oferece toggle gratuito)
- Limite de tamanho de arquivo no cache?
- Exportar manualmente o conteúdo para um caminho do usuário (`Ctrl+Shift+S`)?
