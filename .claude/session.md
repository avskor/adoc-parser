# Session context

## Сессия (2026-06-11) — R9: общий канал document-attrs → inline-парсер (InlineOptions)

Запрос «продолжи R9». R8 уже в master (`1fbbde4`, merge `refactor/html-modules`;
session.md прошлой сессии писалась ДО мержа — как всегда). Новая ветка
**`refactor/inline-doc-attrs-channel`** (СТАТУС: НЕ закоммичено).
`/tmp/adoc_base` пересобран из чистого master `1fbbde4` ПЕРЕД правками.

### Что сделано
- **`InlineOptions`** (adoc-parser/src/inline.rs, перед InlineParser; реэкспорт
  `pub use inline::{InlineOptions, InlineParser}` в lib.rs): pub-struct
  (Debug/Clone/Copy/Default/PartialEq/Eq), поле `experimental: bool`. Единый канал
  document-attrs → inline-парсер, ДВА пути заполнения (оба задокументированы doc-комментом
  «новые attr-гейтящие фичи = поле + arm в обоих конструкторах»):
  - **streaming** `apply_attribute(&mut self, name)` — имя КАК В `Event::Attribute`
    (unset-формы `!name`/`name!` нормализуются генерически strip'ом `!`; `!name!` —
    no-op, как и в старом match). Зовёт `Parser` в arm'е Event::Attribute —
    mid-document set/unset семантика сохранена 1:1.
  - **snapshot** `from_attr_lookup(is_set: impl FnMut(&str) -> bool)` — для рендереров
    поверх готовой таблицы атрибутов.
- **API InlineParser**: `parse_str_with_subs_options(text, subs, options)` — новая
  основная точка; `parse_str_with_subs` = wrapper с `InlineOptions::default()`;
  `parse_str_with_subs_experimental` УДАЛЁН (мигрированы все 3 потребителя:
  parser.rs ×2 — `self.inline_options`; adoc-html lib.rs `render_inline_value` —
  `from_attr_lookup(|n| document_attrs.contains_key(n))`; тест-хелпер
  `parse_experimental` — литерал `InlineOptions { experimental: true }`).
- **InlineState**: поле `experimental: bool` → `options: InlineOptions`; 5 inner-reparse
  вызовов `InlineState::new(…, self.options)` наследуют ВЕСЬ набор опций (раньше — один
  bool). Гейты kbd/btn/menu читают `self.options.experimental`.
- **Parser**: поле `experimental: bool` → `inline_options: InlineOptions`; ad-hoc match
  трёх spelling'ов заменён на `self.inline_options.apply_attribute(name.as_ref())`.
- +1 тест `test_inline_options_channel` (set/обе unset-формы/чужие атрибуты игнорируются;
  snapshot-путь зеркалит streaming).

### Статус (верифицировано)
- clippy --workspace 0 warnings; `cargo test --workspace` ВСЁ зелёное (18 suites ok:
  parser 461→**462**, html 328+36, render-core 12, parsing-lab ok, html-compat, integration 25).
- **Рефакторинг-нейтральность: вывод нового release-бинаря байт-в-байт совпадает с
  `/tmp/adoc_base` (master `1fbbde4`) на ВСЕХ 344 файлах корпуса (0 diffs, 0 exit-diffs)**.
- Корпус `compare_full.py`: **Identical 204, Different 140, Errors 0** (= baseline).

### Что дальше
- **Спросить про коммит/мерж/пуш** (только по запросу). В diff: adoc-parser/src/{inline,
  parser,lib}.rs, adoc-html/src/lib.rs, TODO.md, session.md.
- **Аудит рендерера R1–R9 ПОЛНОСТЬЮ ЗАКРЫТ** (R3 — частично by design: новые block-arm'ы
  писать через `open_block_with_title`). Дальше — возврат к Фазе 3 (флипы корпуса:
  pass/index 6-diff, special-section-numbers 10-diff, callout 20-diff, part 22-diff;
  архитектурные кластеры — наследование `m`/`e`/`s` стиля колонки таблицы, author-header
  `<div class="details">`, sect0-heading) либо старт второго рендерера (core готов).

### Предостережения (без изменений)
- НЕ cargo fmt. Коммит только по запросу. Корпус: python3 /mnt/c/tmp/adoc-test/compare_full.py
  (release, `cargo build --release -p adoc-cli`). Нейтральность: цикл cmp /tmp/adoc_base vs
  /tmp/adoc_new по всем .adoc корпуса. CLI: `adoc [--no-standalone] file` (флага `-e` НЕТ).

---

## Сессия (2026-06-10, восьмая) — R8: распил adoc-html/src/lib.rs на модули

Запрос «продолжи R8». R7 этап 5 уже в master (`490a082`, merge
`refactor/render-core-author-revision`; session.md прошлой сессии писалась ДО мержа — как
всегда). Новая ветка **`refactor/html-modules`** (СТАТУС: НЕ закоммичено).
`/tmp/adoc_base` пересобран из чистого master `490a082` ПЕРЕД правками.

### Что сделано
- **lib.rs (6220 строк) распилен на 8 файлов** скриптом `/tmp/split_html.py` (экстракция
  чанков по line-ranges с пере-привязкой предшествующих комментариев; ловушка: хвостовые
  комментарии чанка = doc следующего элемента, иначе дублирование — починено strip'ом):
  - **lib.rs** (417): doc, uses, mod-декларации, `use blocks::*/escape::*/media::*`,
    HtmlOptions, push_html/to_html(+_with_options), DlistStyle, BlockMeta,
    parse_highlight_spec, struct HtmlRenderer, impl-core (new, new_with_options,
    apply_attribute, current_subs, default_subs_for_delimited, run, render_inline_value).
  - **events.rs** (1009): push_event, start_tag, end_tag (3 диспетчера).
  - **blocks.rs** (861): start_admonition/table/table_cell/lists/section_title/section_div/
    paragraph/delimited_block/source_block, emit_pending_block_title, open/close_li_paragraph,
    open_block_with_title, push_caption_prefix, take_block_meta, trim_verbatim_content,
    col-width/tableblock-class хелперы, strip_block_style, write_meta_attrs,
    current_dlist_style, is_book/find_section_level/is_inside_*-контекст-хелперы +
    free fns parse_manpage_title, section_level_to_h.
  - **inline.rs** (251): start_cross_reference, push_inline_id_class, render_kbd_keys,
    render_menu, render_icon, render_inline_stem, render_stem_block.
  - **media.rs** (342): start_block_image, start_inline_image, image_base_class +
    MediaAttrs, parse_media_attrs, detect_video_provider, render_video_tag,
    push_media_time_fragment, render_audio_tag, auto_alt_from_target.
  - **finish.rs** (317): finish, render_footnotes, generate_toc, render_author_details,
    write_document_head/tail + resolve_sentinels_into + DEFAULT_STYLESHEET (include_str —
    путь относительный файлу, работает из src/), MATHJAX_DOCINFO.
  - **escape.rs** (84): html_escape, html_escape_text, push_hardbreaks_text,
    rstrip_line_trailing_ws, write_attr.
  - **tests.rs** (2972): бывший `mod tests` (дедент на 4; raw-строк в тестах нет —
    единственный `r#"` в файле был MATHJAX const).
- **Видимость**: всё перенесённое в дочерние модули — `pub(crate)` (методы siblings зовут
  кросс-модульно); элементы корня (struct, BlockMeta, parse_highlight_spec, uses) НЕ меняли
  видимость — приватные элементы корня видны потомкам, `use crate::*` в каждом модуле
  глоб-импортирует их (тот же механизм, что старый `use super::*` в tests).
- **Сохранность кода доказана**: diff отсортированных мультимножеств непустых строк
  (без отступов, без `pub(crate) `) old vs new — отличие ТОЛЬКО обёртки `impl HtmlRenderer {`,
  mod/use-строки и module-docs. Попутная микро-правка: doc-комментарий
  rstrip_line_trailing_ws был прилипшим над push_hardbreaks_text (pre-existing) — отлеплен
  на своё место (только комментарии).

### Статус (верифицировано)
- clippy --workspace 0 warnings; `cargo test --workspace` ВСЁ зелёное (parser 461,
  html 328+36, render-core 12, parsing-lab ok, html-compat 6/6+6, integration 25).
- **Рефакторинг-нейтральность: вывод нового release-бинаря байт-в-байт совпадает с
  `/tmp/adoc_base` (master `490a082`) на ВСЕХ 344 файлах корпуса (0 diffs, 0 exit-diffs)**
  — проверено дважды (после распила и после комментарной правки).
- Корпус `compare_full.py`: **Identical 204, Different 140, Errors 0** (= baseline).

### Что дальше
- **Спросить про коммит/мерж/пуш** (только по запросу). В diff: adoc-html/src/lib.rs +
  7 НОВЫХ файлов (events/blocks/inline/media/finish/escape/tests .rs), TODO.md, session.md.
- Из аудита остался **R9** (канал document-attrs → inline-парсер вместо ad-hoc
  `Parser.experimental`). Либо возврат к Фазе 3 (флипы корпуса: pass/index 6-diff,
  special-section-numbers 10-diff, callout 20-diff, part 22-diff; архитектурный кластер —
  наследование `m`/`e`/`s` стиля колонки таблицы, sect0-heading).

### Предостережения (без изменений)
- НЕ cargo fmt. Коммит только по запросу. Корпус: python3 /mnt/c/tmp/adoc-test/compare_full.py
  (release, `cargo build --release -p adoc-cli`). Нейтральность: цикл cmp /tmp/adoc_base vs
  /tmp/adoc_new по всем .adoc корпуса. CLI: `adoc [--no-standalone] file` (флага `-e` НЕТ).

---

## Сессия (2026-06-10, седьмая) — R7 этап 5 (ФИНАЛ): Author/Revision в adoc-render-core

Запрос «продолжи R7». Этап 4 уже в master (`de4decd`, merge
`refactor/render-core-captions`; session.md прошлой сессии писалась ДО мержа — как
всегда). Новая ветка **`refactor/render-core-author-revision`** (СТАТУС: НЕ закоммичено).
`/tmp/adoc_base` пересобран из чистого master `de4decd` ПЕРЕД правками.

### Что сделано
- **Граница со scanner.rs проверена, корректна**: `parse_authors`/`parse_revision_line`
  (включая revnumber-strip нецифрового префикса `\D*`) — парсинг строки заголовка,
  остаются в ПАРСЕРЕ. В core уезжает рендер-семантика поверх событий Author/Revision.
- **adoc-render-core** (+author/revision-секция после FootnoteRegistry):
  - `Author` — 6 pub-полей String (fullname/firstname/middlename/lastname/initials/
    address), plain-текст.
  - `AuthorRegistry`: `add(author) -> Vec<(String,String)>` — document-attribute-entries
    с suffix-правилом (index 0 → без суффикса `author`/`email`/…; дальше `author2`…;
    `middlename{s}`/`email{s}` ТОЛЬКО non-empty); `attr_suffix(index)` (статич.);
    `authors()`; `is_empty()`.
  - `Revision { version, date, remark }`: `attr_entries() -> Vec<(&'static str,&str)>`
    (revnumber/revdate/revremark, пустые компоненты — ничего), `display_version()`
    (strip ОДНОГО ведущего `v`/`V` — семантика details-заголовка; revision-line приходит
    уже стрипнутой scanner'ом → no-op, важно для explicit-установленных строк).
  - +2 юнит-теста (author_registry_attr_entries: suffix-правило/conditional-поля;
    revision_entries_and_display: non-empty-фильтр/strip v|V/Default-пусто). Всего 12.
- **adoc-html**: удалены `AuthorData`/`RevisionData`; поля → `authors: AuthorRegistry`,
  `revision: Option<Revision>`. Arm `Event::Author` → `add` + `document_attrs.extend`;
  arm `Event::Revision` → `Revision` + цикл по `attr_entries`. `render_author_details`:
  итерация `authors()`, `AuthorRegistry::attr_suffix(i)`, `rev.display_version()`;
  весь details-div HTML (span'ы id=author/email/revnumber/revdate/revremark,
  mailto-ссылка, формат «version X,», `<br>`) остался в рендерере.
- **R7 ЗАВЕРШЁН** (все 5 этапов): attr-refs, xref, section-numbering/TOC, captions,
  footnotes, author/revision — в adoc-render-core. TODO.md: R7 отмечен `[x]`.

### Статус (верифицировано)
- clippy --workspace 0 warnings; `cargo test --workspace` ВСЁ зелёное (18 suites ok:
  parser 461, html 328+36, render-core 12, parsing-lab **233/233**, html-compat 6/6+6,
  integration 25).
- **Рефакторинг-нейтральность: вывод нового release-бинаря байт-в-байт совпадает с
  `/tmp/adoc_base` (master `de4decd`) на ВСЕХ 344 файлах корпуса (0 diffs, 0 exit-diffs)**.
- Корпус `compare_full.py`: **Identical 204, Different 140, Errors 0** (= baseline).

### Что дальше
- **Спросить про коммит/мерж/пуш** (только по запросу). В diff: adoc-render-core/src/lib.rs,
  adoc-html/src/lib.rs, TODO.md, session.md.
- R7 закрыт. Остались из аудита: **R8** (распил adoc-html/src/lib.rs ~6300 строк на
  модули — независим), **R9** (канал document-attrs → inline-парсер вместо ad-hoc
  `Parser.experimental`). Либо возврат к Фазе 3 (флипы корпуса, near-miss-кандидаты
  в TODO.md).

### Предостережения (без изменений)
- НЕ cargo fmt. Коммит только по запросу. Корпус: python3 /mnt/c/tmp/adoc-test/compare_full.py
  (release, `cargo build --release -p adoc-cli`). Нейтральность: цикл cmp /tmp/adoc_base vs
  /tmp/adoc_new по всем .adoc корпуса. CLI: `adoc [--no-standalone] file` (флага `-e` НЕТ).

---

## Сессия (2026-06-10, шестая) — R7 этап 4: CaptionCounters + FootnoteRegistry в adoc-render-core

Запрос «продолжи R7». Этап 3 уже в master (`86d8685`, merge
`refactor/render-core-section-toc`; session.md прошлой сессии писалась ДО мержа — как
всегда). Новая ветка **`refactor/render-core-captions`** (СТАТУС: НЕ закоммичено).
`/tmp/adoc_base` пересобран из чистого master `86d8685` ПЕРЕД правками.

### Что сделано
- **adoc-render-core** (+caption/footnote-секция в lib.rs после SectionNumberer):
  - `CaptionKind { Figure, Table, Example }`, `CaptionPrefix::{None, Custom(&str),
    Numbered { label, number }}` — plain-текст, потребитель экранирует и форматирует
    («Label N. » в HTML).
  - `CaptionCounters::caption_prefix(kind, caption_attr, doc_label)`: `caption=""` →
    None; `caption=X` → Custom verbatim; иначе Numbered при doc_label=Some, None при
    unset. **Bump-семантика по kind (зеркало старого рендерера, 1:1)**: figure/table
    бампят счётчик ТОЛЬКО в Numbered-ветке; example — на КАЖДЫЙ titled-блок, даже под
    caption=-override/подавлением (задокументировано в doc-комменте).
  - `Footnote { number, id: Option<String>, text }` (text — plain, потребитель
    экранирует), `FootnoteRegistry`: `define(id, text) -> usize` (номер = document-order;
    named id регистрируется, redefinition — обе записи остаются, id указывает на
    новейшую), `lookup(id)`, `footnotes()`, `is_empty()`.
  - +2 юнит-теста (caption_counters: bump-различия/подавление/custom/независимость
    kind'ов; footnote_registry: нумерация/named lookup/redefinition last-wins). Всего 10.
- **adoc-html**: удалены поля `figure_counter`/`table_counter`/`example_counter`,
  `footnotes`/`footnote_counter`/`named_footnotes`; добавлены `caption_counters:
  CaptionCounters`, `footnote_registry: FootnoteRegistry`. `push_caption_prefix` —
  static fn → метод `&mut self` поверх core (match CaptionPrefix → html_escape/формат);
  example-arm (бывшее inline-дублирование с хардкодом «Example N. ») переведён на
  него же (`Some("Example")` как doc_label — рендерер решает, откуда label).
  Footnote-arms (`Event::Footnote` → `define`, `Event::FootnoteRef` → `lookup`) и
  `render_footnotes` (итерация по `footnotes()`) — на реестре. Label-источники
  (document_attrs `figure-caption`/`table-caption` с дефолтами, `:figure-caption!:` →
  None) остались в рендерере.

### Статус (верифицировано)
- clippy --workspace 0 warnings; `cargo test --workspace` ВСЁ зелёное (18 suites ok:
  parser 461, html 328+36, render-core 10, parsing-lab **233/233**, html-compat 6/6+6,
  integration 25).
- **Рефакторинг-нейтральность: вывод нового release-бинаря байт-в-байт совпадает с
  `/tmp/adoc_base` (master `86d8685`) на ВСЕХ 344 файлах корпуса (0 diffs, 0 exit-diffs)**.
- Корпус `compare_full.py`: **Identical 204, Different 140, Errors 0** (= baseline).

### Что дальше
- **Спросить про коммит/мерж/пуш** (только по запросу). В diff: adoc-render-core/src/lib.rs,
  adoc-html/src/lib.rs, TODO.md, session.md.
- R7 этап 5 (последний кандидат): author/revision-семантика (details-div в standalone;
  revnumber-strip уже в scanner.rs парсера — проверить границу). R9 стыкуется (канал
  document-attrs). Уже хорошо разделено (не трогать): subs — inline.rs, table-grid — block.rs.
- R8 (распил lib.rs на модули) — независим.

### Предостережения (без изменений)
- НЕ cargo fmt. Коммит только по запросу. Корпус: python3 /mnt/c/tmp/adoc-test/compare_full.py
  (release, `cargo build --release -p adoc-cli`). Нейтральность: цикл cmp /tmp/adoc_base vs
  /tmp/adoc_new по всем .adoc корпуса. CLI: `adoc [--no-standalone] file` (флага `-e` НЕТ).

---

## Сессия (2026-06-10, пятая) — R7 этап 3: SectionNumberer + TocBuilder в adoc-render-core

Запрос «продолжи R7 — этап 3, SectionNumberer+TocBuilder». Этап 2 уже в master
(`280e0ce`, merge `refactor/render-core-xref-resolver`). Новая ветка
**`refactor/render-core-section-toc`** (СТАТУС: НЕ закоммичено).
`/tmp/adoc_base` пересобран из чистого master `280e0ce` ПЕРЕД правками.

### Что сделано
- **adoc-render-core** (+секция TOC/нумерации в lib.rs):
  - `TocEntry { level, id, title }` (pub-поля) — бывший приватный тип рендерера; title —
    plain-текст (потребитель экранирует), включая префиксы, которые потребитель доклеил.
  - `TocBuilder` — push/entries (entries() двойного назначения: TOC + реестр секций для
    XrefResolver) + **`toc_steps(toc_levels) -> Vec<TocStep>`**: структурная раскладка
    дерева TOC (EnterLevel(u8)/Item(&TocEntry)/CloseItem/LeaveLevel), фильтрация уровней
    2..=toc_levels+1 (u16-арифметика — нет переполнения u8 при toclevels=255), пустой
    Vec = «TOC не эмитить». Раскладка 1:1 повторяет цикл старого generate_toc
    (включая многоуровневые прыжки вглубь: 3→5 открывает два ul подряд).
  - `DEFAULT_TOC_TITLE` = "Table of Contents".
  - `SectionNumberer` — `number_prefix(level)`: счётчики sectnums («1.2.3. », хвостовой
    ". " включён; инкремент уровня + сброс глубже; None вне 2..=5 БЕЗ изменения
    счётчиков) и `appendix_caption()` («Appendix A: », счётчик внутри).
  - +2 юнит-теста (toc_structure_steps: вложенность/прыжки/фильтрация/пусто;
    section_numbering: последовательность/сброс/out-of-range/appendix). Всего в core 8.
- **adoc-html**: удалены приватный `struct TocEntry` и поля `toc_entries`/
  `section_counters`/`appendix_counter`; добавлены `toc_builder: TocBuilder`,
  `section_numberer: SectionNumberer`. `generate_toc` = map TocStep→HTML (div/ul/li,
  sectlevel{l-1}, newline-guard, html_escape — всё осталось тут). Гейтинг нумерации
  (`sectnums`-флаг + подавление `pending_section_caption`) остался в рендерере —
  семантика «когда нумеровать» завязана на спец-секции, механика счётчиков — в core.
  toc_title init через DEFAULT_TOC_TITLE. Накопление current_toc_entry (Text/Code
  events) и manpage-NAME-захват не тронуты (работают с core-типом, поля pub).

### Статус (верифицировано)
- clippy --workspace 0 warnings; `cargo test --workspace` ВСЁ зелёное (parser 461,
  html 328+36, render-core 8, parsing-lab **233/233**, html-compat 6/6+6, integration 25).
- **Рефакторинг-нейтральность: вывод нового release-бинаря байт-в-байт совпадает с
  `/tmp/adoc_base` (master `280e0ce`) на ВСЕХ 344 файлах корпуса (0 diffs, 0 exit-diffs)**.
- Корпус `compare_full.py`: **Identical 204, Different 140, Errors 0** (= baseline).

### Что дальше
- **Спросить про коммит/мерж/пуш** (только по запросу). В diff: adoc-render-core/src/lib.rs,
  adoc-html/src/lib.rs, TODO.md, session.md.
- R7 этап 4 (кандидаты): счётчики caption'ов (figure/table/example — у каждого своя
  логика: figure bump только titled; общий push_caption_prefix уже есть в рендерере;
  footnote-нумерация/дедуп named), author/revision-семантика (details-div;
  revnumber-strip уже в scanner.rs парсера — проверить границу). R9 стыкуется
  (канал document-attrs).
- R8 (распил lib.rs на модули) — независим.

### Предостережения (без изменений)
- НЕ cargo fmt. Коммит только по запросу. Корпус: python3 /mnt/c/tmp/adoc-test/compare_full.py
  (release, `cargo build --release -p adoc-cli`). Нейтральность: цикл cmp /tmp/adoc_base vs
  /tmp/adoc_new по всем .adoc корпуса. CLI: `adoc [--no-standalone] file` (флага `-e` НЕТ).

---

## Сессия (2026-06-10, четвёртая) — R7 этап 2: XrefResolver в adoc-render-core

Запрос «продолжи R7 — этап 2, XrefResolver». Этап 1 уже в master (`cf39bb1`,
merge `refactor/render-core-attr-resolver`). Новая ветка
**`refactor/render-core-xref-resolver`** (СТАТУС: НЕ закоммичено).
`/tmp/adoc_base` пересобран из чистого master `cf39bb1` ПЕРЕД правками.

### Что сделано
- **adoc-render-core** (+xref-секция в lib.rs):
  - `RefText::{Plain, Markup}` — решение проблемы «завязан на html_escape»: секции
    регистрируются Plain (потребитель экранирует под свой формат), заголовки блоков и
    bibliography-reftext'ы — Markup (готовая разметка потребителя, verbatim).
  - `XrefResolver<'a>` — бывший `ResolutionContext`: `add_section` (id last-wins,
    natural-xref title→id first-wins), `add_block` (or_insert — секции побеждают, звать
    ПОСЛЕ секций), `link_text` (известный id → текст, иначе по заголовку секции),
    `href_id` (известный id литерально → title→id → литерально).
  - `unresolved_xref_label(target)` → `[target]` (дефолтный xreflabel asciidoctor),
    `is_interdoc_xref_target` (`.` и не `#…`), `interdoc_xref_href` (.adoc→.html,
    `file.adoc#anchor` → `file.html#anchor`; = auto-text безлейбльного inter-doc xref).
  - +2 юнит-теста (precedence/natural-xref/href; interdoc-таргеты). Всего в core 6.
- **adoc-html**: `ResolutionContext` (struct+impl, ~60 строк) удалён; `finish()` строит
  `XrefResolver` из трёх реестров (toc_entries → Plain; block_ref_titles,
  bibliography_reftexts → Markup), матчит `RefText` при сборке replacements
  (Markup → push verbatim, Plain → html_escape; None → unresolved_xref_label для
  internal, экранирование снаружи скобок эквивалентно — `[`/`]` не экранируются).
  `start_cross_reference` использует core-функции interdoc. Сентинель-машинерия
  (`resolve_sentinels_into`, XREF_/XREFHREF_-плейсхолдеры) НЕ тронута — это механика
  отложенного резолва HTML-вывода, не семантика.

### Статус (верифицировано)
- clippy --workspace 0 warnings; `cargo test --workspace` ВСЁ зелёное (parser 461,
  html 328+36, render-core 6, parsing-lab **233/233**, html-compat, integration 25).
- **Рефакторинг-нейтральность: вывод нового release-бинаря байт-в-байт совпадает с
  `/tmp/adoc_base` (master `cf39bb1`) на ВСЕХ 344 файлах корпуса (0 diffs)**.
- Корпус `compare_full.py`: **Identical 204, Different 140, Errors 0** (= baseline).

### Что дальше
- **Спросить про коммит/мерж/пуш** (только по запросу). В diff: adoc-render-core/src/lib.rs,
  adoc-html/src/lib.rs, TODO.md, session.md.
- R7 этап 3 (кандидаты): SectionNumberer+TocBuilder (toc_entries/generate_toc/sectnums в
  adoc-html), счётчики (figure/table/example/footnote — каждый со своей caption-логикой,
  push_caption_prefix), author/revision-семантика (details-div, revnumber-strip уже в
  scanner.rs парсера — проверить границу). R9 стыкуется (канал document-attrs).
- R8 (распил lib.rs на модули) — независим.

### Предостережения (без изменений)
- НЕ cargo fmt. Коммит только по запросу. Корпус: python3 /mnt/c/tmp/adoc-test/compare_full.py
  (release, `cargo build --release -p adoc-cli`). Нейтральность: цикл cmp /tmp/adoc_base vs
  /tmp/adoc_new по всем .adoc корпуса. CLI: `adoc [--no-standalone] file` (флага `-e` НЕТ).

---

## Сессия (2026-06-10, третья) — R7 этап 1: крейт adoc-render-core (AttributeResolver)

Запрос «продолжи R7». master `2a9bf9f` (R5 уже смержен), новая ветка
**`refactor/render-core-attr-resolver`** (СТАТУС: НЕ закоммичено).
`/tmp/adoc_base` пересобран из чистого master `2a9bf9f` ПЕРЕД правками.

### Что сделано (этап 1 R7 — attr-refs; XrefResolver/SectionNumberer/счётчики — следующие этапы)
- **Новый крейт `adoc-render-core`** (zero-dep; добавлен в workspace `members` ПОСЛЕ
  adoc-parser). API:
  - `IntrinsicAttribute { name, text, html }` + `INTRINSIC_ATTRIBUTES` (sorted) +
    `intrinsic_attribute(name)`. ДВЕ колонки данными: `text` — семантика (для ASG/будущих
    рендереров), `html` — байт-в-байт форма asciidoctor. Кодировку НЕЛЬЗЯ вывести правилом
    из text: `plus`/`pp` → `&#43;` (защита от passthrough-реинтерпретации), но `cpp` →
    литеральный `C++` — это per-attribute данные самого asciidoctor.
  - `resolve_attribute_reference(name, doc_lookup, env_lookup, fallback, attribute_missing)
    -> AttrRefOutcome` — полный precedence (doc → intrinsic → `env-*` → fallback →
    missing-mode). Generic через closures — потребители с разными типами мап работают
    без конверсий. Нюанс сохранён: env-miss БЕЗ fallback → MissingSkip (литерал `{name}`)
    НЕ консультируя attribute-missing (зеркало старого html-кода).
  - `resolve_attr_refs_text(value, doc_lookup)` — eager `{name}`-резолв в строке
    (бывший builder::resolve_attr_refs), intrinsic через `text`-колонку.
  - 4 юнит-теста (sorted-инвариант+колонки, precedence, env-семантика, строковый резолв).
- **adoc-html**: удалены `INTRINSIC_ATTRIBUTES`/`intrinsic_attribute` (lib.rs:44-82);
  arm `Event::AttributeReference` → match по `AttrRefOutcome` (Document — прежний
  combine-and-reparse с trailing_brackets; Intrinsic — `attr.html` raw; Env/Fallback —
  html_escape; MissingSkip — литерал+brackets; MissingDrop — ничего). Поведение 1:1.
- **adoc-compat-tests/builder.rs**: удалены `INTRINSIC_ATTRIBUTES_TEXT`/
  `intrinsic_attribute_text`/тело `resolve_attr_refs` (теперь делегат в core);
  arm AttributeReference → core (`attr.text`). **Дрейф-фикс**: builder НЕ знал
  `apos`/`pp`/`quot` — теперь знает (parsing-lab не дрогнул: кейсов нет).
- trailing_brackets НЕ в core — это consumer-policy (html реparse'ит `value[...]`,
  ASG дописывает литералом); зафиксировано доками на `AttrRefOutcome`.

### Статус (верифицировано)
- clippy --workspace 0 warnings; `cargo test --workspace` ВСЁ зелёное (parser 461,
  html 328+36, render-core 4, parsing-lab **233/233** `--nocapture`, html-compat 6/6+1,
  integration 25, author_rendering 6).
- **Рефакторинг-нейтральность: raw-вывод нового release-бинаря байт-в-байт совпадает с
  `/tmp/adoc_base` на ВСЕХ 344 файлах корпуса (0 diffs, 0 exit-diffs)**.
- Корпус `compare_full.py` (release): **Identical 204, Different 140, Errors 0** (= baseline).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки (только по запросу). В diff: Cargo.toml (workspace),
  adoc-render-core/* (новый), adoc-html/Cargo.toml+src/lib.rs,
  adoc-compat-tests/Cargo.toml+src/builder.rs, TODO.md, session.md, Cargo.lock.
- R7 этап 2 (кандидаты): XrefResolver — вынести семантику `ResolutionContext` из
  adoc-html/finish() (precedence link-text/href, natural xref, .adoc→.html) в core;
  сложность: завязан на html_escape/готовый HTML заголовков блоков — нужна абстракция
  «как экранировать». Затем SectionNumberer+TocBuilder, счётчики, author/revision.
  R9 (канал document-attrs → inline-парсер) стыкуется с этим же выносом.
- R8 (распил lib.rs ~6300 строк) — независим, можно в любой момент.

### Предостережения (без изменений)
- НЕ cargo fmt. Коммит только по запросу. Корпус: python3 /mnt/c/tmp/adoc-test/compare_full.py
  (release, `cargo build --release -p adoc-cli`). Нейтральность: цикл cmp /tmp/adoc_base vs
  /tmp/adoc_new по всем .adoc корпуса. CLI: `adoc [--no-standalone] file` (флага `-e` НЕТ).

---

## Сессия (2026-06-10, вторая) — R5 завершён: ResolutionContext + однопроходный резолв сентинелей

`fix/block-image-figure-caption` УЖЕ смержена в master (`eab7a20`, дерево чистое; session.md
прошлой сессии писалась ДО мержа — как всегда). Новая ветка
**`refactor/finish-single-pass-resolution`** (от master `eab7a20`; СТАТУС: НЕ закоммичено).
`/tmp/adoc_base` пересобран из чистого master `eab7a20` ПЕРЕД правкой.

### Что сделано (R5-остаток из аудита рендерера, `adoc-html/src/lib.rs`)
- **`ResolutionContext<'a>`** (module-level, перед `html_escape`): единые lookup'ы `finish()`,
  строятся ОДИН раз из toc_entries/block_ref_titles/bibliography_reftexts. Поля: `id_to_text:
  HashMap<&str, CowStr>` (секции html_escape'ятся в Owned, block/biblio-HTML — Borrowed;
  precedence сохранён: секции plain insert — last wins, block/biblio `or_insert`; членство
  ключей = бывший `known_ids` href-пасса) и `title_to_id` (natural xref, first wins). Методы
  `link_text(target)` (id → текст, иначе title→id→текст) и `href_id(target)` (known id —
  литерал, иначе title→id, иначе литерал).
- **Однопроходный резолв**: вместо `*output = output.replace(placeholder, …)` на КАЖДЫЙ
  плейсхолдер (полное сканирование + реаллокация, O(n²)) — обе группы (XREF_/XREFHREF_)
  собираются в `HashMap<&str плейсхолдер, String замена>`, затем `resolve_sentinels_into`
  (module-level fn) один раз сканирует output по `find('\0')`: кандидат = `\x00…\x00`,
  lookup в map; не-сентинельный NUL остаётся как есть. **Вложенные сентинели в заменах**
  (xref внутри `.Title` блока, на который ссылаются `<<id>>`) резолвятся рекурсивно
  (depth cap 8 — self-referential title не зациклится).
- **Попутный багфикс**: на master кейс «блок с id + `.See <<Later>>` + `<<blk>>`» ТЁК сырым
  сентинелем (`Ref: …` со встроенным ` XREF_2 `, 2 NUL-байта в выводе — верифицировано пробой
  /tmp/p_nested.adoc против /tmp/adoc_base): старый порядок replace'ов уже обработал XREF_2 к
  моменту вставки заголовка блока. Новый код резолвит (0 NUL). +1 тест
  `test_xref_to_block_whose_title_contains_xref` (adoc-html/tests/html_output.rs).
- **Перф**: стресс /tmp/p_stress.adoc (2000 секций + 4000 xref): base 807ms → new 33ms (~24×).
  Вывод на стрессе IDENTICAL base vs new (cmp).

### Статус (верифицировано)
- clippy 0 warnings; `cargo test --workspace` ВСЁ зелёное (867 passed суммарно: parser 461,
  html 328+36+2, parsing-lab **233/233** `--nocapture`, html-compat **70/70**, integration 25).
- **Рефакторинг-нейтральность: raw-вывод нового release-бинаря байт-в-байт (cmp) совпадает
  с `/tmp/adoc_base` на ВСЕХ 344 файлах корпуса (0 diffs)**.
- Корпус `compare_full.py` (release): **Identical 204, Different 140, Errors 0** (= baseline).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `refactor/finish-single-pass-resolution` (только
  по запросу). В diff: adoc-html/src/lib.rs, adoc-html/tests/html_output.rs, TODO.md, session.md.
- Из аудита остались: **R7** (adoc-render-core — перед вторым рендерером), **R8** (распил
  lib.rs ~6300 строк на модули), **R9** (Parser.experimental ad-hoc канал). R3 — частично
  (новые block-arm'ы писать через `open_block_with_title`).
- video.adoc 4-diff — near-miss кандидат (youtube/vimeo iframe нюансы, разведать fdiff.py).

### Предостережения (без изменений)
- НЕ cargo fmt. Коммит только по запросу. Корпус: python3 /mnt/c/tmp/adoc-test/compare_full.py
  (release, `cargo build --release -p adoc-cli`). blast: /tmp/blast.py (base /tmp/adoc_base =
  чистый master `eab7a20`). fdiff: /tmp/fdiff.py <relpath> [base-бинарь]. Пробы /tmp/p_nested.adoc,
  /tmp/p_stress.adoc. CLI: `adoc [--no-standalone] file` (флага `-e` НЕТ).

---

## Сессия (2026-06-10) — Реализация R1/R2/R4/R6 + частично R3/R5 (ветка `fix/block-image-figure-caption`)

Продолжение аудита поздней-29: по «приступай» реализованы находки. master `532c10a`,
ветка `fix/block-image-figure-caption` (СТАТУС: НЕ закоммичено; в diff также TODO.md/session.md
от сессии-аудита). `/tmp/adoc_base` пересобран из чистого master `532c10a` ПЕРЕД правками,
baseline подтверждён: Identical 204, Different 140, Errors 0.

### Что сделано (всё верифицировано пробами asciidoctor через ФАЙЛ, p1–p8 в /tmp)
- **R1 — figure caption на block-image** (2 слоя). РЕНДЕРЕР (`adoc-html/lib.rs`): поле
  `figure_counter`, дефолтный attr `figure-caption`=«Figure», эмиссия `<div class="title">
  Figure N. Title</div>` ПОСЛЕ content-div в `start_block_image` (стал `&mut self`-методом);
  общий хелпер `push_caption_prefix` (table-caption переведён на него, поведение 1:1).
  Правила (пробы): bump только titled; `caption=` verbatim БЕЗ bump; `:figure-caption!:` →
  голый title; `:figure-caption: Рисунок` → кастомный label; `title=`-attr ПОБЕЖДАЕТ `.Title`.
  ПАРСЕР: `attributes.rs::parse_image_attrs` +поля `caption`/`title`; alt-fallback при
  named-only скобках "" (был сырой bracket_content — `image::a.png[width=100]` давал
  alt="width=100"); `block.rs::scan_block_macros` мёржит caption в block_attrs.named,
  title= синтезирует BlockTitle-events (vec![Start,Text,End]), заменяя pending `.Title`.
- **R2 + бонус stem**: `open_block_with_title` хелпер (wrapper+title+content), применён к
  video (НОВОЕ: title эмитится, ДО content — зеркало audio), stem (ТОТ ЖЕ баг утечки title —
  найден этой сессией пробой p8), audio/openblock (чистый дедуп). video.adoc 47→4 diff.
- **R4**: `push_media_time_fragment` (общий `#t=` для audio/video). Порядок boolean-атрибутов
  НЕ объединён — намеренно разный.
- **R6**: `open_li_paragraph`/`close_li_paragraph`; ListItem 3 arm'а → 1 (match checked);
  DescriptionDescription не тронут (dd_output_start rollback асимметричен).
- **R5 частично**: `title_to_id` hoisted (строится 1 раз для обоих xref-пассов).
- Тесты: +4 (html: figure-caption-сценарии, video+stem-title leak-guard; parser:
  caption/title/alt-fallback в attributes.rs). parser 460→461, html 326→328.

### Статус (верифицировано)
- clippy 0 warnings; test --workspace ВСЁ зелёное; parsing-lab 233/233, html-compat 6/6.
- Корпус: **Identical 204, Different 140, Errors 0** (флипов нет — image/video-файлы корпуса
  Different по другим каскадам). Blast vs `/tmp/adoc_base`: 3 файла изменили вывод,
  **0 регрессий**, все улучшены: image.adoc 135→128, id.adoc 49→45, video.adoc 47→**4**.
- Рефакторинг-нейтральность: raw-вывод нового бинаря vs `/tmp/adoc_r1` (пост-R1 эталон) по
  всем 344 файлам — отличие ТОЛЬКО video.adoc (= ожидаемый эффект R2). p1–p8 IDENTICAL.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки (только по запросу). В diff входят TODO.md/session.md.
- video.adoc теперь 4-diff — near-miss кандидат (остаток: youtube/vimeo iframe нюансы?
  разведать `fdiff.py`).
- R5-остаток (ResolutionContext + один проход вместо output.replace-цикла), R7 (adoc-render-core,
  перед вторым рендерером), R8 (распил lib.rs на модули), R9.

### Предостережения (без изменений)
- НЕ cargo fmt. Коммит только по запросу. Корпус: python3 /mnt/c/tmp/adoc-test/compare_full.py
  (release). blast: /tmp/blast.py (base /tmp/adoc_base = чистый master `532c10a`).
  fdiff: /tmp/fdiff.py <relpath> [base-бинарь]. `/tmp/adoc_r1` — пост-R1 бинарь (эталон
  рефакторинг-нейтральности). Пробы /tmp/p1–p8.adoc.

---

## Сессия (2026-06-09, поздняя-29) — Аудит рендерера: мульти-рендерер + дедупликация (БЕЗ правок кода)

Запрос: изучить архитектуру, найти недочёты; фокус — готовность к НЕ-HTML-рендерерам и
дублирование в рендерере. Только анализ; код НЕ менялся. master `532c10a` (stem-mathjax уже
смержена), дерево чистое. Проверено: clippy 0 warnings, test --workspace ВСЁ зелёное
(parser 460, html 326, parsing-lab 233/233, html-compat 6/6, integration 25).

### Результат — раздел «Аудит рендерера 2026-06-09» в TODO.md (R1–R9), главное:
- **R1 — НОВЫЙ БАГ (верифицирован CLI-пробой vs asciidoctor)**: `.Title` на block-image
  теряется из imageblock И УТЕКАЕТ в следующий блок (paragraph получает чужой
  `<div class="title">`). Asciidoctor: `Figure 1. Title` ПОСЛЕ content + figure-counter
  (счётчика у нас нет вообще). `start_block_image` (lib.rs:1372) — static fn, title не
  потребляет; TagEnd::BlockImage (2395) не сбрасывает.
- R2: BlockVideo title-баг (известный); R3: системный корень — ввести start_block_wrapper
  с TitlePos; R4: дуп `#t=` audio/video (порядок boolean-атрибутов разный НАМЕРЕННО);
  R5: finish() — title_to_id дважды + O(n²) output.replace; R6: li_p_open стеки ×3;
  R7: вынос семантики в adoc-render-core (доказательство — builder.rs УЖЕ дублирует
  intrinsic-таблицу/resolve_attr_refs/trailing_brackets); R8: распил lib.rs 6291 строк
  на модули; R9: Parser.experimental ad-hoc канал.
- Находки Explore-агентов верифицированы чтением кода: ложная — «BlockImage чинить как
  audio» (у image title ПОСЛЕ content с Figure-caption, НЕ before); порядок boolean-атрибутов
  audio≠video — намеренный (оба соответствуют asciidoctor).

### Что дальше
- R1 — лучший кандидат на следующую правку (реальный баг + вероятные флипы корпуса:
  image-цепочка). Перед правкой пересобрать /tmp/adoc_base от master `532c10a`.
- R3+R4+R6 — механический дедуп, низкий риск, корпус не должен сдвинуться (verify blast=0).
- R7 — крупный рефакторинг, делать ПОСЛЕ исчерпания дешёвых флипов Фазы 3 либо перед
  стартом второго рендерера.

### Предостережения (без изменений)
- НЕ cargo fmt. Коммит только по запросу. Корпус: python3 /mnt/c/tmp/adoc-test/compare_full.py
  (release-бинарь, cargo build --release -p adoc-cli). blast: /tmp/blast.py. fdiff: /tmp/fdiff.py.

---

## Сессия (2026-06-09, поздняя-28) — Фаза 3: инъекция MathJax-loader при `:stem:`

`fix/rowspan-row-placement` УЖЕ смержена в master (`a312a0e`, origin == master, дерево чистое;
session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base` пересобран из ТЕКУЩЕГО
чистого master `a312a0e` ПЕРЕД правкой (скопирован свежий release-бинарь). Baseline подтверждён:
Identical 203, Different 141, Errors 0. near-miss: топ — revision-line-with-version-prefix (1-diff,
`{docdate}` — дата-зависим, НЕ флипается). Из трёх «сложных» 6-10-diff кандидатов (pass/index случай A —
рабит-хол; stem/index — «архитектурный»; special-section-numbers — QUOTES в `[label]`) переоценил
**stem/index**: ярлык «архитектурный» преувеличен — фактически ДЕТЕРМИНИРОВАННАЯ строковая вставка.

### Ветка `fix/stem-mathjax-docinfo` (от master `a312a0e`; СТАТУС: НЕ закоммичено)
- **Правило** (верифицировано пробами через ФАЙЛ): при установленном атрибуте `stem` (ЛЮБОЕ значение,
  даже без stem-контента — P1) asciidoctor вставляет ФИКСИРОВАННЫЙ блок перед `</body>` (после футера):
  `<script type="text/x-mathjax-config">` c `MathJax.Hub.Config({...})` + CDN-loader
  `https://cdnjs.cloudflare.com/ajax/libs/mathjax/2.7.9/MathJax.js?config=TeX-MML-AM_HTMLorMML`. Блок
  ИДЕНТИЧЕН для asciimath и latexmath (P1==P2). Без `:stem:` — НЕТ блока, даже если в тексте есть inline
  `stem:[x]` (P3 → литерал `\$x\$`). `:!stem:` удаляет ключ → нет вставки.
- **Корень (слой!)**: чистая standalone-обёртка РЕНДЕРЕРА. `get_body_content` (compare_full) берёт всё от
  `<body>` до `</body>` включительно → MathJax входит в сравнение; текст `<script>` (JS-конфиг) сравнивается
  как отдельный токен (`.strip()` внешних пробелов, внутренние `\n` сохранены) → нужно совпадение JS
  байт-в-байт. `document_attrs` хранит `stem` (через `apply_attribute` стр.376, вызывается из
  `Event::Attribute` стр.588 ДО `write_document_tail`).
- **Фикс (1 точка, `adoc-html/lib.rs`)**: const `MATHJAX_DOCINFO` (raw-строка `r#"..."#` — в JS два
  ЛИТЕРАЛЬНЫХ `\` перед `(`/`[`/`$`, подтверждено `od -c`: байты `\ \ (`); 4-строчная вставка в
  `write_document_tail` (стр.~2960) под `self.document_attrs.contains_key("stem")` — после `docinfo_footer`,
  перед `</body>`. Вызывается только из standalone-ветки `run()`. +1 тест `test_stem_mathjax_docinfo`
  (asciimath инъектит config+loader, mathjax<</body>; latexmath тоже инъектит; без stem — нет).
- **Остаток (НЕ в корпусе)**: `eqnums` атрибут изменил бы `TeX.equationNumbers.autoNumber` (хардкод
  `"none"` = дефолт asciidoctor). MathJax-версия `2.7.9` хардкод (совпадает с установленным asciidoctor).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (html 325→326, parser 460,
  parsing-lab **233/233**, html-compat 6/6, integration 25 — правка только в standalone-tail, события
  парсера не задеты).
- Корпус `compare_full.py` (release): **Identical 203→204 (+1), Different 140, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `a312a0e`): **2 файла** изменили
  вывод — **1 FLIP→IDENTICAL** (stem/index, verified 0 diffs len 720==720), **0 регрессий**.
  1 changed-still-different: stem/examples/stem.adoc 99→104 (MathJax байт-в-байт верен — exp==got;
  Different по пре-существующему каскаду level-0 `<h1>My...Opus</h1>` vs `<div class="sect0">` diff #2–#4;
  +5 = ровно добавленные корректные MathJax-токены под доминирующим sect0-сдвигом).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/stem-mathjax-docinfo` (только по запросу).
  master == origin сейчас, после мержа потребуется пуш (по запросу).
- Near-miss-кандидаты на 204: **pass/index** (6-diff — случай A, single-plus pass-extraction-ordering,
  риск/рабит-хол ~9 сессий), **special-section-numbers** (10-diff, monospace в ТЕКСТЕ xref — архитектурный
  QUOTES в `[label]`, полный inline-проход текста ссылки), **callout** (20-diff, verbatim callout —
  неразведан), **part** (22-diff, len_delta=0 — sections part, неразведан, структура совпадает).
- **Высокоценный архитектурный кластер**: наследование `m`/`e`/`s` стиля колонки таблицы → `<code>`/`<em>`/
  `<strong>` в ячейках (сделано только `h`); author-header `<div class="details">` (standalone); level-0
  sect0 heading `<h1 class="sect0">` vs `<div class="sect0"><h1>` (доминирует в stem.adoc и др.).

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически (пробы asciidoctor через
  ФАЙЛ; `od -c` для точных байтов при копировании литералов). Дамп событий парсера: throwaway
  `adoc-parser/examples/dump_events.rs`. **`target/debug/adoc` НЕ пересобирается от `cargo test`** — для
  CLI/корпуса `cargo build --release -p adoc-cli`.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `a312a0e`). near-miss `/tmp/nearmiss.py` (МЕДЛЕННО). Точечный
  diff: `/tmp/fdiff.py <relpath> [base-бинарь]`. `get_body_content` = всё от `<body>` до `</body>`;
  сравнение семантическое (DOM, `convert_charrefs=True`; `style` игнорится; атрибуты сортируются;
  текст `.strip()`). LSP, context7 MCP.

---

## Сессия (2026-06-09, поздняя-27) — Фаза 3: rowspan-размещение ячеек в спанированных строках

`fix/callout-item-block-and-shifted-source-lang` УЖЕ смержена+запушена в master (`7fe4190`, origin ==
master, дерево чистое; session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base`
пересобран из ТЕКУЩЕГО чистого master `7fe4190` ПЕРЕД правкой. Baseline подтверждён: Identical 202,
Different 142, Errors 0. Взят **docinfo/index** (14-diff, неразведанный, span-cell row-placement) —
точечным `fdiff.py`. Корень оказался чистым (не архитектура) — узкий баг в `build_table_rows`.

### Ветка `fix/rowspan-row-placement` (от master `7fe4190`; СТАТУС: НЕ закоммичено)
- **Корень — двойной декремент occupancy в `build_table_rows`** (`adoc-parser/src/block.rs`, ПАРСЕР).
  Ячейка с rowspan `.N+` занимает свою колонку в N строках → следующая строка держит на 1 ячейку меньше.
  Проба (2-кол. таблица, `.2+|X`/`|1` / `|2` / `|Y|Z`): asciidoctor даёт строки `[X,1]`,`[2]`,`[Y,Z]`
  (`2` в КОЛОНКЕ 1, т.к. X спанит кол.0); мы давали `[X,1]`,`[2,Y]`,`[Z]` (`2` ошибочно в кол.0).
  **Механизм бага**: при старте новой строки (`col >= num_cols`) код СНАЧАЛА «decrement all»
  (`for r in &mut col_remaining { if *r>0 {*r-=1} }`) уменьшал `col_remaining[0]` 1→0, ПОТОМ skip-цикл
  проверял `>0` (уже 0 → не пропускал) → ячейка `2` падала в спанированную кол.0. Двойной счёт: и
  «decrement all», и skip-цикл декрементят. **Фикс (1 точка)**: убран «decrement all» цикл — skip-циклы
  (top-of-loop для mid-row occupied + row-start для leading occupied) САМИ декрементят каждую
  occupied-колонку ровно раз за строку (трассировано: каждая occupied-кол. либо leading-skip на старте
  строки, либо walked-past mid-row через top-of-loop — оба декрементят 1 раз).
- **Тесты**: +1 html `test_table_rowspan_shifts_following_row_cells_html` (флип-кейс + regression:
  continuation-ячейка `2` закрывает свою `<tr>`, `Y` начинает новую, 4 `<tr>` всего). Сущест.
  `test_table_rowspan_html` (`.2+|A|B`/`|C` → 2 строки) и `test_table_colspan_rowspan_html`
  (`2.3+|cell` colspan2 rowspan3) целы — трассированы вручную перед правкой.
- **Остаток (НЕ нужен для флипа, латентен)**: пасс `emit_row_cells` (макрос, ~стр.1389) считает col_idx
  суммой colspan БЕЗ учёта rowspan-сдвига из предыдущих строк → выравнивание/стиль ячейки в спанированной
  строке берётся от неверной колонки. В docinfo все колонки `[cols="<10,<20,<30,<30"]` left → нюанс не
  виден. Если флипнуть файл с rowspan + разным halign по колонкам — чинить ЭТОТ пасс (нужна grid-aware
  col_idx, зеркало `build_table_rows`).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 460, html 324→325,
  parsing-lab **233/233** verified `--nocapture` — rowspan-таблиц с continuation-сдвигом в фикстурах нет;
  правка в grid-логике парсера, ASG читает события напрямую). html-compat 6/6.
- Корпус `compare_full.py` (release): **Identical 202→203 (+1), Different 141, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `7fe4190`): **4 файла** изменили
  вывод — **1 FLIP→IDENTICAL** (docinfo/index, verified 0 diffs len 982==982), **0 регрессий**.
  3 changed-still-different: table-ref 887→**871** (−16, улучшение — rowspan-строки теперь верны),
  cell 960→960 и toc-ref 205→205 (нейтрально по числу — вывод сдвинулся, но файлы во власти
  доминирующего несвязанного каскада: `2*` дублирование по колонкам [diff #21] + наследование `m`/`e`
  стиля колонки + halign; rowspan-часть в них теперь корректна, погребена под каскадом).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/rowspan-row-placement` (только по запросу).
  master == origin сейчас, после мержа потребуется пуш (по запросу).
- Чистые near-miss-кандидаты на 203: **pass/index** (6-diff — случай A, single-plus pass-extraction-
  ordering, риск/рабит-хол), **stem/index** (6-diff, MathJax standalone — архитектурный),
  **special-section-numbers** (10-diff, monospace в ТЕКСТЕ xref — архитектурный QUOTES в `[label]`).
- **Высокоценный архитектурный кластер** (много flip'ов, риск): наследование `m`/`e`/`s` стиля колонки
  таблицы → ячейки `<code>`/`<em>`/`<strong>` (сделано только `h`); `2*`/`3*` дублирование контента по
  колонкам (cell.adoc diff #21 — НЕ поддержано, оператор повтора в спецификаторе ячейки);
  author-header `<div class="details">` (standalone).

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически (пробы asciidoctor через
  ФАЙЛ; `asciidoctor -e -o -` для embedded). Дамп событий парсера: throwaway
  `adoc-parser/examples/dump_events.rs`. **`target/debug/adoc` НЕ пересобирается от `cargo test`** — для
  CLI-проб `cargo build -p adoc-cli` (debug) / `--release` (для корпуса).
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `7fe4190`). near-miss `/tmp/nearmiss.py` (МЕДЛЕННО). Точечный
  diff: `/tmp/fdiff.py <relpath> [base-бинарь]` (2-й арг — бинарь для сравнения до/после). Сравнение
  семантическое (DOM, `convert_charrefs=True`; `style` игнорится; атрибуты сортируются). LSP, context7 MCP.

---

## Сессия (2026-06-09, поздняя-26) — Фаза 3: callout-элемент с continuation-блоком + сдвиг source-языка

`fix/audio-start-opts-and-title` УЖЕ смержена+запушена в master (`54cf378`, origin == master, дерево
чистое; session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base` пересобран из
ТЕКУЩЕГО чистого master `54cf378` ПЕРЕД правкой. Baseline подтверждён: Identical 200, Different 144,
Errors 0. Взят **db-migration** (16-diff, неразведанный, 2 корня) — точечным `fdiff.py` (не гонял
nearmiss целиком — медленно). Оба корня оказались чистыми (не архитектура).

### Ветка `fix/callout-item-block-and-shifted-source-lang` (от master `54cf378`; СТАТУС: НЕ закоммичено)
- **Корень 1 — NOTE/continuation-блок внутри callout-элемента** (`adoc-html/lib.rs`, РЕНДЕРЕР; 15 из 16
  diff'ов db-migration, #516–530 — один сдвиг). `+`-continuation-блок (`NOTE:` стр.215–216), присоединённый
  к callout-элементу `<2>`, не закрывал принципиальный `<p>` → `<li><p>text<div admonition>…</div></p></li>`
  (блок ВНУТРИ незакрытого `<p>`, `</p>` уехал в конец после `</div>`). asciidoctor: `<li><p>text</p><div
  admonition>…</div></li>`. **Корень (слой!)**: парсер эмитит верно (дамп событий: `CalloutListItem`→`Text`
  →`Admonition`); баг в рендерере: `CalloutListItem` (стр.1146) пушил `<li><p>` БЕЗ стека `li_p_open`/
  `li_para_count` (в отличие от `ListItem`), И `Tag::Admonition` отсутствовал в guard-списке закрытия `<p>`
  (стр.1006–1013). Фикс: (a) `CalloutListItem` зеркалит `ListItem` — push `li_p_open=true`+`li_para_count=1`;
  (b) `TagEnd::CalloutListItem` (стр.2308) условный — `</p></li>` если p открыт, иначе `</li>`; (c)
  `Tag::Admonition { .. }` добавлен в guard. Заодно чинит continuation-ПАРАГРАФ в callout (теперь
  оборачивается в `<div class="paragraph">` через is_continuation_para). Простой callout-элемент не
  затронут (`<li><p>x</p></li>`).
- **Корень 2 — сдвиг позиционных слотов ведущим named/shorthand** (`adoc-parser/attributes.rs`, ПАРСЕР;
  diff #38 db-migration). `[id=app, source, yaml]` (стр.27): asciidoctor даёт `language-source`, мы
  `language-yaml`. **Правило (верифицировано пробами через ФАЙЛ)**: AsciiDoc инкрементит позиционный индекс
  для КАЖДОГО атрибута (named `id=`/`role=`, shorthand `#id`/`.role`/`%opt` — каждая shorthand-ГРУППА = 1
  слот); стиль=слот1, язык=слот2. `[id=app, source, yaml]`→id слот1, source слот2(язык), yaml слот3(игнор),
  стиль пуст→source-блок lang=`source`. Подтверждено: `[role=x,…]`/`[#id,…]`/`[.r,…]`/`[%o,…]` так же;
  `[#id.role,…]`/`[.r1.r2,…]` (одна группа) → source/source; ДВА ведущих named `[id, role, source, yaml]`
  → НЕ source (слот2 занят role); `[src, yaml]` (src≠source, слот1) → НЕ source; `[foo, source, yaml]` →
  стиль foo, НЕ source. Наш `positional` Vec СХЛОПЫВАЛ named/shorthand → `[id=app, source, yaml]` выглядел
  как explicit `[source, yaml]` (positional=["source","yaml"]), и `source_language()` брал explicit-путь
  (positional[0]=="source" → язык=positional[1]="yaml"). Фикс: поле `first_positional_is_style` (=первый
  comma-часть bare-позиционал); убран ложно-срабатывающий guard `positional.first() != Some("source")`
  в `implied_source_lang` (он редундантен с `!first_positional_is_style` и блокировал верный кейс);
  `source_language()`/`is_source_block()` стали слот-осознанными (explicit-путь только при
  `first_positional_is_style && positional[0]=="source"`). `[id=app, foo, yaml]` УЖЕ работал (через
  implied); ломался ТОЛЬКО когда слот-2 буквально `source`. block_style_kind/admonition_kind/is_verse_style
  НЕ тронуты (source-блок возвращается раньше, leading-named edge-кейсы для них не в корпусе).
- **Тесты**: +1 parser `test_leading_named_attr_shifts_positionals` (id/`#`-сдвиг, два-named→не-source,
  explicit неизменен, `src`≠source); +2 html `test_callout_item_with_continuation_note_html` (флип + простой
  callout regression guard) и `test_source_lang_shifted_by_leading_named_attr_html`.

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 459→460, html 322→324,
  parsing-lab **233/233** verified `--nocapture` — callout+continuation и `[id=…,source,…]` в фикстурах нет;
  правка в рендерере + слот-логике). html-compat 6/6.
- Корпус `compare_full.py` (release): **Identical 200→202 (+2), Different 142, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `54cf378`): **4 файла** изменили
  вывод — **2 FLIP→IDENTICAL** (db-migration verified 0 diffs 577==577 [оба корня]; localization verified
  0 diffs 263==263 [корень 1 — callout `<1>`+`+`-continuation стр.85–86; все его source-блоки явные
  `[source,lang]`]), **0 регрессий**. 2 changed-still-different улучшены: java/index 2290→2265,
  software-development-cookbook 2595→2463 (гигантские Antora-include-агрегаты флипнутых файлов, Different
  по доминирующему несвязанному каскаду).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/callout-item-block-and-shifted-source-lang` (только по
  запросу). master == origin сейчас, после мержа потребуется пуш (по запросу).
- Чистые near-miss-кандидаты на 202: **pass/index** (6-diff — ТОЛЬКО случай A, single-plus
  pass-extraction-ordering, риск/рабит-хол), **stem/index** (6-diff, MathJax standalone — архитектурный),
  **special-section-numbers** (10-diff, monospace в ТЕКСТЕ xref — архитектурный QUOTES в `[label]`).
  Неразведанные: **docinfo/index** (14-diff, span-cell row-placement — структурный таблицы).
- **Известные родственные edge-кейсы (НЕ трогал, не в корпусе)**: (a) `block_style_kind`/`admonition_kind`/
  `is_verse_style` НЕ слот-осознаны — `[id=x, verse]`/`[id=x, NOTE]` дали бы неверный стиль (стиль на
  слоте2), но таких в корпусе нет; (b) `LiteralParagraph`/`BlockVideo`/`BlockAudio` отсутствуют в
  guard-списке закрытия `<p>` в list-item (как continuation-блоки) — тривиальное зеркало Admonition, если
  понадобится; (c) arm `Tag::BlockVideo` имеет title-баг (не зовёт `emit_pending_block_title`), video далеко
  от флипа.
- Высокоценный архитектурный кластер (много flip'ов, риск): наследование `m`/`e`/`s` стиля колонки таблицы
  → ячейки `<code>`/`<em>`/`<strong>` (сделано только `h`); author-header `<div class="details">`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически (пробы asciidoctor через
  ФАЙЛ; heredoc `<<'EOF'` ок). Дамп событий парсера: throwaway `adoc-parser/examples/dump_events.rs`
  (создать Write → `cargo run -p adoc-parser --example dump_events` → удалить; различает баг парсера vs
  рендерера — в этой сессии подтвердил, что callout-NOTE парсится верно). **`target/debug/adoc` НЕ
  пересобирается от `cargo test -p adoc-html`** — для CLI-проб пересобрать `cargo build -p adoc-cli` (в этой
  сессии словил stale debug-бинарь).
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `54cf378`). near-miss `/tmp/nearmiss.py` (МЕДЛЕННО — гонит
  asciidoctor по 344 файлам; для точечного — `/tmp/fdiff.py <relpath> [base-бинарь]`). Сравнение
  семантическое (DOM, `convert_charrefs=True`; `style` игнорится; атрибуты сортируются). LSP, context7 MCP.

---

## Сессия (2026-06-09, поздняя-25) — Фаза 3: audio `start`/`end` + `opts=` alias + `.Title`

`fix/intrinsic-quot-apos-and-pass-constrained` УЖЕ смержена+запушена в master (`a691601`, origin ==
master, дерево чистое; session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base`
пересобран из ТЕКУЩЕГО чистого master `a691601` ПЕРЕД правкой. Baseline подтверждён: Identical 199,
Different 145, Errors 0. near-miss: топ — revision-line-with-version-prefix (1-diff, `{docdate}` —
дата-зависим, НЕ флипается). Разведаны 6-12-diff кандидаты: pass/index (случай A, риск), stem/index
(MathJax архитектурный), special-section-numbers (QUOTES в `[label]` архитектурный) — все отложены.
Взят **audio** (12-diff) — два КОНКРЕТНЫХ корня (не архитектура).

### Ветка `fix/audio-start-opts-and-title` (от master `a691601`; СТАТУС: НЕ закоммичено)
- **Три бага в audio-макросе** (audio.adoc, 3 блока: базовый уже был Identical; флип дают блоки 2+3):
  - **Корень 1 — `opts=` НЕ парсился** (`adoc-html/lib.rs::parse_media_attrs`): match-arm ловил только
    ключ `"options"`, а `audio::x[opts=autoplay]` использует shorthand `opts` → `autoplay`/`loop`/
    `nocontrols` терялись. Проба asciidoctor через файл: `opts=autoplay` ≡ `options=autoplay`. Фикс:
    `"options"` → `"opts" | "options"`. **Затрагивает и video** (общий парсер) — video.adoc `opts=autoplay`
    теперь тоже парсится (48→47 diff, улучшение, не регрессия).
  - **Корень 1b — `start`/`end` НЕ применялись к audio src** (`render_audio_tag`): писал `src=target`
    голым через `write_attr`, в отличие от `render_video_tag` (который строит `#t=start,end` фрагмент).
    Проба: `start=60`→`src="...#t=60"`, `start=10,end=20`→`#t=10,20`. Фикс: переписан src-билд audio
    дословно зеркалит video (html_escape target + match (start,end) → `#t=` фрагмент). Заодно порядок
    boolean-атрибутов подогнан под asciidoctor: **autoplay, loop, controls** (был controls,autoplay,loop;
    нормализатор корпуса сортирует, но raw-вывод теперь байт-в-байт — `controls` on по умолчанию кроме
    `nocontrols`).
  - **Корень 2 — `.Title` терялся** (arm `Tag::BlockAudio`, ~стр.1175): пушил
    `<div class="audioblock">\n<div class="content">\n` одной строкой, НЕ зовя `emit_pending_block_title`.
    Парсер эмитит BlockTitle ВЕРНО (через `push_title_then_events`, block.rs:643) — баг чисто в рендерере
    (тот же класс, что toc/literal-paragraph фикс). Фикс: разбит push, вставлен
    `self.emit_pending_block_title(output)` между wrapper-div и content-div (audio/video кладут title
    ДО content, в отличие от image — `<div class="title">` ПОСЛЕ content с «Figure N.»). Title-less
    не затронут (`emit_*` no-op при None).
- **Тесты**: обновлён `test_audio_options_html` (новый порядок autoplay/loop/controls), +1 тест
  `test_audio_start_opts_and_title` (start-фрагмент + opts-alias + title в одном). `test_audio_basic_html`/
  `test_audio_nocontrols_html` целы (только controls / без controls).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (html 321→322, parser 459,
  parsing-lab **233/233** verified `--nocapture` — audio-макросов с start/opts/title в фикстурах нет;
  правка в рендерере + media-парсере). html-compat 6/6.
- Корпус `compare_full.py` (release): **Identical 199→200 (+1), Different 144, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `a691601`): **2 файла** изменили
  вывод — **1 FLIP→IDENTICAL** (audio.adoc, verified 0 diffs len 30==30), **0 регрессий**.
  1 changed-still-different: video.adoc 48→47 (улучшение — `opts=autoplay` теперь даёт `<video autoplay
  controls src="...#t=60" width="640">`, точно как asciidoctor; остаток 47 diff — youtube/vimeo iframe
  и пр., вне рамок).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/audio-start-opts-and-title` (только по запросу).
  master == origin сейчас, после мержа потребуется пуш (по запросу).
- Чистые near-miss-кандидаты на 200: **pass/index** (6-diff — ТОЛЬКО случай A: `` `+pass:[]+` ``→пустой
  `<code>` через single-plus, асимметричный pass-extraction-ordering, риск/рабит-хол ~8 сессий),
  **stem/index** (6-diff, MathJax `<script>`-инъекция — архитектурный standalone), **special-section-numbers**
  (10-diff, monospace в ТЕКСТЕ xref-ссылки — архитектурный QUOTES в `[label]`). Неразведанные:
  **docinfo/index** (14-diff — rowspan-ячейка размещается в КОНЦЕ предыдущей строки вместо НАЧАЛА новой:
  span-cell row-placement, структурный таблицы), **db-migration** (16-diff, 2 корня: `language-source` vs
  `language-yaml` на `[source]` + пропущенный `</p>` перед NOTE-админишеном).
- **Известный родственный баг** (НЕ трогал, video далеко от флипа): arm `Tag::BlockVideo` ИМЕЕТ тот же
  title-баг (не зовёт `emit_pending_block_title`) — но video.adoc Different по 47 причинам, флипа не даст;
  фикс тривиален (зеркало audio), если понадобится.
- Высокоценный архитектурный кластер (много flip'ов, риск): наследование `m`/`e`/`s` стиля колонки
  таблицы → ячейки `<code>`/`<em>`/`<strong>` (сделано только `h`); author-header `<div class="details">`.
- Архитектурные/отложенные (без изменений): `{docdate}`/`{localdate}` (дата-зависим), counters в verbatim,
  case A (single-plus pass-extraction-ordering), nested-форматирование в ТЕКСТЕ ссылки (QUOTES в `[label]`),
  inline-monospace passthrough char-ref, inline-anchor reftext из dt-терма, link-role `class="external"`,
  trailing ` +` в reparsed monospace → `<br>`, level-0 sect0 heading, doctype=book.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически (пробы asciidoctor через
  ФАЙЛ — `-e` embedded или standalone; heredoc `<<'EOF'` ок). Дамп событий парсера: throwaway
  `adoc-parser/examples/dump_events.rs`.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `a691601`). near-miss `/tmp/nearmiss.py`. Точечный diff:
  `/tmp/fdiff.py <relpath> [binary]` (2-й арг — base-бинарь для сравнения до/после). Сравнение
  семантическое (DOM, `convert_charrefs=True`; `style` игнорится; атрибуты сортируются — порядок
  невидим). LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-24) — Фаза 3: intrinsic `{quot}`/`{apos}`/`{pp}` + `pass:[…]` в monospace (случай G)

`fix/gate-experimental-ui-macros` УЖЕ смержена+запушена в master (`968e913`, origin == master, дерево
чистое; session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base` пересобран из
ТЕКУЩЕГО чистого master `968e913` ПЕРЕД правкой. Baseline подтверждён: Identical 198, Different 146,
Errors 0. near-miss: топ — revision-line-with-version-prefix (1-diff, `{docdate}` — дата-зависим, НЕ
флипается). Следующий чистый кандидат — **quotation-marks-and-apostrophes** (4-diff, len_delta=0,
неразведан): разведка пробами через ФАЙЛ дала ДВА корня в одном файле.

### Ветка `fix/intrinsic-quot-apos-and-pass-constrained` (от master `968e913`; СТАТУС: НЕ закоммичено)
- **Два корня, оба нужны для флипа quotation-marks-and-apostrophes.adoc** (ровно 4 diff'а: #79/#92
  intrinsic, #228/#230 case-G).
- **Корень 1 — intrinsic char-replacement атрибуты** (`adoc-html/lib.rs::INTRINSIC_ATTRIBUTES`):
  таблица не содержала `quot`/`apos`/`pp`. Asciidoctor резолвит (верифицировано пробой): `{quot}`→
  `&#34;`, `{apos}`→`&#39;`, `{pp}`→`&#43;&#43;` (= `++`), `{cpp}`→`C&#43;&#43;` (у нас `cpp`→`C++`
  уже был, семантически совпадает после норм.). Резолв и в plain, и ВНУТРИ `` `…` `` monospace
  (`` `{quot}` ``→`<code>&#34;</code>`). Добавлены 3 записи (алфавит: apos после amp; pp/quot между
  plus и rdquo). Резолв-порядок в рендерере: document_attrs → intrinsic → env → fallback (intrinsic
  пушится как сырой HTML).
- **Корень 2 — `pass:[…]` в constrained-marker matching, случай G** (`inline.rs::find_closing_constrained`):
  `pass:[…]` извлекается ДО quote-подстановки → quote-маркер внутри его скобок НЕ должен закрывать
  внешний span. `` `pass:[`']` `` → Asciidoctor `<code>`'</code>`; мы ломались на внутреннем backtick
  (`find_closing_constrained` брал его как закрывающий) → `<code>pass:[</code>']` `. Добавлен хелпер
  `pass_macro_span_len(s,i)` (strip `pass:[`, контент до первого `]`, вернуть длину) — ТОЧНЫЙ аналог
  уже сделанного `passthrough_span_len` (skip `++…++`); в цикле `find_closing_constrained` новый branch
  `b'p'` пропускает регион `pass:[…]`. Inner-reparse монospace уже корректно эмитит pass-макрос
  (`try_pass_macro`→`InlinePassthrough`). Применяется ко ВСЕМ constrained-маркерам (`*`/`_`/`` ` ``/`#`).
- **Случай A НЕ сделан** (отложен, риск): `` `+pass:[]+` `` через single-plus (pass/index стр.15).
  Асимметрия Asciidoctor (пробы): `+pass:[x]+`→`x` (pass обработан ДО `+…+`), но `++pass:[y]++`→
  `pass:[y]` (НЕ обработан внутри `++…++`). Дискриминатор `` `+pass:[]+more+` ``→`<code>+more</code>`
  (pass→empty placeholder, потом `` `+…+` `` берёт внешние `+`) ломает наивный «pure-pass-macro shortcut»
  в `try_single_plus_passthrough`. Нужна faithful pass-extraction-ordering (pass приоритетнее single-plus,
  но не double-plus) — рабит-хол, отложен ~8 сессий.

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 457→459, html
  320→321, parsing-lab **233/233** verified `--nocapture` — pass-в-quote/quot/apos в фикстурах нет;
  правка в close-finder + intrinsic-таблице, ASG читает события парсера напрямую). html-compat 6/6.
- Корпус `compare_full.py` (release): **Identical 198→199 (+1), Different 145, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `968e913`): **5 файлов** изменили
  вывод — **1 FLIP→IDENTICAL** (quotation-marks-and-apostrophes, verified 0 diffs len 381==381),
  **0 регрессий**. 4 changed-still-different: pass-macro 250→249 (`{pp}` стр.115 резолвится),
  literal-monospace 61→59, troubleshoot-unconstrained 216→212 (pass-в-monospace лучше),
  character-replacement-ref 645→645 (нейтрально — `{quot}`/`{apos}`/`{pp}` теперь верны, но погребены в
  доминирующем несвязанном каскаде len 756 vs 581: table-column-style `m`/`e` + footnote `<sup>`).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/intrinsic-quot-apos-and-pass-constrained` (только по
  запросу). master == origin сейчас, после мержа потребуется пуш (по запросу).
- Чистые near-miss-кандидаты на 199: **pass/index** (6-diff — ТОЛЬКО случай A остался, см. выше; риск),
  **stem/index** (6-diff, MathJax `<script>`-инъекция — архитектурный standalone), **special-section-numbers**
  (10-diff, monospace в ТЕКСТЕ xref-ссылки — архитектурный QUOTES в `[label]`), **audio** (12-diff,
  2 корня: `audio::x[start=,opts=autoplay]` атрибуты + `.title`). Неразведанные: docinfo/index (14,
  len_delta=0), db-migration (16, table `<table>` vs `<tr>`).
- **Высокоценный архитектурный кластер** (много flip'ов, но риск): наследование `m`/`e`/`s` стиля
  колонки таблицы → ячейки `<code>`/`<em>`/`<strong>` (сделано только `h` в `block.rs::scan_table::resolve_style`
  + рендерер). Завязаны character-replacement-ref, pass-macro, subs-group-table, format-column-content,
  image-position и др. — НО у них co-occurring корни (footnote `<sup>[1]`, `stretch`-класс таблицы,
  `col`/`colgroup`) → чистого флипа может не быть. `a` (AsciiDoc-стиль) требует nested-парсинга ячейки —
  НЕ трогать.
- Архитектурные/отложенные (без изменений): `{docdate}`/`{localdate}` (дата-зависим), counters в verbatim,
  case A (single-plus pass-extraction-ordering), nested-форматирование в ТЕКСТЕ ссылки (QUOTES в `[label]`),
  inline-monospace passthrough char-ref, inline-anchor reftext из dt-терма (lexicon), link-role
  `class="external"`, trailing ` +` в reparsed monospace → `<br>`, level-0 sect0 heading, doctype=book,
  author-header `<div class="details">`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически (пробы asciidoctor через
  ФАЙЛ — shell экранирует backtick'и/`+`/`\`; heredoc `<<'EOF'` ок). Дамп событий парсера: throwaway
  `adoc-parser/examples/dump_events.rs` (создать → `cargo run -p adoc-parser --example dump_events` →
  удалить; различает баг парсера vs рендерера).
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `968e913`). near-miss `/tmp/nearmiss.py`. Точечный diff:
  `/tmp/fdiff.py <relpath> [binary]` (2-й арг — base-бинарь для сравнения до/после). Сравнение
  семантическое (DOM, `convert_charrefs=True` → `&#34;`≡`"`; `style` игнорится). LSP для навигации,
  context7 MCP.

---

## Сессия (2026-06-09, поздняя-23) — Фаза 3: experimental UI-макросы (`kbd:`/`btn:`/`menu:`) за `:experimental:`

`fix/revision-prefix-and-hardbreaks` УЖЕ смержена+запушена в master (`bddedb5`, origin == master,
дерево чистое; session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base` пересобран
из ТЕКУЩЕГО чистого master `bddedb5` ПЕРЕД правкой. Baseline подтверждён: Identical 194, Different 150,
Errors 0. near-miss: топ — revision-line-with-version-prefix (1-diff, `{docdate}` — дата-зависим, НЕ
флипается). Остальные одиночные near-miss архитектурны/fiddly (pass/index nested-passthrough, stem/index
MathJax, audio 2 корня, db-migration 2 корня). Сменил методику: **агрегировал ПЕРВЫЙ diff по каждому
Different-файлу** (корень до позиционного каскада) → нашёл крупный чистый корень: text-truncation в
subs/attributes-страницах = **experimental-макросы парсятся без `:experimental:`**.

### Ветка `fix/gate-experimental-ui-macros` (от master `bddedb5`; СТАТУС: НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано пробами через ФАЙЛ): `kbd:`/`btn:`/`menu:` — experimental-
  макросы, распознаются ТОЛЬКО при `:experimental:`. Без него — оставляются ЛИТЕРАЛОМ (`press kbd:[Enter]`
  → `press kbd:[Enter]`). Мы парсили безусловно (вывод с/без `:experimental:` идентичен). Из 10 файлов
  корпуса с этими макросами НИ ОДИН не имеет `:experimental:` (standalone-asciidoctor → все литералом;
  keyboard-macro уже Identical — его kbd внутри `` `+...+` `` passthrough, asciidoctor даёт 0 `<kbd>`).
- **Корень (слой!)**: распознавание макроса идёт в ПАРСЕРЕ (`inline.rs::handle_inline_macro`), который
  НЕ знал document-атрибутов. Рендерер знает `document_attrs`, но макрос уже распознан раньше.
- **Фикс** (3 файла): (1) `inline.rs` — поле `InlineState.experimental`; новый pub
  `parse_str_with_subs_experimental(text,subs,experimental)` (`parse_str_with_subs` = обёртка `…,false`);
  5 внутренних reparse `InlineState::new` наследуют `self.experimental`; 3 arm'а kbd/btn/menu гейтятся:
  при ВКЛ — try_*_macro как раньше, при ВЫКЛ — хелпер `skip_disabled_ui_macro(prefix_len)` поглощает
  весь токен `name:target[…]` как литерал. **КРИТИЧНО**: наивный гейтинг (`&& self.experimental` в guard)
  ввёл бы баг — после `self.pos+=1` остаток (`bd:[…]`/`enu:File[…]`/lowercase `file[x]`) мисспарсится
  catch-all'ом `try_custom_inline_macro` в `CustomInlineMacro`. Хелпер скипает через первую пару `[...]`.
  (2) `parser.rs` — поле `Parser.experimental`, наблюдается из `Event::Attribute{name}` в `match &event`
  (`experimental`→true, `!experimental`/`experimental!`→false; mid-document семантика сохранена — флаг
  меняется по ходу), протянуто в обе точки inline-парсинга (multiline 137 + single 151). (3)
  `adoc-html/lib.rs::render_inline_value` — передаёт `document_attrs.contains_key("experimental")`.
- **Тесты**: обновлено 12 (7 inline kbd/btn/menu → хелпер `parse_experimental`; 5 html → `:experimental:`-
  префикс, expected БЕЗ изменений — attribute-entry в embedded невидим), +2 guard (parser
  `test_experimental_macros_literal_without_experimental` incl. lowercase-target catch-all; html
  literal+not-custom; обновлён `test_kbd_not_captured_as_custom` под обе ветки). html-compat
  `kbd-btn-menu.adoc` (УЖЕ содержит `:experimental:`!) валидирует рендеринг end-to-end.

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 458, html 320,
  parsing-lab **233/233** `--nocapture` — kbd/btn/menu в фикстурах нет; html-compat **70/70** incl.
  `PASS: inline/kbd-btn-menu.adoc`).
- Корпус `compare_full.py` (release): **Identical 194→198 (+4), Different 146, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `bddedb5`): **8 файлов** изменили
  вывод — **4 FLIP→IDENTICAL** (unset-attributes, build-basic-block, paragraphs, ui), **0 регрессий**.
  4 changed-still-different (attribute-entries, boolean-attributes, build-a-basic-table,
  quotation-marks-and-apostrophes) — их kbd/btn/menu теперь литералом верно, Different по ДР. причинам
  (author-header `<div class="details">`, контент таблиц/абзацев).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/gate-experimental-ui-macros` (только по запросу).
  master == origin сейчас, после мержа потребуется пуш (по запросу).
- **Методика выбора кластера обновлена**: при исчерпании одиночных near-miss — агрегировать ПЕРВЫЙ diff
  по Different-файлам (категория корня до каскада) inline-скриптом, переиспользующим `compare_full`
  (`categorize_diff`/`normalize_html`/`get_body_content`). Топ-категории на 198: `attr_diff on <div>`
  (≈29, в осн. author-header `<div class="details">` — pre-existing standalone, + реальный баг
  `[quote, Имя]` утечка attribution в class у assign-id), `text_content_diff` (≈26), `tag_mismatch
  (div vs p)` (14), `tag_mismatch (h1 vs div)` (14 — level-0 sect0 heading `<h1 class="sect0">` vs наш
  `<div class="sect0"><h1>`, завязан на header/doctype=book), `col vs colgroup` (7 — структура `<colgroup>`).
- Чистые near-miss на 198 (дата-зависимый revision-line-with-version-prefix `{docdate}` НЕ в счёт):
  pass/index (6-diff, nested-passthrough), stem/index (6-diff, MathJax — архитектурный standalone),
  special-section-numbers (10-diff, monospace в xref-тексте). Кандидат на разведку: **col/colgroup**
  (7 файлов один корень — таблицы) или **assign-id** `[quote, attribution]` (реальный баг утечки в class).
- Архитектурные/отложенные (без изменений): `{docdate}`/`{localdate}` (дата-зависим), counters в verbatim,
  наследование `m`/`e`/`s` стиля колонки, nested-форматирование в ТЕКСТЕ ссылки (QUOTES в `[label]`),
  inline-monospace passthrough char-ref (`Event::Code`), inline-anchor reftext из dt-терма (lexicon),
  link-role `class="external"`, trailing ` +` в reparsed monospace → `<br>`, level-0 sect0 heading,
  doctype=book (`<body class="book">`, part/preface), author-header `<div class="details">`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически (пробы asciidoctor через
  ФАЙЛ). Дамп событий парсера: throwaway `adoc-parser/examples/dump_events.rs` (создать → `cargo run
  -p adoc-parser --example dump_events` → удалить; БЫСТРО различает баг парсера vs рендерера) — в этой
  сессии так подтвердил, что `:experimental:` эмитится как `Event::Attribute` до body.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `bddedb5`). near-miss `/tmp/nearmiss.py`. Точечный diff:
  `/tmp/fdiff.py <relpath>`. Сравнение семантическое (DOM, `convert_charrefs=True`; `style` игнорится).
  LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-22) — Фаза 3: revnumber prefix-strip + `[%hardbreaks]`

`fix/literal-paragraph-block-title` УЖЕ смержена+запушена в master (`5255036`, origin == master, дерево
чистое; session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base` пересобран из
ТЕКУЩЕГО чистого master `5255036` ПЕРЕД правкой. Baseline подтверждён: Identical 193, Different 151,
Errors 0. near-miss: pass/index (6-diff, фидли nested-passthrough `` `+pass:[]+` ``→пустой `<code>`,
верифицировано: `pass:[x]`→`x` ВНУТРИ monospace-passthrough — многократно отложен), stem/index
(6-diff, MathJax — архитектурный). Разведан кластер **revision-line** (reference-revision-line 11-diff,
revision-line-with-version-prefix 13-diff, reference-revision-attributes 31-diff): эмпирические пробы
asciidoctor через ФАЙЛ дали точное правило. Взят reference-revision-line — флипается двумя чистыми корнями.

### Ветка `fix/revision-prefix-and-hardbreaks` (от master `5255036`; СТАТУС: НЕ закоммичено)
- **Два корня** (оба нужны для флипа reference-revision-line.adoc — каждый по отдельности НЕ флипает):
  (1) **Revnumber prefix-strip** и (2) **`[%hardbreaks]`** (новая фича).
- **Правило revision-строки** (верифицировано пробами, зеркалит Asciidoctor `RevisionInfoLineRx`
  `^(?:\D*(.*?),)?...`): версия = часть до ПЕРВОЙ запятой со снятым ведущим нецифровым прогоном (`\D*`):
  `v8.3`→`8.3`, `LPR55`→`55`, `Version 2.5 RC1`→`2.5 RC1` (внутр. буквы/пробелы сохраняются). Дата =
  между первой `,` и первым `:` (внутр. запятые даты переживают: `July 29, 2025`). No-comma: head —
  версия ТОЛЬКО при префиксе `v`/`V`, иначе дата; `:` вводит remark. Разделители голые (без требования
  пробела). **Strip — ТОЛЬКО в парсинге revision-СТРОКИ**; явный `:revnumber: v8.3` (attribute-entry)
  НЕ стрипается (верифицировано: asciidoctor → `version v8.3,`; reference-revision-attributes это
  подтверждает). Рендерер header уже стрипал `v` для отображения (`strip_prefix('v')` line ~2539) →
  теперь no-op (идемпотентно), а для `LPR`-префикса даже чинится (был `version LPR55,`).
- **Правило hardbreaks** (новая фича — раньше НЕ поддерживалась, 6 файлов корпуса используют, но в
  hard-line-breaks.adoc все примеры внутри `[source]`-листингов → он уже Identical): `[%hardbreaks]`
  опция параграфа (или doc-attr `hardbreaks-option`) → каждый soft-break → `<br>` (asciidoctor:
  `Line one<br>\nLine two`).
- **Фикс**: (a) `scanner.rs::parse_revision_line` переписан (хелпер `strip_nondigit_prefix`, голые
  `,`/`:`-split, v-детекция для no-comma); (b) `adoc-html/lib.rs` — поле `para_hardbreaks` (set в
  `start_paragraph` из `meta.options.contains("hardbreaks")` ИЛИ `document_attrs["hardbreaks-option"]`,
  clear в `TagEnd::Paragraph`), хелпер `push_hardbreaks_text(out, text, escape)` (split `\n`, join
  `<br>\n`, последняя строка без `<br>`, escape опц.), применён в обеих ветках Text-arm (SPECIALCHARS +
  else). Обновлены 8 тестов, кодировавших `v`-префикс (5 scanner + 2 block + 1 html `test_builtin_attr_revision`),
  +2 теста (`test_parse_revision_line_nondigit_prefix`, `test_paragraph_hardbreaks_option`).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 457, html ↑,
  parsing_lab **233/233** verified `--nocapture` — revision-строк с `v`-префиксом и `[%hardbreaks]`-
  параграфов в фикстурах либо нет, либо ASG-значения не сдвинулись; правка scanner+renderer).
- Корпус `compare_full.py` (release, **standalone**): **Identical 193→194 (+1), Different 150, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `5255036`): **5 файлов** изменили
  вывод — **1 FLIP→IDENTICAL** (reference-revision-line.adoc, verified 0 diffs), **0 регрессий**.
  4 changed-still-different, все корректны на моих измерениях: paragraph 60→38 (6 `<br>` идентичны
  asciidoctor, отличие — line-offset от pre-existing), **revision-line-with-version-prefix 13→1**
  (единственный остаток `{docdate}`→`2026-03-15` — reference заморожен на дату генерации, наш рендер
  дал бы mtime/сегодня, всё равно ≠ → дата-зависим, НЕ флипается), reference-revision-attributes 31→31
  (явный `:revnumber: v8.3` верно НЕ стрипнут; pre-existing header-span gap — explicit revnumber не
  рендерится в header), text 633→650 (позиц. каскад от pre-existing sect0 level-0 heading `<h1 class="sect0">`
  vs `<div class="sect0">`; hardbreaks-контент байт-в-байт совпал с asciidoctor).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/revision-prefix-and-hardbreaks` (только по запросу).
  master == origin сейчас, после мержа потребуется пуш (по запросу).
- Чистые near-miss-кандидаты на 194: **pass/index** (6-diff, nested-passthrough `pass:[]` внутри
  `` `+...+` `` monospace — фидли, многократно отложен; правило: `pass:[x]`→`x` обрабатывается ВНУТРИ
  monospace-passthrough), **stem/index** (6-diff, MathJax `<script>`-инъекция — архитектурный standalone).
  Неразведанные/сложные: special-section-numbers (10-diff, monospace в ТЕКСТЕ xref-ссылки — архитектурный
  QUOTES в `[label]`), audio (12-diff, 2 корня: `audio::x[start=,opts=autoplay]` атрибуты + `.title` на
  audio-блоке), docinfo/index (14-diff, len_delta=0), db-migration (16-diff, таблица).
- **Обнажённый pre-existing gap** (НЕ регрессия, обнаружен этой сессией): явный `:revnumber: v8.3`
  attribute-entry НЕ рендерится в header `<span id="revnumber">` (только revision-СТРОКА создаёт
  `Event::Revision`→header-span; explicit attr идёт только в `document_attrs`). Asciidoctor рендерит
  header-span и из explicit attr (`version v8.3,`, БЕЗ strip). Узкий отдельный фикс (reference-revision-
  attributes).
- Архитектурные/отложенные (без изменений): `{docdate}`/`{localdate}` резолюция (дата-зависима, флип
  невозможен на замороженном reference), counters.adoc (счётчики в verbatim), наследование `m`/`e`/`s`
  стиля колонки таблицы, nested-форматирование в ТЕКСТЕ ссылки (QUOTES в `[label]`), inline-monospace
  passthrough char-ref (`` `&#167;` ``→`Event::Code`), inline-anchor reftext из dt-терма (lexicon),
  link-role `class="external"`, trailing ` +` в reparsed monospace → спурьезный `<br>`, level-0 sect0
  heading рендер (`<h1 class="sect0">` vs `<div class="sect0"><h1>` — text.adoc и др.).

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически (пробы asciidoctor через
  ФАЙЛ; revision-строка идёт ТОЛЬКО сразу после author-строки в header). Дамп событий парсера: throwaway
  `adoc-parser/examples/dump_events.rs`. **`compare_full.py` сравнивает STANDALONE** (полный документ с
  header/footer, виден `<span id="revnumber">`, `<div id="header">`) — не embedded.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `5255036`). near-miss `/tmp/nearmiss.py`. Точечный diff:
  `/tmp/fdiff.py <relpath>`. Сравнение семантическое (DOM, `convert_charrefs=True`; `style` игнорится).
  LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-21) — Фаза 3: `.Title` на отступном literal-параграфе

`fix/counter-bare-reference` УЖЕ смержена+запушена в master (`f9324ea`, origin == master, дерево
чистое; session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base` пересобран из
ТЕКУЩЕГО чистого master `f9324ea` ПЕРЕД правкой. Baseline подтверждён: Identical 192, Different 152,
Errors 0. near-miss: два 6-diff (pass/index — фидли `` `+pass:[]+` ``; stem/index — MathJax,
архитектурный), special-section-numbers (10-diff, monospace в тексте xref — архитектурный),
toc/index (11-diff), revision-line-кандидаты (11-13 diff). Разведка пробами: toc/index дал ЧИСТЫЙ
узкий корень. Взят toc/index.

### Ветка `fix/literal-paragraph-block-title` (от master `f9324ea`; СТАТУС: НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано пробой через файл): `.Title` перед ОТСТУПНЫМ
  literal-параграфом (` $ ...`) рендерится как `<div class="literalblock"><div class="title">Title</div>
  <div class="content"><pre>...</pre></div></div>` — ровно как у delimited literal (`....`).
- **Корень** (важно — слой!): ПАРСЕР эмитит `BlockTitle` ВЕРНО (проверено дампом событий через
  throwaway `examples/dump_events.rs`: `Start(BlockTitle)`/`Text`/`End(BlockTitle)` идут перед
  `Start(LiteralParagraph)`). Баг в РЕНДЕРЕРЕ: inline-arm `Tag::LiteralParagraph` (`adoc-html/lib.rs`
  ~1058) пушил `<div class="literalblock">\n<div class="content">\n<pre>` ОДНОЙ строкой, НЕ вызывая
  `emit_pending_block_title` → захваченный в `block_title_inner_html` заголовок терялся. Delimited
  `DelimitedBlockKind::Literal` (~1793) его зовёт — отсюда расхождение delimited vs indented.
- **Фикс** (1 точка, ТОЛЬКО `adoc-html/lib.rs`): разбит push на `">\n"` + `self.emit_pending_block_title(output)`
  + `"<div class=\"content\">\n<pre>"` — дословно зеркалит delimited-literal-arm. Title-less параграф
  не затронут (`emit_pending_block_title` — no-op при `block_title_inner_html=None`). +1 тест
  `test_literal_paragraph_block_title` (флип toc-кейс + regression guard: title-less не даёт `class="title"`).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (html 317→318, parser 456,
  parsing_lab **233/233** — правка только в рендерере, ASG читает события парсера напрямую, не задет).
- Корпус `compare_full.py` (release): **Identical 192→193 (+1), Different 151, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `f9324ea`): **1 файл** изменил
  вывод — **1 FLIP→IDENTICAL** (toc/index.adoc, verified 0 diffs len 140==140), **0 регрессий**,
  **0 changed-still-different**. Идеально узко.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/literal-paragraph-block-title` (только по запросу).
  master == origin сейчас, после мержа потребуется пуш (по запросу).
- Чистые near-miss-кандидаты на 193: **pass/index** (6-diff, `` `+pass:[]+` ``→пустой `<code></code>`,
  asciidoctor; мы `<code>pass:[]</code>` — фидли pass-в-single-plus-в-monospace, единичный),
  **stem/index** (6-diff, MathJax `<script>`-инъекция — архитектурный standalone). Неразведанные/сложные:
  **special-section-numbers** (10-diff — `<code>` внутри текста xref-ссылки `<a>`: архитектурный
  «nested-форматирование в тексте ссылки», QUOTES в `[label]`), **reference-revision-line** (11-diff —
  `{revnumber}` даёт `v8.3` вместо `8.3` (не снят префикс `v`) + `[%hardbreaks]` блок склеивает строки
  через `\n` вместо `<br>`; 2 корня), **revision-line-with-version-prefix** (13-diff — revision-строка
  `LPR55, {docdate}:...` не парсится в revnumber/revdate/revremark: мы кладём всё в revdate; нужен
  парсер revision-строки с обдиркой non-digit префикса до `\d+`, version-label localization).
- Архитектурные/отложенные (без изменений): counters.adoc (счётчики в verbatim-блоках), наследование
  `m`/`e`/`s` стиля колонки таблицы, nested-форматирование в ТЕКСТЕ ссылки (QUOTES в `[label]`),
  inline-monospace passthrough char-ref (`` `&#167;` ``→`Event::Code`), inline-anchor reftext из
  dt-терма (lexicon), link-role `class="external"`, trailing ` +` в reparsed monospace → спурьезный `<br>`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически (пробы asciidoctor
  через ФАЙЛ). Дамп событий парсера: throwaway `adoc-parser/examples/dump_events.rs`
  (`Parser::new(input)` + цикл `println!("{:?}", ev)`, `cargo run -p adoc-parser --example dump_events`,
  удалить после) — БЫСТРО различает баг парсера vs рендерера.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `f9324ea`). near-miss `/tmp/nearmiss.py`. Точечный diff:
  `/tmp/fdiff.py <relpath>`. Сравнение семантическое (DOM, `convert_charrefs=True`; `style` игнорится).
  LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-20) — Фаза 3: голая ссылка `{name}` на счётчик в document-order

`fix/section-id-dots-and-dedup` УЖЕ смержена+запушена в master (`3d8db5c`, origin == master, дерево
чистое; session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base` пересобран из
ТЕКУЩЕГО чистого master `3d8db5c` ПЕРЕД правкой. Baseline подтверждён: Identical 191, Different 153,
Errors 0. near-miss: ближайший — **counter.adoc** (2-diff), много сессий помечался «архитектурным».
Разведка показала: «архитектурность» преувеличена — есть узкий корректный фикс. Взят counter.adoc.

### Ветка `fix/counter-bare-reference` (от master `3d8db5c`; СТАТУС: НЕ закоммичено)
- **Правило Asciidoctor**: счётчик — это спец-документ-атрибут; `{counter:name}` инкрементит И
  отображает, `{counter2:name}` инкрементит молча, голый `{name}` отдаёт ТЕКУЩЕЕ значение. Всё
  резолвится в document-order (значение МЕНЯЕТСЯ по документу). В counter.adoc: `.Parts{counter2:index:0}`
  → index=0; `PX-{counter:index}` → 1, потом 2; `Description of PX-{index}` → 1, потом 2.
- **Корень**: счётчики живут в препроцессоре (`expand_counters`, построчно в document-order, пишет
  значение в локальную `attributes`), но голый `{index}` препроцессор НЕ трогал — он доезжал до
  рендерера, который резолвит из ПЛОСКОГО снимка `document_attrs` (где `index` нет, т.к. задаётся
  ТОЛЬКО счётчиком, не `:index:` entry) → `Description of PX-{index}` литералом. Рендерер с одним
  снимком в принципе не может (значение позиционное) — поэтому резолв обязан быть в препроцессоре.
- **Фикс** (1 файл, `preprocessor.rs`): (a) `preprocess_with_attrs` — поле `counter_names:
  HashSet<String>`; (b) `try_parse_counter(+counter_names)` регистрирует имя при успехе; (c) новый
  хелпер `try_expand_counter_reference(input, attributes, counter_names)` — раскрывает голый `{name}`
  ТОЛЬКО если name ∈ counter_names и attributes.get(name).is_some(); (d) `expand_counters(+counter_names)`
  — fast-path расширен (`{counter` ИЛИ (counter_names непуст И есть `{`)), в цикле новый arm для голой
  ссылки ПОСЛЕ counter-arm. Обычные атрибуты НЕ трогаются (только зарегистрированные счётчики). +2 теста
  (`test_preprocess_bare_counter_reference` — table-сценарий; `test_preprocess_bare_reference_non_counter_untouched`
  — не-счётчик остаётся `{index}` для рендерера). 14 существующих тест-вызовов `expand_counters`
  обновлены на 3-й арг `&mut HashSet::new()`.

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 454→456, html 317,
  parsing_lab **233/233** verified `--nocapture` — счётчиков-с-голой-ссылкой в фикстурах нет).
- Корпус `compare_full.py` (release): **Identical 191→192 (+1), Different 152, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `3d8db5c`): **2 файла** изменили
  вывод — **1 FLIP→IDENTICAL** (counter.adoc, verified 0 diffs), **0 регрессий**. 1 changed-still-
  different: counters.adoc (271→271 diffs БЕЗ изменений — контент чуть сместился из-за раскрытия
  голых `{seq1}`/`{pnum}`, но файл и так полностью рассинхронен каскадом author-блока + счётчики
  внутри verbatim-блоков `[source]`/`----` которые asciidoctor НЕ раскрывает а наш препроцессор
  раскрывает безусловно — архитектурный pre-existing, вне рамок).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/counter-bare-reference` (только по запросу).
  master == origin сейчас, после мержа потребуется пуш (по запросу).
- Чистые near-miss-кандидаты на 192: **pass/index.adoc** (6-diff, len_delta=-1 — `` `+pass:[]+` `` →
  asciidoctor пустой `<code></code>`, мы `<code>pass:[]</code>`; фидли pass-в-single-plus-в-monospace,
  единичный), **stem/index.adoc** (6-diff, MathJax `<script>`-инъекция — архитектурный standalone).
  Неразведанные: special-section-numbers (10-diff), toc/index (11-diff), reference-revision-line (11-diff),
  audio (12-diff), revision-line-with-version-prefix (13-diff), docinfo/index (14-diff).
- Архитектурные/отложенные (без изменений): counters.adoc (счётчики в verbatim-блоках — препроцессор
  раскрывает безусловно, нет block-context awareness), наследование `m`/`e`/`s` стиля колонки таблицы,
  nested-форматирование в ТЕКСТЕ ссылки (QUOTES в `[label]`), inline-monospace passthrough char-ref
  (`` `&#167;` ``→`Event::Code`), inline-anchor reftext из dt-терма (lexicon), link-role
  `class="external"`, trailing ` +` в reparsed monospace → спурьезный `<br>`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически (пробы asciidoctor
  через ФАЙЛ — shell экранирует спецсимволы; heredoc `<<'EOF'` или `printf` ок).
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `3d8db5c`). near-miss `/tmp/nearmiss.py`. Точечный diff:
  `/tmp/fdiff.py <relpath>`. Сравнение семантическое (DOM, `convert_charrefs=True`; `style` игнорится).
  LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-19) — Фаза 3: section-id точки-разделитель + дедуп дубликатов

`fix/escaped-inline-macro` УЖЕ смержена+запушена в master (`41bba68`, origin == master, дерево
чистое; session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base` пересобран из
ТЕКУЩЕГО чистого master `41bba68` ПЕРЕД правкой. Baseline подтверждён: Identical 190, Different 154,
Errors 0. near-miss: 2-diff counter (архитектурный, отложен), 6-diff stem/index (архитектурный
MathJax, отложен). Разведаны два неразведанных near-miss: **pass/index** (6-diff: `` `+pass:[]+` `` →
asciidoctor даёт ПУСТОЙ `<code></code>`, мы — `<code>pass:[]</code>`; фидли pass-макрос-внутри-single-
plus-внутри-monospace, единичный — ОТЛОЖЕН) и **CHANGELOG** (7-diff, section-id). Выбран CHANGELOG —
принципиальнее, затрагивает много файлов.

### Ветка `fix/section-id-dots-and-dedup` (от master `41bba68`; СТАТУС: НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано пробами через файл, НЕ по памяти): автогенерация section-id —
  (1) `.` → разделитель (как ` `/`-`/`_`): `0.3.0 Milestone Build`→`_0_3_0_milestone_build`. Прочий
  пунктуатор (`@`/`#`/`:`/`!`/`(`/`)`) ОТБРАСЫВАЕТСЯ (`Hello@World#Tag`→`_helloworldtag`,
  `Foo: Bar! (baz)`→`_foo_bar_baz`); прогон разделителей схлопывается; `...`→ellipsis→дроп (в корпусе
  таких заголовков нет, не реализовывал). (2) Дубликаты автогенерируемых заголовков → суффикс
  `_2`/`_3` (`Added`×3 → `_added`/`_added_2`/`_added_3`). Явные id (`[#id]`) НЕ переименовываются
  (asciidoctor только warning), но регистрируются → авто-id дедупится и против них. **Doctitle
  (level 0) НЕ регистрируется** (проба `= Intro` + `== Intro` → `_intro`, НЕ `_intro_2`). Discrete-
  заголовки участвуют в ОБЩЕМ реестре (проба `== Real` + 2×`[discrete] == Real` → `_real`/`_real_2`/
  `_real_3`).
- **Корень**: (1) `scanner.rs::generate_id` (~813) в else-ветке принимал разделителями только
  ` `/`-`/`_`, `.` падал в «дроп» (нет else) → `0.3.0`→`030`. (2) Дедупа не было — каждый
  `generate_id` независим, дубликаты давали одинаковый id.
- **Фикс** (2 файла): (a) `scanner.rs::generate_id` — `.` добавлен в условие разделителя (1 символ);
  (b) `block.rs` — поле `used_ids: std::collections::HashSet<String>` (+init), хелперы
  `register_explicit_id(&str)` (insert как есть) и `unique_auto_id(String)->String` (при коллизии
  `format!("{base}{sep}{n}")`, n от 2, `HashSet::insert` возвращает was-new). Маршрутизированы
  `scan_section` (явный→register+verbatim, авто→unique_auto_id) и `scan_discrete_heading` (то же).
  **scan_document_header/_with_pre_attrs (doctitle, 905/1062) НЕ тронуты** (doctitle не в реестре).
  +4 теста (scanner: dot/collapse; block: dedup/auto-vs-explicit/dots).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 451→454, html 317,
  parsing_lab **233/233** verified `--nocapture` — правка в section-сканере, ASG читает события парсера;
  дублей-заголовков/точек в фикстурах нет, id-события не сдвинулись).
- Корпус `compare_full.py` (release): **Identical 190→191 (+1), Different 153, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `41bba68`): **15 файлов**
  изменили вывод — **1 FLIP→IDENTICAL** (CHANGELOG.adoc), **0 регрессий**. 14 changed-still-different:
  спот-проверка (counters/section/outline/title) — секционные id теперь СОВПАДАЮТ с asciidoctor;
  остаток Different по др. причинам (class `sect0` на level-0, toc-расположение, author-id-дедуп,
  `{counter:seq}` не резолвится). counters.adoc: моя дедупликация корректно работает на пре-
  существующем неверном базисе (`_section_seq1`/`_section_seq1_2` вместо двух одинаковых).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/section-id-dots-and-dedup` (только по запросу).
  master == origin сейчас, после мержа потребуется пуш (по запросу).
- Чистые flip-кандидаты (near-miss на 191): **counter.adoc** (2-diff, `{counter:index}`→`{index}` —
  АРХИТЕКТУРНЫЙ, счётчик в локальной мапе препроцессора, отложен), **pass/index.adoc** (6-diff,
  `` `+pass:[]+` ``→пустой `<code>`; фидли pass-макрос-в-single-plus-в-monospace, единичный),
  **stem/index.adoc** (6-diff, MathJax `<script>`-инъекция — архитектурный standalone). Неразведанные:
  special-section-numbers (10-diff), toc/index (11-diff), reference-revision-line (11-diff).
- Архитектурные (отложены): наследование `m`/`e`/`s` стиля колонки таблицы, nested-форматирование в
  ТЕКСТЕ ссылки (QUOTES в `[label]`), inline-monospace passthrough char-ref (`` `&#167;` ``→`Event::Code`),
  inline-anchor reftext из dt-терма (lexicon), link-role `class="external"`, trailing ` +` в reparsed
  monospace → спурьезный `<br>` (обнажён single-plus, см. поздняя-17).

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически (пробы asciidoctor
  через ФАЙЛ — shell экранирует спецсимволы; heredoc `<<'EOF'` или `printf` ок).
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `41bba68`). near-miss `/tmp/nearmiss.py`. Точечный diff:
  `/tmp/fdiff.py <relpath>`. Сравнение семантическое (DOM, `convert_charrefs=True`; `style` игнорится).
  LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-18) — Фаза 3: escaped inline-макрос `\name:target[attrs]`

`fix/single-plus-passthrough-constrained` УЖЕ смержена в master (`3ca24a3`, origin == master, дерево
чистое; session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base` пересобран из
ТЕКУЩЕГО чистого master `3ca24a3` ПЕРЕД правкой (был stale от `1688344`). Baseline подтверждён:
Identical 189, Different 155, Errors 0. near-miss: 2-diff counter (архитектурный, отложен), 4-diff
**user-index** (escaped indexterm). Разведка пробами через ФАЙЛ дала ЧИСТЫЙ общий корень — escaped
inline-макрос. Выбран user-index (4-diff, один корень).

### Ветка `fix/escaped-inline-macro` (от master `3ca24a3`; СТАТУС: НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано пробами, НЕ по памяти): `\` перед inline-макросом снимается,
  макрос выводится ЛИТЕРАЛОМ как текст, макрос НЕ обрабатывается. Действует и в plain, и внутри
  monospace (`` `\indexterm2:[<primary>]` ``→`<code>indexterm2:[<primary>]</code>`). Снимается ТОЛЬКО
  перед макросом, который asciidoctor распознаёт ПО УМОЛЧАНИЮ: stem/latexmath/asciimath/link/xref/
  mailto/icon/image (single `:`)/indexterm/indexterm2/footnote (pass уже был отдельным arm'ом).
  Перед experimental kbd/btn/menu (выкл по умолчанию) и custom-catch-all (`\notamacro:foo[bar]`,
  `\unknown:[just]`) `\` СОХРАНЯЕТСЯ — это не макрос, нечего экранировать. Block-форма `image::`
  (двойное двоеточие) НЕ трогается. Внутреннее содержимое экранированного макроса у asciidoctor
  форматируется (`\link:u[*b*]`→`link:u[<strong>b</strong>]`, quotes до macros) — я СОЗНАТЕЛЬНО
  эмитю весь run одним литеральным span (для корпуса форматирования внутри нет; избегаю ре-диспатча
  на autolink URL-таргета типа `\link:https://x[t]` — патологический случай у asciidoctor).
- **Корень**: парсер оставлял `\` как обычный текст И обрабатывал макрос. `\indexterm2:[primary]`→
  `\primary` (indexterm2 — flow-term, видимый контент), `\footnote:[x]`/`\image:p[a]` → `\`+рендер.
- **Фикс** (1 точка + хелпер, ТОЛЬКО `inline.rs`): новый arm в `handle_inline_escape` ПОСЛЕ `\pass:`-arm
  (`b'\\' if self.inline_macro_escape_len(self.pos+1) > 0`): flush, снять `\`, эмитить
  `input[macro_start..pos]` одним `Event::Text(Cow::Borrowed)` (рендерер html_escape'ит `<`/`&`).
  Хелпер `inline_macro_escape_len(p)->usize` (рядом с `char_ref_len_at`): gated на
  `subs.has(MACROS)`; матчит распознаваемое имя из NAMES[11]+`:`, отклоняет `name::` (block),
  target = run не-whitespace до `[`, требует `[`…`]`, возвращает длину run от p до `]` включительно.
  +2 теста (`test_escaped_inline_macro` 6 позитивных кейсов incl. monospace; `test_backslash_before_
  unrecognized_macro_kept` — kbd/btn/menu/`image::` сохраняют `\`).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 449→451, html 317,
  parsing_lab **233/233** — escaped-макросов в фикстурах НЕТ → ASG не задет).
- Корпус `compare_full.py` (release): **Identical 189→190 (+1), Different 154, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `3ca24a3`): **1 файл** изменил
  вывод — **1 FLIP→IDENTICAL** (user-index.adoc), **0 регрессий**, 0 changed-still-different. Идеально
  узко: escaped `\image::` block в outline.adoc корректно НЕ тронут (хелпер отклоняет `::`).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/escaped-inline-macro` (только по запросу).
  master == origin сейчас, после мержа потребуется пуш (по запросу).
- Чистые flip-кандидаты (near-miss на 190): **counter.adoc** (2-diff, `{counter:index}`→`{index}` —
  АРХИТЕКТУРНЫЙ, счётчик в локальной мапе препроцессора, отложен). Новые неразведанные:
  **pass/index.adoc** (6-diff, len_delta=-1 — один ЛИШНИЙ элемент у нас; НЕ про пустой `pass:[]`
  (он у нас уже верен — `empty==mark` совпал), нужна разведка корня), **CHANGELOG.adoc** (7-diff).
  **stem/index.adoc** (6-diff) — инъекция MathJax `<script>` в конец body при `:stem:`; АРХИТЕКТУРНАЯ
  standalone-фича (docinfo/footer-скрипты), отложена.
- Отложенный pre-existing баг (НЕ регрессия, обнажён single-plus): trailing ` +` в reparsed
  monospace-контенте → спурьезный `<br>` (`` `z +` ``→`<code>z<br></code>`; outline.adoc стр 390).
- Архитектурные (отложены): наследование `m`/`e`/`s` стиля колонки таблицы, nested-форматирование в
  ТЕКСТЕ ссылки (QUOTES в `[label]`), inline-monospace passthrough char-ref (`` `&#167;` ``→`Event::Code`),
  inline-anchor reftext из dt-терма (lexicon), link-role `class="external"`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически (пробы asciidoctor
  через ФАЙЛ — shell экранирует `\`/`+`/backtick'и; heredoc `<<'EOF'` или `printf` ок).
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `3ca24a3`). near-miss `/tmp/nearmiss.py`. Точечный diff
  одного файла: `/tmp/fdiff.py <relpath>` (норм. из compare_full). Сравнение семантическое (DOM,
  `convert_charrefs=True` → `&#8217;`≡`’`; `style`-атрибут игнорится). LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-17) — Фаза 3: single-plus `+…+` как constrained-пара

`fix/passthrough-inside-monospace` УЖЕ смержена в master (`1688344`, origin == master, дерево чистое;
session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base` пересобран из ТЕКУЩЕГО
чистого master `1688344` ПЕРЕД правкой. Baseline подтверждён: Identical 188, Different 156, Errors 0.
near-miss на 188: 1-diff **keyboard-macro** (`` `+kbd:[key(+key)*]+` `` — многократно отложен как
«фидли»), 2-diff counter (архитектурный), 4-diff user-index (escape-в-monospace). Разведка пробами
показала, что keyboard-macro — ЧИСТЫЙ корень (constrained-правила single-plus), решён.

### Ветка `fix/single-plus-passthrough-constrained` (от master `1688344`; СТАТУС: НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано пробами через файл, НЕ по памяти): single-plus passthrough
  `+X+` — **constrained-пара** (как `*`/`_`/`` ` ``/`#`). (1) open `+` не после word-char: `C+a+b+`→
  литерал, ` +a+`/`(+a+` ок; (2) контент первый символ не пробел: `+ a+`→литерал; (3) контент последний
  символ не пробел; (4) close `+` не перед word-char и не часть `++`/`+++`: `+a+b+`→`a+b` (внутренний
  `+` за ним `b` НЕ закрывает — поиск ПРОДОЛЖАЕТСЯ до хвостового `+`), `+a + b+`→`a + b`, `(+a+b+)`→`(a+b)`;
  (5) нет валидного close → ведущий `+` остаётся литералом: `+a+b`→`+a+b`. Применяется и к
  `` `+...+` `` (literal-monospace): backtick-constrained находит backtick→backtick, контент `+X+`
  reparse'ится → single-plus с правильным закрывающим (`` `+kbd:[key(+key)*]+` ``→`<code>kbd:[key(+key)*]</code>`).
- **Корень**: `inline.rs::try_single_plus_passthrough` брал ПЕРВЫЙ встречный `+` (не часть `++`) как
  закрывающий, без проверки границ. Для `+kbd:[key(+key)*]+` это давал внутренний `+` на offset 9 →
  `<code>kbd:[key(</code>...` каскад → `kbd:[key(key)*]+`.
- **Фикс** (1 точка, ТОЛЬКО `inline.rs::try_single_plus_passthrough`): добавлены guard
  `is_word_char_before(start_pos)` (open-граница) и `bytes[after_open]==b' '` (контент не с пробела);
  close-loop теперь требует `!preceded_by_plus && !preceded_by_space && !followed_by_plus &&
  !followed_by_word` (preceded_by_space = контент не кончается пробелом; followed_by_word = constrained-
  close), и при невалидном кандидате ПРОДОЛЖАЕТ скан (а не сдаётся). Зеркалит `try_constrained`, но с
  продолжающимся поиском (у quote-маркеров `find_closing_constrained` сдаётся на первом — отдельный
  pre-existing баг с БОЛЬШИМ blast radius, НЕ трогал: `*a*b*`→`<strong>a*b</strong>` мы даём `*a*b*`).
  +2 теста (`test_single_plus_passthrough_constrained` 5 кейсов, `test_monospace_passthrough_inner_plus`).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 447→449, html 317,
  parsing_lab **233/233** verified `--nocapture`). Правка в single-plus close-finder; ASG читает события
  парсера, но кейсов `+a+b+` (внутренний `+`) в фикстурах нет → не задет.
- Корпус `compare_full.py` (release): **Identical 188→189 (+1), Different 155, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `1688344`): **3 файла** изменили
  вывод — **1 FLIP→IDENTICAL** (keyboard-macro.adoc), **0 регрессий**. 2 changed-still-different
  УЛУЧШЕНЫ (raw-difflines vs asciidoctor): asciidoc-vs-markdown 406→404 (`Markdown + X` теперь верно),
  outline 325→324 (`signifier + reference`, `section + doctype` теперь верно).

### Обнажённый pre-existing баг (НЕ регрессия, отложен)
- Trailing ` +` (space-plus) в **reparsed monospace-контенте** трактуется как hard-break → спурьезный
  `<br>` вместо литерала. Верифицировано: `` `z +` `` даёт `<code>z<br></code>` и на ЧИСТОЙ БАЗЕ
  (без single-plus) — **независим от моей правки**, она лишь обнажила его в случае `` `` + +`` ``
  (база раньше поедала `+` через single-plus → `<code>  </code>`, теперь `+` выживает → `<br>`).
  asciidoctor: `<code>z +</code>`. Виден в outline.adoc строка 390 `` `` + +`` ``. Корень — hard-break
  детект в inline-reparse подстроки (а не end-of-source-line). Узкий, отдельный фикс.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/single-plus-passthrough-constrained` (только по запросу).
  master == origin сейчас, после мержа потребуется пуш (по запросу).
- Чистые flip-кандидаты (near-miss на 189): **counter.adoc** (2-diff, `{counter:index}`→`{index}` —
  АРХИТЕКТУРНЫЙ, счётчик в локальной мапе препроцессора, отложен). Неразведанный 4-diff:
  **user-index.adoc** — escape `\indexterm2:[<primary>]` внутри monospace (asciidoctor: литерал
  `indexterm2:[<primary>]`, `\` снят; мы парсим макрос). Корень — `\` перед макросом внутри `` ` ``
  не подавляет макрос (handle_inline_escape / indexterm-парсинг).
- Возможные расширения (НЕ нужны для флипов): (a) hard-break-в-reparsed-monospace (см. выше, обнажён
  этой правкой — узкий); (b) quote-маркеры тоже должны продолжать поиск закрывающего
  (`find_closing_constrained` — `*a*b*`→`<strong>a*b</strong>`; БОЛЬШОЙ blast radius, рискован).
- Архитектурные (отложены): наследование `m`/`e`/`s` стиля колонки таблицы, nested-форматирование в
  ТЕКСТЕ ссылки (QUOTES в `[label]`), inline-monospace passthrough char-ref (`` `&#167;` ``→`Event::Code`),
  inline-anchor reftext из dt-терма (lexicon), link-role `class="external"`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически (пробы asciidoctor
  через ФАЙЛ — shell экранирует `+`/backtick'и; heredoc `<<'EOF'` ок).
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `1688344`). near-miss `/tmp/nearmiss.py` (вывод в
  `/tmp/nearmiss_out.txt`). Сравнение семантическое (DOM, `convert_charrefs=True` → `&#8217;`≡`’`;
  `style`-атрибут игнорится). LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-16) — Фаза 3: passthrough внутри monospace/quote (`` `++`++` ``)

`fix/attr-ref-path-before-brackets` УЖЕ смержена в master (`a57aeda`, origin == master, дерево чистое;
session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base` обновлён из ТЕКУЩЕГО чистого
master `a57aeda` ПЕРЕД правкой (скопирован свежесобранный release-бинарь). Baseline подтверждён:
Identical 186, Different 158, Errors 0. near-miss: 1-diff keyboard-macro (passthrough `+...+`, фидли —
отложен), 2-diff counter (архитектурный — отложен). Из трёх неразведанных 4-diff (role/user-index/
text-index) разведка дала ДВА корня: (A) **`` `++`++` ``** passthrough-внутри-monospace (role.adoc +
text/index.adoc — ОДИН корень, 2 флипа), (B) user-index.adoc — escape `\indexterm2:[...]` внутри
monospace (отдельный, отложен). Выбран A — чистый, 2 флипа.

### Ветка `fix/passthrough-inside-monospace` (от master `a57aeda`; СТАТУС: НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано пробами через файл, НЕ по памяти): passthrough
  (`++…++`/`+++…+++`/`+…+`) извлекается в пре-пасс ДО quote-подстановки. Поэтому quote-маркер ВНУТРИ
  passthrough не закрывает внешний span. `` (`++`++`) ``→`<code>`</code>`, `` `++b++` ``→`<code>b</code>`,
  `` `x ++ y` ``→`<code>x ++ y</code>` (одиночный незакрытый `++` остаётся литералом — span матчится
  нормально), `` `pre ++*bold*++ post` ``→`<code>pre *bold* post</code>` (passthrough-контент сырой),
  `` `+++<b>r</b>+++` ``→`<code><b>r</b></code>`.
- **Корень**: `inline.rs::find_closing_constrained` сканировал байты слева-направо и возвращал ПЕРВЫЙ
  `marker` (для backtick — backtick ВНУТРИ `++`++` на offset 2) → `<code>++</code>` + каскад остатка
  (`++`)` литералом). Парсер однопроходный, пре-пасса извлечения passthrough нет.
- **Фикс** (1 точка + хелпер, ТОЛЬКО `inline.rs`): новый `passthrough_span_len(s, i) -> Option<usize>`
  (зеркалит `try_double/triple_plus_passthrough`: `+++…+++` или `++…++`, non-empty контент, ближайший
  закрывающий делимитер; возвращает длину региона в байтах; `i` указывает на `+`; все делимитеры ASCII →
  валидные char-границы). `find_closing_constrained` переписан с byte-индекс-цикла на `while i<len`:
  если `bytes[i]==b'+'` и `passthrough_span_len` вернул Some(skip) → `i += skip; continue` (пропуск
  passthrough-региона); иначе проверка marker как раньше (`i>0`, next != marker). Применяется ко ВСЕМ
  constrained-маркерам (`*`/`_`/`` ` ``/`#` — общая функция). Внутренний reparse в `try_constrained`
  уже корректно эмитит passthrough через `handle_inline_passthrough` (`InlinePassthrough`, raw).
  Одиночный `+…+` СОЗНАТЕЛЬНО НЕ пропускается (для корпуса не нужен — `` `+*+` ``/`` `+_+` `` уже
  работали, внутри них нет backtick; меньше риска). `find_closing_unconstrained` НЕ тронут (корпусу не
  нужен). +1 тест `test_passthrough_inside_monospace` (3 кейса). Использует let-chain (`&& let`) —
  валидно в Rust 2024.

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 446→447, html 317,
  parsing_lab **233/233** verified `--nocapture`). Правка в close-finder, ASG читает события парсера —
  но кейсов с passthrough-внутри-quote в фикстурах нет, `br`/события не задеты.
- Корпус `compare_full.py` (release): **Identical 186→188 (+2), Different 156, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `a57aeda`): **5 файлов** изменили
  вывод — **2 FLIP→IDENTICAL** (role.adoc, text/index.adoc), **0 регрессий**. 3 changed-still-different
  УЛУЧШЕНЫ: bold.adoc/italic.adoc 6→2 raw-diff (body-passthrough совпал; остаток — pre-existing diff в
  author-блоке standalone-обёртки `<div class="details">`/author-span/комментарий, НЕ связан с фиксом,
  проверено), troubleshoot-unconstrained 38→36 (`_++__kernel++_`→`<em>__kernel</em>` точно совпал с
  asciidoctor).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/passthrough-inside-monospace` (только по запросу).
  master == origin сейчас, после мержа потребуется пуш (по запросу).
- Чистые flip-кандидаты (near-miss на 188): **keyboard-macro** (1-diff, kbd `+`-passthrough — фидли),
  **counter.adoc** (2-diff, архитектурный). Неразведанный остаток 4-diff: **user-index.adoc** — escape
  макроса внутри monospace `` `\indexterm2:[<primary>]` `` (asciidoctor: литерал `indexterm2:[<primary>]`,
  `\` снят; мы: `<code>\&lt;primary&gt;</code>` — теряем `indexterm2:[`/`]`, парсим макрос). Корень —
  `\` перед макросом внутри `` ` `` не подавляет макрос. Стоит глянуть role.adoc/user-index ещё раз на
  предмет чистого корня.
- Возможное расширение ЭТОГО фикса (НЕ нужно для флипов): пропуск passthrough в
  `find_closing_unconstrained` (для `**`/`__`/`` `` ``/`##`) и одиночный `+…+` — для симметрии/
  корректности, если всплывёт в корпусе. Сейчас сознательно узко.
- Архитектурные (отложены): наследование `m`/`e`/`s` стиля колонки таблицы, nested-форматирование в
  ТЕКСТЕ ссылки (QUOTES в `[label]`), inline-monospace passthrough char-ref (`` `&#167;` `` в `<code>`,
  `Event::Code`), inline-anchor reftext из dt-терма (lexicon), link-role `class="external"`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически (пробы asciidoctor
  через ФАЙЛ — shell экранирует backtick'и в `echo`/heredoc-`<<'EOF'` ок).
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `a57aeda`; зовёт `-a nofooter`, сравнивает `<body>`).
  near-miss `/tmp/nearmiss.py` (вывод в `/tmp/nearmiss_out.txt`). Сравнение семантическое (DOM,
  `convert_charrefs=True` → `&#8217;`≡`’`; `style`-атрибут игнорится). LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-15) — Фаза 3: путь между `}` и `[` в attr-ref (`{url}/issues[text]`)

`fix/attr-ref-respect-block-subs` УЖЕ смержена в master (`107b7e2`/`18f7ca2`, дерево чистое, master ==
origin — предыдущие мержи запушены; session.md прошлой сессии писалась ДО мержа — как всегда).
`/tmp/adoc_base` пересобран из ТЕКУЩЕГО master `107b7e2` ПЕРЕД правкой (был stale от `9069890`).
Baseline подтверждён: Identical 185, Different 159, Errors 0. near-miss дал 1-diff keyboard-macro
(passthrough `+...+`, фидли — отложен) и 2-diff counter (архитектурный — отложен); выбран 3-diff
**reference-attributes** — принципиальное продолжение attr-ref-link-macro, чистый и понятный.

### Ветка `fix/attr-ref-path-before-brackets` (от master; СТАТУС: НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано): attributes-sub ДО macros-sub. `{url-repo}/issues[text]`
  где `:url-repo: https://…` → атрибут раскрывается, затем `value/issues[text]` переразбирается как
  URL-макрос → `<a href="…/issues">text</a>`. Мы захватывали `[...]` ТОЛЬКО вплотную за `}` (фикс
  attr-ref-link-macro), а путь `/issues` между `}` и `[` утекал → голый URL автолинковался bare +
  leftover `/issues[text]` литералом.
- **Корень**: `inline.rs::try_attribute_reference` (~1635) — захват `trailing_brackets` требовал
  `tail.starts_with('[')` сразу после `}`.
- **Фикс** (1 точка, ТОЛЬКО `inline.rs`): перед проверкой `[` считается `path_len` = run байтов
  без пробела/`[`/`]` (все стоп-символы ASCII → `path_len` всегда валидная char-граница, даже с UTF-8
  в пути); если `after_path` начинается на `[` (не `[[`) и есть `]` → захват `tail[..path_len+rb+1]`
  (путь+скобки). Рендерер (`lib.rs` arm AttributeReference) и ASG-builder работают с `trailing_brackets`
  обобщённо (`format!("{value}{br}")` + reparse / `resolved.push_str(&br)`) — путь едет ВНУТРИ `br`,
  БЕЗ их изменений. Не-URL значение → склейка остаётся литералом (как раньше). +2 теста
  `test_attribute_reference_captures_path_before_brackets` (parser: capture/space-stops/no-bracket),
  `test_attribute_reference_path_before_brackets_link` (html: ссылка, без leftover/bare).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 445→446,
  html 316→317, parsing_lab **233/233** — инлайн-`{attr}path[...]` в фикстурах нет, ASG читает события
  парсера, но `br` теперь длиннее только когда есть путь+скобки → кейсы не задеты).
- Корпус `compare_full.py` (release): **Identical 185→186 (+1), Different 158, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `107b7e2`): **3 файла** изменили
  вывод — **1 FLIP→IDENTICAL** (reference-attributes.adoc), **0 регрессий**. CHANGELOG.adoc улучшен
  (10→7 diff: `{url-repo}/-/commits/main[commit history]` → ссылка, verified байт-в-байт vs asciidoctor),
  outline.adoc нейтрально (8840→8840 — `{url-issues}/25[#25]` ссылки совпали с asciidoctor, но файл
  позиционно рассинхронен на 8840/9363, число diff не сдвинулось).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/attr-ref-path-before-brackets` (только по запросу).
  master == origin сейчас, так что после мержа потребуется пуш (по запросу).
- Чистые flip-кандидаты (near-miss на 186): **keyboard-macro** `` `+kbd:[key(+key)*]+` `` (1-diff,
  passthrough `+...+` ест внутренний `+`; фидли, многократно отложен), **counter.adoc** (2-diff,
  `{counter:index}`→`{index}` не резолвится; АРХИТЕКТУРНЫЙ — счётчик в локальной мапе препроцессора).
  Далее НЕ разведанные 4-diff: role.adoc, user-index.adoc, text/index.adoc — стоит посмотреть, нет ли
  среди них чистого корня. Архитектурные (отложены): наследование `m`/`e`/`s` стиля колонки таблицы,
  nested-форматирование в ТЕКСТЕ ссылки (QUOTES в `[label]`), inline-monospace passthrough char-ref
  (`Event::Code`), inline-anchor reftext из dt-терма (lexicon), link-role `class="external"`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `107b7e2`). near-miss `/tmp/nearmiss.py` (вывод в
  `/tmp/nearmiss_out.txt`). Сравнение семантическое (DOM); нормализатор стрипает leading ws.
  LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-14) — Фаза 3: значение `{attr-ref}` уважает subs блока

`fix/verbatim-paragraph-comment` УЖЕ смержена в master (`9069890`/`47ecc17`, дерево чистое; session.md
прошлой сессии писалась ДО мержа — как всегда; master впереди origin — пуш НЕ делался). `/tmp/adoc_base`
пересобран из ТЕКУЩЕГО master `9069890` ПЕРЕД правкой (был stale от `e1768d2`). Baseline подтверждён:
Identical 184, Different 160, Errors 0. near-miss дал два 1-diff: keyboard-macro `kbd:[key(+key)*]`
(passthrough `` `+...+` `` ест внутренний `+` — известный фидли, отложен) и **listing-blocks.adoc**
(апостроф в значении атрибута). Выбран listing-blocks — принципиальнее и узче.

### Ветка `fix/attr-ref-respect-block-subs` (от master; СТАТУС: НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано): резолвнутое значение `{attr}` подставляется в рамках
  subs-пайплайна ТЕКУЩЕГО блока. В verbatim listing (`[subs="+attributes"]` → SPECIALCHARS|CALLOUTS|
  ATTRIBUTES, БЕЗ replacements) апостроф в значении остаётся прямым (`I've`); в обычном параграфе
  (NORMAL) — курлится (`I’ve`). Источник: `listing-blocks.adoc` `:replace-me: I've been replaced!`
  внутри `[subs="+attributes"]----`.
- **Корень**: `adoc-html/lib.rs::render_inline_value` (стр ~436) ЖЁСТКО форсил `SubstitutionSet::NORMAL`
  при разборе значения атрибута → внутри listing-блока (где `current_subs()`=VERBATIM+attributes)
  всё равно применялись REPLACEMENTS → апостроф курлился. Единственный вызывающий путь — arm
  `Event::AttributeReference` (588/590), резолв из `document_attrs`.
- **Фикс** (1 строка + коммент, ТОЛЬКО `adoc-html/lib.rs`): `parse_str_with_subs(value, NORMAL)` →
  `parse_str_with_subs(value, self.current_subs())`. В NORMAL-контексте поведение НЕ меняется
  (current_subs()=NORMAL); в verbatim — value идёт одним Text → early-return `html_escape` (прямой
  апостроф, спецсимволы экранируются — VERBATIM имеет SPECIALCHARS). intrinsic/env/fallback/missing
  ветки уже шли через `html_escape` (без replacements) — корректны. +1 тест
  `test_listing_block_attr_ref_no_replacements` (listing+attributes держит прямой апостроф; NORMAL
  параграф курлит — regression guard NORMAL-пути).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (html 315→316, parser 445,
  parsing_lab **233/233** — правка ТОЛЬКО в adoc-html, ASG читает события парсера напрямую, не задет).
- Корпус `compare_full.py` (release): **Identical 184→185 (+1), Different 159, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `9069890`): **2 файла** изменили
  вывод — **1 FLIP→IDENTICAL** (listing-blocks.adoc), **0 регрессий**. reference-attributes.adoc
  КРУПНО улучшен (**330→3 diff-строк**: в нём много attr-ref в verbatim-контексте, NORMAL-курлинг давал
  позиционный каскад) — остаётся Different по ОТДЕЛЬНОМУ багу `{url}/issues[text]` (путь между `}` и
  `[` не захватывается `trailing_brackets` — расширение attr-ref-link-macro, вне рамок).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/attr-ref-respect-block-subs` (только по запросу).
  NB: master впереди origin (накопились предыдущие мержи) — пуш тоже не делался.
- Чистые flip-кандидаты (near-miss на 185): **keyboard-macro** `` `+kbd:[key(+key)*]+` `` (1-diff,
  passthrough `+...+` ест внутренний `+`; фидли — inline passthrough-парсер), **counter.adoc**
  (`{counter:index}`→`{index}` не резолвится; архитектурный — счётчик в локальной мапе препроцессора).
  Смежное (НЕ flip в одиночку): `{url}/path[text]` link-macro с путём между `}` и `[` (расширить
  захват `trailing_brackets` — задевает reference-attributes 3-diff). Архитектурные (отложены):
  наследование `m`/`e`/`s` стиля колонки таблицы (`a`/AsciiDoc рискован), nested-форматирование в
  ТЕКСТЕ ссылки (QUOTES в `[label]`), inline-monospace passthrough char-ref (`Event::Code`),
  inline-anchor reftext из dt-терма (lexicon, ~14 ссылок), link-role `class="external"`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `9069890`). near-miss `/tmp/nearmiss.py` (вывод в
  `/tmp/nearmiss_out.txt`). Сравнение семантическое (DOM); нормализатор стрипает leading ws.
  LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-13) — Фаза 3: verbatim-параграф сохраняет `//`-комментарий

`fix/table-header-column-style` УЖЕ смержена в master (`e1768d2`/`1280aa6`, дерево чистое; master
впереди origin на 2 коммита — пуш НЕ делался, только по запросу; session.md прошлой сессии писалась
ДО мержа — как всегда). `/tmp/adoc_base` пересобран из ТЕКУЩЕГО master `e1768d2` ПЕРЕД правкой (был
stale от `12065d0`). Baseline подтверждён near-miss: Identical 182, 162 Different-файлов. Выбор
кандидата: counter.adoc (2-diff) при разведке подтверждён АРХИТЕКТУРНЫМ (счётчик пишет в attrs
препроцессора, рендерер берёт `{index}` из document_attrs, значение МЕНЯЕТСЯ по документу — чистого
моста нет; отложен). Выбран **verse `// end::para[]`** (1-diff) — при пробах оказался ШИРЕ и ЧИЩЕ:
общее правило verbatim-комментариев, корень общий с literal.adoc.

### Ветка `fix/verbatim-paragraph-comment` (от master; СТАТУС: НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано пробами, НЕ по памяти): строки verbatim-параграфов
  читаются СЫРЫМИ → `//`-комментарии внутри СОХРАНЯЮТСЯ как контент. keep-set = **verse, literal,
  listing, source** (рендерятся в `<pre>`). STRIP-set (комментарий вырезается): нормальный, quote,
  example, sidebar, admonition, **pass** (pass — raw, но комментарий стрипает!). Delimited verbatim-
  блоки (`....`/`----`/`____`) у нас УЖЕ работали верно (пробы подтвердили) — баг ТОЛЬКО в verbatim-
  ПАРАГРАФАХ (стиль без делимитеров). Отступной literal-параграф тоже сохраняет col-0 комментарий
  (asciidoctor вообще продолжает literal-параграф на flush-тексте до пустой строки — НЕ реализовывал,
  только комментарий, узко).
- **Корень**: `block.rs::scan_paragraph` (цикл чтения `para_lines`, ~1645) безусловно ломался на
  `is_line_comment` → verbatim-стиль доходил до match'а уже без комментарной строки; `scan_literal_
  paragraph` (~1880) ломался на любой неотступной строке, включая col-0 комментарий.
- **Фикс** (2 точки, ТОЛЬКО `block.rs`):
  - `scan_paragraph`: перед циклом флаг `verbatim_paragraph` = `pending_block_attrs.is_some_and(|a|
    matches!(a.block_style_kind(), Some("verse"|"literal"|"listing")) || a.is_source_block())`;
    условие в цикле `scanner::is_line_comment(line)` → `(!verbatim_paragraph && is_line_comment(line))`.
    (Первая строка scan_paragraph НИКОГДА не комментарий — block-level comment-handling в
    `scan_block_containers` идёт раньше; комментарий только как continuation.)
  - `scan_literal_paragraph`: разбит break — `if is_blank break;` затем `if !starts_with(' '/'\t')
    && !is_line_comment(line) break;`. Включение col-0 комментария делает min_indent=0 → лидирующий
    пробел контента сохраняется (совпадает с asciidoctor; нормализатор всё равно стрипает leading ws).
  - +1 тест `test_verbatim_paragraph_keeps_line_comment` (verse+отступной literal СОХРАНЯЮТ
    `// end::...`; нормальный параграф СТРИПАЕТ — regression guard; через `events.contains`).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 444→445,
  html 315, parsing_lab **233/233** — правка только в paragraph-сканере, ASG-кейсы не задеты).
- Корпус `compare_full.py` (release): **Identical 182→184 (+2), Different 160, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `e1768d2`): **4 файла**
  изменили вывод — **2 FLIP→IDENTICAL** (verse.adoc, literal.adoc), **0 регрессий**. 2 changed-still-
  different УЛУЧШЕНЫ: block.adoc 8→7, listing.adoc 25→24 diff-строк (verbatim-комментарий теперь
  верен, Different по др. причинам).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/verbatim-paragraph-comment` (только по запросу).
  NB: master впереди origin на 2 коммита (table-header-column-style) — пуш тоже не делался.
- Чистые flip-кандидаты (near-miss на 184): **kbd `+`-разделитель** `kbd:[key(+key)*]` (1-diff,
  passthrough `+...+` ест внутренний `+` — фидли), **listing-blocks.adoc** (1-diff в демонстрационном
  `------` listing с `[subs="+attributes"]` — verbatim-нюанс, надо смотреть полный diff),
  **inline-anchor reftext из dt-терма** (`[[id]]term::` → `<<id>>`=текст терма; lexicon, ~14 ссылок,
  БОЛЬШЕ по объёму). Отложены архитектурные: counter.adoc (`{counter:index}`→`{index}`, значение
  меняется по документу), nested-форматирование в ТЕКСТЕ ссылки (QUOTES в `[label]`), inline-monospace
  passthrough char-ref (`Event::Code`), link-role `class="external"`.
- Возможный остаток ЭТОГО фикса (НЕ нужен для флипов): literal-параграф продолжается на flush-тексте
  до пустой строки (asciidoctor); я реализовал только продолжение на комментарии (узко, без риска).

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `e1768d2`). near-miss `/tmp/nearmiss.py` (вывод в
  `/tmp/nearmiss_out.txt`). Сравнение семантическое (DOM); нормализатор стрипает leading ws у
  text-узлов. LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-12) — Фаза 3: header-style колонка таблицы (`h`) → `<th>`

`fix/attr-ref-link-macro` УЖЕ смержена в master (`12065d0`, origin == master, дерево чистое;
session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base` пересобран из master
`12065d0` ПЕРЕД правкой (был stale от `e41ad48`). Baseline подтверждён: Identical 180, Different
164, Errors 0. Выбор по near-miss на 180: 1-diff (verse `// end::para[]`, kbd `+`, listing-blocks
subs) рискованны/архитектурны. Из 2-diff: counter.adoc (препроцессор раскрывает счётчик в локальную
мапу, но плоский `{index}` резолвит рендерер из document_attrs — отложен, архитектурный) vs width.adoc
(`th` vs `td`). Выбран **width.adoc** — инфраструктура (`CellStyle::Header` уже парсится) есть.

### Ветка `fix/table-header-column-style` (от master; СТАТУС: НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано пробами, НЕ по памяти): тег `<th>` ⇔ (thead-строка) ИЛИ
  (стиль ячейки = Header, т.е. явная `h|` ИЛИ `h`-колонка `[cols="25h,..."]`). Обёртка
  `<p class="tableblock">` ⇔ НЕ thead. Т.е. header-ROW ячейка = `<th>текст</th>` БЕЗ обёртки;
  body `h`-ячейка = `<th><p class="tableblock">текст</p></th>` С обёрткой. Стиль ячейки побеждает
  стиль колонки. footer (`tsec==:foot`) с `h` → тоже `<th>` + обёртка.
- **Корень**: путь A эмиссии таблиц (`block.rs::scan_table`, 2 пути: A нативный `|===` / B
  `scan_delimited_format_table` csv/dsv) проверял `cell.style == CellStyle::Header`, но `cell.style`
  из спеки ЯЧЕЙКИ; `h` в `25h` — стиль КОЛОНКИ. `resolve_align` доносил от колонки только
  halign/valign, не стиль. Вдобавок маршрутизация `cell.style==Header → TableHeaderCell` латентно
  неверна для body `h|` (давала `<th>` БЕЗ обёртки).
- **Фикс** (2 файла):
  - `block.rs::scan_table` (~стр 1327): новый клоужер `resolve_style` — промоутит Default→Header
    ТОЛЬКО для `h`-колонки (стили `a/e/m/s/l` НЕ наследуются — отдельный риск, особенно `a`/AsciiDoc
    меняет обёртку). Маршрутизация в `emit_row_cells!` изменена: `if cell.style==Header ||
    $is_header_section` → `if $is_header_section` (thead→`TableHeaderCell` с `cell.style`),
    body/foot → `TableCell` с резолвнутым `style`.
  - `adoc-html/lib.rs`: `start_table_cell` (~1518) — `use_th = is_header || matches!(style, Header)`;
    обёртка `<p>` в body-ветке для Header уже шла через `_`-arm (1541). `TagEnd::TableCell` (~2300) —
    закрытие `</th>` если style==Header, иначе `</td>`.
  - +1 тест `test_table_header_column_style_html` (`[cols="1h,1"]`: col0→wrapped th, col1→td);
    ОБНОВЛЁН `test_table_cell_style_header_in_body_html` — кодировал НЕВЕРНОЕ старое (`<th>x</th>`
    без обёртки), теперь `<th><p class="tableblock">x</p></th>` + `<tbody>` (верифицировано пробой).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (html 313→315,
  parser 444, parsing_lab **233/233** — нет `h`-таблиц в фикстурах, маршрутизация body-Header не
  затронула ASG; html_output 35).
- Корпус `compare_full.py` (release): **Identical 180→182 (+2), Different 162, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `12065d0`): **7 файлов**
  изменили вывод — **2 FLIP→IDENTICAL** (width.adoc, spec/ROOT/paragraph.adoc), **0 регрессий**.
  5 changed-still-different УЛУЧШЕНЫ: число `<th>` теперь точно = asciidoctor (subs-group-table 7→12,
  image-position 3→6, strong-span 0→10, format-column-content 6→8, pass-macro body `h|`-ячейки
  верны 3/3). pass-macro позиц. счётчик +3 — АРТЕФАКТ выравнивания (semantically верно: `h|`-ячейки
  совпали с asciidoctor; файл Different из-за ненаследуемого `m`-стиля колонок, вне рамок фикса).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/table-header-column-style` (только по запросу).
- Смежный (НЕ flip в одиночку): наследование `m`/`e`/`s`/`a`/`l` стиля колонки на ячейки
  (`[cols="1m,3m"]`→`<code>`). `m`/`e`/`s` дали бы flip'ы, но `a`/AsciiDoc меняет обёртку (`{}`) и
  требует nested-парсинга содержимого ячейки → рискованно. Отложено сознательно.
- Чистые flip-кандидаты (near-miss на 182): counter.adoc (`{counter:index}`→`{index}` не резолвится —
  препроцессор пишет в локальную мапу, рендерер берёт из document_attrs; архитектурный мостик), 1-diff
  (рискованные): verse `// end::para[]`, kbd `+`-разделитель, listing-blocks `[subs="+attributes"]`.
  Архитектурные (отложены): inline-anchor reftext из dt-терма (lexicon), nested-форматирование в
  ТЕКСТЕ ссылки (QUOTES в `[label]`), inline-monospace passthrough char-ref (`Event::Code`),
  link-role `class="external"`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `12065d0`). near-miss `/tmp/nearmiss.py` (вывод в
  `/tmp/nearmiss_out.txt`). Сравнение семантическое (DOM); позиц. счётчик может «врать» при сдвиге
  (см. pass-macro). LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-11) — Фаза 3: `{attr-ref}[text]` как ссылка (subs-order)

`fix/paragraph-trailing-whitespace` УЖЕ смержена+запушена в master (`e41ad48`, origin == master,
дерево чистое; session.md прошлой сессии писалась ДО мержа — как всегда). `/tmp/adoc_base` пересобран
из master `e41ad48` ПЕРЕД правкой. Baseline подтверждён near-miss: Identical 175, Different 169, Errors 0.

Выбор кандидата по near-miss на 175: 1-diff (verse `// end::para[]`, kbd `+`, listing-blocks subs)
рискованны/архитектурны. Самый крупный доступный кластер — **4 файла по 3-diff с ОДНИМ корнем**
(`index/icons-font/auto-ids/custom-ids`): паттерн `{url-xxx}[text^]`. Откладывался ~6 сессий как
«архитектурный subs-order», но решился ЧИСТО через combine-and-reparse (проба asciidoctor подтвердила
семантику: раскрыть атрибут → склеить со скобкой → переразобрать как макрос; не-URL → литерал).

### Ветка `fix/attr-ref-link-macro` (от master; СТАТУС: НЕ закоммичено)
- **Правило Asciidoctor** (верифицировано пробой, НЕ по памяти): attributes-sub идёт ДО macros-sub.
  `{url-x}[the page^]` где `:url-x: https://…` → атрибут раскрывается в URL, затем `URL[the page^]`
  распознаётся как URL-макрос → `<a href=URL target=_blank rel=noopener>the page</a>` (НЕ bare, текст
  из скобок, `^`→blank-window). `{nm}[bracket]` где nm=John (не-URL) → `John[bracket]` литерал.
  `{undef}[bracket]` (forward/undefined) → `{undef}[bracket]` (оба сохранены).
- **Корень**: парсер эмитил `Event::AttributeReference` и оставлял `[text^]` отдельным `Text`;
  рендерер раскрывал атрибут через `render_inline_value`, который переразбирал URL В ИЗОЛЯЦИИ →
  bare-autolink, а `[text^]` доезжал литералом.
- **Фикс** (4 файла, combine-and-reparse):
  - `event.rs`: `Event::AttributeReference` + поле `trailing_brackets: Option<CowStr>` (+ into_static).
  - `inline.rs::try_attribute_reference`: захват `[...]` СРАЗУ после `}` (локальная копия `self.input`
    для lifetime; `tail.starts_with('[') && !"[["`; первый `]`; без пробела/без `]` → None). `pos`
    сдвигается за скобки.
  - `adoc-html/lib.rs` arm `AttributeReference`: при резолве из `document_attrs` → `format!("{value}{br}")`
    + `render_inline_value` (URL→ссылка, не-URL→тот же литерал). В ветках intrinsic/env/fallback/missing-skip
    скобки дописываются `html_escape_text` (были отдельным Text до фикса).
  - `adoc-compat-tests/builder.rs` arm: `resolved.push_str(&br)` — ASG-слой сохраняет скобки как текст
    после резолва (в parsing-lab инлайн-`{attr}[...]` НЕТ — единственный `}[` это block-`image::{target}[]`,
    идёт мимо inline; 233/233 целы).
  - +2 теста: `test_attribute_reference_captures_trailing_brackets` (inline: capture/`[[`-skip/space-skip/
    no-close), `test_attribute_reference_link_target` (html: url→link+blank-window, non-url→литерал, undef→оба).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 443→444, html 313→314,
  parsing_lab 233/233 целы, остальные без изменений).
- Корпус `compare_full.py` (release): **Identical 175→180 (+5), Different 164, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `e41ad48`): **17 файлов** изменили
  вывод — **5 FLIP→IDENTICAL** (CONTRIBUTING, index, icons-font, auto-ids, custom-ids), **0 регрессий**.
  12 changed-still-different: число позиционных diff'ов улучшено/без изменений (links 235→232,
  replacements 155→148, index 20→14/22→6, image-size лучше); **audio-and-video 766→796 ВЫРОС — но это
  АРТЕФАКТ выравнивания**: фикс семантически верен (`<a target=_blank rel=noopener>`mono`</a>` вместо
  bare+leftover), в base leftover-bracket случайно содержал `<code>` токены, позиционно совпадавшие с
  ожидаемым asciidoctor. Остаток — nested-mono в тексте ссылки (backticks не → `<code>`, текст ссылки
  проходит REPLACEMENTS но не QUOTES — предсуществующее, см. baseline).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/attr-ref-link-macro` (только по запросу).
- Следующие чистые flip-кандидаты (near-miss на 180): counter.adoc (`{counter:index}` не пишет в attrs →
  `{index}` не резолвится; п.36 — 2-diff один корень), width.adoc (`th` vs `td` header-row из include +
  autowidth `~`; 2-diff), 1-diff (рискованные): verse `// end::para[]`, kbd `+`-разделитель, listing-blocks
  `[subs="+attributes"]`. Архитектурные (отложены): nested-форматирование в ТЕКСТЕ ссылки (QUOTES в
  `[label]` — флипнул бы audio-and-video/links/replacements; текст ссылки сейчас только REPLACEMENTS через
  `push_macro_label`), inline-monospace passthrough char-ref (`Event::Code`), link-role `class="external"`.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `e41ad48`). near-miss `/tmp/nearmiss.py` (вывод в
  `/tmp/nearmiss_out.txt`). Сравнение семантическое (DOM); позиц. счётчик может «врать» при сдвиге.
  LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-10) — Фаза 3: trailing whitespace строк параграфа

`fix/table-caption-doc-attr` УЖЕ смержена в master (`759771c`, origin == master, дерево чистое;
session.md прошлой сессии писалась ДО мержа). `/tmp/adoc_base` пересобран из master `759771c`
ПЕРЕД правкой (был stale c37bcf6). Baseline подтверждён: Identical 173, Different 171, Errors 0.

Кандидат из плана (merge attr-блоков) при разведке распался на ДВА независимых бага, ни один не
даёт чистый флип: (1) `cols="N*"` repeat-синтаксис не парсится в colgroup рендерера
(`parse_col_widths`, adoc-html:2391 — trailing-digits от `"3*"` пусто → 1 col); НО compare_full.py
ИГНОРИРУЕТ `style`-атрибут (строки 15/29-31) → ширины невидимы, важно только КОЛИЧЕСТВО `<col>`;
файлы с `N*` (align-by-column и др.) Different по куче др. причин (`pass:q` макрос, softbreak) —
0 флипов. (2) merge attr-блоков (block.rs:480 безусловное присваивание `pending_block_attrs`) —
тоже не флипает customize-title-label (Antora-include не резолвится). ОБА отложены.

Вместо них выбран чистый кластер по near-miss: trailing-whitespace перед softbreak (_responses +
http-api-design, по 2 diff, ОДИН корень → 2 флипа).

### Ветка `fix/paragraph-trailing-whitespace` (от master; НЕ закоммичено)
- **Правило** (верифицировано пробами): Asciidoctor rstrip'ит КАЖДУЮ исходную строку — trailing
  spaces/tabs не доходят до HTML (и перед softbreak в многострочном параграфе, и в listing/verse,
  и на однострочном `==  `→`==`). Hard-break ` +` сохраняется.
- **Слой КРИТИЧЕН**: ASG (parsing-lab, 233 кейса) СОХРАНЯЕТ trailing-ws в Text-узле (`Text("==  ")`,
  `Text("*  ")` — block/section + block/list isolated-marker). compat-тест (parsing_lab.rs:217)
  читает события `Parser` НАПРЯМУЮ → правка в block.rs/parser.rs ломает 2 ASG-кейса (проверено:
  первая попытка `para_lines.push(line.trim_end())` в block.rs дала 231/233). Обрезка — концерн
  ТОЛЬКО HTML-рендеринга (adoc-html ASG-тест не использует).
- **Корень потока**: parser.rs (104-161) при многострочном параграфе склеивает Text+SoftBreak+Text
  в ОДНУ строку с встроенными `\n` и кормит InlineParser → `\n` остаётся ВНУТРИ `Event::Text`,
  SoftBreak-arm рендерера НЕ вызывается. Verbatim (source/listing, без inline-парсинга) — раздельные
  Text+SoftBreak.
- **Фикс** (`adoc-html/lib.rs`, только этот файл, +61/-2): (1) хелпер `rstrip_line_trailing_ws`
  (рядом с `html_escape_text`, ~3186): CowStr, borrow без аллокации если нет ws-перед-`\n`; через
  `split_inclusive('\n')` дропает spaces/tabs перед каждым `\n`, последний сегмент без `\n` НЕ
  трогает (mid-line перед inline-элементом). Применён в Text-arm (496-512) к обеим веткам
  (`html_escape_text` + `push_str`). (2) В SoftBreak-arm (526) перед `</span>`/`\n` —
  `trim_end_matches([' ','\t'])` (для verbatim). +2 html-теста.

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (html 311→313,
  parser 443, parsing_lab 233/233 — ASG сохранён!, html_output 35).
- Корпус `compare_full.py` (release): **Identical 173→175 (+2), Different 169, Errors 0**.
- Blast radius (`/tmp/blast.py`, base `/tmp/adoc_base` = чистый master `759771c`): **6 файлов**
  изменили вывод — **2 FLIP→IDENTICAL** (_responses, http-api-design), **0 регрессий**. 4 (sdr-004,
  db-migration, cookbook java/index + root) улучшены (−2 diff каждый), Different по др. причинам.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/paragraph-trailing-whitespace` (только по запросу).
- Остаток ЭТОГО фикса (мелкий, НЕ нужен для флипов): trailing-ws на ПОСЛЕДНЕЙ строке verbatim-блока
  (перед `</pre>`, без `\n`) не обрезается — требует lookahead на End(DelimitedBlock).
- Отложенные в этой сессии (реальные баги, но не чистые флипы в одиночку):
  - **`cols="N*"` в colgroup рендерера** (`adoc-html parse_col_widths`): repeat-multiplier не
    раскрывается → 1 col вместо N. Парсер (`parse_col_spec`/`table_col_specs`) `N*` УМЕЕТ — баг
    только в рендерере. Затрагивает align-by-column, striping, data-format и др. (все Different по
    др. причинам тоже). NB: compare_full игнорит `style`, важно КОЛИЧЕСТВО `<col>`.
  - **merge attr-блоков** (`block.rs:480`): `[caption=]`+`.title`+`[cols=]` → второй `[...]` затирает
    первый (`pending_block_attrs = Some(...)` безусловно). Asciidoctor мёржит. Затрагивает
    customize-title-label (но там ещё Antora-include не резолвится → не флипнет).
- Прочие near-miss на 175 (1-diff, рискованные): verse `// end::para[]` (comment-handling
  блок-сканера в verbatim), keyboard-macro kbd `+`-разделитель (passthrough `+...+`),
  listing-blocks `[subs="+attributes"]`. 2-diff: counter.adoc (`{counter:index}` не пишет в attrs),
  width.adoc (`th` vs `td` header-row в include + autowidth `~`).
- Архитектурные (отложены): inline-anchor reftext из dt-терма (lexicon), `{attr-ref}[text]` (subs-
  порядок), link-role `class="external"`, inline-monospace passthrough char-ref (`Event::Code`).

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). blast: `/tmp/blast.py`
  (base `/tmp/adoc_base` = чистый master `759771c`). near-miss `/tmp/nearmiss.py` (вывод в
  `/tmp/nearmiss_out.txt`; capped по числу diff). Сравнение семантическое (DOM), `style`-атрибут
  ИГНОРИТСЯ. LSP для навигации, context7 MCP.

---

## Сессия (2026-06-09, поздняя-9) — Фаза 3: `table-caption` document-атрибут

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
