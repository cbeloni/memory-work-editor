# memory-work-editor — Copilot Instructions

Editor de texto em Rust com abas, focado em textos efêmeros com auto-save automático.

## Comandos

```bash
# Compilar (debug)
export PATH="/snap/bin:$HOME/.cargo/bin:$PATH"
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
├── main.rs          # Bootstrap eframe::run_native
├── app.rs           # EditorApp: estado global + loop update() + on_exit()
├── tab.rs           # Struct Tab com display_title() dinâmico
├── shortcuts.rs     # Detecta Ctrl+T/W/Tab/Shift+Tab via ctx.input()
├── cache/
│   ├── mod.rs       # API pública: create_tab_file(), scan_cache()
│   ├── naming.rs    # Gera nome: tab-<hex6>-<YYYYMMDD-HHMMSS>.txt
│   └── writer.rs    # CacheWriter: thread background com mpsc + JoinHandle
└── ui/
    ├── tab_bar.rs   # render_tab_bar() → TabBarAction
    └── editor.rs    # render_editor() → bool (changed)
```

### Fluxo principal

- `EditorApp` mantém `Vec<Tab>` + `active_tab: usize` + `Option<CacheWriter>`
- Cada frame (`update()`): verifica timer de 2s → `flush_dirty_tabs()` → envia ao writer thread
- Writer thread faz escrita atômica: `.txt.tmp` → `rename` → `.txt`  
- `on_exit()`: flush síncrono de tabs sujas → `writer.shutdown()` (drena canal + join thread)
- `Tab::dirty` é zerado ao enviar para o writer (otimista; `on_exit` garante consistência)

## Convenções-chave

**Cache de abas**
- Diretório: `.memory-work-cache/` relativo ao `cwd` no momento de execução
- Formato do nome: `tab-<hex6>-<YYYYMMDD-HHMMSS>.txt` (hex6 = 3 bytes aleatórios em hex)
- Fechar aba → arquivo deletado; fechar app → arquivos mantidos (restaurados no próximo start)

**Thread de escrita**
- `CacheWriter` em `src/cache/writer.rs` é o único ponto de escrita/deleção assíncrona
- Comandos: `Write { path, content }`, `Delete { path }`, `Shutdown`
- Nunca escrever diretamente em arquivos de cache fora do `CacheWriter` (exceto `on_exit`)

**UI (egui 0.28)**
- `render_tab_bar()` retorna `TabBarAction` — app.rs decide o que fazer com cada ação
- `render_editor()` retorna `bool` — app.rs seta `tab.dirty = true` se mudou
- Atalhos checados com `ctx.input(|i| ...)` antes da renderização no mesmo frame

**display_title()**
- Aba vazia → mostra o hash (ex: `a3f9c1`)
- Com conteúdo → primeira linha não-vazia truncada em 22 chars + `…`
- Tab dirty → sufixo `*` (ex: `Hello world*`)

**Versões de API**
- egui 0.28: usar `id_source()` (não `id_salt()`) em `ScrollArea`
- `to_string_lossy()` para `PathBuf` → chamar `.as_ref()` antes de passar a `on_hover_text()`
