# TODO — adoc-parser

Roadmap по итогам архитектурного аудита. Источник задач совместимости — `COMPAT-DIFF.md`
(числа в скобках — затронутые файлы корпуса `/mnt/c/tmp/adoc-test/`, 344 шт.).

Перед каждым коммитом: `cargo clippy --workspace` (0 warnings) + `cargo test --workspace`
(всё зелёное). Никогда не коммитить прямо в master — сначала ветка (см. CLAUDE.md).

---

## Аудит рендерера 2026-06-09 (мульти-рендерер + дедупликация; находки верифицированы)

Сделано в ветке `fix/block-image-figure-caption` (2026-06-10, НЕ закоммичено): R1, R2, R4, R6,
R3/R5 частично + бонус-фикс stem-title (тот же класс, что R1/R2). Детали в session.md.
Корпус: Identical 204 (без флипов), 0 регрессий; улучшения: video.adoc 47→4 diff,
image.adoc 135→128, id.adoc 49→45. clippy 0, test --workspace зелёное (parser 461, html 328).

- [x] **R1 (БАГ)**: `.Title` на block-image терялся из imageblock и УТЕКАЛ в следующий блок.
  Теперь: `<div class="title">Figure N. Title</div>` ПОСЛЕ content, счётчик `figure_counter`
  (bump только titled), `figure-caption` в дефолтных document_attrs («Figure»; `:figure-caption!:`
  → без префикса, `:figure-caption: X` → кастомный label). Хелпер `push_caption_prefix`
  (общий с table-caption — у таблиц логика уже была, переведена на хелпер). ПАРСЕР:
  `parse_image_attrs` извлекает `caption=`/`title=` (caption — verbatim-префикс без bump;
  title= создаёт BlockTitle и ПОБЕЖДАЕТ `.Title` — верифицировано пробой); alt-fallback при
  named-only скобках теперь "" → авто-alt из имени файла (был сырой bracket_content).
  +2 теста (html figure-caption сценарии; parser caption/title/alt-fallback).
- [x] **R2**: `Tag::BlockVideo` не эмитил `.Title` — починено через `open_block_with_title`
  (title ДО content, зеркало audio; верифицировано пробой). video.adoc 47→4 diff.
  **Бонус**: `Tag::StemBlock` имел ТОТ ЖЕ баг (title терялся+утекал) — починен там же,
  проба p8 IDENTICAL. +1 тест (video+stem title + leak-guard).
- [~] **R3 (системный корень)**: введён хелпер `open_block_with_title(output, meta, class)`
  (wrapper+title+content-div), применён к audio/video/stem/openblock. НЕ покрыты (другая
  форма): image (title ПОСЛЕ content), sidebar (title ВНУТРИ content), quote/verse
  (blockquote/pre вместо content-div), example (details/summary). Новые block-arm'ы писать
  через хелпер.
- [x] **R4**: `#t=start,end` → общий `push_media_time_fragment` (audio/video). Boolean-атрибуты
  оставлены раздельно — порядок НАМЕРЕННО разный (video: controls,autoplay,loop;
  audio: autoplay,loop,controls — оба соответствуют asciidoctor).
- [x] **R5**: ЗАВЕРШЕНО (ветка `refactor/finish-single-pass-resolution`, 2026-06-10).
  (1) `ResolutionContext<'a>` — единые lookup'ы `finish()`, строятся ОДИН раз из всех
  реестров (toc_entries/block_ref_titles/bibliography_reftexts): `id_to_text`
  (`CowStr` — секции экранируются, block/biblio-HTML заимствуется; членство ключей =
  бывший `known_ids`) + `title_to_id` (natural xref); методы `link_text`/`href_id`
  кодируют precedence asciidoctor. (2) Квадратичный `output.replace(placeholder,…)`
  на каждый плейсхолдер → map плейсхолдер→замена + ОДИН проход `resolve_sentinels_into`
  по `\x00`-сентинелям (вложенные сентинели в заменах — xref внутри заголовка блока —
  резолвятся рекурсивно, depth cap 8; ранее ТАКОЙ кейс ТЁК сырым `\x00XREF_N\x00` в
  вывод — попутный багфикс, +1 тест). Стресс 2000 xref: 807ms→33ms (~24×).
  Рефакторинг-нейтральность: raw-вывод байт-в-байт на всех 344 файлах корпуса.
- [x] **R6**: хелперы `open_li_paragraph`/`close_li_paragraph`; 3 arm'а ListItem схлопнуты
  в один (match по checked), CalloutListItem/TagEnd-обработчики на хелперах.
  DescriptionDescription НЕ тронут (асимметричный rollback-механизм dd_output_start).
- [x] **R7 (АРХИТЕКТУРА — подготовка ко 2-му рендереру)**: ЭТАП 1 СДЕЛАН (ветка
  `refactor/render-core-attr-resolver`, в master `cf39bb1`): создан крейт
  **`adoc-render-core`** (zero-dep, workspace-member) — единая таблица интринсиков
  (`IntrinsicAttribute { name, text, html }`: `text` — семантическое значение, `html` —
  байт-в-байт форма asciidoctor; обе колонки — данные, кодировка НЕ выводима правилом:
  `plus`→`&#43;`, но `cpp`→литеральный `C++`), `resolve_attribute_reference()` (полный
  precedence doc→intrinsic→env-*→fallback→attribute-missing, generic через closure-lookups)
  и `resolve_attr_refs_text()` (eager `{name}`-резолв в строках). adoc-html и
  adoc-compat-tests/builder.rs переведены, локальные копии удалены. Попутно закрыт ДРЕЙФ:
  в builder.rs отсутствовали `apos`/`pp`/`quot` — теперь таблица общая. Корпус байт-в-байт
  (344/344, 0 diffs), parsing-lab 233/233, clippy 0, +4 юнит-теста core.
  ЭТАП 2 СДЕЛАН (ветка `refactor/render-core-xref-resolver`, в master `280e0ce`):
  **XrefResolver** вынесен в core — `RefText::{Plain,Markup}` (решение проблемы html_escape:
  секции хранятся Plain и экранируются потребителем, заголовки блоков/библиография — Markup
  verbatim), `XrefResolver` (add_section/add_block, link_text/href_id с precedence
  asciidoctor: known-id → natural xref по заголовку секции, case-sensitive; last-wins для
  id секций, or_insert для блоков), `unresolved_xref_label` (`[target]`),
  `is_interdoc_xref_target`, `interdoc_xref_href` (.adoc→.html с сохранением #fragment;
  = auto-text). `ResolutionContext` удалён из adoc-html; сентинель-машинерия
  (`resolve_sentinels_into`, плейсхолдеры) осталась в рендерере — это механика отложенного
  резолва HTML-вывода, не семантика. Корпус байт-в-байт (344/344), parsing-lab 233/233,
  clippy 0, +2 юнит-теста core (всего 6).
  ЭТАП 3 СДЕЛАН (ветка `refactor/render-core-section-toc`, в master `86d8685`):
  **SectionNumberer + TocBuilder** вынесены в core. `TocEntry` (pub-поля level/id/title;
  бывший приватный тип рендерера), `TocBuilder` (push/entries + `toc_steps(toc_levels)
  -> Vec<TocStep>` — структурная раскладка дерева TOC: EnterLevel/Item/CloseItem/LeaveLevel,
  фильтрация уровней 2..=toc_levels+1, пустой результат = «TOC не эмитить вообще»),
  `DEFAULT_TOC_TITLE`, `SectionNumberer` (`number_prefix(level)` — счётчики sectnums
  «1.2.3. » с инкрементом+сбросом глубже, None вне 2..=5; `appendix_caption()` —
  «Appendix A: »). Гейтинг (`sectnums`-флаг, подавление caption'ом спец-секций) и вся
  HTML-механика (div/ul/li, sectlevel-классы, html_escape, insert-позиция, toc2-классы
  body, newline-guard) остались в рендерере; generate_toc — теперь map TocStep→HTML.
  Поля toc_entries/section_counters/appendix_counter удалены из HtmlRenderer. Корпус
  байт-в-байт (344/344, 0 diffs), parsing-lab 233/233, clippy 0, +2 юнит-теста core (всего 8).
  ЭТАП 4 СДЕЛАН (ветка `refactor/render-core-captions`, в master `de4decd`):
  **CaptionCounters + FootnoteRegistry** вынесены в core. `CaptionKind`
  (Figure/Table/Example), `CaptionPrefix::{None,Custom,Numbered}` (plain-текст, потребитель
  экранирует/форматирует «Label N. »), `CaptionCounters::caption_prefix(kind, caption_attr,
  doc_label)` — правило выбора префикса: `caption=""` подавляет, `caption=X` verbatim,
  иначе нумерованный при наличии doc_label; bump-семантика по kind: figure/table бампят
  ТОЛЬКО при Numbered, example — на КАЖДЫЙ titled-блок (даже под caption=-override) —
  зеркало старого кода рендерера. `FootnoteRegistry` (define → номер в document-order +
  реестр named id last-wins; lookup; footnotes() для финальной секции; text — plain,
  экранирует потребитель). adoc-html: удалены поля figure/table/example_counter,
  footnotes/footnote_counter/named_footnotes; `push_caption_prefix` стал методом поверх
  core (example-arm переведён на него — было inline-дублирование), footnote-arms и
  render_footnotes на FootnoteRegistry. Откуда берётся label (document_attrs
  `figure-caption`/`table-caption`, хардкод «Example») — осталось в рендерере. Корпус
  байт-в-байт (344/344, 0 diffs), parsing-lab 233/233, clippy 0, +2 юнит-теста core (всего 10).
  ЭТАП 5 СДЕЛАН (ветка `refactor/render-core-author-revision`, 2026-06-10; НЕ закоммичено):
  **Author/AuthorRegistry + Revision** вынесены в core. `Author` (6 pub-полей plain-текстом),
  `AuthorRegistry`: `add(author) -> Vec<(String,String)>` — document-attribute-entries с
  suffix-правилом (первый автор без суффикса: `author`/`email`; дальше `2`/`3`…;
  `middlename`/`email` только non-empty), `attr_suffix(index)`, `authors()`, `is_empty()`.
  `Revision { version, date, remark }`: `attr_entries()` (revnumber/revdate/revremark,
  пустые компоненты — ничего), `display_version()` (strip одного ведущего `v`/`V`).
  ГРАНИЦА со scanner.rs проверена, корректна: parse_authors/parse_revision_line (включая
  revnumber-strip нецифрового префикса) — парсинг строки заголовка, остаются в ПАРСЕРЕ;
  display_version в core — рендер-семантика для explicit-строк (revision-line приходит уже
  стрипнутой — там strip no-op). adoc-html: удалены AuthorData/RevisionData, arms
  Event::Author/Revision и render_author_details на core-типах; details-div HTML (span'ы,
  mailto-ссылка, формат «version X,») остался в рендерере. Корпус байт-в-байт (344/344,
  0 diffs), parsing-lab 233/233, clippy 0, +2 юнит-теста core (всего 12).
  **R7 ЗАВЕРШЁН** — вынесенная семантика: attr-refs, xref, section-numbering/TOC,
  captions, footnotes, author/revision. Уже хорошо разделено (не трогать): subs — в парсере
  (inline.rs), table-grid (colspan/rowspan) — в парсере (block.rs).
- [x] **R8 (структура)**: СДЕЛАНО (ветка `refactor/html-modules`, 2026-06-10).
  adoc-html/src/lib.rs (6220 строк) распилен на модули: **lib.rs** (417 — API, BlockMeta/
  DlistStyle, struct HtmlRenderer, core-методы new/run/apply_attribute/render_inline_value),
  **events.rs** (1009 — диспетчеры push_event/start_tag/end_tag), **blocks.rs** (861 —
  start_* блоков/таблиц/списков/секций, caption/meta-хелперы, parse_manpage_title,
  section_level_to_h), **inline.rs** (251 — xref, kbd/btn/menu, icon, stem),
  **media.rs** (342 — image/video/audio, MediaAttrs), **finish.rs** (317 — finish/TOC/
  footnotes/author-details/document head+tail, resolve_sentinels_into, консты
  DEFAULT_STYLESHEET/MATHJAX_DOCINFO), **escape.rs** (84 — html_escape*/write_attr/
  push_hardbreaks_text/rstrip_line_trailing_ws), **tests.rs** (2972 — бывший mod tests).
  Методы/функции в дочерних модулях — `pub(crate)`; модули видят приватные элементы корня
  через `use crate::*` (потомки корня). Код перенесён байт-в-байт (проверено diff'ом
  мультимножеств строк); попутно doc-комментарий rstrip_line_trailing_ws отлеплен от
  push_hardbreaks_text (был прилипший — pre-existing). Корпус байт-в-байт (344/344,
  0 diffs, 0 exit-diffs), Identical 204/Different 140/Errors 0 (= baseline), clippy 0,
  test --workspace зелёное.
- [x] **R9 (wart)**: СДЕЛАНО (ветка `refactor/inline-doc-attrs-channel`, 2026-06-11).
  Ad-hoc `Parser.experimental: bool` заменён общим каналом document-attrs → inline-парсер:
  pub-тип **`InlineOptions`** (adoc-parser/inline.rs, реэкспорт из lib.rs; Copy/Default/Eq,
  поле `experimental`) с ДВУМЯ путями заполнения: streaming `apply_attribute(name)` — имя как
  в `Event::Attribute`, unset-формы `!name`/`name!` нормализуются генерически (Parser зовёт
  в arm'е Event::Attribute — mid-document семантика сохранена); snapshot
  `from_attr_lookup(is_set)` — для рендереров поверх таблицы атрибутов (adoc-html
  `render_inline_value`). API: `parse_str_with_subs_options(text, subs, options)`;
  `parse_str_with_subs_experimental` удалён (все 3 потребителя мигрированы),
  `parse_str_with_subs` = wrapper с Default. `InlineState.experimental` → `options:
  InlineOptions` (5 inner-reparse наследуют целиком). Новые attr-гейтящие фичи inline-парсинга
  = поле в InlineOptions + arm в обоих конструкторах (задокументировано doc-комментом).
  +1 тест (set/unset-формы, snapshot, чужие атрибуты игнорируются). Рефакторинг-нейтральность:
  байт-в-байт vs master `1fbbde4` на всех 344 файлах корпуса (0 diffs, 0 exit-diffs);
  Identical 204/Different 140/Errors 0 (= baseline); clippy 0, test --workspace зелёное
  (parser 461→462). **АУДИТ РЕНДЕРЕРА R1–R9 ЗАКРЫТ** (R3 — частично by design:
  новые block-arm'ы писать через `open_block_with_title`).

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

## Свежий baseline корпуса (2026-06-09, ПОСЛЕ stem-mathjax-docinfo)

`/mnt/c/tmp/adoc-test/` 344 файла: **Identical 204, Different 140, Errors 0** (после
stem-mathjax-docinfo; `python3 compare_full.py`, release-бинарь). **COMPAT-DIFF.md
устарел** (числа от 2026-03-23).
Следующие near-miss-кандидаты (на 204): **pass/index** (6-diff — остаток `` `+pass:[]+` `` через
single-plus, «случай A»: асимметричный — pass извлекается ДО `+...+`, но НЕ внутри `++...++`; риск),
**special-section-numbers** (10-diff, monospace в ТЕКСТЕ xref-ссылки — архитектурный QUOTES в `[label]`),
**callout** (20-diff, verbatim callout), **part** (22-diff, len_delta=0 — sections part).
**stem/index ЗАКРЫТ** (ветка `fix/stem-mathjax-docinfo`, НЕ закоммичена): при `:stem:` (любое значение,
независимо от наличия stem-контента) asciidoctor инъектит ФИКСИРОВАННЫЙ блок MathJax (`<script
type="text/x-mathjax-config">`+CDN-loader `mathjax/2.7.9`) перед `</body>`. Блок одинаков для
asciimath/latexmath (верифицировано пробами P1==P2). Фикс (1 точка, РЕНДЕРЕР `adoc-html/lib.rs`): const
`MATHJAX_DOCINFO` (raw-строка, байт-в-байт) + 4-строчная вставка в `write_document_tail` под
`document_attrs.contains_key("stem")` (после `docinfo_footer`, перед `</body>`). `:!stem:` снимает ключ
→ нет вставки (matches asciidoctor). **Остаток (НЕ в корпусе)**: `eqnums` атрибут изменил бы
`autoNumber` (хардкод `"none"` — дефолт).
**docinfo/index ЗАКРЫТ** (ветка `fix/rowspan-row-placement`, НЕ закоммичена): `build_table_rows`
(`adoc-parser/block.rs`) двойным декрементом терял occupancy rowspan-колонки → ячейка следующей строки
ошибочно занимала спанированную колонку (`.2+|X` → `2` уезжала к `Y` вместо отдельной строки). Корень:
при старте новой строки «decrement all» цикл уменьшал счётчик ДО skip-цикла, который повторно декрементил
при пропуске. Фикс — убрать «decrement all» (skip-цикл сам декрементит каждую occupied-колонку ровно раз
за строку). **Остаток (НЕ нужен для флипа)**: пасс `emit_row_cells` col_idx (выравнивание/стиль) не
учитывает rowspan-сдвиг — для docinfo все колонки `<` (left), нюанс латентен.
**db-migration + localization ЗАКРЫТЫ** (ветка `fix/callout-item-block-and-shifted-source-lang`, НЕ
закоммичена; 2 корня: (1) NOTE/continuation-блок внутри callout-элемента не закрывал принципиальный
`<p>` — рендерер; (2) ведущий named/shorthand-атрибут сдвигает позиционные слоты, `[id=app, source, yaml]`
→ язык `source` а не `yaml` — парсер). Архитектурные кластеры (много файлов): **наследование `m`/`e`/`s` стиля колонки таблицы**
(`<code>`/`<em>`/`<strong>` в ячейках — сделано только `h`; character-replacement-ref, pass-macro,
subs-group-table завязаны + footnote `<sup>`/`stretch`-каскады), **author-header `<div class="details">`**
(standalone). **Известный родственный баг**: arm `Tag::BlockVideo` имеет тот же title-баг, что был
исправлен в audio (не зовёт `emit_pending_block_title`) — но video далеко от флипа (47 diff).
**toc/index.adoc ЗАКРЫТ** (ветка `fix/literal-paragraph-block-title`, НЕ закоммичена): `.Title`
перед ОТСТУПНЫМ literal-параграфом терялся (рендерер `adoc-html/lib.rs`, inline-arm
`Tag::LiteralParagraph` не звал `emit_pending_block_title` — в отличие от delimited literal `....`).
Следующие near-miss-кандидаты (на 193): **pass/index** (6-diff, `` `+pass:[]+` ``→пустой `<code>`,
фидли), **stem/index** (6-diff, MathJax — архитектурный), **special-section-numbers** (10-diff,
monospace в ТЕКСТЕ xref-ссылки — архитектурный nested-форматирование в `[label]`),
**reference-revision-line**/**revision-line-with-version-prefix** (11-13 diff — парсинг
revision-строки: `{revnumber}` не снимает префикс `v`/`LPR`, version-label localization,
`[%hardbreaks]`; 2-3 корня).
**counter.adoc ЗАКРЫТ** (ветка `fix/counter-bare-reference`, НЕ закоммичена): голый `{name}` на
счётчик резолвится в препроцессоре в document-order (не «архитектурный» — рендерер с плоским
снимком в принципе не мог, но препроцессор идёт построчно и знает текущее значение; п.36 закрыт
для прозы/таблиц, остаётся counters.adoc — счётчики в verbatim-блоках, нет block-context awareness).
Методика выбора кластера: `/tmp/nearmiss.py` ранжирует Different-файлы по числу позиционных
diff'ов (переиспользует нормализацию compare_full) — берём фиксы, у которых файл «1 diff away».
Следующие чистые flip-кандидаты (по near-miss на 192): **pass/index.adoc** (6-diff,
`` `+pass:[]+` ``→пустой `<code>`), **stem/index.adoc** (6-diff, MathJax — архитектурный).
Прежний 4-diff
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

- [x] **sect0-heading standalone + admonition image-иконки при `:icons:`** — СДЕЛАНО (ветка
  `fix/callout-rendering`, 2026-06-11; НЕ закоммичено). Два корня callout.adoc (20 diff),
  оба — только РЕНДЕРЕР. (1) **sect0**: level-0 секция в теле документа (article И book) у
  asciidoctor — голый `<h1 id class="sect0">` БЕЗ обёртки-div и БЕЗ sectionbody; контент
  после — на верхнем уровне (проба p_sect0). У нас article-ветка оборачивала в
  `<div class="sect0">`. Фикс: `start_section_div` — условие `is_book()` снято
  (book-part-механизм обобщён), `book_part_stack` → `sect0_stack`, неиспользуемый `is_book()`
  удалён. (2) **`:icons:` image-ветка** (`blocks.rs::start_admonition`): любое значение
  `icons`, кроме `font`, → `<img src="{iconsdir|./images/icons}/{name}.{icontype|png}"
  alt="{label}">` (html5.rb:435-443; `:icons: font <1>` mid-document ≠ `font` → image,
  проба подтверждена). НЕ реализовано: init-нормализация header-значений
  (document.rb:1207: `:icons: svg` в header → icons='', icontype='svg' — в корпусе нет;
  mid-document её НЕТ и у asciidoctor), colist-таблица и image-маркеры callout при
  `:icons:` (см. новую задачу ниже). +2 теста (`test_admonition_icons_image`,
  `test_sect0_heading_standalone`). **Корпус: Identical 208→210 (+2)** (callout.adoc,
  paragraph.adoc); blast 22 файла: 2 флипа, **0 регрессий**, сильные улучшения
  changed-still-different: abstract-block 61→5, preface 82→7, url 142→9, strong-span →5;
  лёгкий рост счётчика part 22→24 / header 63→69 — позиционный шум поверх доминирующего
  pre-existing корня «header после ведущего комментария» (п.41). clippy 0,
  test --workspace зелёное (html 329→331, всего 885).

- [ ] **`:icons:`-машинерия для callout-списков** (icons-image.adoc, icons-font.adoc,
  icons.adoc, admonitions.adoc): при `icons=font` inline-маркер `<i class="conum"
  data-value="n"></i><b>(n)</b>` — уже есть; при `icons` (не-font) маркер —
  `<img src="{iconsdir}/callouts/{n}.{icontype}" alt="{n}">` (html5.rb:1159-1169), а
  colist рендерится ТАБЛИЦЕЙ `<table><tr><td>{иконка}</td><td>{текст}</td>` вместо
  `<ol>` (html5.rb:476-513). Рендерер-only.

- [x] **QUOTES/ATTRIBUTES-форматирование в тексте метки link/xref/mailto** — СДЕЛАНО (ветка
  `fix/macro-label-inline-formatting`, 2026-06-11; НЕ закоммичено). Закрыт архитектурный
  кластер «nested-форматирование в тексте ссылки». Asciidoctor прогоняет QUOTES, ATTRIBUTES
  и REPLACEMENTS ДО macros-пасса, поэтому `` xref:t[see `x`] `` → `<a>see <code>x</code></a>`,
  `*b*`/`_i_`/`{attr}` в метке тоже преобразованы; макрос, поглощённый меткой
  (`xref:t[see <<other>>]`), повторно НЕ сканируется — литерал (верифицировано пробами
  /tmp/p_label.adoc, /tmp/p_label2.adoc: 7/8 кейсов байт-в-байт). Фикс — только ПАРСЕР,
  1 точка (`inline.rs::push_macro_label`): вместо одного Text с replacements — inner-reparse
  метки с `subs.without(MACROS)` (стандартный механизм InlineState::new, как у quote-спанов).
  Рендереру ничего не нужно: labeled-xref/link рендерят вложенные события как есть.
  +1 тест `test_macro_label_quotes_formatting` (5 кейсов: mono в xref, bold+italic в link,
  shorthand `<<id,label>>`, `{attr}`-ref, guard внутреннего макроса). **Остаток
  (pre-existing, НЕ метко-специфичен)**: экранированный `` \` `` в метке — asciidoctor
  сохраняет второй backslash, мы съедаем оба (нюанс escape-обработки quote-спанов).
  **Корпус: Identical 206→208 (+2)** (special-section-numbers, link-macro-attribute-parsing);
  blast 4 файла: 2 флипа, **0 регрессий**, 2 changed-still-different улучшены
  (url.adoc 192→142, audio-and-video 796→457). clippy 0, test --workspace зелёное
  (parser 463→464, всего 883).

- [x] **`pass:[…]` внутри single-plus passthrough (случай A)** — СДЕЛАНО (ветка
  `fix/pass-macro-in-single-plus`, 2026-06-11; НЕ закоммичено). Asciidoctor извлекает
  `pass:[…]` ДО матчинга `+…+` span'а (первый regex-пасс substitutor'а), поэтому
  `+pass:[x]+`→`x`, `` `+pass:[]+` ``→`<code></code>`; внутри `++…++`/`+++…+++` макрос
  НЕ извлекается (double/triple-plus побеждает позиционно в том же пассе) —
  `` `++pass:[y]++` ``→`<code>pass:[y]</code>` (у нас уже было верно). Дискриминатор
  `` `+pass:[]+more+` ``→`<code>+more</code>` подтверждён пробой. Фикс — только ПАРСЕР
  (`inline.rs::try_single_plus_passthrough`): (1) close-скан пропускает регион `pass:[…]`
  (переиспользован `pass_macro_span_len`, зеркало `find_closing_constrained`); (2) эмиссия
  контента через новый `push_single_plus_content` — литеральный Text с извлечением
  `pass:[…]`→`InlinePassthrough` (вместо одного сырого Text). +1 тест
  `test_pass_macro_inside_single_plus` (5 вариантов, вкл. дискриминатор и `++…++`-guard).
  **Корпус: Identical 205→206 (+1)** (pass/index.adoc); blast ровно 1 файл: 1 флип,
  **0 регрессий**, 0 changed-still-different. clippy 0, test --workspace зелёное
  (parser 462→463).

- [x] **YouTube-плейлисты в video-макросе** — СДЕЛАНО (ветка `fix/youtube-playlist-params`,
  2026-06-11; НЕ закоммичено). Остаток video.adoc после R2 (4 diff). Asciidoctor
  (html5.rb:1049-1093, верифицировано пробами через файл): target `video_id/list_id` несёт
  плейлист (приоритет над атрибутом `list=`) → `&list=`; иначе target `id1,id2,...` —
  динамический плейлист (приоритет над атрибутом `playlist=`) → `&playlist=` с **ID видео,
  подставленным в начало** (`{target},{playlist}`); голый `loop` без плейлиста →
  `&playlist={video_id}` (иначе YouTube не зацикливает). Попутно порядок query-параметров
  приведён к asciidoctor: `rel, start, end, autoplay, loop, controls, list/playlist`
  (было `rel, autoplay, loop, controls, start, end` — латентное расхождение при комбинациях,
  подтверждено пробой `start=60,opts="autoplay,loop"`). Фикс — только РЕНДЕРЕР
  (`adoc-html/src/media.rs`): `MediaAttrs` +поля `list`/`playlist`, youtube-ветка
  `render_video_tag` сплитит target и эмитит параметры 1:1. НЕ реализованы (нет в корпусе):
  опции `muted`/`modest`/`related`/`nofullscreen`, атрибуты `theme`/`lang`. +1 тест
  `test_video_youtube_playlist_params` (4 варианта + loop-fallback).
  **Корпус: Identical 204→205 (+1)** (video.adoc); blast ровно 1 файл: 1 флип, **0 регрессий**,
  0 changed-still-different. clippy 0, test --workspace зелёное (html 328→329).

- [x] **STEM: инъекция MathJax-loader при `:stem:`** — СДЕЛАНО (ветка `fix/stem-mathjax-docinfo`,
  2026-06-09; НЕ закоммичено). Asciidoctor при установленном атрибуте `stem` (любое значение, даже
  без stem-контента) вставляет ФИКСИРОВАННЫЙ блок: `<script type="text/x-mathjax-config">` с
  `MathJax.Hub.Config({...})` + CDN-loader `https://cdnjs.cloudflare.com/ajax/libs/mathjax/2.7.9/MathJax.js?config=TeX-MML-AM_HTMLorMML`,
  перед `</body>` (после футера). Блок ИДЕНТИЧЕН для asciimath и latexmath (верифицировано пробами:
  P1[asciimath]==P2[latexmath]; без `:stem:` — нет блока даже при inline `stem:[]`). Это standalone-фича
  («архитектурная» по ярлыку прошлых сессий, но фактически детерминированная строковая вставка).
  Фикс (1 точка, РЕНДЕРЕР `adoc-html/lib.rs`): const `MATHJAX_DOCINFO` (raw-строка `r#"..."#`, байт-в-байт
  — два литеральных `\` перед `(`/`[`/`$` в JS, подтверждено `od -c`) + 4-строчная вставка в
  `write_document_tail` под `self.document_attrs.contains_key("stem")`. `:!stem:` удаляет ключ → вставки
  нет. +1 тест `test_stem_mathjax_docinfo` (флип-инъекция asciimath/latexmath + guard без stem).
  **Корпус: Identical 203→204 (+1)** (stem/index, verified 0 diffs len 720==720); blast 2 файла: 1 флип,
  **0 регрессий**, stem/examples/stem.adoc changed-still-different (MathJax байт-в-байт верен — exp==got;
  Different по пре-существующему каскаду level-0 `<h1>` vs `<div class="sect0">`; 99→104 = +5 корректных
  MathJax-токенов под доминирующим sect0-сдвигом). parsing-lab 233/233, html-compat 6/6, clippy 0,
  test --workspace зелёное (html 325→326). **Остаток (НЕ в корпусе)**: `eqnums` атрибут изменил бы
  `TeX.equationNumbers.autoNumber` (хардкод `"none"` = дефолт).

- [x] **Rowspan: размещение ячеек в спанированных строках** — СДЕЛАНО (ветка
  `fix/rowspan-row-placement`, 2026-06-09; НЕ закоммичено). `build_table_rows`
  (`adoc-parser/src/block.rs`): ячейка с `.N+` (rowspan) занимает свою колонку в N строках, поэтому
  следующая строка должна держать на 1 ячейку меньше. Баг — **двойной декремент**: при старте новой
  строки «decrement all» цикл уменьшал `col_remaining[c]` ДО skip-цикла, который повторно декрементил
  при пропуске → спанированная колонка не пропускалась, ячейка следующей строки уезжала в неё
  (`.2+|X`+`|1` / `|2` / `|Y|Z` → `2` сливалась с `Y` в одну строку, `Z` оставался один). Фикс (1 точка):
  убран «decrement all» — skip-цикл (top-of-loop для mid-row + row-start для leading) сам декрементит
  каждую occupied-колонку ровно раз за строку. +1 тест `test_table_rowspan_shifts_following_row_cells_html`
  (флип + regression: continuation-ячейка закрывает свою строку, следующая начинает новую, 4 `<tr>`).
  Сущест. `test_table_rowspan_html`/`test_table_colspan_rowspan_html` целы (трассированы вручную).
  **Корпус: Identical 202→203 (+1)** (docinfo/index, verified 0 diffs len 982==982); blast 4 файла:
  1 флип, **0 регрессий**, table-ref 887→871 (−16, улучшение), cell/toc-ref нейтральны (доминирующий
  несвязанный каскад: `2*` дублирование колонок + наследование `m`/`e` стиля). parsing-lab 233/233,
  html-compat 6/6, clippy 0. **Остаток**: пасс `emit_row_cells` col_idx (выравнивание/стиль) не учитывает
  rowspan-сдвиг — латентно (docinfo все колонки `<`).

- [x] **Callout-элемент с continuation-блоком + сдвиг source-языка ведущим named-атрибутом** —
  СДЕЛАНО (ветка `fix/callout-item-block-and-shifted-source-lang`, 2026-06-09; НЕ закоммичено).
  Два независимых корня, оба валидированы реальными флипами. (1) **NOTE/блок внутри callout-элемента**
  (`adoc-html/lib.rs`): `+`-continuation-блок (напр. `NOTE:`), присоединённый к callout-элементу, не
  закрывал принципиальный `<p>` → блок вкладывался в незакрытый `<p>`, а `</p>` уезжал в конец. Корень:
  `CalloutListItem` (в отличие от `ListItem`) не вёл стек `li_p_open`/`li_para_count`, и `Tag::Admonition`
  отсутствовал в списке тегов-триггеров закрытия `<p>`. Фикс: `CalloutListItem` зеркалит `ListItem`
  (push/pop стеков, end-обработчик закрывает `</p>` условно), `Tag::Admonition` добавлен в guard.
  Заодно чинит continuation-параграф в callout (теперь оборачивается в `<div class="paragraph">`).
  (2) **Сдвиг позиционных слотов** (`adoc-parser/attributes.rs`): AsciiDoc инкрементит позиционный
  индекс для КАЖДОГО атрибута (named/shorthand тоже), поэтому `[id=app, source, yaml]` кладёт `source`
  в слот 2 (язык), а не слот 1 (стиль) → язык `source`, не `yaml` (верифицировано пробами:
  `[role=x,…]`/`[#id,…]`/`[.r,…]`/`[%o,…]` так же; два ведущих named `[id, role, source, yaml]` → НЕ
  source). Наш `positional` Vec схлопывал named/shorthand, и `[id=app, source, yaml]` выглядел как
  explicit `[source, yaml]`. Фикс: флаг `first_positional_is_style` (первый comma-часть — bare-позиционал),
  убран ложно-срабатывающий guard `positional.first() != Some("source")`, `source_language()`/
  `is_source_block()` стали слот-осознанными. +2 теста (parser slot-shift; html callout-NOTE + lang-shift).
  **Корпус: Identical 200→202 (+2)** (db-migration оба корня, localization — корень 1[callout]); blast
  4 файла: 2 флипа, **0 регрессий**, 2 changed-still-different улучшены (index 2290→2265,
  software-development-cookbook 2595→2463 — гигантские include-агрегаты). parsing-lab 233/233. clippy 0,
  test --workspace зелёное (parser 459→460, html 322→324).

- [x] **Audio-макрос: `start`/`end` фрагмент + `opts=` alias + `.Title`** — СДЕЛАНО (ветка
  `fix/audio-start-opts-and-title`, 2026-06-09; НЕ закоммичено). Три бага в `audio::x[…]` (audio.adoc,
  блоки 2+3 давали флип). (1) **`opts=` не парсился** (`adoc-html/lib.rs::parse_media_attrs`): arm ловил
  только `"options"`, но shorthand `opts=autoplay` (≡ `options`, верифицировано пробой) → `autoplay`/
  `loop`/`nocontrols` терялись; фикс `"options"` → `"opts" | "options"` (затрагивает и video — общий
  парсер, улучшение). (2) **`start`/`end` не применялись к audio src** (`render_audio_tag`): писал голый
  `src=target`, в отличие от video; фикс — src-билд зеркалит video (`#t=start,end` фрагмент), порядок
  boolean-атрибутов подогнан под asciidoctor (autoplay, loop, controls). (3) **`.Title` терялся** (arm
  `Tag::BlockAudio`): не звал `emit_pending_block_title` (тот же класс бага, что toc/literal-paragraph);
  парсер эмитит BlockTitle верно — фикс в рендерере (title ДО content-div). Обновлён `test_audio_options_html`
  (новый порядок), +1 тест `test_audio_start_opts_and_title`. **Корпус: Identical 199→200 (+1)**
  (audio.adoc, verified 0 diffs); blast 2 файла: 1 флип, **0 регрессий**, video 48→47 (улучшение —
  `opts=autoplay` теперь верно). parsing-lab 233/233. clippy 0, test --workspace зелёное (html 322, parser 459).
  **Остаток**: arm `Tag::BlockVideo` имеет тот же title-баг (video далеко от флипа, не трогал).

- [x] **Intrinsic char-replacement атрибуты `{quot}`/`{apos}`/`{pp}` + `pass:[…]` внутри monospace
  (случай G)** — СДЕЛАНО (ветка `fix/intrinsic-quot-apos-and-pass-constrained`, 2026-06-09; НЕ
  закоммичено). Два корня, оба нужны для флипа quotation-marks-and-apostrophes.adoc (ровно 4 diff'а).
  (1) **Intrinsic-атрибуты**: таблица `INTRINSIC_ATTRIBUTES` (`adoc-html/lib.rs`) не содержала `quot`/
  `apos`/`pp`; Asciidoctor резолвит их (верифицировано пробой) → `&#34;`/`&#39;`/`&#43;&#43;`, в т.ч.
  внутри `` `…` `` monospace. Добавлены 3 записи (алфавитный порядок). `cpp` (`C++`) уже был. (2)
  **`pass:[…]` в constrained-marker matching** (`inline.rs::find_closing_constrained`, случай G):
  `pass:[…]` извлекается ДО quote-подстановки, поэтому quote-маркер внутри его скобок не должен
  закрывать внешний span (`` `pass:[`']` `` → `<code>`'</code>`; мы ломались на внутреннем backtick →
  `<code>pass:[</code>']` `). Добавлен хелпер `pass_macro_span_len` (контент до первого `]`) — точный
  аналог уже сделанного skip `++…++` (`passthrough_span_len`); `find_closing_constrained` пропускает
  регион `pass:[…]`. Inner-reparse уже корректно эмитит pass-макрос. **Случай A** (`` `+pass:[]+` ``
  через single-plus, pass/index стр.15) НЕ сделан — асимметричен (pass извлекается ДО `+…+`, но НЕ
  внутри `++…++`: `+pass:[x]+`→`x`, `++pass:[y]++`→`pass:[y]`; дискриминатор `` `+pass:[]+more+` ``→
  `<code>+more</code>` ломает наивный shortcut), отложен. +2 теста (parser `test_pass_macro_inside_monospace`;
  html `test_intrinsic_char_replacement_attrs`). **Корпус: Identical 198→199 (+1)**
  (quotation-marks-and-apostrophes); blast 5 файлов: 1 флип, **0 регрессий**, 4 changed-still-different
  — pass-macro 250→249 (`{pp}` резолвится), literal-monospace 61→59, troubleshoot-unconstrained 216→212
  (pass-в-monospace лучше), character-replacement-ref 645→645 (нейтрально — доминирующий несвязанный
  каскад len 756 vs 581 от table-column-style/footnote). parsing-lab 233/233 целы (правка в
  close-finder + intrinsic-таблице; кейсов pass-в-quote в фикстурах нет). clippy 0, test зелёное
  (parser 459, html 321).

- [x] **Experimental UI-макросы `kbd:`/`btn:`/`menu:` за `:experimental:`** (п.29/п.39) — СДЕЛАНО
  (ветка `fix/gate-experimental-ui-macros`, 2026-06-09; НЕ закоммичено). Asciidoctor распознаёт
  `kbd:`/`btn:`/`menu:` ТОЛЬКО при установленном `:experimental:`; иначе оставляет литералом. Мы
  парсили их безусловно (вывод с/без `:experimental:` был идентичен). **Корень**: распознавание
  макроса идёт в ПАРСЕРЕ (`inline.rs`), который не знал document-атрибутов. Фикс — протянуть флаг:
  (1) `inline.rs` — поле `InlineState.experimental`, новый публичный `parse_str_with_subs_experimental`
  (`parse_str_with_subs` стал обёрткой `…, false`), 5 внутренних reparse-вызовов `InlineState::new`
  наследуют `self.experimental`; 3 arm'а kbd/btn/menu в `handle_inline_macro` гейтятся: при выкл.
  experimental — хелпер `skip_disabled_ui_macro(prefix_len)` поглощает весь токен `name:target[…]`
  как литерал (КРИТИЧНО: иначе остаток вроде `bd:[…]`/`file[…]` мисспарсится catch-all'ом
  `try_custom_inline_macro`). (2) `parser.rs` — поле `Parser.experimental`, наблюдается из
  `Event::Attribute{name}` (`experimental` set / `!experimental`/`experimental!` unset; mid-document
  семантика сохранена), протянуто в обе точки inline-парсинга. (3) `adoc-html/lib.rs` —
  `render_inline_value` передаёт `document_attrs.contains_key("experimental")`. Обновлено 12 тестов
  (7 inline + 5 html — кодировали парсинг БЕЗ experimental: добавлен `:experimental:`-префикс/хелпер
  `parse_experimental`), +2 guard-теста (parser literal-без-experimental incl. lowercase-target;
  html literal+not-custom). html-compat `kbd-btn-menu.adoc` (УЖЕ с `:experimental:`) валидирует
  рендеринг end-to-end. **Корпус: Identical 194→198 (+4)** (unset-attributes, build-basic-block,
  paragraphs, ui); blast 8 файлов: 4 флипа, **0 регрессий**, 4 changed-still-different
  (attribute-entries/boolean-attributes/build-a-basic-table/quotation-marks — kbd/btn/menu теперь
  верны, Different по др. причинам: author-header `<div class="details">`, таблицы). parsing-lab
  233/233 целы (kbd/btn/menu в фикстурах нет). clippy 0, test --workspace зелёное (parser 458, html 320).

- [x] **Revision-номер: strip нецифрового префикса + `[%hardbreaks]`** — СДЕЛАНО (ветка
  `fix/revision-prefix-and-hardbreaks`, 2026-06-09; НЕ закоммичено). Два чистых корня, оба нужны для
  флипа reference-revision-line.adoc. (1) **Revnumber prefix-strip** (`scanner.rs::parse_revision_line`):
  Asciidoctor `\D*(.*?),` — версия = часть до ПЕРВОЙ запятой со снятым ведущим нецифровым прогоном
  (`v8.3`→`8.3`, `LPR55`→`55`, `Version 2.5 RC1`→`2.5 RC1`); внутренние буквы/пробелы сохраняются. Дата
  между первой запятой и первым `:` (внутр. запятые даты переживают: `July 29, 2025`). No-comma: head —
  версия ТОЛЬКО при префиксе `v`/`V` (`v1.0`), иначе дата (`2024-01-01`); `:` вводит remark. Разделители
  голые (`,`/`:`, без требования пробела). Strip — ТОЛЬКО в парсинге revision-СТРОКИ; явный
  `:revnumber: v8.3` (attribute-entry) НЕ стрипается (верифицировано: asciidoctor рендерит `version v8.3,`).
  Рендерер header (`strip_prefix('v')`) теперь no-op (идемпотентно), для `LPR`-префикса даже чинится.
  (2) **`[%hardbreaks]`** (`adoc-html/lib.rs`, новая фича): опция параграфа (или doc-attr
  `hardbreaks-option`) → каждый soft-break → `<br>`. Поле `para_hardbreaks` (set в `start_paragraph` из
  `meta.options`/doc-attr, clear на `TagEnd::Paragraph`), хелпер `push_hardbreaks_text` (split по `\n`,
  join `<br>\n`, последняя строка без `<br>`; escape опционально). Обновлены 8 тестов (5 scanner + 2 block
  + 1 html кодировали `v`-префикс), +2 теста (scanner nondigit-prefix, html hardbreaks).
  **Корпус: Identical 193→194 (+1)** (reference-revision-line); blast 5 файлов: 1 флип, **0 регрессий**,
  4 changed-still-different — все корректны: paragraph 60→38 (`<br>` идентичны asciidoctor), revision-line-
  with-version-prefix 13→**1** (остаток = замороженный `{docdate}` reference на 2026-03-15 — дата-зависим,
  не флипается), reference-revision-attributes 31→31 (явный `:revnumber:` верно не стрипнут; pre-existing
  header-span gap), text 633→650 (позиц. каскад от pre-existing sect0 level-0 heading; hardbreaks байт-в-байт).
  parsing-lab 233/233 целы. clippy 0, test --workspace зелёное (parser 457).

- [x] **`.Title` на отступном literal-параграфе** — СДЕЛАНО (ветка
  `fix/literal-paragraph-block-title`, 2026-06-09). `.TOC enabled via the CLI` перед отступным
  literal-параграфом (` $ asciidoctor ...`) терял заголовок: `<div class="literalblock">` без
  `<div class="title">`. Корень: ПАРСЕР эмитит `BlockTitle` верно (проверено дампом событий) — баг в
  РЕНДЕРЕРЕ: inline-arm `Tag::LiteralParagraph` (`adoc-html/lib.rs:~1058`) пушил
  `<div class="literalblock">\n<div class="content">\n<pre>` одной строкой, НЕ вызывая
  `emit_pending_block_title` (в отличие от `DelimitedBlockKind::Literal` для `....`, который его зовёт).
  Фикс (1 точка): разбить push, вставить `self.emit_pending_block_title(output)` между wrapper-div и
  content-div — дословно зеркалит delimited-literal. Title-less параграф не затронут (`emit_*` no-op
  при `block_title_inner_html=None`). +1 тест `test_literal_paragraph_block_title` (флип + regression
  guard без title). **Корпус: Identical 192→193 (+1)** (toc/index.adoc); blast radius **1 файл,
  1 флип, 0 регрессий, 0 changed-still-different** (идеально узко). parsing-lab 233/233 целы
  (правка только в рендерере, ASG читает события парсера напрямую).

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
- [x] **п.41 header после комментариев** — СДЕЛАНО (ветка `fix/header-after-leading-comments`,
  2026-06-11; НЕ закоммичено). Корень: `scan_next_block_once` ставил `body_started=true` ДО
  обработки комментария в `scan_block_containers` → `// tag::…[]` перед `= Title` ломал
  детекцию header. Asciidoctor пропускает `//`-строки И `////`-блоки как ДО header'а, так и
  МЕЖДУ его строками (title/author/revision/attrs), не завершая header (пробы
  /tmp/p_hdrcmt{,2,3,4}.adoc); blank-строка по-прежнему завершает. Фикс — только ПАРСЕР
  (`block.rs`): хелпер `skip_header_comments` (строчные + `////`-блоки с точным матчем длины
  закрывашки, зеркало scan_delimited_block) + 5 точек вызова: верх `scan_header_constructs`
  (гейт `!body_started`, rescan-паттерн), перед author/revision-проверками
  `scan_document_header`, верх трёх attr-циклов (scan_document_header /
  scan_attribute_only_header / scan_document_header_with_pre_attrs). +1 тест
  `test_document_header_after_leading_comments` (комменты до/между + guard «после blank —
  body»). **Корпус: Identical 210→228 (+18)** (boolean-attributes, include-*×5, title,
  subtitle, url-macro, preface, abstract-block, numbers, adjust-column-widths,
  duplicate-cells, bold, highlight, italic, strong-span); blast 35 файлов: 18 флипов,
  **0 регрессий**, кластер document/* раздавлен: header 69→16, multiple-authors 75→7,
  version-label 28→2, toc 1, part 24→18. clippy 0, test --workspace зелёное (parser 464→465).
  **Попутно обнаружен pre-existing баг (НЕ в корпусе — он с nofooter)**: рендерер не эмитит
  `Version X<br>` в footer-text при установленном revnumber (asciidoctor эмитит).
  **п.27 source-language attr** (7) — НЕ делался, отдельная задача.
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
  п.28 (TOC), п.36 (`{counter}` в таблицах). (п.29 `kbd:` и п.39 `btn:`/`menu:` СДЕЛАНЫ —
  гейтинг за `:experimental:`, см. выше.)

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
