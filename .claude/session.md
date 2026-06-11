# Session context

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
