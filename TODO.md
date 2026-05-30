# TODO — adoc-parser

Roadmap по итогам архитектурного аудита. Источник задач совместимости — `COMPAT-DIFF.md`
(числа в скобках — затронутые файлы корпуса `/mnt/c/tmp/adoc-test/`, 344 шт.).

Перед каждым коммитом: `cargo clippy --workspace` (0 warnings) + `cargo test --workspace`
(всё зелёное). Никогда не коммитить прямо в master — сначала ветка (см. CLAUDE.md).

---

## Сделано (в master)

- [x] **Фаза 1 — баги корректности** (коммит 20b3e3f)
  - UTF-8-порча в `preprocessor.rs::expand_counters` (байтовая индексация → char-safe)
  - Единый include-резолвер: `preprocessor::resolve_includes` стал рекурсивным с
    защитой циклов/глубины (`MAX_INCLUDE_DEPTH=64`, `seen: HashSet`); CLI зовёт его и
    вернул `lines/tags/indent/optional` (удалён дубль в `adoc-cli/main.rs`)
  - Хрупкие unwrap'ы рендерера (`adoc-html/lib.rs:444, ~1961`) → безопасные паттерны
  - Рекурсия на комментариях (`block.rs`) → итеративное потребление (+ тест на 50k строк)
- [x] **Фаза 2 — регрессия source-shorthand** (20b3e3f)
  - `[,lang]` / `[#id,lang]` / `[.role,lang]` → `BlockAttributes::implied_source_lang`,
    подавление утечки языка в class (`emit_block_metadata`)
  - Корпус: `pre.highlight` 26→5, listingblock-class 28→7

## Сделано в ветке (ожидает коммита/мержа)

- [x] **Inline `[.role]`/`[#id]` на форматировании** (п.13, п.16; сессия 2026-05-30,
  ветка `feat/inline-role-formatting`). `Tag::Strong/Emphasis/Monospace` теперь несут
  `{ id, roles }`; `try_inline_attr_span` (`inline.rs`) обрабатывает `_`/`*`/`` ` `` после
  `[…]` (не только `#`); рендерер эмитит id/class (`push_inline_id_class`).
  `[.path]_x_`→`<em class="path">`, `[.term]*x*`→`<strong class="term">`,
  `[#id.term]*x*`→`<strong id="id" class="term">`.
  **Корпус: Identical 71→79; `attr_diff on <strong>` 20→1, `<em>` 7→2.** Тесты/clippy зелёные.

## Свежий baseline корпуса (2026-05-30, ПОСЛЕ фикса)

`/mnt/c/tmp/adoc-test/` 344 файла: **Identical 79, Different 265, Errors 0**
(`python3 compare_full.py`, release-бинарь). **COMPAT-DIFF.md устарел** (числа от 2026-03-23,
до Фаз 1–2). Доминирующий остаточный шум — NCR-кодировка типографики (`’`→`&#8217;`,
229 файлов; в одиночку 0 flips). Топ-кластеры почти-проходящих (после нейтрализации NCR,
d≤2): xref авто-текст `<<id>>`/`xref:f.adoc[]`→заголовок цели (~25), backslash перед entity
(п.15, ~10). 89 файлов в пределах 5 правок.

---

## Фаза 3 — Совместимость с Asciidoctor (основной объём)

Приоритет по числу файлов. После каждого пункта — пере-сравнение на корпусе.

- [~] **п.40 Подстановка document-атрибутов** — ОПИСАНИЕ УСТАРЕЛО. Рендерер уже резолвит
  `Event::AttributeReference` из `document_attrs` (`adoc-html/lib.rs:~531`). Остаток —
  forward-ссылки (`{x}` до `:x:`) и `{counter:...}` (п.36); не «архитектурный корень».
- [~] **п.11 Роли на блоках** — УЖЕ ИСПРАВЛЕНО. `write_meta_attrs` доносит роли до
  image/paragraph/admonition wrapper div; на корпусе расхождений по `[.lead]` нет.
- [ ] **п.38 Ссылки: текст вместо URL** (25) — в description-list terms и сложных
  inline-контекстах не парсится текст ссылки. `inline.rs` link/url-макросы.
- [x] **п.13 `class="term"` на `<strong>`** — СДЕЛАНО (сессия 2026-05-30, ветка
  `feat/inline-role-formatting`). `[.term]*x*` → `<strong class="term">`. Категория
  `attr_diff on <strong>` 20→1. См. ниже «Сделано в ветке».
- [ ] **п.14 Ссылки: лишний `class="bare"`, нет `target`+`rel`** (23). `inline.rs`.
- [ ] **п.37 Типографские замены** (~10) — `--`→—, `...`→…, `->`→→, `'`→’ (REPLACEMENTS sub).
- [ ] **п.40-смежное: остаток регрессий source** (5/7 остались после Фазы 2):
  - неизвестный verbatim-style не должен идти в класс (`[ruby]`, `[src,yaml]` →
    Asciidoctor даёт `class="listingblock"` без языка; мы выводим style как класс)
  - markdown code-fences ` ``` ` (asciidoc-vs-markdown.adoc: 52 случая `pre.highlight`)
  - source внутри table-cells (cell.adoc, format-column-content.adoc)
- [ ] **п.15 Entity backslash** (10) — не выводить `\` перед `&entity;`. `inline.rs`.
- [x] **п.16 `class="path"` на `<em>`** — СДЕЛАНО (та же ветка): `[.path]_x_` →
  `<em class="path">`. Категория `attr_diff on <em>` 7→2 (остаток — рассинхрон по др. причинам).
- [ ] **п.41 header после комментариев** (8) — корень: `block.rs:~492` ставит
  `body_started=true` при встрече комментария ДО header, ломает детекцию `= Title`.
  **п.27 source-language attr** (7).
- [ ] **Точечные**: п.17 (остаток: `[.line-through]#`→`<del>`, `#`→`<mark>`; inline-роль
  на `_`/`*`/`` ` `` уже сделана в п.13/16),
  п.18 (image alt двойные кавычки), п.19 (xref-id норм.), п.20 (`[[id,reftext]]`),
  п.24 (точки в id секций), п.25 (audio/video attrs), п.26 (frame/grid),
  п.28 (TOC), п.29 (`kbd:`), п.36 (`{counter}` в таблицах), п.39 (`btn:`/`menu:`).

---

## Фаза 4 — Качество и архитектура

- [ ] Декомпозиция гигантских функций: `start_tag` (933 стр., `adoc-html/lib.rs:842`),
  `parse_inline` (~390, `inline.rs`), `scan_next_block` (~380, `block.rs`).
- [ ] Дедупликация `try_*_macro` в `inline.rs` (общий `parse_bracket_macro`-helper).
- [ ] Doc-тесты для публичного API (`to_html`, `push_html`, `Parser`) — сейчас 0.
- [ ] Остаток рекурсии `scan_next_block`: хвостовые вызовы на `[attr]`/`.title`
  (`block.rs:481/488`) → `loop`-обёртка. **Внимание:** файл НЕ fmt-clean, переотступать
  только целевую функцию вручную (см. предостережение ниже).
- [ ] Обновить README (устаревшее «233 cases» → актуальные 242 ASG-пары / числа тестов).

---

## Предостережения

- **НЕ запускать `cargo fmt` на крейт** — проект не fmt-clean (компактный стиль),
  fmt разворачивает весь файл (~4300 строк шума). Любой переотступ — вручную, точечно.
- **Верификация совместимости**: корпус `/mnt/c/tmp/adoc-test/` (рекурсивно, 344 `.adoc`),
  `asciidoctor` установлен. Сравнение файла:
  `asciidoctor -e -o - <f.adoc>` (embedded) vs `target/debug/adoc --no-standalone <f.adoc>`.
- Использовать rust-analyzer LSP для навигации (CLAUDE.md), context7 MCP для доков библиотек.
