# Session context

## Последняя сессия (2026-06-09, поздняя-9) — Фаза 3: `table-caption` document-атрибут

link-macro-empty-bare смержена в master (`c37bcf6`, origin == master, дерево чистое). `/tmp/adoc_base`
пересобран из master `c37bcf6` ПЕРЕД правкой. Baseline корпуса подтверждён: Identical 172, Different
172, Errors 0. Выбор кандидата по near-miss на 172: 1-diff кандидаты (verse `// end::para[]`, kbd
`+`-разделитель, listing-blocks subs) рискованны/архитектурны; среди 2-diff `turn-off-title-label`
(оба diff — один корень: подавление лейбла «Table N.») оказался самым чистым.

### Ветка `fix/table-caption-doc-attr` (от master; НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано пробами, НЕ по памяти): лейбл таблицы = `{table-caption} N. `
  где `table-caption` — built-in document-атрибут, дефолт «Table». `:table-caption!:` (unset) →
  лейбл подавлён у ВСЕХ таблиц; `:table-caption: Data Set` → «Data Set N. ». Блочный `[caption=…]`
  (любое значение, в т.ч. пустое) ПОБЕЖДАЕТ document-атрибут: литеральный префикс БЕЗ номера
  (`[caption="X "]`→«X Title», `[caption=]`→«Title»). **Счётчик инкрементируется ТОЛЬКО когда
  показан номер** — подавлённый caption (блочный `caption=` ИЛИ unset `table-caption`) НЕ увеличивает
  счётчик (T1=«Table 1.», подавлённая T2, T3=«Table 2.»). `{table-caption}` резолвится в «Table».
- **Корень**: `adoc-html/lib.rs::start_table` caption-рендер хардкодил «Table N.» в `None`-arm и
  инкрементировал `table_counter` БЕЗУСЛОВНО (перед match) — игнорировал document-атрибут.
- **Фикс** (2 точки): (1) `document_attrs` инициализируется `table-caption`=«Table» (стр ~255) —
  так `:table-caption!:` удаляет ключ (existing `apply_attribute` strip_suffix('!')→remove), а
  `{table-caption}` корректно резолвится; (2) `None`-arm (нет блочного `caption=`) консультирует
  `document_attrs.get("table-caption").cloned()`: `Some(label)`→инкремент+«{label} N. »(html_escape),
  `None`→без лейбла. Безусловный инкремент убран, перенесён внутрь `Some(label)`-ветки. Блочные
  `Some("")`/`Some(prefix)` arm'ы НЕ инкрементируют (correct). +2 теста (`test_table_caption_doc_attr_html`:
  unset/custom-numbered/{table-caption}-ref; `test_table_caption_suppressed_not_counted_html`).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (html 309→311, parser 443).
- Корпус `compare_full.py` (release): **Identical 172→173 (+1), Different 171, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `c37bcf6`): **2 файла**
  изменили вывод — **1 FLIP→IDENTICAL** (turn-off-title-label), **0 регрессий**. customize-title-label
  улучшён (2/3 caption'а верны: «Data Set 1./2.»), но остаётся Different по др. причинам (Antora-
  include `example$table.adoc` не резолвится; colgroup; + отдельный merge-баг ниже).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/table-caption-doc-attr` (только по запросу).
- **СЛЕДУЮЩИЙ чистый кандидат — merge attr-блоков** (обнаружен в этой сессии): `[caption="Table A. "]`
  + `.title` + `[cols="3*"]` → второй attr-блок (`[cols]`) ЗАТИРАЕТ `caption=` из первого. Asciidoctor
  МЁРЖИТ несколько `[...]` строк вокруг заголовка. Корень — в block.rs (накопление block-attrs).
  Затрагивает customize-title-label (caption 3/3) и turn-off-title-label table B (уже флипнул через
  `:table-caption!:`, но caption= там тоже теряется). Проверить blast radius (merge может задеть много).
- Прочие near-miss на 173 (1-diff, рискованные): verse `// end::para[]` (comment-handling блок-сканера),
  kbd `+`-разделитель (passthrough `+...+`), listing-blocks subs (`[subs="+attributes"]`).
- Другие 2-diff: width.adoc (`th` vs `td` — header-row в include `row.adoc[tag=base-h]`),
  counter.adoc (`{index}` ref после `{counter:index}` не резолвится — counter не пишет в attrs, п.36),
  _responses/http-api-design (trailing-space перед softbreak).
- Архитектурные (отложены): `{attr-ref}[text]` (порядок subs), link-role `class="external"`,
  nested-форматирование в тексте ссылки, inline-monospace passthrough char-ref (`Event::Code`).

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `c37bcf6`). near-miss `/tmp/nearmiss.py` (вывод в
  `/tmp/nearmiss_out.txt`). Сравнение семантическое (DOM). LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-8) — Фаза 3: `link:url[]` пустой текст → `class="bare"` (п.14)

bare-char-reference-preserved УЖЕ смержена в master (`8b7ee64`, origin == master; worktree
`/tmp/master-wt` убран, `/tmp/adoc_base` пересобран из master перед правкой). session.md прошлой
сессии была устаревшей (писалась до мержа). Выбор кандидата: near-miss на 171 дал 3 «1-diff»
(verse `// end::para[]`, keyboard-macro `+`-passthrough, listing-blocks subs) — все рискованные/
обрезанные. Кластер `index/icons-font/auto-ids` = `{url-xxx}[text^]` (attr-ref как target —
архитектурный subs-order, отложен). Чистый флип нашёлся в README.adoc (2 diff, оба `link:X[]`
без `class="bare"`).

### Ветка `fix/link-macro-empty-bare` (от master; НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано пробами, НЕ по памяти): link-макрос/URL-макрос/autolink
  БЕЗ явного текста (`link:LICENSE[]`, `https://x[]`, `link:++url++[]`, голый `https://x`) →
  «bare» → `class="bare"` (видимый текст = target). Явный текст (даже равный target,
  `link:LICENSE[LICENSE]`) → НЕ bare. **Исключения**: `mailto:a@b.com[]` НЕ bare; email-autolink
  `a@b.com` тоже НЕ bare у asciidoctor (у нас ставится bare — ПРЕДСУЩЕСТВУЮЩЕЕ расхождение в
  ОБРАТНУЮ сторону, отдельный путь `try_email_autolink:1817` + тест `test_email_autolink_html`
  кодирует старое; НЕ трогал — мой фикс его не регрессирует). С ролью asciidoctor даёт
  `class="bare external"` — роль на ссылке у нас не захватывается (нет поля role в `Tag::Link`).
- **Корень**: `inline.rs` — `link:`/url-макрос при пустом тексте пушил URL как видимый текст, но
  `is_bare: false` жёстко. Bare ставился только в голом autolink (`try_autolink:1747`).
- **Фикс**: `let is_bare = link_attrs.text.is_empty();` в 3 точках — `try_link_macro` (`++url++`-путь
  ~1389 и обычный ~1429) и `try_autolink` with-text при пустом `[]` (~1730). mailto (~1479) НЕ
  тронут (остаётся `is_bare: false`). +2 теста (`test_link_macro_empty_text_is_bare` в inline.rs:
  link/explicit/url-empty; `test_link_macro_empty_text_bare_class` в lib.rs: render + mailto-NOT-bare).
  Обновлены 2 теста под верное поведение: `test_macro_label_replacements` (`link:a'b.html[]`
  false→true), `test_link_passthrough_url_empty_text` (добавлен `class="bare"`; verified пробой).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 442→443, html 307→309).
- Корпус `compare_full.py` (release): **Identical 171→172 (+1), Different 172, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` пересобран из чистого master `8b7ee64`):
  **2 файла** изменили вывод — **1 FLIP→IDENTICAL** (README.adoc), **0 регрессий**. url.adoc
  улучшен (`link:tools.html#editors[]`→`class="bare"` верно, verified vs asciidoctor), но остаётся
  Different по др. причинам (irc custom-macro, nested `*…*` в тексте ссылки, sect0-обёртка, link-role
  `class="green"`). TODO.md: baseline 171→172, п.14 под-пункт `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/link-macro-empty-bare` (только по запросу).
- Следующие чистые flip-кандидаты (near-miss на 172, все 1-diff, все рискованные/архитектурные):
  - **verse `// end::para[]`** утечка тег-региона: verse-параграф (verse.adoc) КЕЕРS comment-строку
    в выводе, мы дропаем как комментарий. Трогает comment-handling блок-сканера в verbatim — риск шире.
  - **kbd `+`-разделитель** `kbd:[key(+key)*]` (keyboard-macro): mono-literal `+...+` passthrough
    ест внутренний `+` → даём `kbd:[key(key)*]+`. Passthrough-парсинг, риск выше.
  - **listing-blocks subs** (`[subs="+attributes"]` внутри outer `------` listing): обрезанный
    1-diff, надо смотреть полностью — вероятно `{replace-me}` или verbatim-нюанс.
  - **inline-anchor reftext из dt-терма** `[[id]]term::` (lexicon, ~14 ссылок; БОЛЬШЕ по объёму).
- Архитектурные (отложены): `{attr-ref}[text]` как target ссылки (порядок subs — index/icons-font/
  auto-ids/custom-ids), link-role `class="external"`/`green` (нет поля role в `Tag::Link`),
  nested-форматирование в тексте ссылки (`link:u[*bold*]`), inline-monospace passthrough char-ref.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `8b7ee64`). near-miss `/tmp/nearmiss.py`. Сравнение
  семантическое (DOM): `class="bare"` на `<a>` сравнивается (виден как `attr_diff on <a>`).
  LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-7) — Фаза 3: preserve bare char-ref (остаток п.15)

literal-unknown-style-class смержена в master (`a4547fd`, origin == master). Выбран следующий
чистый flip по near-miss на 170 — `§`/bare char-ref (title-links, 2-diff один корень). Кандидаты
verse `// end::para[]` (1-diff, но трогает comment-handling блок-сканера — рискованнее) и
keyboard-macro `kbd:[key(+key)*]` (1-diff, passthrough `+...+` — фидли) отложены.

### Ветка `fix/bare-char-reference-preserved` (от master; НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано пробами `[subs=…]`, НЕ по памяти): валидный char-ref
  (`&#167;`/`&copy;`/`&amp;`) в тексте сохраняется как сущность ТОЛЬКО при `specialchars`+
  `replacements` ВМЕСТЕ (specialchars экранирует `&`→`&amp;`, replacements разэкранирует валидный
  ref обратно); `[subs=specialchars]`-only → экранирует (`&amp;#167;`); `replacements`/`quotes`/
  `none` (без specialchars) → `&` и так не экранируется. Невалидный (`&#1;` 1 цифра, bare `&`,
  без `;`) → экранируется. Verbatim (specialchars БЕЗ replacements) → экранирует (совпадает).
- **Фикс 1 (`inline.rs::parse_inline`)**: в главном цикле новый arm перед fallthrough — bare `&`,
  начинающий валидный char-ref (переиспользует `char_ref_len_at` из backslash-ветки п.15), при
  `preserve_char_refs = specialchars && replacements` эмитится как `Event::InlinePassthrough`
  (raw; рендерер не экранирует, в отличие от `Event::Text`). +2 теста (`test_bare_char_reference_
  preserved`: 167/copy/amp/x1F600 → passthrough; `test_bare_invalid_char_reference_not_preserved`).
- **Фикс 2 (`parser.rs`, СОПУТСТВУЮЩИЙ — обязателен)**: литеральный параграф без `[attr]` НЕ
  эмитит `BlockMetadata(VERBATIM)` (block.rs:1877 — только при наличии attrs) → в parser.rs:90
  `Tag::LiteralParagraph` падал на `current_subs()`=NORMAL вместо VERBATIM. Латентный баг (был
  безвреден: рендерер всё равно экранировал `&` под specialchars NORMAL), но фикс 1 его обнажил —
  preserve_char_refs срабатывал в literal-параграфе → регрессия attribute-entry-substitutions
  (`&amp;` → `&amp;` вместо `&amp;amp;`). Дефолт изменён на `unwrap_or(VERBATIM)` (как у
  SourceBlock/DelimitedBlock Literal/Listing). Это и корректнее (literal-параграф verbatim по
  определению), и закрывает регрессию. ПОБОЧНО улучшило outline.adoc literal-пример `*\*foo**`
  (теперь verbatim, совпал с asciidoctor) и pass-macro.

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 440→442, html 308).
- Корпус `compare_full.py` (release): **Identical 170→171 (+1), Different 173, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` ПЕРЕСОБРАН из чистого master a4547fd через
  worktree `/tmp/master-wt` — старый base был устаревший!): **12 файлов** изменили вывод —
  **1 FLIP→IDENTICAL** (title-links), **0 регрессий**. 11 остались Different по др. причинам, но
  их char-ref/литеральные параграфы стали верны (проверены поштучно: число diff'ов better/same,
  кроме outline.adoc — там +24 ложно от позиц. рассинхрона в файле с 8800+ diff; контент verified
  совпал с asciidoctor). Остаток: inline-monospace passthrough char-ref `` `&#167;` `` в `<code>`
  (`Event::Code`, не задет) — replacements.adoc остаётся Different по этой причине.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/bare-char-reference-preserved` (только по запросу).
  NB: создан git worktree `/tmp/master-wt` (для чистого base-бинаря) — удалить после
  (`git worktree remove /tmp/master-wt`). `/tmp/adoc_base` теперь = чистый master a4547fd.
- Следующие чистые flip-кандидаты (по near-miss на 171, 1-diff):
  - **`// end::para[]` утечка** тег-региона: verse-параграф (verse.adoc) и literal-параграф
    (literal.adoc `// end::indent[]`) КЕЕРS comment-строку в выводе, мы дропаем как комментарий.
    Трогает comment-handling блок-сканера в verbatim/paragraph-контексте — осторожно (риск шире).
  - **kbd `+`-разделитель** `` `+kbd:[key(+key)*]+` `` (keyboard-macro): mono-literal passthrough
    `` `+...+` `` ест внутренний `+` → даём `kbd:[key(key)*]+`. Passthrough-парсинг, риск выше.
  - **inline-anchor reftext из dt-терма** `[[id]]term:: ...` → `<<id>>` = текст терма (lexicon,
    ~14 ссылок; родственно bibliography, но захват текста терма в парсере — БОЛЬШЕ по объёму).
- Архитектурные (отложены): inline-monospace passthrough char-ref (`Event::Code`), nested-
  форматирование/`{attr}` в тексте макроса, `{attr-ref}[text]` (порядок subs), link-role
  `class="external"`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base в `/tmp/adoc_base` — ПЕРЕСОБРАН из чистого master). near-miss `/tmp/nearmiss.py`.
  Сравнение семантическое (DOM): `&#167;`/`§` декодируются HTMLParser'ом одинаково → diff виден
  через escaped `&amp;#167;` vs raw `&#167;`. LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-6) — Фаза 3: неизвестный verbatim-style → class

admonition-custom-caption уже смержена в master (`cc390a0`, origin == master). Выбран самый
чистый 1-diff по near-miss на 169: неизвестный verbatim-стиль на delimited-блоке (monitoring).
При эмпирической пробе оказалось ШИРЕ заявленного в session.md: баг есть и на literal (`....`),
и на listing (`----`).

### Ветка `fix/literal-unknown-style-class` (СМЕРЖЕНА+ЗАПУШЕНА в master, `a4547fd`)
- **Правило Asciidoctor** (верифицировано пробами): verbatim delimited-блок (literal/listing)
  берёт CSS-класс ТОЛЬКО из контекста; неизвестный блок-стиль (`[plantuml]`, `[ditaa]`,
  `[src,yaml]`) ОТБРАСЫВАЕТСЯ из class. `[plantuml]....`→`literalblock` (мы: `literalblock
  plantuml`), `[plantuml]----`/`[src,yaml]----`→`listingblock` (мы: `listingblock plantuml`/`src`).
  Роли (`[plantuml.diagram]`) и id СОХРАНЯЮТСЯ (`literalblock diagram`). `[literal]`/`[listing]`
  на `....`/`----` → контекст-конверсия в парсере (уже верно). `[source,lang]` идёт ОТДЕЛЬНЫМ
  путём `Tag::SourceBlock` (style→language/data-lang) — НЕ задет.
- **Корень**: `write_meta_attrs` (adoc-html/lib.rs:~2543) дописывает `meta.style` в class после
  default_class. Для verbatim-блоков это неверно.
- **Фикс** (`adoc-html/lib.rs`): хелпер `strip_block_style(meta)` (клон с `style=None`), применён
  в arm'ах `Literal`+`Listing` в `start_delimited_block` (стр ~1750). Узко: только эти 2 arm'а.
  +1 тест `test_verbatim_block_unknown_style_dropped_from_class` (literal+listing drop, role survives).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (html 307→308, parser 440).
- Корпус `compare_full.py` (release): **Identical 169→170 (+1), Different 174, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` из master): **5 файлов** изменили вывод —
  **1 FLIP→IDENTICAL** (monitoring), **0 регрессий**. 4 остались Different по др. причинам
  (architecture/index, java/index, software-development-cookbook — include-родители monitoring;
  db-migration — admonition `div vs table`), НО их verbatim-стиль теперь верен (`[src,yaml]`/
  `[plantuml]` стиль отброшен). TODO.md: baseline 169→170, п.40-смежное под-пункт `[x]`.

### Что дальше
- Ветка смержена+запушена (`a4547fd`, origin == master). Локальная ветка удалена.
- Следующие чистые flip-кандидаты (по near-miss на 170):
  - **inline-anchor reftext из dt-терма** `[[id]]term:: ...` → `<<id>>` = текст терма (lexicon,
    ~14 ссылок; родственно bibliography, но захват текста терма в парсере — БОЛЬШЕ по объёму).
  - **kbd `+`-разделитель** `kbd:[key(+key)*]` → `+...+` инлайн-пасстру ест `+` (keyboard-macro).
  - **`§`/bare char-ref** сохранять как сущность (title-links — остаток п.15).
  - **`// end::para[]` утечка** тег-региона (verse, literal).
- Архитектурные (отложены): nested-форматирование/`{attr}` в тексте макроса, `{attr-ref}[text]`
  (порядок subs), link-role `class="external"`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (нужен base-бинарь в `/tmp/adoc_base` — копировать ДО изменений). near-miss `/tmp/nearmiss.py`.
  Сравнение семантическое (DOM). LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-5) — Фаза 3: custom caption на админишене

macro-text-replacements уже смержена в master (`e2c0b96`, origin == master). Выбран следующий
чистый flip по near-miss на 168 — самый чистый 1-diff: `[caption="…"]` на админишене (glossary).

### Ветка `fix/admonition-custom-caption` (от master; НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано пробами): блочный `[caption="Work in Progress"]` перед
  `CAUTION:` → отображаемый label = caption (вместо дефолтного «Caution»), но класс
  `admonitionblock caution` и `icon-caution` остаются по ТИПУ. text-режим →
  `<div class="title">caption</div>`; `icons=font` → `title="caption"` у `<i class="fa icon-caution">`;
  пустой `[caption=]` → пустой title (`<div class="title"></div>`). Asciidoctor caption НЕ
  экранирует (`A & B` сырьём) — я экранирую (дисциплина D1/D7, строже; glossary без спецсимволов).
- **Корень**: `adoc-html/lib.rs::start_admonition` эмитил жёсткий `label` в обеих ветках.
- **Фикс**: извлечь `caption` из `meta.named` (парсер УЖЕ его захватывает — `BlockAttributes.named`,
  как для table-caption на 1422 и figure на 1769); в обеих ветками рендера (text-title и
  `icons=font` title-attr) `match caption { Some(c)=>html_escape, None=>label }`. icons=font
  ветка переписана с `writeln!` на push_str-цепочку (нужен условный html_escape). +1 тест
  `test_admonition_custom_caption` (caption-override + класс по типу + пустой + экранирование).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (html 306→307, parser 440).
- Корпус `compare_full.py` (release): **Identical 168→169 (+1), Different 176→175, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` из master): **ровно 1 файл** изменил вывод —
  **1 FLIP→IDENTICAL** (glossary), **0 регрессий**. Остальные 4 файла с `[caption=` (add-title,
  syntax-quick-reference, customize-title-label, turn-off-title-label) — caption на ТАБЛИЦАХ
  (отдельный путь 1422), не затронуты. TODO.md: baseline 168→169.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/admonition-custom-caption` (только по запросу).
- Следующие чистые flip-кандидаты (по near-miss на 169, все 1-diff):
  - **неизвестный verbatim-style → class** `[plantuml]` на literal-блоке (`....`) → Asciidoctor
    даёт `class="literalblock"` (style ОТБРАСЫВАЕТСЯ); мы — `literalblock plantuml`
    (monitoring.adoc). NB: осторожно — НЕ регрессировать listing (там style→language верно);
    475 файлов матчат `^[word]$`, нужна узкая правка только для literal + неизвестный style.
  - **kbd `+`-разделитель** `` `+kbd:[key(+key)*]+` `` (keyboard-macro): `+...+` инлайн-пасстру
    ест внутренний `+` → даём `kbd:[key(key)*]+` вместо `kbd:[key(+key)*]`. Пасстру-парсинг, риск выше.
  - **`§`/bare char-ref** сохранять как сущность (title-links — остаток п.15; `&#167;` vs `§`).
  - **`// end::para[]` утечка** тег-региона (verse: Asciidoctor КЕЕРS comment в verse-блоке).
  - **inline-anchor reftext из dt-терма** `[[id]]term::` (lexicon, ~14 ссылок; БОЛЬШЕ по объёму).
- Архитектурные (отложены): nested-форматирование/`{attr}` в тексте макроса, `{attr-ref}[text]`
  (порядок subs), link-role `class="external"`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (нужен base-бинарь в `/tmp/adoc_base` — копировать ДО изменений). near-miss `/tmp/nearmiss.py`.
  Сравнение семантическое (DOM). LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-4) — Фаза 3: REPLACEMENTS в тексте макроса (остаток п.37)

xref-fallback-bracketed-id уже смержена в master (`8db12ea`, origin == master). Выбран следующий
чистый flip по near-miss на 165 — кластер «апостроф в тексте макроса» (scope, subs/index — по
1-diff; span-cells — 2-diff, оба апострофа в `xref:[label]`).

### Ветка `fix/macro-text-replacements` (от master; НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано чтением subs-порядка, НЕ по памяти): подстановки идут
  specialchars→quotes→attributes→**replacements**→**macros**→post_replacements. Replacements
  выполняется ДО macros, на ВСЕЙ строке (включая текст внутри `[...]`), поэтому к моменту обработки
  link/xref-макроса апостроф/дефис/стрелки в `[label]` уже сконвертированы. URL/target (фолбэк,
  когда `[...]` пуст) НЕ курлится — бэйр-URL защищён как macro-вывод.
- **Корень**: `inline.rs` — display-текст макроса эмитился сырым `Event::Text(Cow::Borrowed(display))`,
  минуя REPLACEMENTS (которые `flush_text` применяет к обычному тексту через `apply_typographic_replacements`).
- **Фикс**: новый хелпер `push_macro_label(&self, text: &'a str, events)` (зеркалит REPLACEMENTS-
  ветку `flush_text`). Применён к **явному** label в 6 точках: `try_link_macro` (++url++ и link:),
  `try_mailto_macro`, autolink-с-текстом (`https://u[text]`), `try_xref_macro` (`xref:t[label]`),
  `try_cross_reference` (`<<id,label>>`). Паттерн: `if text.is_empty() { push raw url } else
  { push_macro_label }`; для xref — `match label { Some(Borrowed)=>push_macro_label, Some(o)=>raw,
  None=>raw target }`. +1 тест `test_macro_label_replacements` (link/xref/`<<>>` курлят апостроф;
  бэйр-URL `link:a'b.html[]` остаётся сырым).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 439→440, html 306).
- Корпус `compare_full.py` (release): **Identical 165→168 (+3), Different 176, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` из master vs new): **11 файлов** изменили
  вывод; **3 FLIP→IDENTICAL** (subs/index, span-cells, scope), **0 регрессий** (0 Identical→Different),
  8 остались Different по НЕ-апостроф причинам (CONTRIBUTING, README, add-cells-and-rows, align-by-
  cell/column, build-a-basic-table, duplicate-cells, format-cell-content — все table-доки с
  `xref:[cell's...]` + `class="bare"`/др.; апостроф в них стал верным). TODO.md: baseline 165→168.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/macro-text-replacements` (только по запросу).
- Следующие чистые flip-кандидаты (по near-miss на 168):
  - **inline-anchor reftext из dt-терма** `[[id]]term:: ...` → `<<id>>` = текст терма (lexicon.adoc,
    ~14 ссылок; родственно bibliography, но захват текста терма в парсере — БОЛЬШЕ по объёму).
  - **custom caption на админишене** `[caption="Work in Progress"]` → caption вместо дефолтного
    «Caution» (glossary.adoc, 1-diff).
  - **неизвестный verbatim-style → class** `[plantuml]` на literal-блоке (`literalblock plantuml`
    вместо `literalblock`; monitoring.adoc, 1-diff — остаток п.40-смежное).
  - **kbd `+`-разделитель** `kbd:[key(+key)*]` → мы даём `kbd:[key(key)*]+` (keyboard-macro, 1-diff).
  - **`§`/bare char-ref** сохранять как сущность (title-links — остаток п.15).
  - **`// end::para[]` утечка** тег-региона (verse, literal).
- Архитектурные (отложены): nested-форматирование/`{attr}` в тексте макроса (полный inline-проход),
  `{attr-ref}[text]` (порядок subs), link-role `class="external"`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (нужен base-бинарь в `/tmp/adoc_base` — копировать ДО изменений). near-miss `/tmp/nearmiss.py`.
  Сравнение семантическое (DOM) — `’`/`&#8217;` нормализуются. LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-3) — Фаза 3: xref fallback `[id]` + bibliography reftext

link-blank-window-caret УЖЕ смержена+запушена в master (`2e53399`, origin == master). Удалены
устаревшие локальные ветки image-alt-quotes / xref-id-normalization. Выбран следующий чистый
flip по near-miss на 162 — крупнейший 1-diff кластер: bibliography `[pp]`. При эмпирической
пробе оказалось ШИРЕ: общее правило fallback-текста внутреннего xref.

### Ветка `fix/xref-fallback-bracketed-id` (от master; НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано пробами, НЕ по памяти): внутренний `<<id>>` без
  явного текста, чей id НЕ резолвится (нет секции/блока/bibliography) → текст = `[id]`
  (в скобках, default xreflabel), НЕ сырой `id`. Bibliography — частный случай: `[[[pp]]]`→
  `<<pp>>`=`[pp]`; `[[[gof,gang]]]`→`<<gof>>`=`[gang]` (reftext=label в скобках, НЕ `[gof]`).
  Явный текст (`<<id,текст>>`) и natural xref (target==заголовок секции, БЕЗ скобок) побеждают.
  Inter-document (`<<f.adoc#s>>`) НЕ бракетится (путь `.html` сырой).
- **Фикс** (`adoc-html/src/lib.rs`, ленивая резолюция в `finish()`, как для текста xref):
  - новое поле `bibliography_reftexts: Vec<(String,String)>` (id → `[label|id]`), заполняется
    в `push_event` на `Event::BibliographyAnchor` (рендер тот же `[label]`, плюс push в реестр);
  - `xref_placeholders` расширен с 2- до 3-кортежа: `(placeholder, fallback, is_internal)`;
    `is_internal = !is_interdoc` в `start_cross_reference`; обновлены `.last()` (стр ~474),
    push (~1348);
  - текстовая резолюция в `finish()` теперь зеркалит href-резолюцию: `id_to_text.get` (id) →
    `title_to_id`→`id_to_text` (natural xref, БЕЗ скобок) → `[fallback]` если internal → raw;
    `bibliography_reftexts` влиты в `id_to_text`; biblio-id добавлены в `known_ids` (href).
- +4 теста (lib: bibliography-xref bracketed, unresolved→bracket+interdoc-raw+explicit-wins,
  resolved-natural-not-bracketed). 2 старых теста (`test_full_document`,
  `test_xref_unresolvable_falls_back_to_id` в `tests/html_output.rs`) кодировали НЕВЕРНОЕ старое
  поведение (сырой id) → обновлены под `[id]` (проверено пробой asciidoctor: `[introduction]`).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (html lib 302→306,
  html_output 35, parser 439).
- Корпус `compare_full.py` (release): **Identical 162→165 (+3), Different 179, Errors 0**.
- Blast radius (`/tmp/blast.py`, base-бинарь `/tmp/adoc_base` из master vs new): **7 файлов**
  изменили вывод; **3 FLIP→IDENTICAL** (xref.adoc, bibliography, _crud), **0 регрессий**
  (0 Identical→Different), 4 остались Different по НЕ-xref причинам, НО их xref-ссылки теперь
  верны (data-format, _responses, subs/index — xref IDENTICAL; lexicon — остаток ниже).
- TODO.md: baseline 162→165, новый пункт `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/xref-fallback-bracketed-id` (только по запросу).
- Следующие чистые flip-кандидаты (по near-miss на 165):
  - **inline-anchor reftext из dt-терма** `[[id]]term:: ...` → `<<id>>` = текст терма
    (lexicon.adoc: ~14 ссылок `boxed-attrlist`→`boxed attribute list`, `attribute`→`attribute`).
    ВЕРИФИЦИРОВАНО пробой: якорь в НАЧАЛЕ dt-терма берёт reftext из текста терма; якорь в
    параграфе (`[[plain]]...`) reftext НЕ имеет → `[plain]` (наш bracket ВЕРЕН). Родственно
    bibliography, но требует захвата текста терма в парсере (БОЛЬШЕ по объёму). НЕ регрессия
    моей правки (lexicon был Different и до неё; 0 файловых регрессий по blast radius).
  - **апостроф `'`→’ в тексте/макросе** (scope, span-cells, README — остаток п.37).
  - **`§`/bare char-ref** сохранять как сущность (title-links — остаток п.15).
  - **`// end::para[]` утечка** тег-региона (verse, literal).
- Архитектурные (отложены): `{attr-ref}[text]` (порядок subs), link-role `class="external"`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (нужен base-бинарь в `/tmp/adoc_base` — копировать ДО изменений). near-miss `/tmp/nearmiss.py`.
  Сравнение семантическое (DOM). LSP для навигации, context7 MCP для доков.

---

## Сессия (2026-06-09, поздняя-2) — Фаза 3: link blank-window `^` (п.14)

п.19 xref-id-normalization уже смержена в master (755b320). Выбран следующий чистый flip
по near-miss на baseline 158 — крупнейший кластер: суффикс `^` в тексте ссылки.

### Ветка `fix/link-blank-window-caret` (от master; НЕ закоммичено)
- **Симптом**: `https://u[text^]` / `link:u[text^]` → мы оставляли `^` в видимом тексте
  (`macro^`) и НЕ добавляли `target="_blank" rel="noopener"`. Asciidoctor: `^` = blank-window
  shorthand → снять каретку, открыть в новом окне.
- **Семантика** (верифицирована пробами): trailing `^` на тексте ссылки → `window=_blank`
  (рендер: `target="_blank" rel="noopener"`), `^` снимается; явный `window=` побеждает каретку;
  работает для bare-URL, `link:`, mailto, `++url++` (все идут через `parse_link_attrs`).
- **Корень**: `attributes.rs::parse_link_attrs` пушил `text` (первый positional) сырым.
- **Фикс** (1 место, централизованно): после `let mut text = positional.first()...` —
  `if let Some(stripped) = text.strip_suffix('^') { text = stripped; if window.is_none()
  { window = Some("_blank"); } }`. Инфраструктура (`Tag::Link.window/nofollow`, рендер
  `target`/`rel` в `adoc-html/lib.rs:1128`) УЖЕ была — фикс минимален. +1 unit-тест
  `test_link_attrs_blank_window_caret` (4 кейса: caret, caret+role, no-caret, explicit-window).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 438→439).
- Корпус `compare_full.py` (release): **Identical 158→162 (+4), Different 182, Errors 0**.
- Blast radius (base-бинарь из master через `git worktree` vs new, `/tmp/check_blast.py`):
  9 файлов изменили вывод; из них **4 FLIP→IDENTICAL** (description, image-format,
  xref-text-and-style, key-concepts), 5 остались Different по НЕ-caret причинам
  (url — link-role `class="external"`; asciidoc-vs-markdown — md-fences; image-svg,
  ts-url-format, bibliography). **0 регрессий** (0 Identical→Different; net +4 точно объяснён;
  caret-ссылки в 5 оставшихся теперь совпадают с Asciidoctor по target/rel). worktree удалён.
- TODO.md: baseline 158→162, п.14 → `[~]` с под-пунктом blank-window `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/link-blank-window-caret` (только по запросу).
- Следующие чистые flip-кандидаты (near-miss на 162):
  - **bibliography-anchor `[pp]`** — `[[[pp]]]`/`<<pp>>` → reftext в скобках `[pp]`
    (1-diff кластер: _crud `[search_and_sort]`, bibliography `[pp]`/`[gang]`, subs/index
    `[table-subs-groups]`, xref.adoc `[anchors]`/`[paragraphs]`, _responses). `[id]` exp vs `id` got.
  - **апостроф `'`→’** в plain-тексте под каким-то контекстом (scope, span-cells, README) —
    остаток п.37; в тексте макроса (xref/link) — отдельно (архитектурно).
  - **`§`/bare char-ref** сохранять как сущность (title-links — остаток п.15).
  - **`// end::para[]` утечка** тег-региона (verse, literal — Asciidoctor КЕЕРS comment в verse).
- Архитектурные (отложены): `{attr-ref}[text]` (порядок subs — icons-font/auto-ids/custom-ids/
  index), link-role `class="external"` (нет поля role в `Tag::Link` — расширение типа).

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). near-miss `/tmp/nearmiss.py`.
  Сравнение семантическое (DOM). LSP для навигации, context7 MCP для доков.

---

## Сессия (2026-06-09, поздняя) — Фаза 3: п.19 xref-id норм. (natural cross reference)

Следующий чистый flip после image-alt-quotes (тот уже смержен+запушен, master == origin/master).

### Ветка `fix/xref-id-normalization` (от master; НЕ закоммичено)
- **Симптом**: `<<Substitutions>>` + секция `== Substitutions` (forward-ссылка) → мы давали
  `href="#Substitutions"`, Asciidoctor — `href="#_substitutions"` (id секции).
- **Семантика Asciidoctor** (верифицирована пробами, НЕ по памяти):
  - target == **заголовок секции** (case-sensitive) → id этой секции (auto `_substitutions`
    ИЛИ явный `[#myid]` → `#myid`);
  - target — зарегистрированный id → остаётся как есть;
  - иначе сырой target (`<<Foo Bar>>`→`#Foo Bar`, `<<substitutions>>` (lower) → не матчит);
  - резолюция href НЕ зависит от наличия текста (`<<T,текст>>` тоже резолвит href).
- **Корень**: `adoc-html/src/lib.rs::start_cross_reference` писал href сразу из сырого target.
  Но это forward-ссылка → нужна ленивая резолюция в `finish()` (как уже для текста xref).
- **Фикс**:
  - новое поле `xref_href_placeholders: Vec<(String,String)>` (placeholder, raw target);
  - в `start_cross_reference` (internal-ветка) вместо `html_escape(target)` пишу
    плейсхолдер `\x00XREFHREF_N\x00` (счётчик `xref_placeholder_counter` переиспользован;
    префикс XREFHREF ≠ XREF → подстроки не пересекаются при `replace`);
  - в `finish()` отдельный блок: `known_ids` (из toc_entries + block_ref_titles),
    `title_to_id` (из toc_entries, first-wins). Резолв: known id → как есть; иначе title→id;
    иначе сырой. `html_escape` + `output.replace`.
- +1 html-тест `test_natural_cross_reference` (5 кейсов: forward, no-match, explicit-id,
  case-sensitive, labeled).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (html 302→303).
- Корпус `compare_full.py` (release): **Identical 157→158 (+1), Different 186, Errors 0**.
- Blast radius — РОВНО 3 файла изменили вывод (проверены поштучно base vs new бинари):
  1 FLIP (positional-and-named-attributes); 2 остались DIFFERENT по НЕ-xref причинам
  (audio-and-video — av-attrs; link-macro-attribute-parsing — link-парсинг), но их href стал
  ВЕРНЫМ (`#_vimeo_and_youtube_videos`, `#_noopener_and_nofollow`, `#_blank_window_shorthand`).
  0 регрессий.
- TODO.md: baseline 157→158, п.19 помечен `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/xref-id-normalization` (только по запросу).
- Следующие чистые flip-кандидаты Фазы 3 (по near-miss на 158):
  - **link `^`+rel/target** для литеральных `link:`/URL (description, xref-text-and-style — по 2 diff);
    NB: `{attr-ref}[text]` — архитектурно (порядок subs).
  - **`// end::para[]` утечка** тег-региона (verse.adoc, literal.adoc).
  - **остаток п.37**: апостроф `'`→’ в display-тексте макроса (xref/link) не проходит REPLACEMENTS.
  - **п.24** (точки в id секций) — родственно п.19, но отдельная нормализация.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). Сравнение семантическое (DOM).
  LSP для навигации, context7 MCP для доков.

---

## Сессия (2026-06-09) — Фаза 3: п.18 image alt двойные кавычки

Самый чистый near-miss-кандидат на baseline 153 (предсказан в прошлой session.md).
em-dash и escaped-preprocessor уже смержены в master (dfa0819).

### Ветка `fix/image-alt-quotes` (от master; НЕ закоммичено)
- **Симптом**: `image::set-version-label.png["Byline...",role=screenshot]` →
  `<img alt="&quot;Byline...&quot;">` вместо `alt="Byline...">`.
- **Корень**: `adoc-parser/src/attributes.rs::parse_image_attrs`. Именованные значения
  (`key="v"`) снимали обрамляющие `"` (строки ~436-439), а **позиционные** (alt = positional[0],
  width=[1], height=[2]) пушились сырыми → кавычки доезжали до рендерера → `&quot;`.
- **Фикс**: вынесен хелпер `fn strip_enclosing_quotes(&str)->&str` (снимает ОДНУ пару двойных
  кавычек, согласован со `split_respecting_quotes`, который трекает только `"`). Применён в обеих
  ветках разбора (именованной — рефактор, поведение то же; позиционной — новое). Один источник
  `parse_image_attrs` → покрывает block-image (block.rs:563) и inline-image (inline.rs:1521).
  НЕ трогал общий `BlockAttributes::parse` (строка 199) — чтобы не расширять blast radius.
- +1 тест `test_parse_image_attrs_quoted_alt`.

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 437→438).
- Корпус `compare_full.py` (release): **Identical 153→157 (+4), Different 187, Errors 0**.
- Blast radius — РОВНО 6 файлов с закавыченным позиционным alt (`grep image::?…["`), проверены
  поштучно (`/tmp/check6.py`, normalize из compare_full, base vs new бинари):
  4 FLIP DIFFERENT→IDENTICAL (author-attribute-entries, reference-revision-attributes,
  revision-attribute-entries, version-label); 2 остались DIFFERENT по НЕ-alt причинам
  (image.adoc — обёртка `<a class="image">`/block-vs-inline; revision-line — вне alt). 0 регрессий.
- TODO.md: baseline 153→157, п.18 помечен `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/image-alt-quotes` (только по запросу).
  NB: на master 2 незапушенных коммита (em-dash) — `origin/master` отстаёт на 2.
- Следующие чистые flip-кандидаты Фазы 3 (по near-miss на 157):
  - **xref-id норм.** `#Substitutions`→`#_substitutions` (п.19/24, positional-and-named-attributes).
  - **link `^`+rel/target** для литеральных `link:`/URL (description, xref-text-and-style — по 2 diff);
    NB: `{attr-ref}[text]` — архитектурно (порядок subs).
  - **`// end::para[]` утечка** тег-региона (verse.adoc, literal.adoc).
  - **остаток п.37**: апостроф `'`→’ в display-тексте макроса (xref/link) не проходит REPLACEMENTS.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). near-miss: `/tmp/nearmiss.py`.
  Сравнение семантическое (DOM) — сырой `diff` может «врать» (`’`/`&#8217;`, whitespace в `<code>`).
  LSP для навигации, context7 MCP для доков.

---

## Сессия (2026-05-31, поздняя) — Фаза 3: em-dash границы + ZWSP

Кандидат по near-miss на baseline 149. Кластер «типографика» (п.37) — самый безопасный/выгодный
из чистых flip (link через `{attr-ref}` оказался архитектурным — порядок подстановок, отложен).

### Ветка `fix/em-dash-boundaries` (от master; НЕ закоммичено)
- **Корень**: `inline.rs::apply_typographic_replacements`, bare-`--` арм (строка ~28). Старое
  правило: любой `--` (кроме space-space) → `—`. Это (а) слишком агрессивно (` --dir`→`—dir`,
  `S.S.T.--`→`—`; Asciidoctor оставляет `--`) и (б) без ZWSP (`cases--such`→`—` вместо `—​`).
- **Фикс**: bare `--` → `—`+ZWSP (`—​`) ТОЛЬКО для `\w--\w` (Asciidoctor `(\w)--(?=\w)`,
  `\w`=ASCII alnum+`_`). Иначе → **`None`** (не `Some("--",2)`!): первый `-` остаётся литералом,
  второй переразбирается → `-->` корректно даёт `-→` (asciidoctor: `A --> B`→`A -→ B`, проверено).
  Space-space правило (` -- `→thin-em-thin) не тронуто.
- Тесты: обновлены 2 дубля bare-em-dash (2668 и 3801) под ZWSP; `test_arrow_triple_not_replaced`
  (3763) `A --> B`: было `—>`, стало `-→`. +2 теста (`run --dir`, `For S.S.T.--` остаются).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 435→437).
- Корпус `compare_full.py` (release): **Identical 149→153 (+4), Different 191, Errors 0**.
  Flip: asg/README (`--dir`), dedication (`S.S.T.--`), continuation (ZWSP), callouts (бонус).
  0 регрессий (Different −4 ровно; по регэкспу Asciidoctor наш фикс строго консервативнее).
- Побочно резолвило em-dash-diff в revision-attribute-entries (2→1) и image-format (3→2) —
  не флипнули (остался alt-баг / link-баг соответственно).
- TODO.md: baseline 149→153; п.37 помечен `[~]` с под-пунктом em-dash `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/em-dash-boundaries` (только по запросу).
- Следующие чистые flip-кандидаты (по near-miss на 153):
  - **alt двойная кавычка** (п.18): `<img alt=""...">` — author-attribute-entries (1 diff),
    version-label (2 diff, оба alt), revision-attribute-entries (1 diff, теперь только alt).
    Корень — значение alt в image-макросе сохраняет кавычки. Флипнет ~3 файла. САМЫЙ ЧИСТЫЙ.
  - **xref-id норм.** `#Substitutions`→`#_substitutions` (positional-and-named-attributes, 1 diff).
  - **link `^`+rel/target** (литеральные `link:`/URL): description, xref-text-and-style (по 2 diff).
    NB: `{attr-ref}[text]` (icons-font/auto-ids/custom-ids/ROOT-index) — архитектурно (порядок subs).
  - **`// end::para[]` утечка** тег-региона (verse.adoc, 1 diff) + literal.adoc (`// end::indent[]`).
  - **апостроф в тексте макроса** (остаток п.37): xref/link display-текст не проходит REPLACEMENTS.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). near-miss: `/tmp/nearmiss.py`.
  Сравнение семантическое (DOM). LSP для навигации, context7 MCP для доков.

---

## Сессия (2026-05-31) — Фаза 3: escaped preprocessor-директива

Второй кандидат сессии. Выбор по `/tmp/nearmiss.py`: escaped-директива `\ifdef`/`\endif`
(admonitions, inter-document-xref — «1 diff away»). Preprocessor-слой (не inline).

### Ветка `fix/escaped-preprocessor-directive` (от master; НЕ закоммичено)
- **Корень**: `\ifdef::env-github[]` — backslash экранирует preprocessor-директиву. Asciidoctor
  снимает `\` и выводит `ifdef::...[]` литералом без вычисления; мы сохраняли `\`
  (`parse_conditional` возвращает None из-за `\`, строка падала в обычный output).
- **Фикс** (preprocessor.rs, `preprocess_with_attrs`): новый шаг «0» в начале цикла —
  `if let Some(rest) = line.strip_prefix('\\') && starts_with_conditional_directive(rest)`
  → при `!is_skipping` эмитим `rest` (строку без `\`), `continue`. Хелпер
  `starts_with_conditional_directive` проверяет префиксы `ifdef::`/`ifndef::`/`ifeval::`/`endif::`
  (`::` отсекает слова вроде `ifdefinitely`).
- **КРИТИЧНО — колонка 0**: проверяем СЫРОЙ `line`, НЕ `trimmed`. Asciidoctor распознаёт
  директивы только в начале строки. Первая версия на `trimmed` снимала `\` и при отступе →
  сломала conditionals.adoc (` \ifdef::just-an-example[]` в `[source,indent=0]` листинге, где
  отступ НАМЕРЕННО гасит директиву — это написано в комментарии самого файла). Column-0 чинит:
  indented `\ifdef` остаётся как есть.
- +4 unit-теста (block/inline strip, non-directive kept, indented kept).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 431→435).
- Корпус `compare_full.py` (release): **Identical 145→149 (+4), Different 195, Errors 0**.
  Blast radius — ровно 5 файлов с escaped-директивами (вне их вывод побайтово не менялся):
  admonitions, inter-document-xref, conditionals, ifdef-ifndef, ifeval — ВСЕ 5 теперь Identical
  (net +4, т.к. один был Identical уже на baseline 145). 0 регрессий.
- conditionals.adoc остаётся с сырым diff'ом (` \ifdef` vs `\ifdef` — лишний ведущий пробел от
  несрезанного `[source,indent=0]`), но нормализатор его прощает → Identical. Отдельная бага.
- TODO.md: baseline 145→149; пункт отмечен `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/escaped-preprocessor-directive` (только по запросу).
- Следующие чистые flip-кандидаты Фазы 3 (по near-miss):
  - **xref-id норм.** `#Substitutions`→`#_substitutions` (п.19/24): positional-and-named-attributes.
  - **alt двойная кавычка** (п.18): `<img alt=""…">` — author/revision-attribute-entries.
  - **`// end::para[]` утечка** тег-региона в выводе (verse.adoc) — tagged-region/comment.
  - **`[source,indent=0]`** не срезает общий отступ (conditionals.adoc) — блок-скан.
  - **ОТДЕЛЬНО**: preserve bare char-ref (`&#174;` в обычном тексте → сохранять как сущность,
    не экранировать). НЕ изолированный 1-diff; внутри listing/literal оба экранируют — не трогать.

### Предостережения
- НЕ `cargo fmt` (не fmt-clean). Коммит только по запросу. Верифицировать находки аудита.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release `target/release/adoc`, 344 файла).
  near-miss: `/tmp/nearmiss.py`. Сравнение семантическое (DOM): `’`/`&#8217;` и whitespace внутри
  `<code>` нормализуются → сырой `diff` может «врать». LSP для навигации, context7 MCP для доков.
