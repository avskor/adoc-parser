# Session context

## Последняя сессия (2026-05-30)

Архитектурный аудит проекта + реализация Фаз 1–2 исправлений. Всё слито в master
и запушено в origin.

### Что сделано
- **20b3e3f** — Фаза 1 (баги корректности) + Фаза 2 (регрессия source-shorthand):
  - `preprocessor.rs::expand_counters` — char-safe (была UTF-8-порча на байтовой индексации)
  - `preprocessor.rs::resolve_includes*` — рекурсивный, с защитой циклов/глубины;
    `adoc-cli/main.rs` зовёт его (удалён дублирующий резолвер, вернулись lines/tags/indent/optional)
  - `adoc-html/lib.rs` — безопасные паттерны вместо хрупких unwrap (xref_placeholders, source_code_buffer)
  - `block.rs` — итеративное потребление комментариев (защита от stack overflow)
  - `attributes.rs::implied_source_lang` + `emit_block_metadata` — shorthand `[,lang]`/`[#id,lang]`/`[.role,lang]`
  - очищены 5 предсуществующих clippy-warnings
- **82ff78c** — обновление COMPAT-DIFF.md (третий прогон).
- Удалён артефакт `adoc-html-tests/fixtures/block/callout-list.html`.

### Текущий статус
- На `master`, рабочее дерево чистое, синхронизировано с `origin/master`.
- `cargo test --workspace`: зелёное (428 parser, 297+35 html, 6+6 html-tests,
  242 ASG-пары, 23 integration, 1 html_compat с 70 fixtures).
- `cargo clippy --workspace`: 0 warnings.

### Что дальше
См. **TODO.md**. Рекомендуемый старт Фазы 3 — **п.40** (подстановка document-атрибутов
в контенте, 13 файлов, архитектурный корень: атрибуты не прокидываются препроцессор→рендерер)
или **п.11** (роли блоков, 25 файлов).

Перед работой: `git checkout master && git pull && git checkout -b <ветка>` (CLAUDE.md
запрещает коммиты прямо в master). Коммитить только по запросу пользователя.

### Ключевые предостережения (детали в TODO.md)
- НЕ `cargo fmt` на крейт — не fmt-clean, даёт ~4300 строк шума.
- Верификация совместимости: корпус `/mnt/c/tmp/adoc-test/` + `asciidoctor`
  (`asciidoctor -e -o - <f>` vs `target/debug/adoc --no-standalone <f>`).
