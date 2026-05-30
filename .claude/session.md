# Session context

## Последняя сессия (2026-05-30) — Аудит + P0 (в master) + P1 (на ветке)

Полный отчёт аудита: `~/.claude/plans/sequential-dreaming-zebra.md`.
P0 (D1 экранирование атрибутов, D2 паника ifeval) закоммичен и **влит в master** (`51a650e`, запушен).

## P1 — надёжность: ветка `fix/p1-robustness` (от master; НЕ закоммичено)

- **D3** — устранена неограниченная рекурсия `scan_next_block` (`adoc-parser/src/block.rs`).
  Приём: трамплин. Тело переименовано в `scan_next_block_once`; добавлено поле
  `rescan_requested: bool`; 3 внутренних хвостовых `return self.scan_next_block()`
  (`[attr]`, `.title`, comment) заменены на `self.rescan_requested = true; return None;`.
  Новый тонкий `scan_next_block` крутит `loop`, перезапуская `_once` пока стоит флаг →
  O(1) стек. Внешние вызовы остались на обёртке. **НЕ переотступал тело** (файл не fmt-clean).
  +2 стресс-теста в `tests/integration.rs` (50k `[attr]` / 50k `.title`).
- **D4** — три `unreachable!()` в `block.rs` → мягкая деградация: `TableFormat::Native` в
  delimiter-парсере → `continue`; неизвестный block-style → обычный параграф (зеркало ветки
  `else`); лишний контекст в DL-cleanup → `_ => {}`. (Без `debug_assert!(false)` — иначе clippy
  `assertions_on_constants`.)
- **D5** — xref-sentinel `\x00XREF_N\x00` (резолв `String::replace` в `finish`) сделан
  неподделываемым: `html_escape`/`html_escape_text` отбрасывают `\x00` (`'\0' => {}`), поэтому
  NUL не попадает в выводимый esc-текст. Лёгкий фикс вместо рискованного переписывания
  резолвера (xref недавно тщательно чинили). +1 тест `test_nul_byte_stripped_from_text`.
- **D6** — `adoc-parser/src/parser.rs`: обе ветки `self.inline_buffer.pop()` →
  `pop().or_else(|| self.next())`, чтобы пустой результат инлайн-парсинга не обрывал итератор.

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное
  (parser 429, html 300, integration 25, ASG/html_compat OK; 0 failed, 0 паник).
- CLI-санити: `[.lead]`+`.title`+source → корректный вывод (трамплин без регрессий).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/p1-robustness` (по правилу — только по запросу).
- Осталось из аудита (TODO.md): декомпозиция гигантских функций, doc-тесты, дедуп `try_*_macro`,
  README (238), сноска FEATURES.md, метаданные Cargo, единая дисциплина экранирования (P2);
  кластеры совместимости (P3).

### Предостережения
- НЕ `cargo fmt` на крейт (не fmt-clean). Коммит только по запросу пользователя.
- Верификация совместимости: `compare_full.py` (release-бинарь), корпус `/mnt/c/tmp/adoc-test/`.
- LSP для навигации, context7 MCP для доков.
