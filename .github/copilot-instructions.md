# memory-work-editor — Copilot Instructions

Editor de texto em Rust com abas, focado em textos efêmeros com auto-save automático.

## Dependências de sistema

```bash
# GTK4 dev headers (necessário para compilar)
sudo apt-get install -y libgtk-4-dev
```

## Comandos

```bash
# Compilar (debug)
export PATH="/snap/bin:$HOME/.cargo/bin:$PATH"   # usa Rust 1.85+ do rustup/snap
cargo build

# Compilar (release)
cargo build --release

# Executar
cargo run
# ou
./target/debug/memory-work-editor

# Verificar erros sem compilar binário
cargo check

# Linter
cargo clippy

# Formato
cargo fmt
```

> Rust 1.85+ é necessário (dependências usam edition 2024). Use `rustup` via snap se o sistema tiver versão anterior.

## Arquitetura

```
src/
├── main.rs          # Bootstrap gtk4::Application, connect_activate → gtk_app::build_ui
├── gtk_app.rs       # Toda a lógica GTK: estado, widgets, signals, auto-save, atalhos
└── cache/
    ├── mod.rs       # API pública: create_tab_file(), scan_cache(), ensure_cache_dir()
    ├── naming.rs    # Gera nome: tab-<hex6>-<YYYYMMDD-HHMMSS>.txt
    └── writer.rs    # CacheWriter: thread background com mpsc + JoinHandle
```

### Tipos principais (`gtk_app.rs`)

- **`TabEntry`** — dados de cada aba: `id`, `file_path`, `hash`, `text_view`, `tab_label`, `page_widget`, `dirty`
- **`AppState`** — `Vec<TabEntry>` + `Option<CacheWriter>` + `next_id`; compartilhado via `Rc<RefCell<AppState>>`
- Estado é sempre `Rc<RefCell<>>` (single-thread GTK); nunca usar `Arc<Mutex<>>`

### Fluxo principal

1. `build_ui()` cria janela, `gtk4::Notebook` (abas nativas), status bar e restaura sessão
2. `add_tab()` cria `TextView` + `ScrolledWindow` + tab label com botão fechar; conecta `connect_changed` e `connect_clicked`
3. `connect_changed` no `TextBuffer` → marca `dirty = true`, atualiza tab label (`"título*"`) e status bar
4. `glib::timeout_add_local(2s)` → `AppState::flush_dirty()` → envia conteúdo ao `CacheWriter` via channel
5. `CacheWriter` faz escrita atômica em thread separada: `.txt.tmp` → `rename` → `.txt`
6. `connect_close_request` → `save_window_size()` + flush síncrono das tabs sujas + `writer.shutdown()`

## Convenções-chave

**Cache de abas**
- Diretório: `.memory-work-cache/` relativo ao `cwd` no momento de execução
- Formato do nome: `tab-<hex6>-<YYYYMMDD-HHMMSS>.txt` (hex6 = 3 bytes aleatórios em hex)
- Fechar aba → arquivo deletado; fechar app → arquivos mantidos (restaurados no próximo start)
- Tamanho da janela salvo em `.memory-work-cache/window.cfg` (formato `WxH`)

**Thread de escrita**
- `CacheWriter` em `cache/writer.rs` é o único ponto de escrita/deleção assíncrona
- Comandos: `Write { path, content }`, `Delete { path }`, `Shutdown`
- Em `on_exit` (close_request), usar escrita síncrona direta para garantir dados, depois `writer.shutdown()`

**GTK4 / signals**
- Comparar widgets GTK por identidade de objeto: `widget_a.upcast_ref::<Widget>() == widget_b`
- `notebook.page_num(&scrolled_window)` retorna `Option<u32>`
- `notebook.remove_page(Some(u32))` — não `i32`
- Atalhos: `EventControllerKey` com `PropagationPhase::Capture` para interceptar Ctrl+T e Ctrl+W
- `Ctrl+Tab` / `Ctrl+Shift+Tab` é tratado nativamente pelo `Notebook`

**display_title**
- Aba vazia → mostra hash (ex: `a3f9c1`)
- Com conteúdo → primeira linha não-vazia truncada em 22 chars + `…`
- Tab dirty → sufixo `*` (ex: `Hello world*`); removido após auto-save
