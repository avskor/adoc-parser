# Session context

## Последняя сессия (2026-05-30) — Архитектурный аудит + P0-фиксы

Запрос: ознакомиться с проектом, провести архитектурный анализ, определить ошибки и
недоработки. Затем по выбору пользователя — исправить **P0** (баги корректности/безопасности).

### Аудит (полный отчёт: `~/.claude/plans/sequential-dreaming-zebra.md`)
Три Explore-агента + ручная верификация каждой находки по коду (LSP/чтение). Важно: автопроход
дал ~4 ложных «critical-паники» — отсеяны проверкой (edition 2024 валидна на rustc 1.96;
`parser.rs` unwrap защищён `len==1`; UTF-8-срез в `attributes.rs:136` безопасен из-за `take_while`
на ASCII; include-лимит `MAX_INCLUDE_DEPTH=64` уже есть; «бесконечный цикл» continuation
ограничен входом; XSS через имя макроса недостижим — валидатор `[a-z0-9_-]`).

### Сделано: ветка `fix/attr-escaping-and-ifeval` (от master; НЕ закоммичено)
- **D1** — единое HTML-экранирование значений атрибутов (`adoc-html/src/lib.rs`):
  - `render_video_tag` (~2886-2966): `width/height/start/end` → `html_escape` (было `push_str`).
  - `image_base_class` (2450-2474): `float`/`align`-значения → `html_escape(&mut class, …)`.
  - Корень: несогласованность (id/roles/style/target/poster/alt экранировались, эти — нет).
    Достижимо: внутренние `"` в значениях не удаляются парсером → пробой атрибута.
  - +2 теста: `test_video_width_attr_escaped`, `test_block_image_align_float_class_escaped`.
- **D2** — `preprocessor.rs:908 extract_operand`: guard `len() >= 2` (одиночная кавычка
  давала `trimmed[1..0]` → паника на `ifeval::[" < 5]`). +1 тест `test_ifeval_lone_quote_operand_no_panic`.

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное
  (parser 428→429, html 297→299, остальные без изменений; 0 failed).
- CLI end-to-end: `width="1&quot; onmouseover=&quot;alert(1)"`, `class="imageblock a&lt;b&gt;c"`
  (инъекция нейтрализована); `#t=60,120` / `text-center` — позитив без регрессий.

### Что дальше
- **Спросить про коммит** ветки `fix/attr-escaping-and-ifeval` (по правилу — коммит только по запросу).
- Отложенные находки аудита (в TODO.md, раздел «Из аудита 2026-05-30»): **D3** (рекурсия
  `scan_next_block` 481/488 → loop), **D4** (`unreachable!()`→graceful), **D5** (xref-плейсхолдер
  через map), **D6** (parser empty-inline), гигиена (FEATURES-сноска, Cargo-метаданные, semver),
  единая дисциплина экранирования.
- Совместимость (Фаза 3): кластеры NCR-типографики, bare-links, header-после-комментариев.

### Предостережения
- НЕ `cargo fmt` на крейт (не fmt-clean). Коммит только по запросу пользователя.
- Верификация совместимости: `compare_full.py` (release-бинарь), корпус `/mnt/c/tmp/adoc-test/`.
- LSP для навигации, context7 MCP для доков.
