# memory-work-editor

Editor de texto em Rust para **memória de trabalho** — textos efêmeros com auto-save automático, sem necessidade de salvar manualmente.

## Como usar

```bash
cd /algum/diretório
memory-work-editor
```

O editor abre com abas. Todo o conteúdo é salvo automaticamente a cada 2 segundos em `.memory-work-cache/` no **diretório corrente** onde o binário foi executado.

## Atalhos

| Atalho | Ação |
|---|---|
| `Ctrl+T` | Nova aba |
| `Ctrl+W` | Fechar aba atual |
| `Ctrl+Tab` | Próxima aba |
| `Ctrl+Shift+Tab` | Aba anterior |

## Comportamento do cache

- **Fechar uma aba** → arquivo de cache é apagado imediatamente.
- **Fechar o app** → arquivos de cache são mantidos; restaurados na próxima execução.
- **Crash/kill** → cache fica intacto e é restaurado normalmente.

## Formato dos arquivos de cache

```
.memory-work-cache/tab-a3f9c1-20260528-091500.txt
                       ^^^^^^ ^^^^^^^^^^^^^^^^^
                       hash   data/hora de criação
```

## Compilar e executar

```bash
cargo build --release
./target/release/memory-work-editor
```

## .gitignore

O diretório `.memory-work-cache/` já está incluído no `.gitignore` gerado pelo projeto.
