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

## Сделано

- [x] **Inline `[.role]`/`[#id]` на форматировании** (п.13, п.16; коммит `f2dd2eb`,
  ветка `feat/inline-role-formatting`). `Tag::Strong/Emphasis/Monospace` несут `{ id, roles }`;
  `try_inline_attr_span` обрабатывает `_`/`*`/`` ` `` после `[…]`; рендерер эмитит id/class.
  `[.path]_x_`→`<em class="path">`, `[.term]*x*`→`<strong class="term">`.
  **Корпус: Identical 71→79.**

## Сделано в ветке `fix/p1-robustness` (2026-05-30, P1 — надёжность; ожидает коммита)

- [x] **D3/D4/D5/D6** — детали в разделе «Из аудита 2026-05-30» ниже (все отмечены `[x]`):
  трамплин против stack-overflow, мягкая деградация вместо `unreachable!()`, неподделываемый
  xref-sentinel, guard пустого инлайн-результата. `clippy` 0 warnings; `test --workspace`
  зелёное (+3 теста: 2× stack-overflow в integration, 1× NUL-strip в html).

## Сделано в ветке `fix/attr-escaping-and-ifeval` (2026-05-30, аудит; в master)

- [x] **D1 — единое HTML-экранирование значений атрибутов** (безопасность/корректность).
  `adoc-html/lib.rs`: в `render_video_tag` поля `width/height/start/end` (iframe YouTube/Vimeo
  и HTML5-video, ~стр. 2886-2966) и в `image_base_class` значения `float`/`align` (2450-2474)
  выводились без экранирования → инъекция атрибута (`video::v[width=1" onmouseover="…]`,
  `image::x[align=y" …]`). Теперь все идут через `html_escape`. `target`/`poster`/`alt`/`id`/
  `roles`/`style` уже экранировались — устранена несогласованность. +2 регресс-теста.
- [x] **D2 — паника `extract_operand`** (`preprocessor.rs:908`). Одиночная кавычка-операнд
  (`ifeval::[" < 5]`) давала `trimmed[1..0]` → паника. Добавлен guard `len() >= 2`. +1 тест.
- Верифицировано: `cargo clippy --workspace` 0 warnings; `cargo test --workspace` зелёное
  (parser 428→429, html 297→299). CLI-проверка: инъекция экранируется, позитив без регрессий.

## Сделано в ветке (ожидает коммита/мержа)

- [x] **Xref авто-текст** (часть п.38; сессия 2026-05-30, ветка `feat/xref-auto-text`).
  Пустой `xref:target[]`/`<<id>>` теперь резолвится как в Asciidoctor (`adoc-html/lib.rs`):
  - inter-doc `xref:f.adoc[]` → авто-текст = путь с `.adoc`→`.html` (был сырой `.adoc`);
  - intra-doc `<<id>>` → заголовок цели; добавлен сбор id→заголовок **блоков**
    (`block_ref_titles`, захват в `start_tag` после `take_block_meta`) в дополнение к секциям
    (`toc_entries`). Резолв в `finish()`: секции экранируются, заголовки блоков — уже HTML.
  **Корпус: Identical 79→135 (+56).** Тесты/clippy зелёные.

## Свежий baseline корпуса (2026-05-30, ПОСЛЕ block-image-role)

`/mnt/c/tmp/adoc-test/` 344 файла: **Identical 142, Different 202, Errors 0**
(`python3 compare_full.py`, release-бинарь). **COMPAT-DIFF.md устарел** (числа от 2026-03-23).
Методика выбора кластера: `/tmp/nearmiss.py` ранжирует Different-файлы по числу позиционных
diff'ов (переиспользует нормализацию compare_full) — берём фиксы, у которых файл «1 diff away».
Следующие чистые flip-кандидаты (по near-miss): backslash перед entity (`\&#32;`, п.15 — link-macro,
ui-macros), escaped-директива `\ifdef`/`\endif` (admonitions, inter-document-xref), апостроф
`'`→`’` (п.37 — scope, span-cells), xref-id норм. `#Substitutions`→`#_substitutions` (п.19/24).

---

## Фаза 3 — Совместимость с Asciidoctor (основной объём)

Приоритет по числу файлов. После каждого пункта — пере-сравнение на корпусе.

- [~] **п.40 Подстановка document-атрибутов** — ОПИСАНИЕ УСТАРЕЛО. Рендерер уже резолвит
  `Event::AttributeReference` из `document_attrs` (`adoc-html/lib.rs:~531`). Остаток —
  forward-ссылки (`{x}` до `:x:`) и `{counter:...}` (п.36); не «архитектурный корень».
- [~] **п.11 Роли на блоках** — УЖЕ ИСПРАВЛЕНО для `[.role]` блок-строк. `write_meta_attrs` доносит
  роли до image/paragraph/admonition wrapper div; на корпусе расхождений по `[.lead]` нет.
- [x] **Роль из макроса `image::x[…,role=…]`** — СДЕЛАНО (ветка `fix/block-image-role`, 2026-05-30).
  `ImageAttrs` не извлекал `role` (попадал в `_ => {}`), а обработчик block-image мёржил из img-attrs
  только `align`/`float` → роль терялась, `<div class="imageblock">` вместо `imageblock screenshot`.
  Фикс: `parse_image_attrs` извлекает `role`; `scan_block_macros` мёржит его в `block_attrs.roles`
  (далее существующий путь roles→class). **Корпус: Identical 135→142 (+7), 0 регрессий.**
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

- [x] Декомпозиция гигантских функций — ВСЕ ТРИ СДЕЛАНЫ:
  - [x] **`start_tag`** — СДЕЛАНО (ветка `refactor/decompose-start-tag`, 2026-05-30): **946→288 строк**.
    14 методов-обработчиков (`start_delimited_block`/`start_source_block`/`start_section_title`/
    `start_section_div`/`start_paragraph`/`start_unordered_list`/`start_ordered_list`/
    `start_description_list`/`start_table`/`start_table_cell`/`start_admonition`/`start_block_image`/
    `start_inline_image`/`start_cross_reference`); внешний `match` остался исчерпывающим (без catch-all).
    Чистый рефакторинг: clippy 0, test зелёное, корпус Identical 135 без изменений.
  - [x] **`parse_inline`** — СДЕЛАНО (ветка `refactor/decompose-parse-inline`, 2026-05-30):
    **393→32 строки**. 4 под-диспетчера (`handle_inline_escape`/`handle_inline_passthrough`/
    `handle_inline_formatting`/`handle_inline_macro`), вызываемые в исходном порядке; arm'ы
    перенесены дословно (`continue`→`return true`), порядок и непересечение guard'ов сохранены.
    Чистый рефакторинг: clippy 0, test зелёное, корпус Identical 135 без изменений.
  - [x] **`scan_next_block_once`** — СДЕЛАНО (ветка `refactor/decompose-scan-next-block`, 2026-05-30):
    **391→49 строк**. 6 групп-детекторов (`scan_header_constructs`/`scan_leaf_blocks`/
    `scan_block_macros`/`scan_block_containers`/`scan_list_constructs`/`scan_paragraph_fallback`),
    вызываемых в исходном порядке. Sentinel `Option<Option<Event>>` (Some=обработано, None=дальше);
    каждый `return X`→`return Some(X)`, `body_started=true` остался в диспетчере между фазами.
    Чистый рефакторинг: clippy 0, test зелёное, корпус Identical 135 без изменений.
- [x] **Дедупликация `try_*_macro`** — СДЕЛАНО (ветка `refactor/dedup-bracket-macros`, 2026-05-30).
  Премиса «14→1» оказалась завышенной: большинство `try_*` (footnote/link/mailto/xref/cross_reference/
  autolink/anchor/index/attr_span/custom) имеют уникальный разбор. Реальный дедуп — у 2 семейств:
  `parse_bracket_macro(prefix)` (content-only: kbd/btn/stem/pass) и `parse_target_bracket_macro(prefix)`
  (target+items: menu/icon). Хелперы делают только разбор `[…]` + расчёт pos; эмиссия/политика пустоты
  остались в callers. Чистый рефакторинг: clippy 0, test зелёное, корпус Identical 135 без изменений.
- [x] **Doc-тесты для публичного API** — СДЕЛАНО (ветка `docs/public-api-doctests`, 2026-05-30).
  Примеры-doctest на `adoc_html::to_html`, `adoc_html::push_html`, `adoc_parser::Parser` (+ крейт-
  докстрока `//!` adoc-html). Было 0 → 3 doctests, все зелёные. Поведение не затронуто (только docs).
- [x] ~~Остаток рекурсии `scan_next_block`: хвостовые вызовы на `[attr]`/`.title`~~ —
  **закрыто D3** (трамплин `scan_next_block`→`scan_next_block_once`, коммит `bc7c1b2`).
  Запись была устаревшей.
- [x] **README — уточнение метрик** — СДЕЛАНО (ветка `docs/readme-test-counts`, 2026-05-30).
  ⚠️ «238» из аудита — **ложная находка**: верифицировано (233 input + 233 output + 233 пары на
  диске; тест печатает `Total: 233, Passed: 233`; submodule запинен). 233/233 уже было верно.
  Вместо ложной правки числа — уточнён *смысл*: `adoc-compat-tests` = структурная ASG-конформность
  (asciidoc-parsing-lab), добавлена строка `adoc-html-tests` (HTML-совместимость vs Asciidoctor) +
  пояснение различия. Числа 135/344 в README не вносил (внешний корпус, не в репозитории).

### Из аудита 2026-05-30 (отложено из P0)

- [x] **D3** — рекурсия `scan_next_block` устранена (ветка `fix/p1-robustness`): трамплин
  `scan_next_block`→`scan_next_block_once` через флаг `rescan_requested`; `[attr]`/`.title`/
  комментарии теперь O(1) стек. +2 стресс-теста (50k строк).
- [x] **D4** — `unreachable!()` (block.rs) → мягкая деградация: native-table-строка пропускается
  (`continue`), неизвестный block-style → обычный параграф, лишний DL-контекст → no-op.
- [x] **D5** — xref-sentinel `\x00XREF_N\x00` сделан неподделываемым: `html_escape`/
  `html_escape_text` отбрасывают `\x00`, поэтому NUL не попадает в выводимый текст. +1 тест.
- [x] **D6** — `parser.rs`: пустой результат инлайн-парсинга → `pop().or_else(|| self.next())`
  вместо обрыва итератора.
- [x] **Гигиена** — СДЕЛАНО (ветка `chore/cargo-metadata-and-features-note`, 2026-05-30).
  - FEATURES.md «202/202 100%» → «Покрытие синтаксиса: 100%» + сноска `[^coverage]`: это
    покрытие *синтаксиса* грамматики, а не побайтовая HTML-совместимость (корпус 135/344).
  - Cargo.toml всех 6 крейтов: `description` (inline, уникален на крейт) + `license`/
    `repository` через наследование `[workspace.package]` в root (`license = "MIT"`,
    `repository = "https://github.com/avskor/adoc-parser"`; `*.workspace = true` в крейтах).
  - Semver запинен до минора: clap `4`→`4.5`, serde `1`→`1.0`, similar `2`→`2.7`; chrono
    оставлен `0.4` (для 0.x минор и есть единица semver-совместимости — пинить тоньше = patch).
  - Верифицировано: `cargo metadata` OK, clippy 0 warnings, `test --workspace` зелёное
    (parser 429, html 302, …), Cargo.lock без изменений.
- [x] **Единая дисциплина экранирования** — СДЕЛАНО (ветка `fix/attr-escaping-discipline`,
  2026-05-30). Введён хелпер `write_attr` (эмитит ` name="value"` через `html_escape`),
  single-value атрибуты (id/href/target/src/alt/width/height/poster/data-lang/title) переведены
  на него. **Закрыт D7** (см. ниже). Экранирование перенесено на границу эмиссии: `default_class`
  в `write_meta_attrs` экранируется (защищает все типы блоков), локальный escape убран из
  `image_base_class` (исключает двойное). Тест-инвариант `test_attribute_escaping_invariant`
  (10 каналов) + `test_attribute_escaping_no_overescape`. clippy 0, `test --workspace` зелёное
  (html 300→302), корпус Identical 135 без изменений.

### D7 — XSS через сырой `style_name` упорядоченного списка (найдено 2026-05-30)

- [x] `[<b>x]` на `ol` → `<ol class="<b>x">` / `<div class="olist <b>x">`: символы `<`/`>`/`&`
  проходили токенизатор позиционных в `meta.style` и писались сырыми (минуя экранирование, которое
  `write_meta_attrs` применяет к `id`/`style`/`roles`). D1 чинил только media/image — этот «сосед»
  остался из-за отсутствия системного правила. Закрыто вместе с дисциплиной экранирования выше.
  (Asciidoctor здесь тоже уязвим — экранируя, мы строго безопаснее.)

---

## Предостережения

- **НЕ запускать `cargo fmt` на крейт** — проект не fmt-clean (компактный стиль),
  fmt разворачивает весь файл (~4300 строк шума). Любой переотступ — вручную, точечно.
- **Верификация совместимости**: корпус `/mnt/c/tmp/adoc-test/` (рекурсивно, 344 `.adoc`),
  `asciidoctor` установлен. Сравнение файла:
  `asciidoctor -e -o - <f.adoc>` (embedded) vs `target/debug/adoc --no-standalone <f.adoc>`.
- Использовать rust-analyzer LSP для навигации (CLAUDE.md), context7 MCP для доков библиотек.
