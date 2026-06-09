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

## Свежий baseline корпуса (2026-06-09, ПОСЛЕ section-id-dots-and-dedup)

`/mnt/c/tmp/adoc-test/` 344 файла: **Identical 191, Different 153, Errors 0**
(`python3 compare_full.py`, release-бинарь). **COMPAT-DIFF.md устарел** (числа от 2026-03-23).
Методика выбора кластера: `/tmp/nearmiss.py` ранжирует Different-файлы по числу позиционных
diff'ов (переиспользует нормализацию compare_full) — берём фиксы, у которых файл «1 diff away».
Следующие чистые flip-кандидаты (по near-miss на 190):
**counter.adoc** (2-diff, `{counter:index}`→`{index}` не резолвится; п.36 — АРХИТЕКТУРНЫЙ:
счётчик в локальной мапе препроцессора, рендерер берёт из document_attrs). Прежний 4-diff
user-index (`` `\indexterm2:[<primary>]` ``) ЗАКРЫТ (ветка `fix/escaped-inline-macro`, см. ниже).
**Отложенный баг** (обнажён single-plus-фиксом, НЕ регрессия): trailing ` +`
(space-plus) в reparsed monospace-контенте трактуется как hard-break → `<br>` вместо литерала
(`` `z +` ``→`<code>z<br></code>`, asciidoctor: `<code>z +</code>`; pre-existing, виден в outline.adoc
строка 390 `` `` + +`` ``). Новые неразведанные near-miss на 190: **pass/index.adoc** (6-diff,
len_delta=-1 — один лишний элемент; НЕ про пустой `pass:[]` — он у нас уже верен, разведать корень),
**stem/index.adoc** (6-diff — инъекция MathJax `<script>` в конец body при `:stem:`; АРХИТЕКТУРНАЯ
standalone-фича, отложена), **CHANGELOG.adoc** (7-diff).
**inline-anchor reftext из dt-терма** (`[[id]]term:: ...` → `<<id>>` = текст терма; lexicon.adoc
— ~14 ссылок `boxed-attrlist`→`boxed attribute list`; родственно bibliography, но захват
текста терма в парсере — БОЛЬШЕ по объёму). Прежний 1-diff **listing-blocks.adoc** ЗАКРЫТ
(см. ниже attr-ref-respect-block-subs). **counter.adoc**
(`{counter:index}` не пишет значение в attrs → последующий `{index}` не резолвится; п.36 —
АРХИТЕКТУРНЫЙ: препроцессор раскрывает счётчик в локальную мапу, рендерер берёт `{index}` из
document_attrs, и значение МЕНЯЕТСЯ по ходу документа — чистого моста нет, отложен).
**Смежное (НЕ flip в одиночку, но реальный баг)**: наследование НЕ-header стиля колонки
(`[cols="1m,3m"]` → ячейки должны быть `<code>`; `e`/`s`/`a`/`l` аналогично) — мой фикс
наследует ТОЛЬКО `h`; `m`/`e`/`s` дали бы много flip'ов, но `a` (AsciiDoc) меняет обёртку и
требует nested-парсинга ячейки (рискованно). Затрагивает pass-macro, subs-group-table и др.
Архитектурные (отложены): **nested-форматирование в тексте ссылки** (`link:u[`<code>`]` /
`{url}[`mono`]` — текст ссылки проходит REPLACEMENTS, но не QUOTES → backticks не → `<code>`;
затрагивает audio-and-video, links, replacements — все ещё Different по этой причине, но attr-ref
часть теперь верна), inline-monospace passthrough char-ref (`` `&#167;` `` → Asciidoctor
сохраняет в `<code>`, мы экранируем — остаток replacements.adoc; `Event::Code`), link-role
`class="external"` (нет поля role в `Tag::Link`).

---

## Фаза 3 — Совместимость с Asciidoctor (основной объём)

Приоритет по числу файлов. После каждого пункта — пере-сравнение на корпусе.

- [x] **Section-id: точки как разделитель + дизамбигуация дубликатов** — СДЕЛАНО (ветка
  `fix/section-id-dots-and-dedup`, 2026-06-09). Две части правила автогенерации id Asciidoctor
  (верифицировано пробами): (1) `.` в заголовке → разделитель (`0.3.0 Milestone Build` →
  `_0_3_0_milestone_build`, не `_030_milestone_build`); прочий пунктуатор (`@`/`#`/`:`/`!`/`(`/`)`)
  по-прежнему отбрасывается, прогон разделителей схлопывается. (2) Повторяющиеся автогенерируемые
  заголовки получают суффикс `_2`/`_3` (`Added`×3 → `_added`/`_added_2`/`_added_3`); явные id
  (`[#id]`) НЕ переименовываются, но регистрируются (авто-id дедупится и против них). Doctitle
  (level 0) НЕ регистрируется (проба: одноимённая секция → `_intro`, не `_intro_2`). Discrete-
  заголовки участвуют в общем реестре. Фикс: (a) `scanner.rs::generate_id` — `.` добавлен в набор
  разделителей; (b) `block.rs` — поле `used_ids: HashSet<String>` + хелперы `register_explicit_id`/
  `unique_auto_id`, маршрутизированы `scan_section` и `scan_discrete_heading` (НЕ doctitle).
  +4 теста (scanner dot/collapse; parser dedup/auto-vs-explicit/dots). **Корпус: Identical 190→191
  (+1)** (CHANGELOG.adoc); blast 15 файлов: 1 флип, **0 регрессий**, 14 changed-still-different —
  их секционные id теперь совпадают с asciidoctor (counters.adoc: дедуп корректно работает на
  пре-существующем неверном базисе `_section_seq1` — не резолвится `{counter:seq}`, отдельный
  архитектурный баг). parsing-lab 233/233 целы.

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
- [x] **`{attr-ref}[text]` как ссылка (subs-order)** — СДЕЛАНО (ветка `fix/attr-ref-link-macro`,
  2026-06-09). `{url-x}[text^]` где `{url-x}` — document-атрибут с URL → Asciidoctor раскрывает
  атрибут ДО macros-подстановки, после чего `URL[text^]` распознаётся как URL-макрос (ссылка с
  blank-window). Мы эмитили `AttributeReference` и оставляли `[text^]` литералом, а раскрытый URL
  переразбирался в изоляции → bare-autolink + мусорный leftover-bracket. Корень — порядок подстановок
  (откладывался как «архитектурный» ~6 сессий; решился чисто через combine-and-reparse). Фикс:
  (1) `Event::AttributeReference` получил поле `trailing_brackets: Option<CowStr>` (event.rs);
  (2) `inline.rs::try_attribute_reference` захватывает `[...]` сразу после `}` (не `[[` anchor,
  первый `]`, без пробела); (3) рендерер (`lib.rs`) при резолве из `document_attrs` склеивает
  `value+[text]` и переразбирает через `render_inline_value` — URL-значение → ссылка, не-URL →
  тот же литерал (как Asciidoctor); в остальных ветках (intrinsic/env/fallback/missing) скобки
  дописываются литералом; (4) ASG-builder (`compat-tests`) дописывает скобки к резолвнутому тексту
  (сохранение слоя — в parsing-lab инлайн-`{attr}[...]` нет, 233/233 целы). +2 теста (parser+html).
  **Корпус: Identical 175→180 (+5)** (CONTRIBUTING, index, icons-font, auto-ids, custom-ids);
  blast 17 файлов: 5 флипов, 0 регрессий. 12 changed-still-different улучшены/без изменений
  (links/replacements/image-size/index лучше; audio-and-video позиционный счётчик вырос —
  артефакт выравнивания, семантически верно, остаток — nested-mono в тексте ссылки, см. baseline).
- [x] **`{attr-ref}<path>[text]` — путь между `}` и `[`** — СДЕЛАНО (ветка
  `fix/attr-ref-path-before-brackets`, 2026-06-09). Продолжение attr-ref-link-macro: `{url}/issues[text]`
  где `{url}` — document-атрибут с URL → Asciidoctor раскрывает атрибут, затем переразбирает
  `value/issues[text]` как URL-макрос (ссылка). Мы захватывали `[...]` ТОЛЬКО вплотную за `}`, а путь
  `/issues` между `}` и `[` утекал литералом → bare-autolink на голом URL + leftover `/issues[text]`.
  Фикс (1 точка, `inline.rs::try_attribute_reference`): захват `trailing_brackets` расширен — перед
  `[...]` допускается путь (run байтов без пробела/`[`/`]`; все стоп-символы ASCII → корректная
  char-граница). Рендерер/ASG-builder работают с `trailing_brackets` обобщённо (путь едет внутри
  `br`) — БЕЗ их изменений; для не-URL значения склейка остаётся литералом (как и раньше). +2 теста
  (parser: capture path / space-stops / no-bracket; html: `{url-repo}/issues[text]`→ссылка, без
  leftover/bare). **Корпус: Identical 185→186 (+1)** (reference-attributes); blast 3 файла: 1 флип,
  0 регрессий, CHANGELOG улучшен (10→7 diff: `{url-repo}/-/commits/main[…]`→ссылка верна),
  outline нейтрально (`{url-issues}/25[#25]` ссылки совпали с asciidoctor; гигантский позиц. рассинхрон).
  parsing-lab 233/233 целы (инлайн-`{attr}path[...]` в фикстурах нет).
- [x] **п.13 `class="term"` на `<strong>`** — СДЕЛАНО (сессия 2026-05-30, ветка
  `feat/inline-role-formatting`). `[.term]*x*` → `<strong class="term">`. Категория
  `attr_diff on <strong>` 20→1. См. ниже «Сделано в ветке».
- [~] **п.14 Ссылки: лишний `class="bare"`, нет `target`+`rel`** (23). `inline.rs`.
  - [x] **пустой текст `link:url[]` → `class="bare"`** — СДЕЛАНО (ветка `fix/link-macro-empty-bare`,
    2026-06-09). Asciidoctor помечает link-макрос/URL-макрос БЕЗ явного текста как «bare»
    (видимый текст = target) → `class="bare"`; мы ставили bare только для голого autolink-URL.
    Правило (верифицировано пробами): bare ⇔ `[]` пустой (явный текст, даже равный target,
    → НЕ bare); `mailto:[]` НЕ bare (исключение); с ролью — `class="bare external"` (роль пока
    не захватывается — отдельный остаток). Фикс (`inline.rs`): `is_bare = link_attrs.text.is_empty()`
    в 3 точках (`try_link_macro` `++url++`+обычный, `try_autolink` with-text при пустом `[]`);
    mailto не тронут. +2 теста (parser+html); обновлены 2 теста, кодировавшие старое поведение
    (`link:a'b.html[]`, passthrough-URL empty). **Корпус: Identical 171→172 (+1)** (README);
    blast 2 файла: 1 флип, 0 регрессий, url.adoc улучшен (`link:tools.html#editors[]`→bare верен),
    но Different по др. причинам (irc-макрос, nested `*…*` в тексте ссылки, link-role). Остаток
    п.14: роль на ссылке (`class="external"`) НЕ захватывается (нет поля role в `Tag::Link`).
  - [x] **blank-window `^`** — СДЕЛАНО (ветка `fix/link-blank-window-caret`, 2026-06-09).
    Trailing `^` в тексте ссылки (`https://u[text^]`, `link:u[text^]`) → `target="_blank"
    rel="noopener"`, `^` снимается с видимого текста. Фикс централизован в
    `attributes.rs::parse_link_attrs` (единая точка для link/mailto/autolink/`++url++`):
    после извлечения `text` снимаем суффикс `^`, выставляем `window=Some("_blank")` (явный
    `window=` побеждает). Инфраструктура (`Tag::Link.window/nofollow`, рендер `target`/`rel`)
    уже была. +1 тест. **Корпус: Identical 158→162 (+4)** (key-concepts, description,
    image-format, xref-text-and-style); 0 регрессий (blast radius 9 файлов: 4 флипа,
    5 остались Different по др. причинам — `class="external"` link-role, markdown-fences,
    `{attr-ref}[text]`). NB: роль на ссылке (`class="external"`) НЕ захватывается (нет поля
    role в `Tag::Link`) — отдельный остаток п.14; `{attr-ref}[text^]` — архитектурный (subs).
- [~] **п.37 Типографские замены** (~10) — `--`→—, `...`→…, `->`→→, `'`→’ (REPLACEMENTS sub).
  - [x] **em-dash границы + ZWSP** — СДЕЛАНО (ветка `fix/em-dash-boundaries`, 2026-05-31).
    `inline.rs::apply_typographic_replacements`: bare `--` теперь даёт em-dash только для
    `\w--\w` (Asciidoctor `(\w)--(?=\w)`) и эмитит `—`+ZWSP (`—​`); во всех прочих
    позициях (` --flag`, хвостовой `S.S.T.--`, кромки строки) возвращает `None` — первый `-`
    остаётся литералом, второй переразбирается отдельно (`-->` → `-→`, как в Asciidoctor).
    Обновлены 2 теста (дубли bare-em-dash под ZWSP) + arrow-triple (`A --> B`→`A -→ B`);
    +2 теста (`run --dir` и `S.S.T.--` остаются). **Корпус: Identical 149→153 (+4)**
    (asg/README, dedication, continuation, callouts); 0 регрессий (Different 195→191).
  - [x] **Апостроф `'`→’ (и вся REPLACEMENTS) внутри текста макроса** — СДЕЛАНО (ветка
    `fix/macro-text-replacements`, 2026-06-09). Asciidoctor выполняет REPLACEMENTS ДО macros-
    подстановки, поэтому апостроф/дефис/стрелки внутри явного `[label]` уже сконвертированы
    к моменту рендера макроса. Display-текст макроса эмитился сырым (`Event::Text(Cow::Borrowed)`).
    Фикс (`inline.rs`): хелпер `push_macro_label` (зеркалит REPLACEMENTS-ветку `flush_text`),
    применён к **явному** label всех 6 точек (link `++url++`/`link:`, mailto, autolink-с-текстом,
    `xref:t[label]`, `<<id,label>>`); URL/target-фолбэк остаётся сырым (бэйр-URL не курлится).
    +1 тест. **Корпус: Identical 165→168 (+3)** (subs/index, span-cells, scope); 0 регрессий
    (blast: 11 файлов изменили вывод — 3 флипа, 8 остались Different по др. причинам, апостроф
    в них теперь верный — `class="bare"`/др.). Остаток: nested-форматирование/attrs в тексте
    макроса (`link:u[*bold* {attr}]`) НЕ обрабатывается — это полный inline-проход (архитектурно).
- [~] **п.40-смежное: остаток регрессий source** (после Фазы 2):
  - [x] **неизвестный verbatim-style → class** — СДЕЛАНО (ветка `fix/literal-unknown-style-class`,
    2026-06-09). `[plantuml]`/`[src,yaml]`/`[ditaa]` на literal (`....`) ИЛИ listing (`----`)
    delimited-блоке → Asciidoctor отбрасывает неизвестный стиль из class (`literalblock`/
    `listingblock` БЕЗ стиля); мы утекали его (`literalblock plantuml`, `listingblock src`).
    Корень: `write_meta_attrs` дописывает `meta.style` в class после default_class. Фикс
    (`adoc-html/lib.rs`): хелпер `strip_block_style` (клон meta с `style=None`), применён в
    arm'ах `Literal`+`Listing` в `start_delimited_block`. Роли/id сохраняются; `[source,lang]`
    идёт ОТДЕЛЬНЫМ путём `Tag::SourceBlock` (не задет). +1 тест. **Корпус: Identical 169→170
    (+1)** (monitoring); blast 5 файлов: 1 флип, 0 регрессий, 4 (index-родители+db-migration)
    остались Different по др. причинам, но verbatim-стиль в них теперь верен.
  - markdown code-fences ` ``` ` (asciidoc-vs-markdown.adoc: 52 случая `pre.highlight`)
  - source внутри table-cells (cell.adoc, format-column-content.adoc)
- [x] **п.15 Entity backslash** — СДЕЛАНО (ветка `fix/escaped-char-reference`, 2026-05-31).
  `\&#174;`/`\&#xA0;`/`\&copy;` → `\` снимается, character reference эмитится как литеральный
  текст (рендерер экранирует `&`→`&amp;`), как в Asciidoctor `CharRefRx`. Новый arm в
  `handle_inline_escape` + хелпер `char_ref_len_at` (named `[A-Za-z][A-Za-z]+\d{0,2}`,
  decimal `#\d\d\d{0,4}` = 2–6 цифр, hex `#x[0-9A-Fa-f]{2,}`; всё лексически, без словаря
  сущностей). Весь ref эмитится одним span'ом → внутренний `#` не путается с mark-синтаксисом.
  **Корпус: Identical 142→145 (+3)** (multiple-authors, link-macro, ui-macros), 0 регрессий.
  - [x] **preserve bare char-ref** (остаток п.15) — СДЕЛАНО (ветка
    `fix/bare-char-reference-preserved`, 2026-06-09). **Голый** валидный char-ref
    (`&#167;`/`&copy;`/`&amp;`) в обычном тексте → Asciidoctor СОХРАНЯЕТ как сущность; мы
    экранировали (`&amp;#167;`). Правило (верифицировано пробами `[subs=...]`): char-ref
    переживает ТОЛЬКО при `specialchars`+`replacements` вместе (specialchars экранирует `&`,
    replacements разэкранирует валидный ref); `specialchars`-only → экранирует; verbatim
    (specialchars БЕЗ replacements) → экранирует (совпадает). Фикс (`inline.rs`): в главном цикле
    `parse_inline` bare `&`, начинающий валидный char-ref (`char_ref_len_at` из backslash-ветки),
    эмитится как `Event::InlinePassthrough` (raw, рендерер не экранирует) — gated на
    `specialchars && replacements`. **Сопутствующий фикс `parser.rs`**: литеральный параграф без
    `[attr]` падал на `current_subs()`=NORMAL вместо VERBATIM (латентный баг — был безвреден, т.к.
    рендерер всё равно экранировал `&`; мой char-ref его обнажил) → дефолт изменён на `VERBATIM`
    (как у SourceBlock/DelimitedBlock Literal/Listing). +2 теста. **Корпус: Identical 170→171
    (+1)** (title-links); blast radius 12 файлов: 1 флип, 0 регрессий, 11 остались Different по
    др. причинам, но их char-ref/литеральные параграфы теперь верны (verified vs asciidoctor:
    replacements, outline literal-para, pass-macro). Остаток (ОТДЕЛЬНО): inline-monospace
    passthrough char-ref `` `&#167;` `` в `<code>` (`Event::Code`, не задет моим фиксом).
- [x] **п.16 `class="path"` на `<em>`** — СДЕЛАНО (та же ветка): `[.path]_x_` →
  `<em class="path">`. Категория `attr_diff on <em>` 7→2 (остаток — рассинхрон по др. причинам).
- [x] **Escaped preprocessor-директива `\ifdef`/`\ifndef`/`\ifeval`/`\endif`** — СДЕЛАНО
  (ветка `fix/escaped-preprocessor-directive`, 2026-05-31). `preprocessor.rs`: новый шаг «0»
  в `preprocess_with_attrs` снимает ведущий `\` у директивы **в колонке 0** и выводит остаток
  литералом без вычисления (хелпер `starts_with_conditional_directive`, проверка `::`).
  Критично: **колонка 0** (проверяем сырой `line`, не `trimmed`) — Asciidoctor распознаёт
  директивы только в начале строки, поэтому indented `\ifdef` (как в `[source,indent=0]`
  листинге conditionals.adoc) остаётся как есть. +4 теста. **Корпус: Identical 145→149 (+4)**
  (admonitions, inter-document-xref, conditionals, +1 из directives-модуля); 0 регрессий
  (blast radius — ровно 5 файлов с escaped-директивами). Остаток conditionals.adoc — отдельная
  бага `[source,indent=0]` (общий отступ не срезается; нормализатор её прощает → Identical).
- [ ] **п.41 header после комментариев** (8) — корень: `block.rs:~492` ставит
  `body_started=true` при встрече комментария ДО header, ломает детекцию `= Title`.
  **п.27 source-language attr** (7).
- [x] **п.18 image alt двойные кавычки** — СДЕЛАНО (ветка `fix/image-alt-quotes`, 2026-06-09).
  `image::x["Alt",role=…]` → alt сохранял обрамляющие `"` → рендерер экранировал в `&quot;`.
  Корень: `attributes.rs::parse_image_attrs` снимал кавычки только у **именованных** значений
  (`key="v"`), а **позиционные** (alt = positional[0]) пушил сырыми. Фикс: хелпер
  `strip_enclosing_quotes` (одна пара двойных кавычек, как в `split_respecting_quotes`),
  применён и к именованным, и к позиционным. Покрывает block- и inline-image (общий парсер).
  +1 тест. **Корпус: Identical 153→157 (+4)** (author/revision/reference-revision-attribute-entries,
  version-label); 0 регрессий (blast radius — 6 файлов с закавыченным alt, проверены поштучно).
- [x] **п.19 xref-id норм. (natural cross reference)** — СДЕЛАНО (ветка `fix/xref-id-normalization`,
  2026-06-09). `<<Substitutions>>` при наличии секции `== Substitutions` → href `#_substitutions`
  (не сырой `#Substitutions`). Это forward-ссылка, поэтому href резолвится лениво в `finish()`
  через плейсхолдер `\x00XREFHREF_N\x00` (как уже делается для текста). Семантика Asciidoctor
  (верифицирована эмпирически): target, совпадающий с **заголовком секции** (case-sensitive),
  → id этой секции (auto `_substitutions` или явный); зарегистрированный id остаётся как есть;
  иначе сырой target (`#Foo Bar`). +1 html-тест (5 кейсов). **Корпус: Identical 157→158 (+1)**
  (positional-and-named-attributes); 0 регрессий (blast radius 3 файла: 1 флип, 2 улучшены —
  href стал верным — но Different по др. причинам: audio/video-attrs, link-macro).
- [x] **Внутренний xref fallback `[id]` + bibliography reftext** — СДЕЛАНО (ветка
  `fix/xref-fallback-bracketed-id`, 2026-06-09). Две части одного правила Asciidoctor:
  (1) нерезолвимый внутренний `<<id>>` без явного текста → текст `[id]` (в скобках,
  default xreflabel), а не сырой `id`; (2) bibliography-якорь `[[[gof,gang]]]` регистрирует
  reftext `[gang]` (label в скобках) → `<<gof>>` даёт `[gang]`, `[[[pp]]]`→`<<pp>>` даёт `[pp]`.
  Фикс в `adoc-html/lib.rs` (ленивая резолюция в `finish()`, как для xref-текста): новое поле
  `bibliography_reftexts` (заполняется в `push_event` на `BibliographyAnchor`); `xref_placeholders`
  расширен флагом `is_internal` (скобки — только для internal, не inter-document); текстовая
  резолюция теперь зеркалит href: id → title (natural xref, БЕЗ скобок) → bracketed fallback.
  biblio-id добавлен в `known_ids`. +4 теста; 2 старых теста (`test_full_document`,
  `test_xref_unresolvable_falls_back_to_id`) кодировали неверное старое поведение — обновлены
  под `[id]` (верифицировано пробой asciidoctor). **Корпус: Identical 162→165 (+3)**
  (xref.adoc, bibliography, _crud); 0 регрессий (blast radius 7 файлов: 3 флипа, 4 остались
  Different по др. причинам, их xref-ссылки теперь верны). Остаток (НЕ регрессия): inline-anchor
  reftext из dt-терма `[[id]]term::` (lexicon.adoc) — отдельная фича, см. baseline выше.
- [x] **Custom caption на админишене** — СДЕЛАНО (ветка `fix/admonition-custom-caption`,
  2026-06-09). `[caption="Work in Progress"]` перед `CAUTION:` → отображаемый label = caption
  (вместо дефолтного «Caution»), но класс `admonitionblock caution` и `icon-caution` остаются
  по типу. Семантика (верифицирована пробой asciidoctor): text-режим → `<div class="title">caption</div>`;
  `icons=font` → `title="caption"` у `<i>`; пустой `[caption=]` → пустой title. Корень:
  `adoc-html/lib.rs::start_admonition` эмитил жёсткий `label`. Фикс: извлечь `caption` из
  `meta.named` (парсер уже его захватывает), переопределить **отображаемый** текст в обеих
  ветках (caption экранируется — дисциплина экранирования, строже Asciidoctor; glossary без
  спецсимволов). +1 тест. **Корпус: Identical 168→169 (+1)** (glossary); 0 регрессий (blast
  radius — ровно 1 файл изменил вывод; остальные 4 `[caption=` — таблицы, отдельный путь).
- [x] **`table-caption` document-атрибут (turn off / customize label)** — СДЕЛАНО (ветка
  `fix/table-caption-doc-attr`, 2026-06-09). `:table-caption!:` (unset) → подавляет лейбл «Table N.»
  у ВСЕХ таблиц документа; `:table-caption: Data Set` → меняет слово лейбла, нумерация остаётся
  («Data Set 1. …»). Блочный `caption=` (любое значение) побеждает document-атрибут (литеральный
  префикс без номера). Семантика (верифицирована пробами): счётчик инкрементируется ТОЛЬКО когда
  показывается номер — подавлённый caption (пустой `[caption=]` ИЛИ unset `table-caption`) НЕ
  увеличивает счётчик (следующая таблица сохраняет верный номер). Фикс (`adoc-html/lib.rs`):
  (1) `document_attrs` инициализируется `table-caption`=«Table» (так `:table-caption!:` его удаляет,
  а `{table-caption}` корректно резолвится в «Table», как у asciidoctor); (2) в `start_table`
  caption-рендере `None`-arm (нет блочного `caption=`) консультирует `document_attrs["table-caption"]`:
  `Some(word)`→«{word} N. »+инкремент, `None`→без лейбла; инкремент перенесён внутрь этого arm'а.
  +2 теста. **Корпус: Identical 172→173 (+1)** (turn-off-title-label); blast radius 2 файла:
  1 флип, 0 регрессий, customize-title-label улучшён (2/3 caption'а верны: «Data Set 1./2.»), но
  остаётся Different по др. причинам (Antora-include `example$table.adoc` не резолвится + отдельный
  баг merge attr-блоков: `[caption="Table A. "]`+`.title`+`[cols="3*"]` теряет caption → следующий
  кандидат, см. baseline).
- [x] **Trailing whitespace на строках параграфа** — СДЕЛАНО (ветка
  `fix/paragraph-trailing-whitespace`, 2026-06-09). Asciidoctor rstrip'ит каждую исходную строку:
  trailing-пробелы/табы перед softbreak (и в любой строке блока) не доходят до HTML. Мы их
  сохраняли (`иначе. \nНе` вместо `иначе.\nНе`). **Слой важен**: ASG (parsing-lab, 233 кейса)
  СОХРАНЯЕТ trailing-ws в Text-узле (`Text("==  ")`, `Text("*  ")` — isolated-marker кейсы),
  и compat-тест читает события `Parser` напрямую → обрезать в block.rs/parser.rs НЕЛЬЗЯ (ломает
  2 ASG-кейса). Фикс ТОЛЬКО в рендерере (`adoc-html/lib.rs`, ASG-тест его не использует):
  (1) обычный многострочный параграф парсер склеивает в один `Text` с встроенными `\n`
  (parser.rs multiline-mode) → новый хелпер `rstrip_line_trailing_ws` (CowStr, borrow без
  аллокации) дропает spaces/tabs перед каждым `\n`, применён в Text-arm (`html_escape_text` +
  push_str ветки); хвостовой ` +` hard-break сохраняется (`+` не whitespace), последний сегмент
  без `\n` не трогается (может быть mid-line перед inline-элементом). (2) verbatim-блоки
  (source/listing — без inline-парсинга) идут раздельными Text+SoftBreak → trim в SoftBreak-arm
  (`trim_end_matches([' ','\t'])`). +2 html-теста. **Корпус: Identical 173→175 (+2)** (_responses,
  http-api-design); blast radius 6 файлов: 2 флипа, 0 регрессий, 4 (sdr-004, db-migration,
  cookbook-index/root) улучшены (−2 diff каждый), но Different по др. причинам. Остаток (НЕ нужен
  для флипов): trailing-ws на ПОСЛЕДНЕЙ строке verbatim-блока (перед `</pre>`, без `\n`) не
  обрезается (asciidoctor обрезает) — требует lookahead на End, отдельный мелкий кейс.
- [x] **Header-style колонка таблицы (`h`) → `<th>`** — СДЕЛАНО (ветка
  `fix/table-header-column-style`, 2026-06-09). `[cols="25h,~,~"]` → ячейки `h`-колонки должны
  быть `<th>`, но с обёрткой `<p class="tableblock">` (в отличие от header-ROW ячейки в `<thead>`,
  где обёртки нет). Корень: путь A (`scan_table`) проверял `cell.style == CellStyle::Header`, но
  `cell.style` берётся из спеки ЯЧЕЙКИ, а `h` в `25h` — стиль КОЛОНКИ; `resolve_align` доносил от
  колонки только halign/valign, не стиль. Вдобавок маршрутизация `cell.style==Header → TableHeaderCell`
  латентно неверна для body `h|`-ячеек (давала `<th>` БЕЗ обёртки). Правило (верифицировано пробами):
  тег `<th>` ⇔ (thead-строка) ИЛИ (стиль ячейки = Header); обёртка `<p>` ⇔ НЕ thead. Фикс (2 файла):
  (1) `block.rs::scan_table` — `resolve_style` промоутит Default→Header для `h`-колонки (стили
  `a/e/m/s/l` НЕ наследуются — отдельный концерн, особенно `a`); маршрутизация: thead→`TableHeaderCell`,
  body→`TableCell` с резолвнутым стилем; (2) `adoc-html/lib.rs` — `TableCell` со стилем Header →
  `<th>` (открытие+закрытие), обёртка `<p>` сохраняется. +1 тест (`test_table_header_column_style_html`);
  обновлён `test_table_cell_style_header_in_body_html` (кодировал НЕВЕРНОЕ старое поведение — body
  `h|` без обёртки; верифицировано пробой asciidoctor: `<th><p class="tableblock">...</p></th>`).
  parsing-lab без `h`-таблиц → 233/233 целы. **Корпус: Identical 180→182 (+2)** (width.adoc,
  spec/paragraph.adoc); blast 7 файлов: 2 флипа, 0 регрессий, 5 улучшены (число `<th>` теперь точно
  совпадает с asciidoctor: subs-group-table 7→12, image-position 3→6, strong-span 0→10,
  format-column-content 6→8, pass-macro h|-ячейки верны — все Different по др. причинам, гл. обр.
  ненаследуемый `m`-стиль колонок).
- [x] **Verbatim-параграф сохраняет `//`-комментарий** — СДЕЛАНО (ветка
  `fix/verbatim-paragraph-comment`, 2026-06-09). Asciidoctor читает строки verbatim-параграфов
  СЫРЫМИ → `//`-комментарии внутри них сохраняются как контент; нормальный/quote/example/sidebar/
  pass-параграф их вырезает. Правило (верифицировано пробами): keep-set = **verse, literal,
  listing, source** (рендерятся в `<pre>`); pass/quote/example/sidebar/admonition/нормальный —
  стрипают. Delimited verbatim-блоки (`....`/`----`/`____`) у нас УЖЕ сохраняли комментарии верно —
  баг был ТОЛЬКО в verbatim-ПАРАГРАФАХ (без делимитеров). Корень: `block.rs::scan_paragraph`
  (цикл чтения строк безусловно ломался на `is_line_comment`) и `scan_literal_paragraph` (отступной
  literal ломался на col-0 комментарии). Фикс (2 точки, `block.rs`): (1) `scan_paragraph` — флаг
  `verbatim_paragraph` (из `pending_block_attrs`: `block_style_kind` ∈ {verse,literal,listing} ИЛИ
  `is_source_block`), условие `(!verbatim_paragraph && is_line_comment)`; (2) `scan_literal_paragraph`
  — не ломать на строке-комментарии (`!starts_with(' '/'\t') && !is_line_comment`). +1 тест
  (`test_verbatim_paragraph_keeps_line_comment`: verse+literal сохраняют, нормальный стрипает —
  regression guard). **Корпус: Identical 182→184 (+2)** (verse.adoc, literal.adoc); blast 4 файла:
  2 флипа, 0 регрессий, 2 улучшены (block.adoc 8→7, listing.adoc 25→24 diff-строк, Different по др.
  причинам). parsing-lab 233/233 целы (правка только в paragraph-сканере, не задевает ASG-кейсы).
- [x] **Значение `{attr-ref}` уважает subs блока** — СДЕЛАНО (ветка `fix/attr-ref-respect-block-subs`,
  2026-06-09). Резолвнутое значение `{attr}` подставляется в рамках subs-пайплайна ТЕКУЩЕГО блока:
  в verbatim listing (`[subs="+attributes"]` → SPECIALCHARS|CALLOUTS|ATTRIBUTES, БЕЗ replacements)
  апостроф в значении остаётся прямым (`I've`); в обычном параграфе (NORMAL) — курлится (`I’ve`).
  Корень: `adoc-html/lib.rs::render_inline_value` жёстко форсил `SubstitutionSet::NORMAL` при разборе
  значения атрибута → внутри listing-блока всё равно применялись REPLACEMENTS. Фикс (1 строка +
  коммент): `parse_str_with_subs(value, NORMAL)` → `…(value, self.current_subs())`. В NORMAL-контексте
  поведение НЕ меняется; в verbatim — value идёт одним Text → early-return `html_escape` (прямой
  апостроф, спецсимволы экранируются). +1 тест `test_listing_block_attr_ref_no_replacements`.
  **Корпус: Identical 184→185 (+1)** (listing-blocks.adoc); blast 2 файла: 1 флип, 0 регрессий,
  reference-attributes.adoc КРУПНО улучшен (330→3 diff — attr-ref в verbatim давали позиционный
  каскад), остаётся Different по ОТДЕЛЬНОМУ багу `{url}/issues[text]` (путь между `}` и `[` не
  захватывается `trailing_brackets` — расширение attr-ref-link-macro, вне рамок).
- [x] **Passthrough внутри monospace/quote** — СДЕЛАНО (ветка `fix/passthrough-inside-monospace`,
  2026-06-09). `` `++`++` `` → `<code>`</code>` (passthrough `++…++` внутри monospace даёт литеральный
  backtick). Asciidoctor извлекает passthrough в пре-пасс ДО quote-подстановки, поэтому quote-маркер
  внутри passthrough не закрывает внешний span. Мы парсили слева-направо одним проходом: constrained
  `` ` `` искал закрывающий backtick через `find_closing_constrained` и натыкался на backtick ВНУТРИ
  `++`++` → `<code>++</code>++`)` (сломанный каскад). Фикс (1 точка, `inline.rs`): хелпер
  `passthrough_span_len` (зеркалит матчинг `try_double/triple_plus_passthrough` — non-empty контент,
  ближайший закрывающий `++`/`+++`); `find_closing_constrained` пропускает сбалансированные
  `++…++`/`+++…+++` регионы при поиске закрывающего разделителя (для всех constrained-маркеров
  `*`/`_`/`` ` ``/`#` — общая функция). Внутренний reparse уже корректно эмитит passthrough
  (`InlinePassthrough`, raw). Одиночный `+…+` НЕ трогал (для корпуса не нужен — `` `+*+` ``/`` `+_+` ``
  уже работали, т.к. внутри нет backtick; меньше риска). +1 тест `test_passthrough_inside_monospace`
  (3 кейса: `` `++`++` ``→backtick, `` `++b++` ``→b, `` `x ++ y` ``→незакрытый `++` литерал).
  **Корпус: Identical 186→188 (+2)** (role.adoc, text/index.adoc); blast 5 файлов: 2 флипа,
  0 регрессий, 3 улучшены (bold.adoc/italic.adoc 6→2 diff — body совпал, остаток pre-existing в
  author-блоке standalone-обёртки; troubleshoot-unconstrained 38→36, `_++__kernel++_`→`<em>__kernel</em>`
  точно совпал). parsing-lab 233/233 целы (правка в close-finder, ASG читает события парсера — `br`
  не задет; кейсов с passthrough-внутри-quote в фикстурах нет).
- [x] **Single-plus passthrough `+…+` как constrained-пара** — СДЕЛАНО (ветка
  `fix/single-plus-passthrough-constrained`, 2026-06-09). `` `+kbd:[key(+key)*]+` `` → Asciidoctor
  даёт `<code>kbd:[key(+key)*]</code>` (внутренний `+` сохраняется); мы давали `<code>kbd:[key(key)*]+</code>`,
  т.к. `try_single_plus_passthrough` брал ПЕРВЫЙ встречный `+` как закрывающий. Правило (верифицировано
  пробами): single-plus `+X+` — **constrained-пара** (как `*`/`_`/`` ` ``): open `+` не после word-char
  (`C+a+`→литерал), контент не начинается/кончается пробелом (`+ a+`→литерал), close `+` не перед
  word-char и не часть `++`/`+++` (`+a+b+`→`a+b` — поиск ПРОДОЛЖАЕТСЯ мимо невалидных закрывающих;
  нет валидного close → ведущий `+` литерал, `+a+b`→`+a+b`). Фикс (1 точка, `inline.rs::try_single_plus_passthrough`):
  добавлены `is_word_char_before(start)` guard + space-after-open guard; close-loop теперь скипает `+`,
  за которым word-char ИЛИ перед которым пробел (контент не кончается пробелом). +2 теста
  (`test_single_plus_passthrough_constrained`, `test_monospace_passthrough_inner_plus`). **Корпус:
  Identical 188→189 (+1)** (keyboard-macro); blast 3 файла: 1 флип, 0 регрессий, 2 улучшены
  (asciidoc-vs-markdown 406→404, outline 325→324 raw-difflines — `+` теперь сохраняется как в asciidoctor).
  parsing-lab 233/233 целы. **Обнажённый pre-existing баг** (НЕ регрессия, отложен): trailing ` +`
  в reparsed monospace-контенте → спурьезный `<br>` (база его маскировала, поедая `+`; верифицировано:
  `` `z +` `` даёт `<code>z<br></code>` и на БАЗЕ — независим от правки). Quote-маркеры (`*a*b*`→`<strong>a*b</strong>`)
  тоже должны продолжать поиск, но это БОЛЬШОЙ blast radius (`find_closing_constrained`) — не трогал.
- [x] **Escaped inline-макрос `\name:target[attrs]`** — СДЕЛАНО (ветка `fix/escaped-inline-macro`,
  2026-06-09). `` `\indexterm2:[<primary>]` `` → Asciidoctor снимает `\` и выводит макрос ЛИТЕРАЛОМ
  как текст (`indexterm2:[<primary>]`), макрос НЕ обрабатывается; мы оставляли `\` И обрабатывали
  макрос (`\indexterm2:[primary]`→`\primary`, footnote/image рендерились). Правило (верифицировано
  пробами): `\` снимается ТОЛЬКО перед макросом, который Asciidoctor распознаёт по умолчанию
  (stem/latexmath/asciimath/link/xref/mailto/icon/image/indexterm/indexterm2/footnote; pass уже был);
  experimental kbd/btn/menu и custom-catch-all (`\notamacro:foo[bar]`) → `\` СОХРАНЯЕТСЯ (не макрос →
  нечего экранировать); block-форма `image::` тоже не трогается. Действует и вне monospace, и внутри
  (`` `...` `` reparse имеет macros вкл). Фикс (1 точка + хелпер, `inline.rs`): новый arm в
  `handle_inline_escape` после `\pass:`; хелпер `inline_macro_escape_len(p)` (gated на MACROS) —
  матчит распознаваемое имя+`:` (не `::`), target `[^\s\[]*`, `[`…`]`; возвращает длину run.
  Снимаем `\`, эмитим весь `name:target[attrs]` ОДНИМ `Event::Text` (рендерер экранирует спецсимволы).
  +2 теста (`test_escaped_inline_macro` 6 кейсов, `test_backslash_before_unrecognized_macro_kept`).
  **Корпус: Identical 189→190 (+1)** (user-index.adoc); blast radius **1 файл, 1 флип, 0 регрессий**
  (escaped `\image::` block в outline.adoc корректно не тронут). parsing-lab 233/233 целы (escaped-
  макросов в фикстурах нет). clippy 0, test --workspace зелёное (parser 449→451).
- [ ] **Точечные**: п.17 (остаток: `[.line-through]#`→`<del>`, `#`→`<mark>`; inline-роль
  на `_`/`*`/`` ` `` уже сделана в п.13/16),
  п.20 (`[[id,reftext]]`),
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
