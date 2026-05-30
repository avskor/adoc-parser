# Session context

## Последняя сессия (2026-05-30) — Фаза 3 (старт): роль из макроса image::

Первая задача Фазы 3 (совместимость с Asciidoctor). В отличие от Фазы 4 — меняет рендеринг,
верификация = рост Identical на корпусе без регрессий.

### Методика выбора кластера (важно для продолжения Фазы 3)
`/tmp/nearmiss.py` (переиспользует нормализацию `compare_full.py`) ранжирует Different-файлы по
числу позиционных diff'ов и печатает diff'ы файлов с ≤3 расхождениями. Берём фиксы, у которых
файл «1 diff away» → гарантированный flip (обходит ловушку «0 flips» из [[compat_corpus_methodology]]).
Запуск: `cd /mnt/c/tmp/adoc-test && python3 /tmp/nearmiss.py`.

### Ветка `fix/block-image-role` (от master; НЕ закоммичено)
- Корень: `image::x[alt,role=screenshot]` — роль задана именованным атрибутом ВНУТРИ макроса.
  `ImageAttrs` (attributes.rs) не имел поля `role` (ключ падал в `_ => {}`); обработчик block-image
  (`scan_block_macros`, block.rs) мёржил из img-attrs только `align`/`float`. → роль терялась.
- Фикс: (1) `ImageAttrs.role: Option<&str>` + захват `"role" => …` в `parse_image_attrs`;
  (2) в `scan_block_macros` смёржить `img_attrs.role` в `block_attrs.roles` (если ещё нет).
  Существующий путь `block_attrs.roles`→emit_block_metadata→`write_meta_attrs` выводит
  `class="imageblock screenshot"` (default_class → style → roles, порядок уже верный).
- Затронуты только block-images (inline `image:` имеет свой путь, Tag::InlineImage без roles — не трогал).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 429, html 302,
  html_output 35, adoc_html_tests 6, author_rendering 6, html_compat 1, integration 25,
  parsing_lab 1, doctests 2+1).
- Корпус `compare_full.py` (release): **Identical 135→142 (+7), Different 209→202, Errors 0** —
  ровно 7 файлов Different→Identical, регрессий ноль.
- TODO.md: baseline обновлён 135→142; фикс отмечен `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/block-image-role` (только по запросу).
- Следующие чистые flip-кандидаты Фазы 3 (по near-miss, каждый ~2 flip'а):
  - **backslash перед entity** (`\&#32;`/`\&#8942;`, п.15): link-macro, ui-macros — не съедаем `\`.
  - **escaped-директива** `\ifdef`/`\endif`: admonitions, inter-document-xref (preprocessor-слой).
  - **апостроф** `'`→`’` (п.37 REPLACEMENTS): scope, span-cells (NB: это РАЗ замена, не NCR-кодировка).
  - **xref-id норм.** `#Substitutions`→`#_substitutions` (п.19/24): positional-and-named-attributes.
  - Крупные тангл-кластеры (link text/bare/target+rel, п.14/38) — много файлов, но по 2-3 diff'а
    каждый, требуют комплексного фикса link-макроса.
- Также видна отдельная бага п.18: `<img alt=""…">` (двойная кавычка в alt) — author-attribute-entries,
  revision-attribute-entries (мешает их flip'у).

### Предостережения
- НЕ `cargo fmt` (не fmt-clean). Коммит только по запросу. Верифицировать находки аудита (см.
  [[audit_2026-05-30]] — «238» было ложным).
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release `target/release/adoc`, 344 файла).
  near-miss: `/tmp/nearmiss.py`. LSP для навигации, context7 MCP для доков.
