# Session context

## Сессия (2026-06-11, двадцать первая) — Фаза 3: assign-id + example-blocks (2 near-miss)

Запрос «продолжи». Ветка **`fix/example-caption-unset-and-positional-shorthand`** —
НЕ закоммичена (рабочее дерево). Baseline: Identical 247, master `172faf5`
(base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**assign-id.adoc (2 diff)** + **example-blocks.adoc (2 diff)** — оба почти-флипа
из прошлой сессии, два независимых корня, взяты вместе в одну ветку.

### Семантика asciidoctor (пробы /tmp/p_ec1..3, p_qa1..2, p_sh1, p_ln1..8)
- `:!example-caption:` → голый title (и mid-document); `:example-caption: Demo` →
  «Demo 1.» с общим счётчиком; дефолт «Example 1.».
- Shorthand attrlist — ТОЛЬКО в первой comma-части: `[quote#roads,Dr. Emmett
  Brown,Back to the Future]` — attribution целиком; `[quote,#bar]`/`[quote,.baz]` —
  verbatim positional; `[.r1,.r2]` → только r1; `[%header,%footer]` → только
  header (`%header%footer` — оба).
- 3-й позиционный СЛОТ source-блока = linenums: любое непустое позиционное
  значение включает (`linenums`/`%linenums`/`#code1`/`yaml`; implied
  `[,ruby,linenums]` тоже), named (`start=10`) слот НЕ занимает.
- linenums РЕНДЕРИТСЯ только под build-time подсветчиком (rouge/pygments/
  coderay); без подсветчика и под highlight.js — игнор целиком (ни класса,
  ни таблицы).

### Что сделано
- **РЕНДЕРЕР** lib.rs: `example-caption: Example` в дефолтных document_attrs;
  blocks.rs арм Example: label из document_attrs (как figure/table).
- **ПАРСЕР** attributes.rs::parse: обе shorthand-ветки гейтятся `idx == 0`;
  +правило linenums-слота по raw-parts (после implied_source_lang).
- **ПАРСЕР** block.rs::emit_block_metadata: style гейтится
  `first_positional_is_style` (позиционал слота 2+ не утекает в style/class).
- **РЕНДЕРЕР** blocks.rs::start_source_block: linenums гейтится
  `rouge|pygments|coderay` (закрыта регрессия db-migration.adoc — `[id=app,
  source, yaml]` слот 3 = `yaml` → linenums on, но подсветчика нет → игнор).
- Тесты: 4 старых переписаны (фиксировали неверное: parse_role, has_option,
  source_with_shorthand_id, table_header_footer_combined `[%header,%footer]`→
  `[%header%footer]`); linenums-тесты переведены на `:source-highlighter:
  rouge` + негативный test_source_block_linenums_needs_build_time_highlighter;
  +4 новых (example-caption, shorthand-first-position html+parser, linenums-слот).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 474, html 347).
- **Корпус: Identical 247→249 (+2)**; blast (base 172faf5): 3 файла — 2 флипа
  (assign-id.adoc, example-blocks.adoc), **0 регрессий**, add-title 252=252
  (семантически ближе: mid-document `:!example-caption:` теперь чтится).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 249: **stem (56 — MathJax-остатки?)**, block (57 — корень
  `.Title` на ulist теряется), literal-monospace (59), source (63),
  customize-title-label (66), include (75), bibliography (77), subs (89);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
- Кандидат-кластер: **xreflabel → reftext для xref-резолва** (label в
  Tag::Anchor + регистрация в XrefResolver; закрыл бы p_id1/2/3-строки и
  lexicon-остаток).
- Прочее: `.Title` на ulist (block.adoc), `cols="2*"` multiplier (row.adoc),
  `[abstract]`-параграф → quoteblock, `:icons:`-colist (TODO), кластер
  `m`/`e`/`s` стиля колонок; pre-existing: лишний `</div>` у standalone
  passthrough, unknown-style течёт в class на quote/sidebar, пустые строки
  в пустых sectionbody, list-merge через continuation-attrlist (p_chk2).
- Латентно (нет в корпусе): наша linenotable-разметка ≠ rouge байт-в-байт
  (нет server-side подсветки) — всплывёт, если в корпусе появится
  rouge+linenums файл.

---

## Сессия (2026-06-11, двадцатая) — Фаза 3: collapsible.adoc (masquerade-параграф — голый контент)

Запрос «продолжи». Ветка **`fix/collapsible-block`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 244, master `184b97d` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**collapsible.adoc (51 diff)**, один корень.

### Семантика asciidoctor (пробы /tmp/p_col1..3)
- Параграф, masquerade'нутый стилем (`[example]`, `[example%collapsible]`,
  `[sidebar]`, `[quote]`, `[open]`) → текст ГОЛЫЙ в `<div class="content">` /
  `<blockquote>` (без `<div class="paragraph"><p>`); multiline сохраняет строки.
- `[partintro]` — ИСКЛЮЧЕНИЕ: paragraph-обёртка внутри openblock сохраняется
  (p_col3, book-контекст; подтверждает сессию 12).
- `[open]`-параграф → `<div class="openblock">` (класс `open` в обёртку НЕ течёт);
  у нас не masquerade'ился вовсе (`paragraph open`).
- `[%collapsible]` без стиля — опция игнорируется, обычный параграф (было верно).
- partintro вне book-part → ERROR + exclude блока (НЕ реализовано, в корпусе нет).

### Что сделано (ПАРСЕР + newline-guard в рендерере)
- `block.rs::scan_paragraph`: арм `quote|example|sidebar|open` — Text без
  Tag::Paragraph (как verse/pass); `partintro` выделен в отдельный арм (с обёрткой).
- `attributes.rs::block_style_kind`: +`"open"`; `block.rs::emit_block_metadata`
  exclusion-список: +`"open"`.
- `events.rs` TagEnd::DelimitedBlock: newline-guard (`!ends_with('\n')`) в армах
  Quote / Example(details) / Example|Sidebar|Open; verse НЕ тронут (отсутствие
  `\n` перед `</pre>` намеренное).
- +1 html-тест `test_style_masqueraded_paragraph_bare_content` (7 кейсов: example,
  collapsible, sidebar, quote, open без утечки класса, multiline, guard настоящего
  delimited-блока).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (908 passed, html 345).
- Пробы p_col1 байт-в-байт; p_col2 — остатки только partintro-вне-book (не в корпусе)
  и trailing newline.
- **Корпус: Identical 244→247 (+3)**; blast (base 184b97d): 8 файлов — 3 флипа
  (collapsible.adoc, sidebars.adoc, release-plan.adoc), **0 регрессий**,
  5 changed-still-different: assign-id 84→2, example-blocks →2 (почти флипы!),
  quote 161→109, add-title 291→252, block 57=57 (нейтрально).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- **assign-id (2 diff!)** и **example-blocks (2 diff!)** — почти флипы, разведать
  первыми. Затем nearmiss: stem (56), block (57 — корень `.Title` на ulist
  теряется), literal-monospace (59), source (63), customize-title-label (66),
  include (75), bibliography (77), quote (109 — стало ближе);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
- Кандидат-кластер: **xreflabel → reftext для xref-резолва** (label в Tag::Anchor +
  регистрация в XrefResolver; закрыл бы p_id1/2/3-строки и lexicon-остаток).
- Прочее: `.Title` на ulist (block.adoc), `cols="2*"` multiplier (row.adoc),
  `[abstract]`-параграф → quoteblock, `:icons:`-colist (TODO), кластер `m`/`e`/`s`
  стиля колонок; pre-existing: лишний `</div>` у standalone passthrough,
  unknown-style течёт в class на quote/sidebar, пустые строки в пустых sectionbody,
  list-merge через continuation-attrlist (p_chk2).

---

## Сессия (2026-06-11, девятнадцатая) — Фаза 3: checklist.adoc (%interactive чекбоксы)

Запрос «продолжи». Ветка **`fix/checklist-rendering`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 243, master `715b17e` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**checklist.adoc (49 diff)**, один корень.

### Семантика asciidoctor (пробы /tmp/p_chk1..2)
- `[%interactive]` (и formal `options=interactive`) на checklist →
  `<input type="checkbox" data-item-complete="1" checked> ` для checked,
  `<input type="checkbox" data-item-complete="0"> ` для unchecked (вместо
  `&#10003;`/`&#10063;`); обычные item'ы списка — без изменений.
- На списке БЕЗ чекбоксов опция ни на что не влияет (нет и класса checklist).
- Вложенный список — свой узел, опцию НЕ наследует.
- Pre-existing (p_chk2, НЕ в корпусе): `+`-continuation с `[%interactive]`+новым
  `*`-item — asciidoctor вливает всё в ОДИН список, мы открываем второй.

### Что сделано (только РЕНДЕРЕР, 3 точки + поле)
- `lib.rs`: поле `interactive_ulist_stack: Vec<bool>` (параллельный стек, по
  образцу admonition_block_stack).
- `blocks.rs::start_unordered_list`: push флага из `meta.options` (interactive).
- `events.rs`: arm `Tag::ListItem` — match (checked, interactive) → input-формы;
  `TagEnd::UnorderedList` — pop.
- +1 html-тест `test_checklist_interactive_html` (4 кейса: shorthand, formal,
  не-наследование вложенным, NCR без опции).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (907 passed, html 344).
- Проба p_chk1 байт-в-байт; p_chk2 — только pre-existing list-merge edge.
- **Корпус: Identical 243→244 (+1)**; blast (base 715b17e): ровно 1 файл —
  1 флип (checklist.adoc), **0 регрессий**.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 244: **collapsible (51 diff)**, release-plan (56), stem (56),
  block (57), literal-monospace (59), source (63), customize-title-label (66),
  include (75), bibliography (77); revision-line-with-version-prefix (1 —
  `{docdate}`, скип).
- Кандидат-кластер: **xreflabel → reftext для xref-резолва** (label в Tag::Anchor +
  регистрация в XrefResolver; закрыл бы p_id1/2/3-строки и lexicon-остаток).
- Прочее: `cols="2*"` multiplier (row.adoc), `[abstract]`-параграф → quoteblock,
  `:icons:`-colist (TODO), кластер `m`/`e`/`s` стиля колонок; pre-existing: лишний
  `</div>` у standalone passthrough, unknown-style течёт в class на quote/sidebar,
  пустые строки в пустых sectionbody, list-merge через continuation-attrlist (p_chk2).

---

## Сессия (2026-06-11, восемнадцатая) — Фаза 3: id.adoc (anchor:-макрос, xreflabel, comment-разделитель списков)

Запрос «продолжи». Ветка **`fix/inline-anchor-macro-and-xreflabel`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 242, master `7e772f6` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**id.adoc (45 diff)**, четыре корня.

### Семантика asciidoctor (пробы /tmp/p_id1..9)
- `anchor:id[]`/`anchor:id[label]` → `<a id="id"></a>`; label НЕ рендерится in place,
  используется как reftext для xref. Target с пробелом — литерал; `\anchor:x[]` —
  литерал без backslash.
- `[[id,xreflabel]]` (inline И block) → id без label; label = reftext для xref
  (`<<bookmark-d>>` → «last paragraph»; block-anchor label ПОБЕЖДАЕТ .Title в xref).
- `<<id>>` на inline-анкер БЕЗ label → fallback `[id]`.
- `[[id]]image:...[]` (строка с хвостом после `]]`) — параграф с inline-анкором,
  НЕ block-attrlist (BlockAttributeListRx: первый символ inner — `[\w{,.#"'%]`).
- Comment-строка ПОСЛЕ blank разделяет смежные списки (даже однотипные, p_id7)
  и отрывает dlist от ulist-item; comment сразу после item (без blank) — НЕ рвёт
  (p_id5/8); dlist после голого blank ПРИКРЕПЛЯЕТСЯ к li (p_id4 — asciidoctor тоже).

### Что сделано (только ПАРСЕР; рендерер Tag::Anchor уже был готов)
- `inline.rs::try_anchor_macro` + dispatch-arm `b'a'`/`anchor:` (при провале
  `pos += 7` — иначе catch-all ел `nchor:`); `anchor:` в NAMES (11→12);
  `try_anchor` — split id по запятой.
- `scanner.rs::is_block_attribute` — ужесточение первого символа + ветка
  BlockAnchorRx для `[[...]]` (вся строка, interior без скобок).
- `attributes.rs` legacy-anchor — split по запятой.
- `block.rs` comment-handler — close_list_contexts при had_blank_line в
  list-контексте (зеркало block-attribute-ветки, строка ~600).
- +4 теста: inline `test_anchor_macro` (4 кейса) + обновлён
  `test_anchor_with_reftext_still_works` (фиксировал НЕВЕРНОЕ поведение);
  scanner `test_is_block_attribute` (+10 ассертов); attributes
  `test_legacy_anchor_xreflabel_stripped`; block
  `test_comment_after_blank_separates_lists`; html
  `test_inline_anchor_macro_and_xreflabel_html` (6 кейсов).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 472, html 343, core 13).
- Пробы: p_id4/5/6/8/9 байт-в-байт; p_id7 — только trailing-newline (норм.);
  p_id1/2/3 — остаток ТОЛЬКО xref-reftext строки (не нужны для флипа).
- **Корпус: Identical 242→243 (+1)**; blast (base 7e772f6): 9 файлов — 1 флип
  (id.adoc), **0 регрессий**, 8 changed-still-different (list-файлы ближе к
  эталону: complex.adoc ulist 1→5 при 13 в ref; checklist 49=49,
  revision-information 94→96 — позиционный шум).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 243: **checklist (49 diff)**, collapsible (51), release-plan (56),
  stem (56), block (57), literal-monospace (59), source (63),
  customize-title-label (66), include (75), bibliography (77);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
- Новый кандидат-кластер: **xreflabel → reftext для xref-резолва** (label в
  Tag::Anchor + регистрация в XrefResolver; закрыл бы p_id1/2/3-строки и
  родственный lexicon-остаток «reftext из dt-терма»).
- Прочее: `cols="2*"` multiplier (row.adoc), `[abstract]`-параграф → quoteblock,
  `:icons:`-colist (TODO), кластер `m`/`e`/`s` стиля колонок; pre-existing:
  лишний `</div>` у standalone passthrough, unknown-style течёт в class на
  quote/sidebar, пустые строки в пустых sectionbody.

---

## Сессия (2026-06-11, семнадцатая) — Фаза 3: author-атрибуты из attribute-entries

Запрос «продолжи». Ветка **`fix/author-attr-entries`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 241, master `2d07b0b` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**reference-author.adoc (37 diff)**, три корня.

### Семантика asciidoctor (пробы /tmp/p_au1..16; источник parser.rb/document.rb читан)
- End-of-header rescan (parse_header_metadata): если `author`-атрибут задан и ≠
  значения от author-line — names-only парсинг значения (split ≤3 whitespace-сегментов,
  4+ слов → хвост в lastname, `_`→пробел в каждом сегменте, initials = первые символы,
  fullname РЕКОМПОЗИРУЕТСЯ) → клоббер firstname/middlename/lastname (даже явных
  entries!); явный `:authorinitials:`, отличный от line-derived, ВЫЖИВАЕТ;
  authorcount → 1 («do not allow multiple»). Email из значения НЕ извлекается
  (`<...>` в attr-entry уже проэкранирован header-subs → ветка sanitize мертва;
  lastname получает `Jones <m@x.org>` verbatim).
- `Document#authors` — полностью attribute-backed: спаны details из `author`/`email`
  + `author_N`/`email_N` (гейт `authorcount`). `:email:` без author → НЕТ details;
  `:!author:` после author-line — details ПОДАВЛЕН (но firstname от line остаётся).
- `:author_2:` attr-entry второго автора НЕ создаёт; mid-document `:author:` ничего
  не дериватит и details не открывает; `:firstname:`+`:lastname:` БЕЗ author author
  не композируют.
- Section auto-id: attr-refs в заголовке резолвятся ДО генерации id
  (`== About {author}` → `_about_kismet_r_lee`); значения entries резолвятся at
  definition (`:nested: x {foo} y`); undefined — литерал (скобки дропает санация id).

### Что сделано
- **CORE** `Author::from_attribute_value(value)` — names-only дериватор (+1 юнит-тест).
- **РЕНДЕРЕР** `finish.rs::finalize_header_authors` (зов в events.rs на TagEnd::Header
  ДО render_author_details, в обоих режимах — derived attrs нужны body-refs);
  `render_author_details` — author-спаны attribute-backed (цикл по authorcount,
  name_suffix/id_suffix из AuthorRegistry); guard details: `author`-attr вместо
  registry. events.rs Event::Author — +`authorcount` в document_attrs (= len реестра).
- **ПАРСЕР** `block.rs`: поле `doc_attrs: HashMap` (имена lowercase, значения
  definition-time resolved); `record_attribute_entry` (unset-формы `!n`/`n!` —
  remove) на всех 5 точках attr-entry (body, header×3, revision) + запись
  author-line-атрибутов (suffix `_N`); `resolve_title_attr_refs` перед
  `generate_id` на всех 4 точках (section/discrete/doc-header×2).
- +1 html-тест `test_author_attrs_from_attribute_entries` (6 кейсов),
  +1 parser-тест `test_section_id_resolves_attr_refs` (5 ids).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (902 passed: parser 469,
  html 342, core 13).
- Пробы: p_au1 (standalone+embedded) байт-в-байт кроме известной NCR-нормализации;
  p_au2..16 OK (p_au16 body — pre-existing пустые строки в пустых sectionbody).
- **Корпус: Identical 241→242 (+1)**; blast (base 2d07b0b): ровно 1 файл — 1 флип
  (reference-author.adoc), **0 регрессий**.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 242: **id (45 diff)**, checklist (49), collapsible (51),
  release-plan (56), stem (56), block (57), literal-monospace (59), source (63),
  customize-title-label (66), include (75), bibliography (77);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
  Прочее: `cols="2*"` multiplier (row.adoc), `[abstract]`-параграф → quoteblock,
  `:icons:`-colist (TODO), кластер `m`/`e`/`s` стиля колонок; pre-existing: лишний
  `</div>` у standalone passthrough, unknown-style течёт в class на quote/sidebar,
  пустые строки в пустых sectionbody. Известные пределы фикса: parser-карта не
  дериватит firstname из entry-`:author:` для ids (нет в корпусе); `:authors:`-атрибут
  (множественный) не поддержан (нет в корпусе).

---

## Сессия (2026-06-11, шестнадцатая) — Фаза 3: subs trailing-plus + attr-value pass-макрос

Запрос «продолжи». Ветка **`fix/subs-trailing-plus-and-attr-pass-macro`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 240, master `1a13391` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**listing.adoc (34 diff)**, два корня.

### Семантика asciidoctor (пробы /tmp/p_subs1..6, p_rec)
- `subs=` (resolve_subs): модификаторы — `+x` append, `x+` PREPEND (trailing plus!),
  `-x` remove; первый МОДИФИКАТОР сидит дефолты блока, первый PLAIN-токен сидит
  ПУСТОЙ набор (замена) — `"quotes,+attributes"` ДРОПАЕТ specialchars; составные
  имена (`verbatim+`/`-normal`) допустимы. ПОРЯДОК применения (prepend = sub ДО
  specialchars → двойное экранирование значения) в bitflag-модели непредставим —
  только membership; два известных edge-предела (p_subs5 case1, p_subs3 case2),
  в корпусе их нет.
- Attr-entry значение `pass:SUBS[content]` (full-value, apply_attribute_value_subs):
  subs применяются при ОПРЕДЕЛЕНИИ; `pass:a[{ref}]` — undefined ref остаётся
  литералом и при использовании НЕ ре-сканится (`:x: pass:a[{x}]` → литерал `{x}`).
- ПОПУТНЫЙ pre-existing КРАШ: `:x: {x}` + `{x}` → stack overflow (рекурсия
  events.rs AttributeReference → render_inline_value). Asciidoctor — литерал.

### Что сделано
- **ПАРСЕР** `attributes.rs::parse_subs_value`: детекция модификаторов +trailing `+`;
  логика asciidoctor (acc: Option<SubstitutionSet>, get_or_insert(default) у
  модификаторов / get_or_insert(NONE) у plain); +`sub_name_to_flags` (составные
  normal/verbatim/none). 2 юнит-теста переписаны под верную семантику (probe-verified),
  +1 `test_subs_parse_trailing_plus`.
- **РЕНДЕРЕР** `lib.rs::apply_attr_value_pass_macro` (зов из apply_attribute):
  full-value `pass:SPEC[content]` — обёртка стрипается, `a`/`attributes` в SPEC →
  definition-time резолв через core `resolve_attr_refs_text`; ПУСТОЙ SPEC (`pass:[…]`)
  НЕ трогается (inline pass-макрос обрабатывает at use, verbatim-вставка).
- **РЕНДЕРЕР** guard рекурсии: поле `attr_refs_in_progress: Vec<String>`;
  arm AttrRefOutcome::Document — повторный вход по тому же (lowercase) имени →
  литерал `{name}` (закрыт краш `:x: {x}` и взаимная рекурсия `:a: {b}`/`:b: {a}`).
- +2 html-теста: `test_subs_trailing_plus_and_attr_value_pass_macro` (5 кейсов),
  `test_self_referential_attribute_no_recursion` (2 кейса).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 467→468, html 339→341).
- Пробы p_subs1/2/6 байт-в-байт; p_rec — литерал как asciidoctor (был abort).
- **Корпус: Identical 240→241 (+1)**; blast (base 1a13391): 4 файла — 1 флип
  (listing.adoc, 0 diffs), **0 регрессий**, 3 changed-still-different:
  include 125→124, subs 92→89, footnote 245→260 (СЕМАНТИЧЕСКИ ЛУЧШЕ:
  `:fn-disclaimer: pass:c,q[footnote:…]` теперь даёт настоящие footnote-`<sup>`
  вместо мусорного custom-macro; рост счётчика — позиционный шум от появившихся
  footnote-определений).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 241: **reference-author (37 diff)**, id (45), checklist (49),
  collapsible (51), release-plan (56), stem (56), block (57), literal-monospace (59),
  source (63), customize-title-label (66), include (75);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
  Прочее: `cols="2*"` multiplier (row.adoc), `[abstract]`-параграф → quoteblock,
  `:icons:`-colist (TODO), кластер `m`/`e`/`s` стиля колонок; pre-existing: лишний
  `</div>` у standalone passthrough, unknown-style течёт в class на quote/sidebar.
  Новый известный предел: порядок subs (prepend/append) не представим bitflag'ом —
  если встретится в корпусе, потребуется упорядоченный Vec<Sub> вместо маски.

---

## Сессия (2026-06-11, пятнадцатая) — Фаза 3: revision-атрибуты из attribute-entries

Запрос «продолжи». Ветка **`fix/revision-attrs-from-entries`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 239, master `77b6302` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**reference-revision-attributes.adoc (31 diff)**, один корень.

### Семантика asciidoctor (пробы /tmp/p_rev1..8, после фикса все 8 байт-в-байт)
- Revision-спаны в `<div class="details">` — attribute-driven (html5.rb смотрит
  document-атрибуты `revnumber`/`revdate`/`revremark`): attr-entries в header дают
  спаны БЕЗ revision-line; автор не обязателен.
- Значение verbatim: `:revnumber: v8.3` → «version v8.3» (`v` стрипается ТОЛЬКО при
  парсинге revision-line).
- attr-entry ПОБЕЖДАЕТ revision-line (later-wins в header); `:!revdate:` снимает
  спан и запятую после version; set-but-empty `:revnumber:` → спан «version ».
- Body-атрибуты (после blank за header'ом / mid-document) в details НЕ попадают.

### Что сделано (только РЕНДЕРЕР)
- `finish.rs::render_author_details`: revision-часть читает
  `document_attrs.get("revnumber"/"revdate"/"revremark")` (метод зовётся на
  `TagEnd::Header` — документ-атрибуты в этот момент = ровно header-состояние);
  guard пустоты details расширен на эти три ключа; запятая после version — по
  наличию revdate; display_version больше не зовётся (verbatim).
- `lib.rs`: поле `revision: Option<Revision>` удалено; `events.rs` arm
  Event::Revision только вливает `attr_entries()` в document_attrs (precedence
  с attr-entries — порядком стрима).
- +1 html-тест `test_revision_attrs_from_attribute_entries` (4 кейса; негативные
  ассерты — по `<span id=…`, т.к. голые имена есть в default-stylesheet).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (html 338→339, parser 467).
- Пробы p_rev1..8 — header-секции байт-в-байт с asciidoctor.
- **Корпус: Identical 239→240 (+1)**; blast (base 77b6302): ровно 1 файл — 1 флип
  (reference-revision-attributes.adoc), **0 регрессий**.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 240: **listing (34 diff)**, reference-author (37), id (45),
  checklist (49), collapsible (51), release-plan (56), stem (56), block (57),
  literal-monospace (59), source (63), customize-title-label (66);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
  Прочее: `cols="2*"` multiplier (row.adoc), `[abstract]`-параграф → quoteblock,
  `:icons:`-colist (TODO), кластер `m`/`e`/`s` стиля колонок; pre-existing: лишний
  `</div>` у standalone passthrough, unknown-style течёт в class на quote/sidebar.

---

## Сессия (2026-06-11, четырнадцатая) — Фаза 3: admonition block-форма (параграф-обёртки)

Запрос «продолжи». Ветка **`fix/admonition-block-paragraph-wrappers`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 235, master `3dfe796` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**apply-subs-to-blocks.adoc (31 diff)**, один корень (len_delta=8 = 2 параграфа × 4
строки обёртки).

### Семантика asciidoctor (пробы /tmp/p_adm1..13)
- paragraph-форма (`NOTE: text` И `[NOTE]` на параграфе) — голый текст в
  `<td class="content">`.
- block-форма (`[NOTE]` на `====` example или `--` open) — compound: дети с обычными
  обёртками (`<div class="paragraph"><p>`, ulist, вложенные admonition и т.д.).
- admonition-стиль чтится ТОЛЬКО на example/open; на listing/literal/sidebar/quote/
  passthrough — ИГНОРИРУЕТСЯ, блок остаётся родным, стиль дропается (как и unknown
  `[foo]` — но у нас на quote/sidebar unknown-стиль ТЕЧЁТ в class, pre-existing).
- Попутно: голый `++++` passthrough у нас даёт лишний `</div>` (pre-existing, есть в
  base; p_adm12 поэтому единственная не-байт-в-байт проба из 13).

### Что сделано
- **ПАРСЕР** `event.rs`: `Tag::Admonition` +поле `block: bool` (+doc-комментарий,
  into_static). `block.rs`: paragraph-точки (scan_paragraph ~1814, scan_admonition
  ~2091) → `block: false`; ранний перехват «admonition style on any delimited block»
  (~2222) УДАЛЁН; в structural-ветке гейт `matches!(delim_type, Example|Open)` →
  `block: true` (verbatim-типы теперь падают в родную ветку, стиль дропается).
- **РЕНДЕРЕР** `lib.rs`: поле `admonition_block_stack: Vec<bool>`; `blocks.rs`:
  start_admonition(+block) пушит; `events.rs`: TagEnd::Admonition попит;
  `is_direct_child_of_admonition` → подавление `<p>` только при `!block`;
  `is_inside_compact_context` arm Admonition → компактность только при `!block`
  (block-форма → полные обёртки; вложенность paragraph-в-block работает: ближайший
  Admonition в tag_stack = вершина параллельного стека).
- Тесты: html `test_block_admonition_html`/`test_note_style_on_listing_delimiter`
  переписаны под верную семантику, +1 `test_admonition_block_vs_paragraph_forms`
  (open-форма, bare-формы, игнор на sidebar/quote, вложенный admonition);
  parser `test_block_admonition`/`_warning` → `block: true`; integration 2 места;
  builder.rs (compat) — паттерн `{ kind, .. }`.

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 467, html 337→338).
- Пробы 11/12 байт-в-байт (искл. p_adm12 — pre-existing passthrough-`</div>`).
- **Корпус: Identical 235→239 (+4)**; blast (base 3dfe796): 10 файлов — 4 флипа
  (header.adoc, icon-macro.adoc, apply-subs-to-blocks.adoc, validation.adoc),
  **0 регрессий**; 6 changed-still-different: ordered 420→232, admonition 223→197,
  special-characters 150→148, cookbook 2604→2582, java/index 2313=2313,
  syntax-quick-reference 2759→2791 (позиционный шум — admonition-сегмент проверен
  локальным diff'ом байт-в-байт).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 239: **reference-revision-attributes (31 diff)**, listing (34),
  reference-author (37), id (45), checklist (49), collapsible (51), release-plan (56),
  stem (56), block (57), literal-monospace (59), source (63);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
  Прочее: `cols="2*"` multiplier (row.adoc), `[abstract]`-параграф → quoteblock,
  `:icons:`-colist (TODO), кластер `m`/`e`/`s` стиля колонок; новые pre-existing
  находки: лишний `</div>` у standalone passthrough-блока, unknown-style течёт в
  class на quote/sidebar (asciidoctor дропает).

---

## Сессия (2026-06-11, тринадцатая) — Фаза 3: add-header-row.adoc (noheader + formal options=)

Запрос «продолжи». Ветка **`fix/table-noheader-option`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 234, master `1c22959` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**add-header-row.adoc (29 diff)**, один корень + попутный пробел.

### Семантика asciidoctor (пробы /tmp/p_nh1..7.adoc)
- `noheader` (shorthand `%noheader` И formal `options=noheader`) подавляет ТОЛЬКО
  implicit-промоушен первой строки в header; явный `header` побеждает
  (`%header%noheader` → `<thead>`).
- `opts=` — alias `options=`; значение comma-separated (`options="header,footer"`).
- Попутно обнаружено: formal `options=header` у нас ВООБЩЕ не работал — в корпусе
  маскировался implicit-правилом (blank после первой строки в formal-таблицах).

### Что сделано (только ПАРСЕР, 3 точки)
- `attributes.rs::parse`: named `options`/`opts` промотируются в вектор `options`
  (split по `,`, trim, тот же путь, что shorthand `%`; named["options"] никто не читал).
- `block.rs`: оба места has_header (psv ~1379, csv/dsv ~1627) —
  `&& !block_attrs.has_option("noheader")` в implicit-ветке.
- +1 html-тест `test_table_noheader_option_html` (5 кейсов: shorthand/formal noheader,
  конфликт, formal header без implicit-layout, opts-alias).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (html 336→337).
- Все 7 проб байт-в-байт (кроме p_nh4 CSV — остаточный pre-existing `<colgroup>`-diff,
  НЕ про header; thead подавлен верно).
- **Корпус: Identical 234→235 (+1)**; blast (base 1c22959): 2 файла — 1 флип
  (add-header-row.adoc), **0 регрессий**; row.adoc 312→310 (changed-still-different,
  доминирует корень `cols="2*"` multiplier — НЕ поддержан, потенциальная задача).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 235: **apply-subs-to-blocks (31 diff)**, reference-revision-attributes (31),
  listing (34), reference-author (37), icon-macro (41), id (45), checklist (49),
  collapsible (51); revision-line-with-version-prefix (1 — `{docdate}`, скип).
  Новое: `cols="2*"` multiplier-синтаксис (row.adoc 310 diff — крупный, но один корень?).
  Прочее: `[abstract]`-параграф → quoteblock, `:icons:`-colist (TODO),
  кластер `m`/`e`/`s` стиля колонок.

---

## Сессия (2026-06-11, двенадцатая) — Фаза 3: part.adoc ([partintro]-параграф → open block)

Запрос «продолжи». Ветка **`fix/partintro-paragraph-openblock`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 233, master `6f82f8a` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**part.adoc (18 diff)**, один корень.

### Семантика asciidoctor (пробы /tmp/p_pi1..4.adoc)
- `[partintro]` на параграфе — masquerade в open block:
  `<div class="openblock partintro"><div class="content"><div class="paragraph"><p>…`.
- Вне book-part — ERROR + exclude всего блока (НЕ реализовано: в корпусе нет таких).
- `[partintro]` на `--`-блоке — у нас уже работало (фолбэк `_ => {}`).
- `[abstract]`-параграф → `<div class="quoteblock abstract"><blockquote>текст` (БЕЗ
  paragraph-обёртки) — НЕ сделано, отдельный potential-кластер (abstract-block 5 diff).

### Что сделано (только ПАРСЕР, 2 точки)
- `attributes.rs::block_style_kind`: +`"partintro"`.
- `block.rs::scan_paragraph`: arm `quote|example|sidebar` → `…|partintro`,
  kind `DelimitedBlockKind::Open`; style не исключён в emit_block_metadata →
  класс `openblock partintro` собирает рендерер.
- +1 html-тест `test_partintro_paragraph_masquerades_as_open_block` (masquerade +
  guard явного open-блока).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (html 335→336).
- **Корпус: Identical 233→234 (+1)**; blast (base 6f82f8a): ровно 1 файл — 1 флип
  (part.adoc, 0 diffs), **0 регрессий**.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 234: **add-header-row (29 diff)**, apply-subs-to-blocks (31),
  reference-revision-attributes (31), listing (34), reference-author (37),
  icon-macro (41), id (45); revision-line-with-version-prefix (1 — `{docdate}`, скип).
  Прочее: `[abstract]`-параграф → quoteblock (см. выше), `:icons:`-colist (TODO),
  кластер `m`/`e`/`s` стиля колонок.

---

## Сессия (2026-06-11, одиннадцатая) — Фаза 3: url.adoc (irc-схема, link role=, mailto query)

Запрос «продолжи». Ветка **`fix/url-macro-irc-role-mailto`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 232, master `4c62625` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**url.adoc (7 diff)**, три корня.

### Семантика asciidoctor (пробы /tmp/p_url1..3.adoc, /tmp/p_u_a..d.adoc)
- `irc://` и `ftp://` — автолинк-схемы как http(s); голые → `class="bare"`.
- `role=green` на link/url/mailto-макросах → class на `<a>`; пустой текст →
  `class="bare green"` (bare первым). Raw-порядок атрибутов: href, class, target, rel.
- mailto positional 2/3 → `?subject=&body=`, percent-encode ERB-стиля (литеральны
  `A-Za-z0-9_.~-`, пробел `%20`, hex UPPERCASE), кавычки снимаются.
  `mailto:a@b[T,,body]` (пустой subject) — asciidoctor ПАДАЕТ (nil) → поведение
  свободно, у нас пустые компоненты опускаются.

### Что сделано
- **ПАРСЕР** `event.rs`: `Tag::Link` +поле `role: Option<CowStr>`.
- **ПАРСЕР** `attributes.rs::parse_link_attrs`: +role/subject/body; named-ветка
  гейтится валидным именем ключа; латентный баг закрыт — named-only attrlist
  (`[role=x]`/`[window=_blank]`) теперь даёт ПУСТОЙ text (→ bare), а не весь
  bracket_content.
- **ПАРСЕР** `inline.rs`: +2 dispatch-арма ftp://+irc:// → try_autolink;
  `url_encode_into` (ERB-стиль); mailto строит query-URL (Cow::Owned).
- **РЕНДЕРЕР** `events.rs` arm Tag::Link: class = bare+role сразу после href.
- +1 parser-тест `test_link_role_mailto_query_irc_scheme` (5 кейсов), +1 html-тест
  `test_link_role_and_mailto_query_html` (6 ассертов). Тестовые инициализаторы
  Tag::Link дополнены `role: None` (perl one-liner).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (893: parser 467, html 335).
- **Корпус: Identical 232→233 (+1)**; blast (base 4c62625): ровно 1 файл — 1 флип
  (url.adoc), **0 регрессий**. Все 5 проб байт-в-байт с asciidoctor.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 233: **part.adoc (18 diff, len_delta=4)**, add-header-row (29),
  apply-subs-to-blocks (31), reference-revision-attributes (31), listing (34),
  reference-author (37), icon-macro (41), id (45), checklist (49);
  revision-line-with-version-prefix (1 — `{docdate}`, скип). Прочее: `:icons:`-colist
  (TODO), кластер `m`/`e`/`s` стиля колонок.

---

## Методика (каноническая, действует во всех сессиях)

- **Git**: никогда не коммитить в master напрямую; `git checkout master && git pull` →
  новая ветка `fix/...`. Коммит/мерж/пуш — ТОЛЬКО по запросу пользователя.
  session.md обычно пишется ДО мержа — статус «НЕ закоммичено» прошлой сессии означает
  «смотри git log: следующая сессия начинается с уже смерженного master».
- **НЕ запускать cargo fmt.**
- **Корпус**: `/mnt/c/tmp/adoc-test/` (344 файла), `python3 compare_full.py`
  (нужен release-бинарь: `cargo build --release -p adoc-cli`).
- **blast**: `/tmp/blast.py` — пофайловое сравнение с `/tmp/adoc_base` (release-бинарь
  чистого master; пересобирать в начале сессии: build → `cp target/release/adoc
  /tmp/adoc_base`). Показывает флипы/регрессии/changed-still-different.
- **fdiff**: `/tmp/fdiff.py <relpath>` — позиционный diff одного файла.
- **nearmiss**: `/tmp/nearmiss.py` — ранжирует Different-файлы по числу diff'ов;
  берём ближайший к флипу. revision-line-with-version-prefix (1 diff, `{docdate}` —
  зависит от даты запуска) всегда скипаем.
- **Семантику asciidoctor проверять пробами** (`asciidoctor -o - [-s] /tmp/p_*.adoc`,
  установлен в /usr/bin/asciidoctor) ДО фикса; фиксировать выводы в session.md/TODO.md.
- CLI: `adoc [--no-standalone] file` (флага `-e` НЕТ).
- Перед коммитом: `cargo clippy --workspace` (0 warnings) + `cargo test --workspace`
  (всё зелёное). После фикса: корпус + blast (0 регрессий — обязательное условие).

---

## Архив сессий (сжато; полные детали каждого фикса — в TODO.md и git log)

Формат: тема — ветка; корпус-дельта. Все смержены в master.

### 2026-06-11 (Фаза 3 + R9)
- **одиннадцатая** — url.adoc: irc/ftp-автолинк, link `role=`→class, mailto subject/body
  query (см. выше); 232→233.
- **десятая** — multi-author `author_2`: name_suffix `_2`/`_3` для attr-entries,
  id_suffix без сепаратора для span-id (CORE AuthorRegistry) —
  `fix/multi-author-attr-underscore`; 231→232 (multiple-authors.adoc).
- **девятая** — email-автолинк без `class="bare"` — `fix/email-autolink-no-bare-class`;
  230→231 (header.adoc). bare — только URL-автолинки и `link:`/URL-макросы с пустым текстом.
- **восьмая** — version-label в revnumber-span + attr-entry внутри текстового блока =
  литерал (в dlist wrapped — дроп) — `fix/version-label-revnumber`; 229→230.
- **седьмая** — toc2/toc-left/toc-right: классы на body (только header-`:toc:` c
  left/right), div — голый `class="toc2"` — `fix/toc2-body-class`; 228→229.
- **шестая** — п.41 header после ведущих комментариев —
  `fix/header-after-leading-comments`; **210→228 (+18)**.
- **пятая** — sect0-heading standalone (без div-обёртки) + admonition image-иконки при
  `:icons:` (не-font) — `fix/callout-rendering`; 208→210. Остаток: `:icons:`-colist
  таблицей (TODO).
- **четвёртая** — QUOTES/ATTRIBUTES в метках link/xref/mailto (inner-reparse
  `subs.without(MACROS)` в `push_macro_label`) — `fix/macro-label-inline-formatting`;
  206→208. Остаток: `\` `` в метке съедает оба backslash (pre-existing).
- **третья** — `pass:[…]` извлекается ДО `+…+` (случай A) —
  `fix/pass-macro-in-single-plus`; 205→206.
- **вторая** — YouTube-плейлисты в video (target `id/list`, `id1,id2`, голый loop →
  `&playlist={id}`; порядок query-параметров) — `fix/youtube-playlist-params`; 204→205.
- **первая** — **R9**: `InlineOptions` — общий канал document-attrs → inline-парсер
  (streaming `apply_attribute` + snapshot `from_attr_lookup`) —
  `refactor/inline-doc-attrs-channel`; нейтрально (байт-в-байт). АУДИТ R1–R9 ЗАКРЫТ.

### 2026-06-10 (аудит рендерера R1–R8)
- **восьмая** — **R8**: распил adoc-html/src/lib.rs (6220 строк) на модули (events,
  blocks, inline, media, finish, escape, tests) — `refactor/html-modules`; байт-в-байт.
- **седьмая** — **R7-5 (финал)**: Author/AuthorRegistry + Revision в adoc-render-core —
  `refactor/render-core-author-revision`; байт-в-байт.
- **шестая** — **R7-4**: CaptionCounters + FootnoteRegistry в core —
  `refactor/render-core-captions`; байт-в-байт.
- **пятая** — **R7-3**: SectionNumberer + TocBuilder (toc_steps) в core —
  `refactor/render-core-section-toc`; байт-в-байт.
- **четвёртая** — **R7-2**: XrefResolver (RefText::{Plain,Markup}, precedence) в core —
  `refactor/render-core-xref-resolver`; байт-в-байт.
- **третья** — **R7-1**: крейт **adoc-render-core** (интринсики
  IntrinsicAttribute{text,html}, resolve_attribute_reference, resolve_attr_refs_text);
  закрыт дрейф builder.rs (apos/pp/quot) — `refactor/render-core-attr-resolver`.
- **вторая** — **R5**: ResolutionContext + однопроходный резолв `\x00`-сентинелей
  (рекурсия depth 8; стресс 2000 xref 807ms→33ms) —
  `refactor/finish-single-pass-resolution`; байт-в-байт + багфикс вложенных сентинелей.
- **первая** — **R1/R2/R4/R6 + частично R3/R5**: figure-caption (title ПОСЛЕ content,
  счётчик, parse_image_attrs caption=/title=), video/stem title-leak, хелпер
  `open_block_with_title` (новые block-arm'ы писать через него!),
  `push_media_time_fragment`, li-paragraph хелперы — `fix/block-image-figure-caption`;
  204 (0 флипов, улучшения diff'ов).

### 2026-06-09 (марафон Фазы 3, поздняя-1…29; 145→204)
- **29** — аудит рендерера (БЕЗ правок): находки R1–R9, верифицированы агентами.
- **28** — MathJax-loader при `:stem:` (const MATHJAX_DOCINFO в write_document_tail) —
  `fix/stem-mathjax-docinfo`; 203→204. Остаток: `eqnums` (не в корпусе).
- **27** — rowspan: двойной декремент occupancy → ячейки уезжали в спанированную
  колонку — `fix/rowspan-row-placement`; 202→203. Остаток: col_idx в emit_row_cells
  не учитывает rowspan-сдвиг (латентно).
- **26** — continuation-блок в callout-элементе (li_p_open для CalloutListItem) + сдвиг
  позиционных слотов ведущим named/shorthand-атрибутом (`[id=app, source, yaml]`) —
  `fix/callout-item-block-and-shifted-source-lang`; 200→202.
- **25** — audio: `opts=` alias, `#t=start,end`, `.Title` —
  `fix/audio-start-opts-and-title`; 199→200.
- **24** — intrinsic `{quot}`/`{apos}`/`{pp}` + `pass:[…]` в constrained-matching
  (случай G, pass_macro_span_len) — `fix/intrinsic-quot-apos-and-pass-constrained`;
  198→199.
- **23** — UI-макросы kbd:/btn:/menu: за `:experimental:` —
  `fix/gate-experimental-ui-macros`; 194→198.
- **22** — revnumber strip нецифрового префикса + `[%hardbreaks]` —
  `fix/revision-prefix-and-hardbreaks`; 193→194. Отложенный баг: trailing ` +` в
  reparsed monospace → ложный `<br>` (pre-existing, outline.adoc).
- **21** — `.Title` на отступном literal-параграфе — `fix/literal-paragraph-block-title`;
  192→193.
- **20** — голый `{name}` на счётчик в document-order (препроцессор) —
  `fix/counter-bare-reference`; 191→192. Остаток: счётчики в verbatim (counters.adoc).
- **19** — section-id: точки-разделители + дедуп `_2` — `fix/section-id-dots-and-dedup`;
  190→191.
- **18** — escaped inline-макрос `\name:target[…]` — `fix/escaped-inline-macro`; 189→190.
- **17** — single-plus `+…+` как constrained-пара —
  `fix/single-plus-passthrough-constrained`; 188→189.
- **16** — passthrough внутри monospace/quote (`` `++`++` ``) —
  `fix/passthrough-inside-monospace`; 186→188.
- **15** — путь между `}` и `[` в attr-ref (`{url}/issues[text]`) —
  `fix/attr-ref-path-before-brackets`; 185→186.
- **14** — значение `{attr-ref}` уважает subs блока — `fix/attr-ref-respect-block-subs`;
  184→185.
- **13** — verbatim-параграф сохраняет `//`-комментарий — `fix/verbatim-paragraph-comment`;
  182→184.
- **12** — header-style колонка `h` → `<th>` — `fix/table-header-column-style`; 180→182.
  Остаток: `m`/`e`/`s`/`a`/`l` стили не наследуются (кластер, TODO).
- **11** — `{attr-ref}[text]` как ссылка (subs-order; render_inline_value) —
  `fix/attr-ref-link-macro`; **175→180 (+5)**.
- **10** — trailing whitespace строк параграфа (rstrip_line_trailing_ws) —
  `fix/paragraph-trailing-whitespace`; 173→175.
- **9** — `table-caption` document-атрибут — `fix/table-caption-doc-attr`; 172→173.
- **8** — `link:url[]` пустой текст → `class="bare"` (п.14) — link-macro-empty-bare;
  171→172.
- **7** — preserve bare char-ref `&#167;` (п.15) — bare-char-reference-preserved;
  170→171. Остаток: char-ref внутри `` `…` `` (Event::Code).
- **6** — неизвестный verbatim-style → class — `fix/literal-unknown-style-class`; 169→170.
- **5** — custom caption на admonition — admonition-custom-caption; 168→169.
- **4** — REPLACEMENTS в тексте макроса (остаток п.37) — macro-text-replacements;
  165→168.
- **3** — xref fallback `[id]` + bibliography reftext — xref-fallback-bracketed-id;
  162→165. Родственный остаток: inline-anchor reftext из dt-терма (lexicon.adoc).
- **2** — link blank-window `^` (п.14) — link-blank-window-caret; 158→162.
- **1** — п.19 xref-id нормализация (natural cross reference) — 157→158.
- **(дневная)** — п.18 image alt двойные кавычки; 153→157.

### 2026-05-31 и ранее
- em-dash границы + ZWSP; 149→153. Escaped preprocessor-директива; 145→149.
- Ранняя история (Фазы 1–2, аудиты D1–D6, xref авто-текст 79→135 и пр.) — в TODO.md,
  разделы «Сделано».
