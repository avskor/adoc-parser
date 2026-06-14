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

## АКТУАЛЬНО (2026-06-14, 71-я сессия): РЕРАЙТ inline — Фаза 2 НАЧАТА (ветка `feat/subst-phase2-passes`)

Корпус неизменен **343/344** (гейт держит). Фаза 2 = перенести оставшиеся пассы пайплайна
asciidoctor в `adoc-parser/src/subst/`, довести FORCE-движок до байт-идентичности, в финале
снять gate → flip outline. **2 коммита на ветке (MERGE+ПУШ ждут авторизации):**
- [x] **(2/N) replacements** (`subst/replacements.rs`) — типографика на всём буфере, переиспользует
  donor `apply_typographic_replacements` (стал `pub(crate)`). Сентинел-байты = `<>`-границы тегов.
- [x] **(2/N) post_replacements** (`subst/post_replacements.rs`) — hard-break ` +`; `TagToken::HardBreak`
  в токенизаторе. edges-флаг не нужен (close-сентинел блокирует break внутри спана естественно).
- **Гейт:** toggle-off 343, toggle-on 343, **0 регрессий, 0 FARTHER** (airtight: toggle-on вывод ≡
  parse_legacy всегда). **FORCE-верность 46 → 85** raw-идентичных файлов; 5 FORCE-FARTHER —
  ожидаемый каскад от нерезолвленных `{attr}`/macro рядом с ` +`, gate их отклоняет.
- clippy 0, test --workspace зелёное (parser 530, html 433), parsing-lab 233/233.
- [ ] **ОСТАЛОСЬ Фаза 2:** passthrough extract/restore (FIRST в пайплайне; Code/passthrough события),
  attributes `{name}`, macros (link/xref/image/footnote/icon/kbd/btn/menu/stem/anchor/autolink/email —
  overhaul токенизатора), char-refs (`&#167;` survival), escape `\*`, curved smart-quotes `"…"`.
  specialchars — фактически NO-OP (Event::Text сырой). ФИНАЛ: снять gate → flip outline при 343.

## (АРХИВ) после 70-й сессии: Identical **343 / 344**; РЕРАЙТ inline — Фаза 1 СДЕЛАНА

Остался РОВНО 1 Different — `spec/outline.adoc` (4813 diff), единственный корень
**cross-span strong @4545** (глубоко архитектурный). Пользователь авторизовал **полный
рерайт inline-субституций на модель asciidoctor (string-rewriting passes)** ради outline
344/344 — план `~/.claude/plans/greedy-yawning-pumpkin.md` (4 фазы, dual-engine за toggle,
blast-гейт 0-регрессий, мерж по авторизации). НЕ путать с корпус-driven-фиксами: это
архитектурный рерайт самого тяжёлого модуля, multi-session, риск ОЧЕНЬ ВЫСОКИЙ, цель —
воспроизвести НЕВАЛИДНЫЙ overlapping HTML (баг asciidoctor) ради байт-идентичности 1 файла.

**Фаза 0 СДЕЛАНА и СМЕРЖЕНА в master** (была ветка `feat/sequential-quotes-engine`):
toggle `ADOC_QUOTES_SEQUENTIAL=1` (env→`crate::subst::enabled()`, OnceLock), скелет
`adoc-parser/src/subst/mod.rs` (`try_parse`→None), ветвление в `parse_str_with_subs_options`.

**Фаза 1 СДЕЛАНА — quotes-пайплайн за differential-equality gate** (ветка
`feat/subst-phase1-quotes`, ЗАКОММИЧЕНА; **MERGE+ПУШ ОЖИДАЮТ авторизации**). Реализован
string-rewriting движок quotes как gsub-последовательность пассов:
- **`subst/tokenize.rs`**: сентинел-модель (`\x01<idx>\x02` в рабочей String → side-table
  `TagToken::{Open{kind,id,roles},Close(kind)}`), `tokenize` → `Vec<Event>` БЕЗ балансировки
  (overlap сохраняется). `SpanKind`→Tag/TagEnd; `utf8_char_len`/`sentinel_end`.
- **`subst/quotes.rs`**: пассы в порядке asciidoctor QUOTE_SUBS — strong(unc/con),
  mono(unc/con), em(unc/con), mark(unc/con→Highlight/InlineSpan), sup, sub; `[attrlist]`
  префикс (constrained требует open-boundary, unconstrained — нет); граничная логика и
  `find_closing_*` портированы из legacy. Сентинел-байты = non-word boundary (как `<`/`>`
  в asciidoctor) — естественно (контрол-байты не alnum/`_`).
- **`subst/mod.rs`**: `try_parse` гоняет пайплайн + legacy, возвращает `Some` ТОЛЬКО при
  побайтовом равенстве событий, иначе `None` (fallback). Gate = **0-регрессий ПО ПОСТРОЕНИЮ**.
  `parse_legacy` вынесена из inline.rs. Диагностика `ADOC_SUBST_FORCE=1` (минует gate).
- **ВАЖНО (бонус):** edge-флаги (`emphasis_leading_edge`) воспроизводятся ЕСТЕСТВЕННО порядком
  пассов — `_`code`_`→`<em>`code`</em>` без хаков (mono-пасс видит `_` word-char перед backtick).
  Cross-span OVERLAP воспроизводится (`a *crosses `code* span`` → Start(Strong)…Start(Mono)…
  End(Strong)…End(Mono)) — ровно то, что рекурсивная модель не может. Проверено unit-тестом.
- **ОТЛОЖЕНО (Фаза 2, fallback через gate):** passthrough-extract, specialchars, attributes,
  replacements, macros, post-replacements, escape `\`, curved smart-quotes `"`…`"`.
- **Гейт верифицирован:** clippy 0, test --workspace (parser 522→528, +6 subst-тестов, html 433),
  parsing-lab 233/233. blast toggle-OFF 0 diff vs base (инертность), toggle-ON+gate **0 diff**
  (корпус не изменён, 343 неизменны). FORCE-диагностика: 46/344 файлов идентичны base на уровне
  файла (занижено — 1 inline-текст с отложенной фичей флипает весь файл); **0 паник на 344
  файлах**; ВСЕ FORCE-расхождения атрибутированы отложенным фичам (don't→don’t, ` +`→`<br>`,
  `<<>>`/`xref:`, `+*word*+` passthrough), НИ ОДНОГО бага quotes.
- Скрипты: `/mnt/c/tmp/adoc-test/gate_check.py` (быстрое прямое base-vs-new сравнение, режимы
  через KEY=VAL), `blast_force.py`. **Дальше — Фаза 2** (остальные пассы → flip outline; снять gate).
Методология — в memory [[compat_corpus_methodology]], [[proj_sequential_quotes_rewrite]].

## Свежий baseline корпуса (2026-06-09, ПОСЛЕ stem-mathjax-docinfo) — УСТАРЕЛ, см. блок выше

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

- [x] **Inline: escape `\*`/`\_`/`` \` `` сохраняет `\` когда спан не образуется
  (consume-on-match)** — СДЕЛАНО (ветка `fix/escape-backslash-keep-when-no-span`,
  2026-06-14; ЗАКОММИЧЕНА, **MERGE+ПУШ ОЖИДАЮТ авторизации**). **БЕЗ флипа — корректное
  улучшение** (outline заблокирован оставшимся cross-span strong). Один из 2 корней outline
  (escape `\*` @2041, изолированный). Семантика (пробы asciidoctor 2.0.23): quote-regexp'ы
  несут `\\?` и снимают ведущий `\` ТОЛЬКО при реальном матче конструкции (`\*bold*`→`*bold*`,
  escaped-спан); маркер без закрывающей пары (`\* is an asterisk`, `` `\* literal` ``) остаётся
  литералом ВМЕСТЕ с `\`. Наш blanket-арм съедал `\` безусловно. **ПАРСЕР** inline.rs: новый
  escape-арм ПЕРЕД blanket'ом — для `*`/`_`/`` ` `` при `find_closing_constrained`=None
  оставляет `\`+маркер литералом. Регрессионно-безопасно ПО ПОСТРОЕНИЮ: None ⇒ нет
  закрывающего маркера ⇒ asciidoctor тоже НЕ матчит ⇒ тоже сохраняет `\` (двигает только
  drop→keep там, где asciidoctor keep'ает). Some-кейс (валидный спан) не тронут — blanket
  по-прежнему дропает `\`. **НЕ покрыто** (не в корпусе, отложено): top-level `\*` + закрывающий
  `*` на латер-строке многострочного параграфа с невалидным контентом (пробел) — нужна полная
  try_constrained-валидность; `#`/`^`/`~`/`{`/`[`/`<`/`'` (каждый со своей нюансной логикой).
  +1 parser (`test_escaped_marker_no_span_keeps_backslash`), +1 html
  (`test_escaped_marker_no_span_keeps_backslash_html`: corpus `` `\* literal` ``→
  `<code>\* literal</code>`, контраст `` `\*bold*` ``→`<code>*bold*</code>`, prose `\_lone`).
  **Корпус: Identical 343 (БЕЗ флипа)**; blast (base 343): outline 4814→4813 closer,
  **0 регрессий, 0 других файлов**. clippy 0, test --workspace зелёное (parser 522, html 433),
  parsing-lab 233/233.
  **Остаток outline (1 корень, флип заблокирован):** **@4545 cross-span strong** (каскад,
  весь остаток +4 токена) — ГЛУБОКО АРХИТЕКТУРНЫЙ (line-level QUOTES-пасс: strong поверх
  всей строки до monospace, тянется через границы code-спанов). См. session.md 66-я/68-я.

- [x] **Inline: ` +` hard-break только на реальном крае строки (не в reparsed-спанах)** —
  СДЕЛАНО (ветка `fix/outline-escape-and-monospace-hardbreak`, 2026-06-14; ЗАКОММИЧЕНА,
  **MERGE+ПУШ ОЖИДАЮТ авторизации**). **БЕЗ флипа — корректное улучшение + схлопывание
  каскада** (outline заблокирован 2 архитектурными корнями). Давний «отложенный баг»
  (`` `x +` ``→`<br>`, упоминался многими сессиями). Семантика (пробы): hard-break = ` +`
  на РЕАЛЬНОМ крае строки; asciidoctor применяет line-break replacement ПОСЛЕ рендера
  спанов → трейлинг ` +` внутри спана ограничен `</code>`, не `$` (`` `x +` ``→`<code>x +</code>`,
  `` `` + +`` ``→`<code> + +</code>`; top-level `foo +`→`<br>`; `+\n` mid-string→`<br>` всегда).
  **ПАРСЕР** inline.rs `check_hard_break`: end-of-string случай гейтнут на
  `edges_are_line_boundaries` (true лишь top-level; зеркало spaced-em-dash из
  `fix/monospace-replacements-subs`); `+\n` без изменений. +1 parser, +1 html теста.
  **Корпус: Identical 342 (БЕЗ флипа)**; blast (base 342): outline 6647→4814 closer,
  **0 регрессий, 0 других файлов**. clippy 0, test --workspace зелёное (parser 520,
  html 430), parsing-lab 233/233.
  **Остаток outline (2 архитектурных корня, флип заблокирован):** (1) **escape `\*`**
  (@2041, изолирован) — asciidoctor сохраняет `\` когда маркер не образует валидную
  разметку; наш blanket-escape съедает `\` безусловно. Низкий ROI (нет др. файлов в
  корпусе), отложен; безопасный путь — KEEP `\` лишь при `find_closing_*`=None.
  (2) **cross-span strong** (@4545, каскад) — `` `[1-9][0-9]*.` `` → `[0-9]*` = strong с
  ролью "0-9", тянется через границы code-спанов (line-level QUOTES-пасс asciidoctor);
  наша рекурсивная модель спанов не воспроизводит. ГЛУБОКО АРХИТЕКТУРНЫЙ.

- [x] **Inline: ведущий край emphasis (`_…_`/`__…__`) подавляет constrained strong/mono
  + интринсики `docyear`/`localyear`** — СДЕЛАНО (ветка
  `fix/emphasis-leading-edge-suppresses-strong-mono`, 2026-06-14; ЗАКОММИЧЕНА `57870bf`,
  **MERGE+ПУШ ОЖИДАЮТ авторизации**). Три корня document-attributes-ref (953 diff, Δ−3),
  все нужны для флипа. (1) **inline** (стр.1216 `_`inline` not yet supported._`): на
  ведущем крае emphasis-спана constrained `*`/`` ` `` остаются литеральными — порядок
  QUOTE_SUBS `strong→monospace→emphasis→mark`, оба видят литеральный внешний `_`
  (word-char), open-ассерт `(^|[^\w…])` его отвергает; mark (`#`) идёт после emphasis
  → открывается; unconstrained/super/sub без open-ассерта → открываются. Этот ОДИН
  десинк каскадил ~950 ложных позиционных diff'ов до конца файла. **ПАРСЕР** inline.rs:
  поле `emphasis_leading_edge` (зеркало `smart_quote_leading_edge`), гейт в
  `try_constrained` (`*`/`` ` `` @pos0), установка флага в обоих репарс-сайтах при
  `marker==b'_'`. (2)+(3) **docyear**/**localyear** утекали литерально — CLI main.rs
  `seed`'ил весь date-family (docdate/localdate/…) через chrono, кроме года; добавлено
  `docyear` (из mtime файла, как doc*) и `localyear` (из now, как local*).
  +3 parser, +1 html теста. **Корпус: Identical 341→342 (+1 ФЛИП, document-attributes-ref
  953→0 байт-в-байт)**; blast (base 341): ровно 1 флип, **0 регрессий, 0 FARTHER**.
  clippy 0, test --workspace зелёное (parser 519, html 429), parsing-lab 233/233.
  Остаток near-miss на 342 (2 Different, оба мульти-root): syntax-quick-reference (2788),
  outline (6647).

- [x] **Таблицы: `~` autowidth-маркер в col-spec не должен съедать стиль колонки** —
  СДЕЛАНО (ветка `fix/session64-nearmiss`, 2026-06-14; ЗАКОММИЧЕНА, **MERGE+ПУШ ОЖИДАЮТ
  авторизации**). character-replacement-ref (625 diff, Δ113): таблица
  `[%autowidth,cols="^~m,^~l,^~"]`. Корень: `~` — токен ширины autowidth (asciidoctor regexp
  `(\d+%?|~)`), но `parse_col_spec` (attributes.rs) парсил ТОЛЬКО цифры → `~` не потреблялся
  → rest=`~m` len 2 → проверка стиля `rest.len()==1` ПРОВАЛИВАЛАСЬ → колонка Default вместо
  Monospace/Literal. (Наследование стиля колонки уже работало — block.rs `resolve_style`,
  рендерер blocks.rs; не хватало лишь разбора `~`.) **ПАРСЕР** attributes.rs::parse_col_spec:
  после цифр-ширины потребляется опц. `%`; при отсутствии цифр потребляется `~`. +1 parser
  (`test_parse_col_spec_autowidth_marker_keeps_style`), +1 html
  (`test_table_col_autowidth_marker_inherits_style_html`). **Корпус: Identical 340→341 (+1
  ФЛИП, character-replacement-ref 625→0 байт-в-байт)**; blast (base 340): ровно 1 флип,
  **0 регрессий**. clippy 0, test --workspace зелёное (parser 516, html 428), parsing-lab 233/233.
  Остаток кластера: голый `cols="^~m,..."` БЕЗ `%autowidth` → asciidoctor голый `<col>`
  (per-column `~`=autowidth), мы эмитим `width:…%` (рендерер гейтит colgroup только на
  table-level `%autowidth`) — НЕ в корпусе как Different, отложено.

- [x] **Inline/parser: callout-маркер `<N>` не прерывает top-level параграф (гейт
  `is_in_callout_list`)** — СДЕЛАНО (ветка `fix/callout-marker-no-paragraph-interrupt`,
  2026-06-14; ЗАКОММИЧЕНА `82b8824`, **MERGE+ПУШ ОЖИДАЮТ авторизации**).
  **БЕЗ флипа — корректное улучшение** (table.adoc требует ещё Root 2). Root 1 из ДВУХ
  корней table.adoc (597, Δ1): `|=== <1>` (суффикс ` <1>` → невалид-делимитер) открывает
  параграф, следующие `<2>`/`<4>` ДОЛЖНЫ продолжать его, а не открывать colist.
  Правило (пробы asciidoctor): callout-маркер открывает НОВЫЙ colist только на границе
  блока (после blank), НЕ как продолжение открытого параграфа. **ПАРСЕР** block.rs: в двух
  break-сайтах (`scan_paragraph`+admonition-continuation) условие `is_callout_list_item`
  гейтнуто на `self.is_in_callout_list()` — внутри colist `<N>` всё ещё завершает
  continuation текущего item'а и открывает sibling (гейт критичен: без него 3 регрессии
  localization/cookbook/java-index на continuation-параграфах `<1>…+…cont…<2>`, blast поймал).
  +1 parser, +2 html теста. **Корпус: Identical 339 (БЕЗ флипа)**; blast (base 339):
  table.adoc closer 597→37, **0 регрессий**. clippy 0, test --workspace зелёное (parser
  513, html 426), parsing-lab 233/233.
  **Pre-existing шире**: `*`/`.` list-маркеры после строки параграфа без blank asciidoctor
  тоже поглощает (мы прерываем) — широкое изменение, отдельная оценка.

- [x] **Таблицы: вложенная `!===`-таблица (разделитель `!`) в `a`-ячейке** — СДЕЛАНО
  (ветка `fix/nested-table-bang-delimiter`, 2026-06-14; ЗАКОММИЧЕНА `05c0c8d`,
  **MERGE+ПУШ ОЖИДАЮТ авторизации**). Root 2 из table.adoc (остаток после 62-й).
  a-ячейка уже ре-парсится рекурсивно (`Parser::new(&raw)`), рендер вложенной таблицы
  (colgroup-ширины 66.6666%/33.3334% из cols="2,1") УЖЕ готов — не хватало распознавания
  `!===` сканером. **scanner.rs**: `is_table_delimiter` +`!`-префикс; сплиттер/escape
  параметризованы байтом — `find_unescaped_sep`/`split_unescaped_sep`/`unescape_cell_sep`
  + `parse_table_cells_with_sep(line, sep)`; `parse_table_cells` стала ТЕСТ-ОНЛИ
  (`#[cfg(test)]`), `unescape_cell_pipes` удалена. **block.rs** `scan_table`: разделитель
  из первого байта (`!`→`b'!'`, иначе `b'|'`), формат Native; `sep` протащен в PSV-цикл.
  **Корпус: Identical 339→340 (+1 ФЛИП, table.adoc 37→0, байт-в-байт)**; blast: ровно 1
  флип, 0 регрессий; delimited.adoc (`!===` = содержимое `|`-ячейки) остался Identical.
  +1 scanner, +1 parser, +1 html тест. clippy 0, test зелёное (parser 515, html 427),
  parsing-lab 233/233.

- [x] **Таблицы: shorthand-делимитеры `,===` (CSV) / `:===` (DSV) + `<colgroup>` для
  format-таблиц без `cols=`** — СДЕЛАНО (ветка `fix/csv-dsv-shorthand-and-colgroup`,
  2026-06-14; ЗАКОММИЧЕНА, **MERGE+ПУШ ОЖИДАЮТ авторизации**). Два корня data.adoc
  (181 diff, Δ77), оба про CSV/DSV-таблицы. (1) **colgroup**: рендерер эмитит
  `<colgroup>` только при наличии `cols` в meta.named; `scan_table` (native) синтезирует
  `cols` из числа колонок (1828), а `scan_delimited_format_table` (CSV/DSV/TSV) — НЕТ →
  format-таблицы шли без colgroup. Добавлен тот же синтез (`block_attrs.named.insert
  ("cols", num_cols)` при отсутствии явного `cols=`), `block_attrs` стал `mut`.
  (2) **shorthand-делимитеры** `,===`/`:===` не распознавались → падали в прозу.
  `is_table_delimiter` расширена с `|`-only на префиксы `|`/`,`/`:` (+ 3+ `=`); `scan_table`
  определяет формат из первого байта `opening_delim` (`,`→Csv, `:`→Dsv, иначе
  `block_attrs.table_format()` — `|===` по-прежнему уважает `format=`-атрибут). `!===`
  (nested) НЕ парсится. Семантически `,===`/`:===` = полноценные делимитеры блока (рвут
  открытый параграф как `|===` — проба подтвердила), поэтому расширение `is_table_delimiter`
  единообразно для всех 3 call-site (диспетч + 2 para-break). (3) escaped `\include::` внутри
  `,===` — УЖЕ работал (backslash снимается препроцессором → литерал-ячейка). +2 parser
  (csv/dsv shorthand routing + синтез cols), +1 scanner (`,===`/`:===`/негативы), +1 html
  (shorthand+colgroup+single-col 100%). **Корпус: Identical 338→339 (+1 ФЛИП, data 181→0)**;
  blast (base 338): ровно 1 флип, **0 регрессий**. clippy 0, test --workspace зелёное
  (parser 512, html 424), parsing-lab 233/233.

- [x] **Счётчики: `{counter:…}` литеральны в verbatim styled-параграфах и passthrough'ах**
  — СДЕЛАНО (ветка `fix/counter-verbatim-and-passthrough`, 2026-06-14; ЗАКОММИЧЕНА
  `1907213`, **MERGE+ПУШ ОЖИДАЮТ авторизации**). Два корня counters.adoc (136 diff, Δ9),
  оба сводятся к «счётчики резолвятся только в attributes-субституции, которой нет в
  verbatim/passthrough». (A) `[source]`/`[listing]`/`[literal]` styled-параграф (одиночный,
  БЕЗ `----`) — verbatim-блок; препроцессор УЖЕ скипал delimited verbatim-fences, но НЕ
  styled-параграф. `{counter2:seq1}` в нём резолвился в ПУСТО → пустой блок дропался →
  каскад @142→@282 (130+ из 136 diff — ОДИН корень). (B) inline-passthrough
  `+…+`/`++…++`/`+++…+++`/`pass:[]` (строки 12/21 `` `+{counter:name}+` ``) — asciidoctor
  извлекает passthrough ДО attributes-субституции. **РЕФАКТОР**: 4 passthrough-сканера
  (`pass_spec_len`/`pass_macro_span_len`/`passthrough_span_len`/`single_plus_span_len`)
  вынесены из `impl InlineState` в `scanner.rs` как stateless `pub fn` (место по CLAUDE.md;
  11 call-sites `Self::`→`crate::scanner::`, байт-в-байт). **ПАРСЕР**: (B) `expand_counters`
  скан по байтовому индексу (single_plus видит реальный предыдущий символ — `C+a+` не span)
  + скип passthrough-региона; (A) поля `verbatim_para_pending`/`in_verbatim_para`, секция 4a
  (зеркало fence-логики), `is_verbatim_style_attr_line`. +2 parser, +1 html теста.
  **Корпус: Identical 337→338 (+1 ФЛИП, counters 136→0)**; blast (base 337, пересобран из
  master): ровно 1 флип, **0 регрессий, 0 FARTHER**. clippy 0, test --workspace зелёное
  (parser 510, html 424), parsing-lab 233/233.

- [x] **Inline: single-plus passthrough `+…+` охватывает backtick'и** — СДЕЛАНО
  (ветка `fix/single-plus-passthrough-spans-backtick`, 2026-06-14; ЗАКОММИЧЕНА,
  **MERGE+ПУШ ОЖИДАЮТ авторизации**). Корень align-by-cell.adoc (371 diff, Δ−16,
  single-root — все 371 diff подряд с @153, повтор в строках 37/52/99).
  `` (`<n>+`) or duplication (`+<n>*+`), place the `+^+` `` → asciidoctor сворачивает
  в ОДИН `<code>&lt;n&gt;`) or duplication (`&lt;n&gt;*`), place the `^+</code>`.
  Механизм: single-plus passthrough `+…+` извлекается ГЛОБАЛЬНО ДО quotes/monospace,
  нежадно слева-направо, контент МОЖЕТ включать backtick'и → пары `+…+` съедают
  внутренние backtick'и (литералы), внешний `` ` `` матчится от первого до последнего.
  **ПАРСЕР** inline.rs: хелпер `single_plus_span_len` (зеркало
  `try_single_plus_passthrough`; open не после word-char НИ `\` [`` `\+` `` экранирован]);
  `find_closing_constrained`/`find_closing_unconstrained` пропускают single-plus регион
  (как уже пропускают `++`/`+++`/`pass:[]`). +2 parser, +1 html теста. **Корпус:
  Identical 336→337 (+1 ФЛИП, align-by-cell 371→0)**; blast (base 336): ровно 1 флип,
  **0 регрессий** (span-cells 0→2 от escaped `\+` найдена blast'ом и исправлена
  backslash-guard'ом). clippy 0, test --workspace зелёное (parser 508, html 422),
  parsing-lab 233/233.

- [x] **Inline: ведущий край smart-quote (`"`…`"`/`'`…`'`) подавляет constrained
  mono/em/mark** — СДЕЛАНО (ветка `fix/curved-quote-double-backtick-literal`,
  2026-06-14; ЗАКОММИЧЕНА `1c5e8b3`, **MERGE+ПУШ ОЖИДАЮТ авторизации**). Корень
  troubleshoot-unconstrained-formatting.adoc (212 diff, Δ−4, single-root —
  весь хвост @366→ чистый позиционный сдвиг +4 от одной вставки `<code>`).
  Конструкция `"``end points``"` (двойной backtick): asciidoctor → `“`end points`”`
  (внутренние одинарные backtick ЛИТЕРАЛЬНЫ), мы → `“<code>end points</code>”`
  («на пару backtick впереди»). Реальный механизм — порядок QUOTES asciidoctor:
  `:double`/`:single` идут ПОСЛЕ constrained strong (`*`) но ПЕРЕД constrained
  monospace (`` ` ``)/emphasis (`_`)/mark (`#`). На ведущем крае span'а эти три
  видят `;` от выведенного `&#8220;`/`&#8216;` → их open-ассерт `(^|[^\w;:…])`
  падает → литерал; strong уже сматчился против исходного backtick (его open-класс
  `` ` `` разрешает). Unconstrained (`**`/`` `` ``/`__`/`##`) и super/sub
  (`^`/`~`) open-ассерта НЕ имеют → открываются (тройной `"```…```"` → inner
  `` ``…`` `` unconstrained → `<code>`). Пробы /tmp/mono_open,edge2,tb_probe
  подтвердили: boldedge→strong, emedge/markedge→литерал, sup/sub/mid→без изменений.
  **ПАРСЕР** inline.rs: поле `smart_quote_leading_edge` (true только для inner-рерана
  smart-quotes), гейт в `try_constrained` (`flag && start_pos==0 && marker∈{`` ` ``,_,#}`
  → return false). +3 parser, +1 html теста. **Корпус: Identical 335→336 (+1 ФЛИП,
  troubleshoot 212→0)**; blast (base 335): ровно 1 флип, **0 регрессий**. Фиксит
  попутно латентные em/mark edge-кейсы. clippy 0, test --workspace зелёное (parser
  506, html 421), parsing-lab 233/233.
  Остаток (pre-existing, НЕ в корпусе): smart-quote `"`…`"` open-диспетч НЕ проверяет
  word-границу перед `"` — `a"`code`"b` сворачивается в curved-quote, asciidoctor
  оставляет литерал (constrained-конструкция требует не-word перед открывающей `"`).

- [x] **Inline: monospace close-граница `` `' ``, sup/sub субституции, bare-word
  role-span** — СДЕЛАНО (ветка `fix/monospace-close-boundary-quote-tick`,
  2026-06-14; ЗАКОММИЧЕНА `c1d183a`, **MERGE+ПУШ ОЖИДАЮТ авторизации**). Три корня
  text.adoc (249 diff, Δ−5), ВСЕ нужны для флипа. (A) Constrained monospace `` `…` ``
  имеет более строгое закрытие чем прочие quotes — `(?![\w"'`])`: закрывающий backtick
  не может сопровождаться `"`/`'`/`` ` ``. Без этого `` `' `` (типографский правый
  апостроф `’`) ошибочно матчился как закрытие monospace, сворачивая `` the `'00s …
  werewolves`' `` в `<code>`. **ПАРСЕР** inline.rs `try_constrained`: monospace-чек
  `marker == b'`'` && after_close ∈ `"'``. (B) Superscript/subscript `^…^`/`~…~`
  получают ПОЛНУЮ normal-группу (attributes/quotes/replacements/macros): `^a{sp}b^`→
  `<sup>a b</sup>`, `^*z*^`→`<sup><strong>`. `try_simple_pair` эмитил сырой текст →
  заменён рекурсивным рераном. (C) Bare-word attrlist без `.`/`#` шортхенда =
  одна роль вербатим (`parse_quoted_text_attributes` `{role => str}`): `[big]##O##`→
  `<span class="big">`, `[a.b]##x##`→role "a.b" (точки НЕ делятся). Только первый
  позиционный (split `,`). Constrained требует opening word-границу
  (`word[role]#x#`→литерал), unconstrained `##…##` может mid-word. **ПАРСЕР**: гейт
  диспетча расширен на bare-word, `is_word_char_before` в constrained-ветку. +3
  parser, +3 html теста. **Корпус: Identical 334→335 (+1 ФЛИП, text.adoc 249→0)**;
  blast (base 334): ровно 1 флип, бонус document-attributes-ref 6363→953 closer,
  **0 регрессий**. clippy 0, test --workspace зелёное (parser 503, html 420),
  parsing-lab 233/233.

- [x] **Inline: double-plus passthrough `++…++` применяет specialchars (экранирует
  `<>&`), не raw** — СДЕЛАНО (ветка `fix/double-plus-passthrough-specialchars`,
  2026-06-14; ЗАКОММИЧЕНА `1d2d6e8`, **MERGE+ПУШ ОЖИДАЮТ авторизации**). Корень
  block-name-table.adoc (431 diff, Δ−2): ячейка m-колонки `++[<LABEL>]++` —
  asciidoctor экранирует `<>` (`[&lt;LABEL&gt;]`), мы выводили сырой `<LABEL>`
  (невалидный HTML-тег). Реальная семантика (пробы): `++…++` (double, unconstrained)
  и `+…+` (single) применяют ТОЛЬКО `specialcharacters`; `+++…+++` (triple) и
  `pass:[]` — raw. НЕ применяются quotes/replacements/attributes (`++*x*++`→`*x*`,
  `++a -- b++`→`a -- b`, `++{foo}++`→`{foo}`) — эти уже совпадали. Single-plus уже
  эмитил `Event::Text` (рендерер экранирует) — верно; double-plus эмитил
  `Event::InlinePassthrough` (raw) — баг. **ПАРСЕР** inline.rs
  `try_double_plus_passthrough`: `InlinePassthrough`→`Text` (1 точка); triple-plus
  остался raw. +1 parser, +1 html теста; 2 parser-теста обновлены. **Корпус:
  Identical 333→334 (+1 ФЛИП, block-name-table 431→0)**; blast (base 333): ровно
  1 флип, **0 регрессий** (outline.adoc «FARTHER» 6586→6647 — артефакт нормализатора:
  page-break `` `++<<<++` `` теперь байт-в-байт с asciidoctor `&lt;&lt;&lt;`, было
  невалидное `<<<`; нормализатор токенизирует сырой `<<<` как 3×`<`, сдвиг
  переразложил позиции мульти-root spec). clippy 0, test --workspace зелёное (parser
  501, html 417), parsing-lab 233/233.

- [x] **Списки: literal-параграф в list-item закрывает принципиальный `<p>`; пустой
  принципал обычного item'а держит `<p></p>`** — СДЕЛАНО (ветка
  `fix/list-item-principal-p-empty-and-literal`, 2026-06-14; ЗАКОММИЧЕНА `36d9642`,
  **MERGE+ПУШ ОЖИДАЮТ авторизации**). Два корня complex.adoc (120 diff, Δ4), оба про
  принципиальный `<p>` list-item в guard'е событий (events.rs start_tag @366).
  (A) Отступный literal-параграф (` $ cmd`, БЕЗ `+`) — отдельный блок; asciidoctor
  закрывает `</p>` ПЕРЕД `<div class="literalblock">`. Guard закрытия `<p>` при
  старте суб-блока НЕ включал `Tag::LiteralParagraph` → literalblock вкладывался в
  открытый `<p>`. Фикс: добавлен `Tag::LiteralParagraph` в match. (B) Обычный
  list-item (olist/ulist/colist) с пустым принципалом (`. {empty}`) + присоединённый
  блок — asciidoctor ВСЕГДА оборачивает принципал `<p></p>` (ПРОТИВОПОЛОЖНО dd:
  `convert_dlist` эмитит `<p>` лишь при `dd.text?`). Откат пустого `<p>` (введён для
  empty-dd) срабатывал для всех list-контекстов. Фикс: enum **`LiPara
  { OpenItem, OpenDd, Closed }`** заменил `li_p_open: Vec<bool>`; guard откатывает
  пустой `<p>` ТОЛЬКО для `OpenDd`, item закрывается как `<p></p>`. +2 html-теста
  (негатив empty-dd сохранён). **Корпус: Identical 332→333 (+1 ФЛИП, complex
  120→0)**; blast (base 332): ровно 1 флип, outline closer 6587→6586, **0 регрессий**.
  clippy 0, test --workspace зелёное (parser 500, html 416), parsing-lab 233/233.

- [x] **Inline: monospace `` `text` `` получает полную normal-группу subs (replacements
  + восстановление char-ref)** — СДЕЛАНО (ветка `fix/monospace-replacements-subs`,
  2026-06-14; ЗАКОММИЧЕНА `bcb9ed5`, **MERGE+ПУШ ОЖИДАЮТ авторизации**). Корень
  replacements.adoc (4 diff, Δ0): `` `&#167;` `` в monospace — asciidoctor восстанавливает
  валидную char-ref внутри `<code>`, мы экранировали `&`→`&amp;`. Реальная семантика
  (substitutors.rb): constrained/unconstrained monospace получает ПОЛНУЮ normal-группу
  как проза (specialchars, quotes, attributes, **replacements**, macros, post_repl) —
  `(C)`→©, `--`→em-dash, `...`→ellipsis, char-ref restore (последнее правило REPLACEMENTS
  через `:bounding`). Наш код хардкодил `self.subs.without(REPLACEMENTS)` для backtick —
  заблуждение. Литеральный passthrough `` `+...+` ``/`pass:[]` перехватывается раньше.
  **ПАРСЕР** inline.rs: убран `.without(REPLACEMENTS)` на обоих сайтах (try_constrained/
  try_unconstrained); спейс-em-dash правило (`(^|\n| |\\)--( |\n|$)`) анкорится на краях
  строки, но asciidoctor гоняет replacements ПОСЛЕ обёртки в `<code>` → `--` на крае спана
  ограничен тегами `>`/`<`, не `^`/`$` → литерал (`` `--` ``). Поле
  `InlineState.edges_are_line_boundaries` (true только top-level @221) + флаги границ в
  `apply_typographic_replacements`; mid-input края = legacy «граница» (пустой attr-ref
  `{empty}--{empty}` прозрачен → em-dash на крае строки). +2 parser, +1 html теста.
  **Корпус: Identical 331→332 (+1 ФЛИП, replacements 4→0)**; blast (base 331): ровно
  1 файл, **0 регрессий** (промежуточно ловились hard-line-breaks/sdr-001 и
  subs-symbol-repl — устранены флагом границ). clippy 0, test --workspace зелёное
  (parser 500, html 414), parsing-lab 233/233.

- [x] **Таблицы: классы `frame-{val}`/`grid-{val}` из атрибутов + interactive SVG
  `opts=interactive` → `<object>`** — СДЕЛАНО (ветка
  `fix/image-svg-frame-grid-and-interactive-svg`, 2026-06-13; смержена в master
  `533d12e`, **ПУШ ОЖИДАЕТ авторизации**). Два корня image-svg.adoc (259 diff),
  оба про этот файл, len_delta=-8. (1) Рендерер ХАРДКОДИЛ `tableblock frame-all
  grid-all` — игнорировал атрибуты. Теперь читает `frame`/`grid` из meta.named с
  fallback на doc-attr table-frame/table-grid (html5.rb convert_table:859-860):
  `frame-{val} grid-{val}`, default «all», `topbot`→`ends`, verbatim без валидации.
  (2) SVG-изображение (format=svg или `.svg` в target) с `opts=interactive` →
  `<object type="image/svg+xml" data="{uri}"{w}{h}>{fallback}</object>` (html5.rb
  convert_image), fallback = `<img>` при `fallback=` attr, иначе `<span
  class="alt">{alt}</span>`. Raster+interactive → `<img>`; `opts=inline` (встроить
  SVG-исходник) НЕ поддержан (нужно читать файл → `<img>`). **ПАРСЕР** attributes.rs
  ImageAttrs +format/fallback/interactive; event.rs Tag::BlockImage
  +interactive/fallback; block.rs вычисляет is_svg&&interactive (путь через meta не
  годился — emit_block_metadata фильтрует "format"). **РЕНДЕРЕР** blocks.rs
  start_table (frame/grid), media.rs start_block_image (object vs img). +2 html-теста,
  1 обновлён (integration). **Корпус: Identical 329→330 (+1 ФЛИП, image-svg
  259→0)**; blast (base 329): ровно 1 файл, **0 регрессий** (frame/grid в одиночку —
  259→258 closer, оба корня нужны для флипа). clippy 0, test --workspace зелёное
  (parser 496, html 409), parsing-lab 233/233.

- [x] **dlist-continuation (`+`) + open-блок `--`: blank-строка внутри обрывала
  вывод** — СДЕЛАНО (ветка `fix/dlist-continuation-openblock-truncation`,
  2026-06-13; смержена в master, **ПУШ ОЖИДАЕТ авторизации**). ts-url-format (110)
  обрывался на 35/143 токенах. Корень: `+`-continuation открывает open-блок,
  `in_continuation`→false; стек = `[…,DescriptionListEntry,DelimitedBlock]`; на
  втором блоке после внутренней blank-строки blank-line-guard `is_in_list_context()
  && !in_continuation && had_blank_line` срабатывает, `close_list_contexts()` видит
  на вершине DelimitedBlock (не список) → пусто → `event_buffer.pop()`=None → парсер
  обрывает поток (вкл. незакрытые врапперы). **ПАРСЕР** block.rs: хелпер
  `is_directly_in_list_context()` (innermost-контейнер = list-item; DelimitedBlock/
  PartIntro — барьер), все 8 blank-line guard-сайтов переведены на него. +1 html
  тест. **Корпус: Identical 328→329 (+1 ФЛИП, ts-url-format 110→0)**; blast (base
  328): флип + complex.adoc 152→120 closer, **0 регрессий**. clippy 0, test
  --workspace зелёное (996), parsing-lab 233/233.

- [x] **Секции: `[float]` = синоним `[discrete]` (standalone-заголовок) +
  `sectnumlevels` ограничивает глубину нумерации + Ruby-`to_i` парсинг значения**
  — СДЕЛАНО (ветка `fix/float-discrete-headings-sectnumlevels`, 2026-06-13;
  смержена в master, **ПУШ ОЖИДАЕТ авторизации**). Три корня section.adoc (347
  diff), все про секции/нумерацию. (1) `[float]` — legacy-синоним `[discrete]`:
  standalone-заголовок (не секция, не в TOC, не нумеруется), класс = буквальное
  имя стиля (`[float]`→`class="float"`). Парсер уже имел scan_discrete_heading +
  Tag::Heading, триггер был только на `[discrete]`. **ПАРСЕР** block.rs: хелпер
  `is_discrete_style` (`discrete`|`float`), три проверки section-маркера
  переведены (пробы /tmp/p_disc, p_disc2). (2) `sectnumlevels` (default 3)
  ограничивает глубину: **РЕНДЕРЕР** поле `sectnumlevels`, гейт в
  start_section_title `display_level <= sectnumlevels+1` (asciidoctor level =
  display−1) — фиксит pre-existing баг (всегда нумеровали level-4). (3)
  `:sectnumlevels: 2 <.>` (callout-суффикс) парсится Ruby-`to_i` (ведущие цифры)
  → 2, наш `parse::<u8>` падал → default 3. +2 html, +1 parser теста. **Корпус:
  Identical 327→328 (+1 ФЛИП, section.adoc 347→0)**; blast (base 327): ровно
  1 файл, **0 регрессий**. clippy 0, test --workspace зелёное (parser 496, html
  406), parsing-lab 233/233.

- [x] **Секции: нумерация частей книги (`:partnums:`) `Part {roman}: ` + TOC
  `sectlevel0` для sect0** — СДЕЛАНО (ветка `fix/book-part-numbering`,
  2026-06-13; смержена в master `ea2e0c2`, **ПУШ ОЖИДАЕТ авторизации**).
  **БЕЗ флипа** (корректное улучшение — нет корпусного файла где это единственное
  расхождение). Части (level-0 секции книги) под `:partnums:` получают префикс
  `{part-signifier+" " если задан}{roman}: ` на `<h1 class="sect0">` И в TOC;
  римские заглавные сквозные глобальные; нумерация частей независима от sectnums,
  главы сквозные через части. Семантика — пробы /tmp/p_part1..7 + html5.rb
  convert_section. **РЕНДЕР-CORE** SectionNumberer: `part_counter` +
  `part_prefix(signifier)` + `to_roman`. **РЕНДЕРЕР** blocks.rs start_section_div:
  book-part ветка через `pending_section_caption` (тот же канал что appendix).
  **Бонус-багфикс (pre-existing)**: TOC внешний `<ul>` класс = реальный
  asciidoctor-уровень (`level-1`, было `(level-1).max(1)`) → body sect0 (book
  part ИЛИ article level-0) теперь `sectlevel0`. +2 html, +2 core теста.
  **Корпус: Identical 327 (без флипа)**; blast: outline closer 6597→6587, **0
  регрессий**. clippy 0, test --workspace зелёное (parser 495, html 404).
  outline (6597, Δ1) оказался МУЛЬТИ-root, не флипнет одним фиксом.

- [x] **Description-list: горизонтальный dlist с labelwidth/itemwidth → `<colgroup>`
  + qanda оборачивает ответ в `<p>` и группирует смежные термы в один `<li>`** —
  СДЕЛАНО (ветка `fix/horizontal-dlist-colgroup-widths`, 2026-06-13; смержена в
  master `10b2174`, **ПУШ ОЖИДАЕТ авторизации**). Два корня description.adoc (299
  diff), оба в description-list. (1) Горизонтальный dlist + labelwidth/itemwidth
  эмитит `<colgroup>` с двумя `<col>` (html5.rb:550-557): colgroup ⟺ есть
  labelwidth ИЛИ itemwidth, каждый `<col>` несёт `style="width: N%;"` только при
  своём атрибуте (иначе голый), хвостовой `%` снимается. (2) qanda оборачивает
  ответ в `<p>{dd.text}</p>` (пустой — без `<p>`) и группирует смежные термы (один
  ответ) в ОДИН `<li>` с `<p><em>…</em></p>` на каждый терм — парсер группировал
  верно (нормальный dlist ок), баг был только в qanda-рендерере. **РЕНДЕРЕР**
  blocks.rs `start_description_list` (colgroup из meta.named) + events.rs qanda-армы
  DescriptionTerm/DescriptionDescription (флаг `hdlist_in_term_group` для
  группировки, `<p>`-обёртка + откат пустого ответа через dd_output_start). +2
  html-теста, 1 обновлён (старый qanda-тест кодировал баг). **Корпус: Identical
  326→327 (+1 ФЛИП, description.adoc 299→0)**; blast (base 326): ровно 1 файл,
  **0 регрессий** (оба корня встречаются вместе только тут; colgroup-корень есть и
  в horizontal/paragraph/CHANGELOG, но там labelwidth внутри listing-блоков → уже
  Identical). clippy 0, test --workspace зелёное (parser 495, html 402),
  parsing-lab 233/233.

- [x] **Параграфы: section-маркер (`== Title`/`==== <.>`) НЕ прерывает открытый
  параграф** — СДЕЛАНО (ветка `fix/section-marker-no-interrupt-paragraph`,
  2026-06-13; смержена в master `a827d7a`, **ПУШ ОЖИДАЕТ авторизации**). Корень
  admonition.adoc (197 diff, тег `bl-c`): `[IMPORTANT] <.>` не заканчивается на
  `]` → не attr-строка, начинает параграф; строка-продолжение `==== <.>` — у
  asciidoctor литеральный текст параграфа, мы трактовали как заголовок секции
  level-3 (`<div class="sect3"><h4>`). Пробы /tmp/p_sec1..4, pb_* подтвердили
  правило asciidoctor (`read_paragraph_lines`/`StartOfBlockProc`): открытый
  параграф рвётся ТОЛЬКО на делимитере блока (`----`/markdown-fence) и
  block-attr-строке `[...]`; section-заголовок, list-маркеры, thematic break,
  block-image, admonition, page break, dlist его НЕ прерывают (распознаются лишь
  на границе блока после blank). **ПАРСЕР** block.rs: убран
  `strip_any_section_marker` из break-условий в `scan_paragraph` и
  `scan_admonition`; section на границе блока ловит диспетчер scan_leaf_blocks
  (после blank). +1 html-тест (мид-параграф `==`/`====` не рвут, негатив — секция
  после blank работает). **Корпус: Identical 325→326 (+1 ФЛИП, admonition
  197→0)**; blast (base 325): ровно 1 файл, **0 регрессий**. clippy 0, test
  --workspace зелёное (parser 495, html 400), parsing-lab 233/233.
  Остаток (отдельные корни, тот же over-eager break-список): table.adoc
  (`<2>`/callout-list-item рвёт параграф + `|=== <1>` не точный делимитер), и
  общая дивергенция — мы рвём параграф на list/image/thematic/admonition/dlist,
  чего asciidoctor не делает (НЕ исправлено, риск over-fix; брать отдельно).

- [x] **Таблицы: пустая стилевая (m/e/s) ячейка → голый `<td></td>` без обёртки** —
  СДЕЛАНО (ветка `fix/empty-styled-table-cell`, 2026-06-13; смержена в master,
  **ПУШ ОЖИДАЕТ авторизации**). Корень table-ref.adoc (135 diff @848,
  `[cols="1m,2,1m,2,2"]`): пустая ячейка m-колонки эмитила
  `<p class="tableblock"><code></code></p>`, asciidoctor — голый `<td></td>`
  (table.rb Cell#content: empty text → `[]`, нет параграфов). Default- и
  header-ячейки уже откатывались корректно; literal/AsciiDoc обёртку сохраняют
  даже пустой (совпадает). Фикс: распространил существующий `p_start`-механизм
  отката пустой default-ячейки на стили e/s/m. **РЕНДЕРЕР** blocks.rs
  `start_table_cell` — arm'ы Emphasis/Strong/Monospace пишут `p_start` после
  обёртки; events.rs `TagEnd::TableCell` — `is_empty` откатывает полную обёртку
  (`<p class="tableblock"><em>` и т.п.). Мультипараграфные ячейки не триггерят
  (p_start после первой обёртки, каждый para непуст). +1 html-тест. **Корпус:
  Identical 324→325 (+1 ФЛИП, table-ref 135→0)**; blast (base 324): ровно 1 файл,
  **0 регрессий**. clippy 0, test --workspace зелёное (parser 495, html 399),
  parsing-lab 233/233.

- [x] **Таблицы: cols-спек бьётся по `;` так же, как по `,` (взаимоисключающе)** —
  СДЕЛАНО (ветка `fix/table-cols-semicolon-separator`, 2026-06-13; смержена в
  master `1745038`, **ПУШ ОЖИДАЕТ авторизации**). Корень add-title.adoc (252 diff,
  `[cols=1;m;m]`) и image-ref.adoc (748 diff, `[cols=2;2;3;3]`): разделитель cols
  у asciidoctor — `,` ИЛИ `;` (есть запятая → split по `,`, иначе по `;`; `;`
  используют без кавычек, т.к. attrlist-сплиттер сам режет запятые). И **ПАРСЕР**
  attributes.rs `table_col_specs`, и **РЕНДЕРЕР** blocks.rs `parse_col_widths`
  (дубль парсинга для colgroup-ширин) бились только по `,` → `1;m;m` схлопывался
  в 1 колонку, ломая colgroup (1 `<col>` вместо N) и через num_cols-зависимую
  header-детекцию подавляя `<thead>`. Фикс: `sep = if contains(',') {','} else
  {';'}` в обоих местах. +1 parser-тест (`1;m;m`→3, `2*;m`→3, смешанный `1,m;m`→2),
  +1 html-тест (3×`<col>`, thead, `<code>` в m-ячейках). **Корпус: Identical
  322→324 (+2 ФЛИПА)**; blast (base 322): ровно 2 файла (add-title, image-ref —
  pre-existing colgroup/thead-корень из сессий 41/42), **0 регрессий**. clippy 0,
  test --workspace зелёное (parser 495, html 398), parsing-lab 233/233.

- [x] **Списки: `-`-маркер вкладывается под `*` (идентичность маркера, не число) +
  класс стиля маркера (`[square]`/`[circle]`/…) на `<ul>` и вложенных списках** —
  СДЕЛАНО (ветка `fix/unordered-dash-marker-nesting`, 2026-06-13; смержена в master
  `65e2113`, **ПУШ ОЖИДАЕТ авторизации** — отклонён авто-классификатором). Два
  корня unordered.adoc (145 diff), оба про маркеры unordered-списка.
  (1) **ПАРСЕР** scanner.rs `is_list_marker_unordered`: `-` возвращал identity 1,
  коллизия с `*` → `- x` под `* y` рендерился плоским sibling. Asciidoctor матчит
  ЛИТЕРАЛЬНЫЙ маркер по стеку (число звёзд = идентичность, не уровень — пробы
  /tmp/p_un1..5: `* a`/`- b`/`* c` вкладывает `-`, `- a`/`** b`/`* c` вкладывает
  `*` ГЛУБЖЕ). Фикс: `-` → identity `0` (вне диапазона `*`-счёта), не коллизирует.
  (2) **РЕНДЕРЕР** blocks.rs `start_unordered_list`: стиль был на div (через
  write_meta_attrs), но НЕ на `<ul>`; вложенные стилевые списки роняли его совсем
  (inside-list-item ветка хардкодила `ulist`, игнорировала meta). Фикс: обе ветки
  унифицированы через write_meta_attrs, класс стиля добавлен на `<ul>` (только
  style — roles/id на div; пробы /tmp/p_sq,p_sqr,p_ov,p_nest_sq). Bibliography
  осталась top-level-only. +2 html-теста, scanner +2 ассерта. **Корпус: Identical
  321→322 (+1 флип, unordered.adoc 145→0)**; blast (base 321): ровно 1 файл,
  **0 регрессий**. clippy 0, test --workspace зелёное (parser 494, html 397),
  parsing-lab 233/233.

- [x] **Includes: `leveloffset` сдвигает и level-0 заголовки (`= Title`)** —
  СДЕЛАНО (ветка `fix/include-leveloffset-level0`, 2026-06-13; в master
  `e5ff3b1`). Корень architecture/index.adoc (189 diff) и двух cookbook-файлов
  (2481, 2313): `include::monitoring.adoc[leveloffset=+1]` — включаемый `=
  Мониторинг` (level 0) должен стать level-1 секцией (`<div class="sect1"><h2>`),
  но `apply_level_offset` (preprocessor.rs) пропускал одиночный `=` (guard
  `eq_count >= 2`) и клампил в `2..=6` → level-0 оставался sect0/h1.
  Семантика — пробы /tmp/p_lo/p1..p5: level-0 сдвигается (`= X` +1 → `== X`),
  отрицательный offset демоутит `==` до level-0 `= X` (`<h1 class="sect0">`),
  минимум — один `=` (level 0). Фикс (preprocessor.rs apply_level_offset):
  guard `(1..=6).contains(&eq_count)`, `clamp(1, 6)`. +2 теста (level0-promote,
  level0-clamp), 1 обновлён (clamp_min: `== Title` -5 → `= Title`, не `== Title`).
  Латентный предел (нет корпуса): level-5 `======` +1 у asciidoctor вообще не
  рендерит секцию (мы клампим в 6 `=`). **Корпус: Identical 318→321 (+3 ФЛИПА)**;
  blast (base 91d4e24): ровно 3 файла, **0 регрессий**. clippy 0, test
  --workspace зелёное (parser 494, html 395), parsing-lab 233/233.

- [x] **Таблицы: явный оператор выравнивания ячейки (`<`/`^`/`>`, `.<`/`.^`/`.>`)
  побеждает дефолт колонки** — СДЕЛАНО (ветка `fix/table-cell-explicit-alignment`,
  2026-06-13; в master `b1f52f2`). ОДИН diff cell.adoc @574: ячейка `.3+<.>m`
  в `[cols="e,m,^,>s"]` (колонка `>`=Right) ставит явный `<`=Left, но старый
  resolve_align не отличал явный Left от дефолтного Left → накрывал дефолтом
  колонки (`halign-right`); asciidoctor уважает оператор (`halign-left`).
  Заметки 42-й сессии предполагали корень в col_idx (rowspan-сдвиг) — гипотеза
  опровергнута разбором грида (col_idx этой ячейки=3 верный в обоих вариантах).
  **ПАРСЕР** scanner.rs: `CellSpec`/`ExactCellSpec` + `halign_explicit`/
  `valign_explicit` (образец `style_explicit`); parse_cell_align_prefix/_suffix
  возвращают флаги. **ПАРСЕР** block.rs resolve_align: `if !cell.halign_explicit
  { halign = col_default }` вместо `value==default`-эвристики — строго более
  корректно, меняет поведение только для явного `<`/`.<` поверх недефолтной
  колонки. +1 html-тест, scanner-тесты на флаги. Латентный остаток (нет
  корпусного кейса): emit_row_cells col_idx всё ещё наивный (не occupancy-aware).
  **Корпус: Identical 317→318 (+1 флип, cell.adoc 1→0)**; blast (base 5b5d958):
  ровно 1 файл, **0 регрессий**. clippy 0, test --workspace зелёное (parser 492,
  html 396).

- [x] **Таблицы: blank-строка в DEFAULT/стилевой ячейке → несколько
  `<p class="tableblock">` параграфов** — СДЕЛАНО (ветка
  `fix/table-cell-multi-paragraph`, 2026-06-13; в master `4b477a9`). Корень,
  общий для highlight-lines (185), subs-symbol-repl (165), cell.adoc (965):
  DEFAULT/стилевая body-ячейка с blank-строкой схлопывалась в один параграф,
  asciidoctor бьёт на несколько. Семантика — исходник table.rb:371-385
  (`Cell#content`: RAW `\n\n` → `split(/\n{2,}/)`, стилевой враппер m/e/s на
  КАЖДЫЙ параграф, html5 оборачивает в `<p class="tableblock">`) + пробы
  /tmp/p_cellp/p1..p6 (все IDENTICAL).
  **ПАРСЕР** event.rs: `Event::TableCellParagraphBreak` (unit-маркер).
  block.rs: `cell_paragraphs()` — split на blank, Literal/AsciiDoc не бьются;
  `len<=1` → старый `cell_text` (байт-в-байт zero-copy), иначе Text(para) +
  маркер между. **РЕНДЕРЕР** events.rs: маркер закрывает/открывает `<p>` (+
  стилевой враппер по `cell_style_stack`). compat builder.rs: no-op.
  +1 html-тест, 1 обновлён (литерал-тест кодировал баговое схлопывание).
  Предел: continuation-отступ внутри параграфа тримится (asciidoctor
  сохраняет; pre-existing, нет корпусной выгоды). **Корпус: Identical
  314→317 (+3 флипа)**: align-cell 211→0, highlight-lines 185→0,
  subs-symbol-repl 165→0; cell.adoc 965→1, image-svg ближе; blast (base
  92ca10a): **0 регрессий** (image-ref 746→748 — позиционный шум поверх
  pre-existing colgroup/thead корня, split сверен с эталоном). clippy 0,
  test --workspace зелёное (html 394).

- [x] **Description-list: пустой principal-текст dd + присоединённый блок →
  без `<p>`-обёртки** — СДЕЛАНО (ветка `fix/empty-dd-principal-paragraph`,
  2026-06-13; в master `23b4420`). Общий корень группы spec/doc-файлов:
  dd с пустым principal-текстом, но с присоединённым блоком (list / open-block
  через `+` / nested dlist через смежность) эмитил пустой `<p></p>` перед
  блоком; asciidoctor `<p>` не выводит вовсе (convert_dlist: `<p>` только при
  `dd.text?`). Семантика — пробы /tmp/p_dd/p1..p7 (все IDENTICAL).
  **РЕНДЕРЕР** events.rs: в guard'е закрытия принципиального `<p>` при старте
  суб-блока — если `output.ends_with("<p>")` (principal пуст), откатить `<p>`
  (`truncate(len-3)`) вместо `</p>`; иначе как раньше. Робастно для
  normal/styled (`<dd>\n<p>`) и horizontal (`<td class="hdlist2">\n<p>`);
  текст/чекбоксы/маркеры дают иное окончание → ложного отката нет. +1 html-тест.
  **Корпус: Identical 304→314 (+10 ФЛИПОВ)** — CHANGELOG 1994→0, sdr-001..008,
  release-and-progress-reviews 406→0; closer cookbook/ts-url-format;
  **0 регрессий** (description.adoc +1 — позиционный шум поверх pre-existing
  `<colgroup>`-корня гориз. dlist). clippy 0, test --workspace зелёное (976).

- [x] **Block-media макросы: якорь `]$` (trailing-контент → параграф) + image
  link из attr-строки, inline role/title, порядок float/align, imagesdir**
  — СДЕЛАНО (ветка `fix/block-media-macro-trailing-content`, 2026-06-13; в
  master `54317ee`). Пять корней image.adoc (125 diff), семантика — пробы
  /tmp/p_img/* + исходник gem'а (rx.rb:421 BlockMediaMacroRx, html5.rb
  convert_image/convert_inline_image, abstract_node.rb image_uri,
  path_resolver.rb web_path).
  (1) **ПАРСЕР** scanner.rs: `match_block_media(line,prefix)` — image/video/
  audio обязаны заканчиваться `]` (strip_suffix вместо rfind); target
  непустой, без whitespace по краям (внутренний OK). Trailing-контент
  (`image::x[] <.>`) → параграф.
  (2) **ПАРСЕР** block.rs: block image `link=` берётся из attr-строки
  (`[#id,link=…]`), если нет в макрос-attrs.
  (3) **ПАРСЕР** event.rs/inline.rs: `Tag::InlineImage` +role +title.
  (4) **РЕНДЕРЕР** media.rs: inline span class = `image`+float+role (align
  не эмитится), title → атрибут img; image_base_class фикс. порядок
  float→align; НОВОЕ image_uri(&self)+is_uriish — imagesdir префиксится к
  non-URI таргетам, читается живо (mid-document). +5 html-тестов, +9 кейсов
  scanner. Предел: `..`/`.` в joined imagesdir-пути не нормализуются.
  **Корпус: Identical 303→304 (+1 флип, image.adoc 125→0)**; blast (base
  32ac8cc): ровно 1 файл, **0 регрессий**. clippy 0, test --workspace
  зелёное (975).

- [x] **Includes: uriish-таргет без allow-uri-read → `link:target[role=include]`**
  — СДЕЛАНО (ветка `fix/uriish-include-link`, 2026-06-13; в master `594d16a`).
  Один корень apply-subs-to-text.adoc (115 diff): `include::pass:example$…`
  (Antora resource-id) — таргет с URI-схемой (UriSniffRx:
  `\A\p{Alpha}[\p{Alnum}.+-]+:/{0,2}`, схема ≥2 символов — Windows-диски
  остаются путями) у asciidoctor заменяется на bare-ссылку
  `link:<target>[role=include]` (reader.rb resolve_include_path), attrlist
  и optional отбрасываются; рендер `<a class="bare include">`. Таргет с
  пробелом — без `pass:c[…]`-обёртки (она нужна только link-regex'у
  asciidoctor; наш link-макрос даёт тот же HTML как есть). **ПАРСЕР**
  preprocessor.rs: is_uriish + ветка в resolve_includes_rec до файловых
  операций. +1 тест (5 кейсов). **Корпус: Identical 302→303 (+1 флип,
  apply-subs-to-text 115→0)**; blast (base ca6a35e): ровно 2 файла — флип +
  syntax-quick-reference 2828→2788 ближе, **0 регрессий**. clippy 0,
  test --workspace зелёное (970).

- [x] **Header: revision line после attr-entries + точная модель
  RevisionInfoLineRx** — СДЕЛАНО (ветка `fix/metadata-revision-line`,
  2026-06-13; в master `d5d3f24`). Один корень metadata.adoc (111 diff),
  семантика — пробы /tmp/p_meta/p1..p16 + parse_header_metadata
  (parser.rb:1815-1866) и RevisionInfoLineRx (rx.rb:42).
  (1) **ПАРСЕР** scanner.rs parse_revision_line — зеркало регэкспа
  (матчит почти всё): freeform-строка → revdate; запятая без цифр до неё
  → revnumber set-empty (рендер `version ,`); хвостовое голое `:` →
  revremark set-empty; v-компонента — slice(1) буквально (`version 5` →
  `ersion 5`), только строчная v; `:`-старт → unshift в body.
  `RevisionInfo.version/remark` → `Option<&str>` (set-empty ≠ absent).
  (2) **ПАРСЕР** block.rs: attr-entries/комментарии прозрачны между
  author и rev line (consume_header_attr_entries, также заменил хвостовой
  цикл); author/rev-строки больше не исключают section-маркеры
  (`= T`+`== Sec` без blank → author «== Sec», по asciidoctor).
  Author после attr-entry ОСТАВЛЕН по спеку parsing-lab (параграф) —
  asciidoctor расходится (author), осознанная дивергенция
  (block/header/adjacent-to-body). +1 html-тест (7 кейсов), scanner-тесты
  на Option. **Корпус: Identical 301→302 (+1 флип, metadata.adoc 111→0)**;
  blast (base 06e6b03): ровно 1 файл, **0 регрессий**. clippy 0,
  test --workspace зелёное (969).

- [x] **Секции: нумерация appendix — буквенные numeral-цепочки, атрибут
  appendix-caption, per-parent ordinals** — СДЕЛАНО (ветка
  `fix/appendix-numbering`, 2026-06-13; в master `be3044a`). Три корня
  appendix.adoc (24 diff), семантика — пробы /tmp/p_appx/p1..p9 (все
  IDENTICAL) + исходник gem'а (abstract_block.rb assign_numeral:408-423,
  section.rb sectnum:119-122, parser.rb:1619).
  (1) **RENDER-CORE** SectionNumberer: appendix_prefix(level, caption) —
  буква из документ-глобального счётчика (сквозь части/уровни); формы
  caption по assign_numeral: атрибут есть → `"{caption} {L}: "` (пустой →
  " A: "), unset → `"{L}. "`; буква занимает уровень в sectnum-цепочках
  потомков (`A.1.`, вложенный appendix → `1.A.1.`), арабский ordinal
  родителя НЕ потребляется (сиблинг после appendix продолжает счёт);
  reset_descendant_ordinals(). Caption appendix виден и БЕЗ :sectnums:
  (numbered=true всегда); подсекции нумеруются только при :sectnums:.
  (2) **РЕНДЕРЕР**: дефолтный атрибут appendix-caption=«Appendix»
  (значение экранируется); doctype фиксируется на закрытии header
  (body-`:doctype:` не меняет структуру); article body-sect0 рестартит
  per-parent ordinals детей, book-части — нет (глобальный chapter-number).
  +3 html-теста, расширен core-тест. **Корпус: Identical 300→301 (+1 флип,
  appendix.adoc 24→0)**; blast (base 18dab28): ровно 1 файл,
  **0 регрессий**. clippy 0, test --workspace зелёное (967).

- [x] **Секции: коэрсия level-0 спец-секций + partintro для book-частей +
  части в TOC + стилевой dlist + TOC после author details** — СДЕЛАНО
  (ветка `fix/part-special-sections`, 2026-06-13; в master `fd99bb7`).
  Корни part-with-special-sections (103 diff) и multipart-book (109 diff),
  семантика — пробы /tmp/p_part/p1..p13, m1 (все IDENTICAL) + исходник gem'а
  (parser.rb initialize_section:1593-1626, next_section:400-440).
  (1) **ПАРСЕР** block.rs scan_section: стиль на секции = спец-секция;
  level-0 → display level 1 (`[preface]` + `= T` → sect1/h2 + sectionbody);
  book `[abstract]` → chapter level 1 с ЛЮБОЙ глубины; sect\d-стили и
  discrete/float исключены. Коэрсия display-only: закрытие по сырому
  уровню маркера (`[appendix] = X` после части ЗАКРЫВАЕТ часть — сиблинг).
  (2) **ПАРСЕР** block.rs: partintro — ведущие блоки голой level-0 части в
  book оборачиваются в open-блок partintro (BlockContext::PartIntro, до
  первой дочерней секции/EOF); голый `--` open-блок рестайлится на месте;
  explicit `[partintro]`-блок — сам себе intro (существующий маскарад
  параграфа), последующие блоки до секции — СНАРУЖИ (error-путь
  asciidoctor); комментарии intro не открывают.
  (3) **RENDER-CORE** TocEntry.depth (глубина дерева): вложенность TOC — по
  дереву, класс sectlevelN — по display-уровню (max(level-1,1));
  toc_steps открывает ul только для реально встреченных уровней
  (скачок уровней — без пустых ul); level-1 (части/body sect0) видимы.
  (4) **РЕНДЕРЕР** events.rs: авто-TOC в header вставляется ПОСЛЕ
  `<div class="details">` (порядок asciidoctor: h1, details, toc).
  (5) **РЕНДЕРЕР** dlist: любой стиль кроме horizontal/qanda → класс
  `dlist <style>`, `<dt>` теряет hdlist1 (`[glossary]`).
  +5 тестов. **Корпус: Identical 298→300 (+2 ФЛИПА)**; blast (base master
  2c4a292): appendix 158→24 и outline 8681→6597 ближе, **0 регрессий**.
  clippy 0, test --workspace зелёное (964).

- [x] **Блоки: quoted-paragraph shorthand + markdown blockquote +
  одиночные кавычки в attrlist-значениях (с subs)** — СДЕЛАНО (ветка
  `fix/quoted-paragraph-and-md-blockquote`, 2026-06-12; в master `6426a5f`).
  Три корня quote.adoc (109 diff), семантика — пробы /tmp/p_subs/p11, p12 +
  asciidoctor parser.rb:770-810.
  (1) **ПАРСЕР** block.rs scan_paragraph: параграф `"...` + предпоследняя
  строка `..."` + последняя `-- attribution[, citetitle]` → quote-блок с
  ГОЛЫМ контентом (без `<p>`-враппера) + attribution; кавычки стрипаются,
  credit получает normal subs (apply_subs).
  (2) **ПАРСЕР** block.rs: markdown-blockquote `> ...` — один уровень `>`
  стрипается, остальное парсится как COMPOUND через вложенный BlockScanner
  (new_nested: body-контекст, готовые строки) — работают вложенные `> >`,
  списки, разбивка параграфов по голому `>`; trailing `-- ...` →
  attribution; md_quote_depth cap 16 от патологической рекурсии.
  (3) **ПАРСЕР** attributes.rs: split_respecting_quotes понимает `'...'`;
  кавычка открывается ТОЛЬКО после `,`/`=` (апостроф в слове — текст);
  позиционные теряют обрамляющие кавычки; single-quoted индексы —
  в новом поле single_quoted_positionals → named-маркеры
  attribution-subs/citetitle-subs → РЕНДЕРЕР render_quote_attribution
  (дедуп quote/verse армов) рендерит флагнутые через render_inline_value
  (только `'...'`-значения получают subs — проба p12). +3 html-теста.
  **Корпус: Identical 297→298 (+1)** (quote.adoc 109→0); blast: 3 флипа за
  сессию, sdr-004/description — позиционный сдвиг поверх pre-existing
  (фрагменты сверены с эталоном), **0 регрессий**. clippy 0, тесты зелёные.

- [x] **Списки: вложенность по стеку маркеров (несматченный маркер вкладывается
  в текущий item) + стиль olist от числа точек маркера** — СДЕЛАНО (ветка
  `fix/mixed-marker-list-nesting`, 2026-06-12; в master `83c71e4`). Корень
  ordered.adoc (90 diff) — известный pre-existing «nested-список с другим
  маркером в li». Семантика (пробы /tmp/p_subs/p6, p8, p9): маркер, матчащий
  ОТКРЫТЫЙ список (текущий или предка) → закрыть до него, sibling-item;
  НЕсматченный (глубже, мельче или другого типа) → НИЧЕГО не закрывать,
  вложить новый список в самый внутренний открытый item (даже `** b`+`* c` —
  вложение!). Стиль olist — от числа точек маркера (`.` arabic,
  `..` loweralpha, `...` lowerroman), НЕ от вложенности `<ol>`.
  (1) **ПАРСЕР** block.rs: scan_ordered получил cross-type
  close_to_parent_list (была асимметрия с unordered); else-ветки обоих —
  без закрытий (вложение); close_list_items_for_depth удалён,
  BlockContext::ListItem без depth. (2) **ПАРСЕР+РЕНДЕРЕР**:
  Tag::OrderedList несёт depth; start_ordered_list — implicit-стиль от
  depth вместо подсчёта открытых ol. +2 html-теста. Вскрытые pre-existing:
  пустой `<p></p>` в dd, держащем только вложенный `:::`-dlist
  (description.adoc); `'''` после списка не закрывает контексты
  (`<hr>` внутри `<p>` — и в base). **Корпус: Identical 296→297 (+1)**
  (ordered.adoc 90→0); blast: 2 файла — 1 флип, description.adoc 295→298 —
  позиционный сдвиг поверх pre-existing (структура вложенности сверена с
  эталоном — теперь совпадает), **0 регрессий**. clippy 0, тесты зелёные (957).

- [x] **Inline: точная модель index-term (скользящее закрытие, paren-формы,
  эскейпы) + `\\` гасит unconstrained-пару; препроцессор: attr-refs в
  attrlist-строках** — СДЕЛАНО (ветка
  `fix/escaped-index-term-and-double-backslash-unconstrained`, 2026-06-12;
  в master `4b66e7a`). Три корня subs.adoc (76 diff), семантика
  верифицирована пробами /tmp/p_subs/p1..p5 + исходником asciidoctor
  (substitutors.rb:439-514, InlineIndextermMacroRx).
  (1) **ПАРСЕР** inline.rs: try_index_term — один паттерн
  `\(\((.+?)\)\)(?!\))`: non-greedy закрытие «скользит» мимо хвостов `)))`;
  скобки САМОГО контента решают форму: с обеих сторон → concealed,
  с одной → литеральная скобка before/after вокруг flow-term, иначе flow.
  Эскейп `\((..))` → весь матч литерально минус backslash; `\(((..)))` →
  `(` + ВИДИМЫЙ flow-term + `)`. Старые try_concealed/try_flow удалены.
  (2) **ПАРСЕР** inline.rs: `\\` перед unconstrained-парой
  (`**`/`__`/`##`/двойной backtick) съедает ОБА backslash, марки литеральны,
  контент с обычными subs (зеркало каскада gsub-пассов asciidoctor:
  unconstrained-pass снимает один `\`, constrained-pass — второй).
  (3) **ПАРСЕР** preprocessor.rs: expand_attr_refs_in_attrlist —
  attr-refs в block-attrlist строках (`[source,subs="{markup}"]`)
  раскрываются в document-order; unknown/escaped refs и строки в
  verbatim-фенсах не трогаются. Пределы (вне корпуса): одиночный `\` перед
  unconstrained (`\__one__` → asciidoctor `<em>_one_</em>`, у нас литерал);
  `` \\`mono` `` (constrained code: asciidoctor хранит один `\` без
  форматирования, у нас `\`+`<code>`). +4 теста.
  **Корпус: Identical 295→296 (+1)** (subs.adoc 76→0); blast (base
  dd7cf69): ровно 1 файл — 1 флип, **0 регрессий**. clippy 0,
  test --workspace зелёное.

- [x] **Таблицы: escaped `\|` + width-атрибут + стилевые веса cols; inline:
  passthrough-скип в unconstrained-спанах** — СДЕЛАНО (ветка
  `fix/pass-macro-and-delimited`, 2026-06-12; в master `dd7cf69`). Четыре
  корня (pass-macro 3 diff + delimited 9 diff), пробы /tmp/p_pm, /tmp/p_dl.
  (1) **ПАРСЕР** scanner.rs: `|` сразу после `\` — не разделитель ячеек,
  ровно один `\` снимается (`\|`→`|`, `\\|`→`\|`), и в continuation-строках;
  unescape_cell_pipes/find_unescaped_pipe/split_unescaped_pipes,
  `TableLineCells.continuation` → `Option<Cow>`. (2) **РЕНДЕРЕР** blocks.rs
  start_table: tablepcwidth = Ruby to_i от width (вне (0..100] → 100, кроме
  literal "0"/"0%"); 100 → `stretch`, иначе `style="width: N%;"`; явный width
  подавляет fit-content даже при %autowidth. (3) **РЕНДЕРЕР**
  parse_col_widths: trailing-стилевая буква не часть веса (`1m,3m` → 25/75).
  (4) **ПАРСЕР** inline.rs find_closing_unconstrained скипает
  `++…++`/`+++…+++`/`pass:[…]` (зеркало constrained) — `**a+++**+++b**` →
  strong над `a**b` (закрыт `+++**+++` в listing с subs="+quotes,+macros").
  +5 тестов. **Корпус: Identical 292→295 (+3)** (pass-macro, delimited,
  data-format 615→0); blast (base 0a1e5fc): 4 файла — 3 флипа,
  character-replacement-ref — позиционный сдвиг поверх pre-existing корня
  (новый фрагмент сверен с эталоном), **0 регрессий**. clippy 0,
  test --workspace зелёное (parser 486, html 374).

- [x] **Footnotes: повторное использование определённого id — ссылка** — СДЕЛАНО
  (ветка `fix/footnote-named-reuse`, 2026-06-12; в master `096bd8d`). Корень
  footnote examples (70 diff, один корень): `footnote:id[…]` с уже
  определённым id — ССЫЛКА на первое определение (`<sup class="footnoteref">`,
  анкор без id-атрибутов, текст игнорируется, счётчик не бампится); пустой
  `footnote:id[]` без определения — unresolved-маркер
  (`<sup class="footnoteref red" …>[id]</sup>`), forward-ref нет. Пробы
  /tmp/p_fnr/p1..p3. Фикс: **РЕНДЕРЕР** events.rs (lookup-сначала +
  push_footnote_ref + unresolved-арм). **Корпус: Identical 284→285**; blast:
  ровно 1 файл — 1 флип, **0 регрессий**. clippy 0, тесты зелёные (943).

- [x] **Таблицы: a-ячейки (nested block-парсинг), наследование колоночных
  стилей, literal-ячейки `<div class="literal">`, blank/indent в контенте
  ячейки, спек-чары d/v** — СДЕЛАНО (ветка `fix/asciidoc-table-cell`,
  2026-06-12; в master `b742c4b`). Закрыт давний архитектурный предел `a|`.
  Семантика (пробы /tmp/p_acell/p1..p12): a-ячейка → `<div class="content">`
  + nested block-парсинг ЧЕРЕЗ ТОТ ЖЕ рендерер (footnote-счётчик/xref общие
  с документом); колоночные стили (a/h/e/m/s/l) наследуются ячейками без
  явного стиля, явный (включая новые `d`/`v` → explicit Default) побеждает,
  header-строки игнорируют колоночные стили; `l` → `<div class="literal">
  <pre>` с VERBATIM-subs; blank-строки/отступы — часть контента ячейки
  (структурны для a, сохраняются в l, схлопываются для остальных).
  **ПАРСЕР**: parser.rs cell_subs_pushed (a→NONE, l→VERBATIM); scanner.rs
  style_explicit + d/v + отступ continuation; block.rs resolve_style
  (полное наследование), blank→append_cell_continuation(""), cell_text по
  стилю на эмиссии. **РЕНДЕРЕР**: acell_capture-стек, guard в Text-арме,
  nested-рендер на TagEnd, literal-армы. Предел (p6): footnote, определённая
  в a-ячейке — div уходит во внешнюю секцию (asciidoctor — внутрь ячейки).
  +3 теста, 1 переписан. **Корпус: Identical 285→292 (+7)**
  (asciidoc-vs-markdown 988→0, blocks/index, ordered, footnote pages,
  bibliography, format-cell-content, format-column-content); blast (base
  096bd8d): 18 файлов — 7 флипов, **0 регрессий**, сильно ближе: pass-macro
  241→3, table-ref 893→135, delimited 307→9, highlight-lines 286→185.
  Новый вскрытый корень: `++…++` НЕ экранирует спецсимволы (pre-existing,
  block-name-table; только `+++` без экранирования — проба p11). clippy 0,
  тесты зелёные (946).

- [x] **Таблицы: blank после `|===` гасит implicit header** — СДЕЛАНО (ветка
  `fix/add-columns-nearmiss`, 2026-06-12; в master `7d9f2eb`). Корень
  add-columns.adoc (40 diff, один корень): blank-строка (одна или несколько)
  между `|===` и первой data-строкой у asciidoctor подавляет промоушн первой
  строки в header; явный `[%header]` промоутит всё равно; colcount
  по-прежнему из первой строки; comment-строки прозрачны (comment без blank
  не гасит, blank до/после comment — гасит). Семантика верифицирована
  пробами /tmp/p_ac/p1..p8 (все IDENTICAL). Фикс: **ПАРСЕР** block.rs
  scan_table — флаг `blank_before_first_data` в цикле сбора + условие в
  гейте `implicit_header`. +1 html-тест (6 кейсов). **Корпус: Identical
  282→284 (+2)** (add-columns 40→0, column.adoc 172→0); blast (base
  43f7ab1): 4 файла — 2 флипа, cell.adoc 975→965 ближе, table.adoc 556→597
  — позиционный шум поверх pre-existing (`|=== <1>` в параграфе → colist;
  изолированная таблица сверена с эталоном), **0 регрессий**. clippy 0,
  test --workspace зелёное (parser 485, html 366).

- [x] **Таблицы: открытая модель ячейки (continuation-строки, пустые ячейки,
  дупликация `N*`, цепочки спеков, дроп неполной строки, comment-строки)** —
  СДЕЛАНО (ветка `fix/align-by-column`, 2026-06-12; НЕ закоммичено). Старт —
  align-by-column.adoc (7 diff), по дороге вскрыт и закрыт целый кластер
  psv-семантики (пробы /tmp/p_abc/p1..p17, все IDENTICAL кроме
  задокументированных пределов).
  (1) **ПАРСЕР** scanner.rs: `parse_table_cells` → `TableLineCells
  { continuation, cells }` — текст до первого `|` строки (или строка без `|`)
  продолжает последнюю ячейку ПРЕДЫДУЩЕЙ строки (join `\n`, тот же `<p>`);
  спек может стоять между текстом и `|` (`tail 2+|wide`); `CellSpec.content`
  → `Cow<str>`. (2) scanner.rs: `parse_cell_spec_exact` — зеркало CellSpecRx:
  span (`2+`/`.3+`/`2.3+`) ИЛИ дупликация (`3*`), затем align, затем style —
  целиком; чинит цепочки `2*>m`, `.2+^.>s` (раньше строка молча дропалась);
  дупликация хранится в CellSpec и раскрывается ПОСЛЕ сбора continuation
  (копии несут полный контент). (3) Пустые ячейки реальны (`|a |` → 2 ячейки,
  `|a | |c` — mid): пушатся всегда; **РЕНДЕРЕР** — пустая ячейка = голый
  `<td></td>` (cell_p_start_stack, truncate пустого `<p class="tableblock">`).
  (4) **ПАРСЕР** block.rs: implicit header = blank СРАЗУ после первой строки
  + следующая non-blank строка начинается с ячейки (continuation гасит);
  implicit colcount = ячейки первой строки ПОКА она открыта (ячейки с
  continuation-строк считаются); неполная последняя строка дропается
  (asciidoctor «dropping cells from incomplete row»); comment-строки невидимы
  (дроп из контента, не влияют на header/colcount). (5) **ПАРСЕР** parser.rs:
  `Event::Text(Cow::Owned)` теперь тоже идёт через inline-парсер
  (into_static) — слитые ячейки и CSV-поля получают normal subs.
  Пределы (вне корпуса): continuation после blank → второй `<p>` в ячейке
  (у нас один, join `\n`, p9/p16); CSV-путь не дропает неполную строку (p11);
  `a|`-ячейка без nested-рендера (p14, давний). +5 тестов (2 scanner,
  3 html-мультикейсовых), 2 ассерта обновлены probe-verified.
  **Корпус: Identical 267→282 (+15)** (align-by-column, build-a-basic-table,
  add-cells-and-rows, row, style-operators, section-ref, header-ref,
  audio-and-video, link-macro-ref, unresolved-references, toc-ref, subs ×4);
  blast (base 4099d62): 37 файлов — 15 флипов, **0 регрессий**, большинство
  остальных ближе (table 612→556, subs-symbol-repl 226→165, replacements
  148→4 — остаток NCR-кластер, document-attributes-ref 6672→6538); рост
  счётчиков image-ref/image-svg/cell/table-ref — позиционный шум поверх
  pre-existing корней (`cols=2;2;3;3` `;`-разделитель не парсится, l|-ячейка
  не `<div class="literal">`), новые фрагменты точечно сверены с эталоном.
  clippy 0, test --workspace зелёное (941).

- [x] **footnotes вне #content + merge стопки attrlist + cols-multiplier +
  trailing cell-spec + счётчики/attr-entries в verbatim** — СДЕЛАНО (ветка
  `fix/pages-include-nearmiss`, 2026-06-12; НЕ закоммичено). Пять корней
  (pages/include.adoc 8 diff + customize-title-label.adoc 66 diff + вскрытые
  по дороге pre-existing), семантика верифицирована пробами /tmp/p_fn,
  /tmp/p_ctl (p1..p11, m*, n*).
  (1) **РЕНДЕРЕР** finish.rs/lib.rs: в standalone `<div id="footnotes">`
  эмитится ПОСЛЕ закрытия `#content`, перед footer (был внутри content).
  (2) **ПАРСЕР** attributes.rs+block.rs: стопка `[...]`-строк над блоком
  МЕРЖИТСЯ (`BlockAttributes::merge`), не заменяется последней: named —
  override по ключу, id — last-wins, roles/options — аккумулируются,
  позиционные — послотно (`[quote,Author]`+`[verse]` → verse+attribution;
  `[source,ruby]`+`[,python]` → python). Закрыт `[caption="Table A. "]` /
  `.Title` / `[cols=...]` (caption терялся).
  (3) **РЕНДЕРЕР** blocks.rs parse_col_widths: multiplier `N*` (`3*` → 3
  колонки, `2*1,3` → 20/20/60) — парсер умел, рендерер колонок нет.
  (4) **ПАРСЕР** scanner.rs parse_table_cells: спек-суффикс (style/span/align)
  привязан к следующему `|` — в конце строки это КОНТЕНТ (`|a` терял ячейку
  целиком — 'a' стиль-буква; `|d |e` терял «e»; conum-текст `<.>` съедался).
  (5) **ПАРСЕР** preprocessor.rs: verbatim-фенсы (`----`/`....`/`++++`/`////`
  точной длины + markdown ```) — внутри счётчики НЕ раскрываются и
  attr-entries НЕ потребляются (asciidoctor: attributes-sub в verbatim нет);
  conditionals/include остаются reader-level (работают внутри). Пределы:
  ячейка `a|` без nested-рендера (content-div); `[subs="+attributes"]` на
  listing не раскрывает счётчик. +7 тестов (scanner, attributes, preprocessor,
  4 html). **Корпус: Identical 262→267 (+5)** (pages/include,
  customize-title-label, subs-group-table ×2, image-position); blast (base
  313a275): 17 файлов — 5 флипов, **0 регрессий**, 10 ближе (align-by-column
  617→7, row 310→81, add-columns 211→40, footnote 101→70), column/table —
  позиционный шум (сверено с эталоном). clippy 0, test --workspace зелёное
  (935).

- [x] **include-директива = форма строки точно + comment в середине параграфов +
  autolink-границы** — СДЕЛАНО (ветка
  `fix/include-directive-shape-and-mid-paragraph-comments`, 2026-06-12;
  НЕ закоммичено). Три корня examples/include.adoc (52 diff), семантика
  верифицирована пробами /tmp/p_inc (16 проб p*/q*/r* — все IDENTICAL).
  (1) **ПАРСЕР** scanner.rs `is_include_directive` — зеркало IncludeDirectiveRx:
  директива только с колонки 0 (индент → литерал), `]` — последний символ
  строки (rstrip; хвост ` <.>` → обычный текст + conum), target без `[` и без
  краевых пробелов; preprocessor.rs `\include::`-escape гейтится
  directive-shape (не-директива хранит backslash). (2) **ПАРСЕР** block.rs —
  line-comment в СЕРЕДИНЕ параграфа дропается, строки сливаются
  (skip_line_comments asciidoctor): scan_paragraph (кроме verbatim-стилей),
  scan_admonition, wrapped-строки ulist/olist/colist item'ов, dd-текст dlist
  (закрыт pre-existing «comment в середине dd-параграфа»); comment+blank
  по-прежнему завершает, `////` по-прежнему рвёт, «comment после blank рвёт
  списки» сохранено. (3) **ПАРСЕР** inline.rs `try_autolink` — граница
  InlineLinkRx: bare-URL линкуется только после старта/пробела/`<>()[];`
  (`:`/`-`/`=`/`,`/straight-кавычки блокируют — литеральная
  `include::https://…[]` больше не линкуется); trailing `)` стрипается у
  bare-URL (все), но НЕ у формы `URL[text]` (regression key-concepts пойман
  blast'ом и закрыт). Пределы (вне корпуса): `a'https://…` у asciidoctor
  линкуется из-за `;` в NCR `&#8217;` (мы — нет, сырой UTF-8); URL после
  inline-спана (`*b*https://…`) у asciidoctor линкуется через `>` от тега.
  (4) **ПАРСЕР** inline.rs — escaped bare autolink `\https://…`: backslash
  дропается, URL литерален — ТОЛЬКО на валидной границе (после `word-`
  backslash остаётся; LinkRx матчит `\` как часть URL-паттерна); хелперы
  `at_autolink_boundary`/`autolink_scheme_at`. Доп. предел: `\\https://…` —
  asciidoctor хранит ОБА backslash, наш eager `\\`-escape съедает первый
  (pre-existing escape-модель, 23-я сессия).
  Тесты: 1 переписан (line_comment_skipped фиксировал старое), +4 (scanner
  include-shape, preprocessor non-directive verbatim, parser comment-в-item,
  2 html: merge-кейсы + autolink-границы/escape). **Корпус: Identical
  259→262 (+3)** (examples/include.adoc 52→0, document-attributes.adoc 284→0,
  links.adoc 232→0); blast (base 248d240): 11 файлов — 3 флипа,
  **0 регрессий**, 5 ближе (pages/include 75→8, image-ref 686→659,
  subs 89→76), metadata/outline — сдвиговый шум при семантическом приближении
  (точечно сверено с эталоном). clippy 0, test --workspace зелёное
  (parser 480, html 356).

- [x] **source.adoc: точные em-dash правила + include-строка в парсере = текст** —
  СДЕЛАНО (ветка `fix/source-block-nearmiss`, 2026-06-12; НЕ закоммичено).
  Два корня source.adoc (63 diff; в файле `---- <.>` — не делимитер, все пары
  `----` смещены у ОБОИХ — asciidoctor даёт warning «unterminated listing
  block», расходились только параграф-остатки). Семантика верифицирована
  пробами /tmp/p_src/p1..p7 (после фикса все IDENTICAL).
  (1) **ПАРСЕР** inline.rs `apply_typographic_replacements`: правила em-dash
  ровно как у asciidoctor — `(\w)--(?=\w)` (em+ZWSP) и `(^|\n| |\\)--( |\n|$)`
  (thin+em+thin, граничный пробел/`\n` поглощается — строки сливаются; gsub:
  в `a -- -- b` второй литерал). Самодельный арм `---`→em УДАЛЁН (`a---b`,
  `g --- h`, `e----f`, `---- <.>` — литералы). `typographic_escape_len`:
  `\--` — escape только где матчился бы unescaped; `\---` → backslash остаётся.
  (2) **ПАРСЕР** block.rs: include-арм удалён из scan_directives + 4
  break-условия из paragraph/list-сканов — asciidoctor резолвит include только
  в reader (наш препроцессор), строка `include::…[]` у парсера (от escaped
  `\include::`) — обычный текст: параграф, ничего не рвёт. Event::Include в
  enum остаётся (API), арм рендерера мёртвый; scanner::is_include_directive
  жив (препроцессор). Тесты: 5 переписаны (фиксировали самодельное), +кейсы
  probe-verified. **Корпус: Identical 258→259 (+1)** (source.adoc 63→0);
  blast (base 6c5d1a3): 7 файлов — 1 флип, **0 регрессий**, include.adoc
  124→52. clippy 0, test --workspace зелёное (parser 478, html 354).

- [x] **stem.adoc: stem-эскейпы + удаление block-macro catch-all + пустой `++++` +
  `{n!}`-литерал** — СДЕЛАНО (ветка `fix/stem-block-macro-and-escapes`,
  2026-06-12; НЕ закоммичено). Четыре корня stem.adoc (56 diff), семантика
  верифицирована пробами /tmp/p_st/p1..p5 (после фикса все байт-в-байт).
  (1) **ПАРСЕР** inline.rs: `parse_bracket_macro_escaped` — в `stem:[…]`
  (latexmath/asciimath тоже) `\]` не закрывает макрос и unescape'ится
  (`stem:[[[a,b\],[c,d\]\]((n),(k))]` → `\$[[a,b],[c,d]]((n),(k))\$`).
  (2) **ПАРСЕР** block.rs+scanner.rs: catch-all блочного custom-макроса УДАЛЁН
  (is_custom_block_macro/is_known_block_macro/is_valid_macro_name) —
  asciidoctor матчит только зарегистрированные имена, `stem::[…]`/`foo::bar[]`
  → литеральный параграф (`.Title` прикрепляется); Tag::CustomBlockMacro в
  enum остаётся (API); зеркало удаления inline catch-all (23-я сессия).
  (3) **ПАРСЕР** inline.rs: `++++` в тексте = пустой `++`-passthrough → ничто;
  triple-арм при провале ретраит double с той же позиции (бэктрек
  `(\+\+\+?)(.*?)\1`), close==0 разрешён. (4) **ПАРСЕР** inline.rs: имя
  attr-ref строго `\w[\w-]*` — `{n!}`/`{name!fallback}` литерал; самодельный
  `!fallback`-синтаксис удалён (поле fallback в Event остаётся, всегда None).
  Тесты: 2 parser + 4 html переписаны (фиксировали самодельное), +2 parser,
  +3 html. **Корпус: Identical 257→258 (+1)** (stem.adoc 56→0); blast
  (base df05b5f): ровно 1 файл — 1 флип, **0 регрессий**. clippy 0,
  test --workspace зелёное (924: parser 479, html 354).

- [x] **xreflabel/dt-терм → reftext для xref-резолва** — СДЕЛАНО (ветка
  `fix/xreflabel-reftext-resolution`, 2026-06-12; НЕ закоммичено).
  Корень lexicon.adoc (34 diff, один корень): `<<id>>` на `[[id]]term:: def`
  должен резолвиться в текст dt-терма; `[[id,label]]`/`anchor:id[label]` —
  в label. Семантика asciidoctor (пробы /tmp/p_xl/p1..7): reftext по
  умолчанию даёт ТОЛЬКО leading-анкер dlist-терма (в параграфах/ulist-item'ах
  без label — fallback `[id]`); label побеждает терм и форматируется при
  использовании; reftext — разметка (`term with <strong>bold</strong>` внутри
  ссылки); mid-term анкер — fallback; forward-ref работает. Фикс: **ПАРСЕР**
  event.rs — `Tag::Anchor` +поле `label: Option<CowStr>`; inline.rs —
  try_anchor/try_anchor_macro заполняют label. **РЕНДЕРЕР** lib.rs — реестр
  `anchor_reftexts` + `dt_term_start`/`pending_term_anchor`; events.rs — арм
  Anchor (label через render_inline_value; leading-анкер в dt → захват HTML
  терма по позициям вывода на TagEnd::DescriptionTerm, все 3 стиля dlist);
  finish.rs — `ctx.add_block(id, Markup(reftext))`. Предел (НЕ в корпусе):
  label block-anchor-строки `[[id,label]]` НАД блоком не побеждает `.Title`
  (требует плюмбинга через Event::BlockMetadata). +1 html-тест (7 кейсов),
  обновлены parser-тесты. **Корпус: Identical 256→257 (+1)** (lexicon.adoc
  34→0); blast (base f2133db): ровно 1 файл — 1 флип, **0 регрессий**.
  clippy 0, test --workspace зелёное (923).

- [x] **Сброс `had_blank_line` в dlist/colist-сканах (comment ошибочно рвал список)** —
  СДЕЛАНО (ветка `fix/revision-information`, 2026-06-12; НЕ закоммичено).
  Корень revision-information.adoc (24 diff, один корень): comment-строка сразу
  после текста dlist-entry (без blank) рвала список, если ПЕРЕД entry была
  blank-строка — `scan_description_list_item`/`scan_callout_list_item` не
  сбрасывали `had_blank_line` (в отличие от unordered/ordered), и правило
  «comment после blank разделяет списки» (18-я сессия) срабатывало ложно.
  Минимальный репро: `a:: x\n\nb:: y\n//c\n\nc:: z` → у asciidoctor один dlist
  (пробы /tmp/p_ri1..15; comment ПОСЛЕ blank по-прежнему рвёт — негатив
  сохранён). Фикс: **ПАРСЕР** block.rs — `had_blank_line = false` в конце обоих
  сканов (зеркало unordered, строка 3034). Попутная pre-existing находка (НЕ
  закрыта): comment в середине dd-параграфа должен сливать строки в один `<p>`
  (у нас второй блок-параграф). +1 parser-тест, +1 html-тест.
  **Корпус: Identical 255→256 (+1)** (revision-information.adoc); blast
  (base 8edb60d): ровно 2 файла — 1 флип, lexicon.adoc 376→34 (тот же корень
  рвал dlist по всему файлу), **0 регрессий**. clippy 0, test --workspace
  зелёное (parser 478, html 353).

- [x] **Standalone passthrough без лишнего `</div>` + doc-интринсики из входного файла** —
  СДЕЛАНО (ветка `fix/passthrough-stray-div-and-doc-intrinsics`, 2026-06-12;
  НЕ закоммичено). Два корня pass.adoc (18 diff) + revision-line-with-version-prefix
  (1 diff). (1) **РЕНДЕРЕР** events.rs TagEnd::DelimitedBlock: catch-all `_ =>
  </div>` ловил Passthrough/Comment, чьи start-армы ничего не открывают —
  каждый `++++`-блок и `[pass]`-параграф оставлял лишний `</div>`; новые армы:
  Passthrough — только trailing-newline guard, Comment — ничего. (2) **CLI**
  main.rs: интринсики из входного файла — `docname`/`docfile`/`docdir`/
  `docfilesuffix` из канонизированного пути, `docdate`/`doctime`/`docdatetime`
  из mtime (`%Y-%m-%d`, `%H:%M:%S %z`; при stdin — now, docdir=cwd),
  `localdate`/`localtime`/`localdatetime` = now; сеются в initial_attrs
  (препроцессор) и html_attrs (рендерер), явный `-a` (включая unset-формы) не
  перетирается; header-entry переопределяет (верно для date-семейства; в
  asciidoctor docname/docfile/docdir locked — предел, в корпусе нет).
  (3) **РЕНДЕРЕР** finish.rs::render_author_details: attr-refs в значениях
  revnumber/revdate/revremark резолвятся через resolve_attr_refs_text
  (undefined — литерал) — `LPR55, {docdate}:` теперь даёт дату. Предел: у нас
  header-FINAL state (asciidoctor — read-time: ref на атрибут, определённый
  ПОЗЖЕ в header, у него литерален, у нас резолвится; в корпусе нет); v-strip
  идёт по сырому значению (`v{docname}` → «vp_rev», asciidoctor «p_rev»).
  Pre-existing (не тронуто): author-line после attr-entry в header не
  распознаётся вовсе (details нет); `outfilesuffix`/`filetype` не определены
  (слой рендерера). +2 html-теста (revision attr-refs; passthrough bare).
  **Корпус: Identical 253→255 (+2)** (pass.adoc,
  revision-line-with-version-prefix.adoc); blast (base 99fab03): 3 файла —
  2 флипа, **0 регрессий**, stem 56=56 (нейтрально). clippy 0,
  test --workspace зелёное (920: parser 477, html 352).

- [x] **Inline `pass:SPEC[…]` + escape-формы + удаление custom-macro catch-all** —
  СДЕЛАНО (ветка `fix/inline-pass-spec-and-custom-macro-removal`, 2026-06-12;
  НЕ закоммичено). Корень literal-monospace.adoc (59 diff, один корень:
  `` `\pass:c[]` `` рассыпался в `\p` + мусорный custom-macro `ass`). Семантика
  верифицирована пробами /tmp/p_ep1..5: (1) **ПАРСЕР** inline.rs —
  `pass:SPEC[content]`: одночар-алиасы `a/c/m/n/p/q/r/v` + полные имена
  (`attributes::sub_name_to_flags` → pub(crate)); контент ре-парсится со
  спекнутым SubstitutionSet, Text→InlinePassthrough при отсутствии SPECIALCHARS
  (рендерер экранирует Text безусловно); без `[` после спека — не макрос.
  (2) Escape: `\pass:SPEC[` (расширение существующего арма `pass:[`) — backslash
  дропается, префикс литерален, контент+`]` через обычные subs; новый арм
  `\\pass:SPEC[` — один backslash остаётся литералом. `pass_macro_span_len`
  spec-aware (границы constrained-спанов, single-plus). (3) **Catch-all
  custom inline macro УДАЛЁН** (try_custom_inline_macro, dispatch-арм,
  scanner::is_known_inline_macro): asciidoctor матчит только зарегистрированные
  имена — неизвестный `name:target[attrs]` остаётся литералом, внутренность
  скобок через обычные subs; наш catch-all жадно матчил прозу
  («…content: `+abc+` [x]» → макрос `content:`). Tag::CustomInlineMacro в enum
  оставлен (API), блочный `name::` не тронут. Пределы (в коде, не в корпусе):
  порядок `pass:c,q` (q по экранированному, `;` блокирует constrained) —
  membership-only; спек-форматирование внутри `+…+` не перегоняется. 3 html-теста
  переписаны (фиксировали неверное), +2 html, +1 parser. **Корпус: Identical
  250→253 (+3)** (literal-monospace, attribute-entries, revision-line); blast
  11 файлов: 3 флипа, **0 регрессий**, 8 ближе (pass 133→18, footnote 260→101,
  revision-information 96→24, align-by-column 637→617, format-column-content
  218→198, apply-subs-to-text 119→115, syntax-quick-reference 2791→2735,
  outline 8718→8664). clippy 0, test --workspace зелёное (918: parser 477,
  html 350).

- [x] **`.Title` на списках: эмит в обёртку + разделение списков + slurp в параграф** —
  СДЕЛАНО (ветка `fix/list-block-title`, 2026-06-12; НЕ закоммичено). Корень
  block.adoc (57 diff, один корень): `.Title` перед списком терялся целиком —
  рендерер не эмитил pending title ни в одной из list-обёрток. Семантика
  asciidoctor (пробы /tmp/p_lt1..6): title → `<div class="title">` внутри обёртки
  перед `<ul>`/`<ol>`/`<dl>`/`<table>` (ulist/olist/dlist/horizontal/qanda/colist);
  `.Title` после blank в list-контексте закрывает списки (как block-attr/comment);
  `.Title`-строка без blank внутри item/dd/параграфа/admonition — wrapped-текст
  (титулы никогда не прерывают параграф). Фикс: **РЕНДЕРЕР** —
  `emit_pending_block_title` в start_unordered_list/start_ordered_list/
  start_description_list (blocks.rs) и arm Tag::CalloutList (events.rs);
  **ПАРСЕР** block.rs — `.Title`-handler закрывает list-контексты при
  had_blank_line (зеркало block-attr); исключение `is_block_title` убрано из
  `is_list_continuation_line`/`is_dlist_continuation_line`/break-условий
  `scan_paragraph`/`scan_admonition`. Попутные pre-existing находки (НЕ
  закрыты): nested-список с другим маркером после blank должен вкладываться
  в li; `[square]` не даёт класс на `<ul>`; colist-`<li><p>` компактен;
  `== heading` не прерывает параграф у asciidoctor. +3 теста (parser 2,
  html 7-кейсовый). **Корпус: Identical 249→250 (+1)** (block.adoc); blast
  6 файлов: 1 флип, **0 регрессий**, 5 ближе (ordered 223→90, unordered
  298→145, outline 8735→8718). clippy 0, test --workspace зелёное
  (parser 476, html 348).

- [x] **`:example-caption:` атрибут + attrlist-shorthand только в 1-й позиции +
  linenums-слот source-блока** — СДЕЛАНО (ветка
  `fix/example-caption-unset-and-positional-shorthand`, 2026-06-11; НЕ закоммичено).
  Два near-miss корня (по 2 diff) + вытащенный по дороге третий, семантика
  верифицирована пробами /tmp/p_ec1..3, p_qa1..2, p_sh1, p_ln1..8.
  (1) **РЕНДЕРЕР** example-blocks.adoc: label «Example» был захардкожен —
  `example-caption: Example` посеян в дефолтные document_attrs (lib.rs), арм
  Example в blocks.rs читает его как figure/table → `:!example-caption:` даёт
  голый title (в т.ч. mid-document), `:example-caption: Demo` — «Demo 1.» с
  общим счётчиком. (2) **ПАРСЕР** assign-id.adoc: shorthand (`#id`/`.role`/`%opt`,
  чистый И mixed) парсится ТОЛЬКО в первой comma-части attrlist (asciidoctor:
  `[quote#roads,Dr. Emmett Brown,…]` — attribution целиком; `[quote,#bar]` →
  attribution «#bar» verbatim; `[.r1,.r2]` → только r1; `[%header,%footer]` →
  только header). `attributes.rs::parse` — обе ветки гейтятся `idx == 0`.
  Попутно `emit_block_metadata` (block.rs): style гейтится
  `first_positional_is_style` — позиционал из слота 2+ больше не утекает в
  style/class. (3) **ПАРСЕР+РЕНДЕРЕР** (обнажено фиксом 2, регрессия
  db-migration поймана blast'ом и закрыта): 3-й позиционный СЛОТ source-блока =
  linenums — любое непустое позиционное значение включает (`[source,ruby,linenums]`,
  `…,%linenums]`, implied `[,ruby,linenums]`; named `start=10` слот НЕ занимает) —
  правило в `attributes.rs::parse` по raw-слотам; РЕНДЕРЕР `start_source_block`:
  linenums рендерится ТОЛЬКО под build-time подсветчиком (rouge/pygments/coderay)
  — без подсветчика и под highlight.js asciidoctor опцию игнорирует целиком
  (ни класса, ни таблицы). Наша linenotable-разметка под rouge всё равно не
  байт-в-байт с rouge (нет server-side подсветки) — несхождение латентно.
  4 старых теста фиксировали неверную семантику — переписаны probe-verified;
  +4 новых (html example-caption 4 кейса, html shorthand-первая-позиция 5
  ассертов, parser shorthand 3 кейса, parser linenums-слот 6 кейсов).
  **Корпус: Identical 247→249 (+2)** (assign-id.adoc, example-blocks.adoc);
  blast 3 файла: 2 флипа, **0 регрессий**, add-title 252=252 (семантически
  ближе: mid-document `:!example-caption:` теперь чтится). clippy 0,
  test --workspace зелёное (parser 474, html 347).

- [x] **Style-masquerade параграфа: голый контент + стиль `open`** — СДЕЛАНО (ветка
  `fix/collapsible-block`, 2026-06-11; НЕ закоммичено). Корень collapsible.adoc
  (51 diff, один корень): параграф, masquerade'нутый блочным стилем
  (`[example]`/`[example%collapsible]`/`[sidebar]`/`[quote]`), у asciidoctor несёт
  текст ГОЛЫМ внутри `<div class="content">`/`<blockquote>` (без
  `<div class="paragraph"><p>`), в отличие от настоящего delimited-блока с
  параграфом внутри (проба /tmp/p_col1 — байт-в-байт, multiline сохраняет строки).
  Исключение — `[partintro]`: обёртку СОХРАНЯЕТ (проба p_col3, book-контекст).
  `[open]`-стиль на параграфе у нас вообще не masquerade'ился (оставался
  `paragraph open`) — добавлен; класс `open` в обёртку НЕ течёт (голый `openblock`).
  `[%collapsible]` без стиля — обычный параграф (опция игнорируется, уже было верно).
  Фикс: **ПАРСЕР** `block.rs::scan_paragraph` — арм `quote|example|sidebar|open`
  эмитит Text без Tag::Paragraph (как verse/pass); partintro выделен в отдельный арм
  (с обёрткой); `attributes.rs::block_style_kind` +`"open"`; exclusion-список
  emit_block_metadata +`"open"`. **РЕНДЕРЕР** `events.rs` TagEnd::DelimitedBlock —
  newline-guard перед закрывающими тегами в армах Quote/Example(details)/
  Example|Sidebar|Open (голый контент не оставляет `\n`; verse не тронут — там
  отсутствие `\n` намеренное). Пределы (НЕ в корпусе): «partintro вне book-part →
  ERROR + exclude» не реализовано. +1 html-тест
  `test_style_masqueraded_paragraph_bare_content` (7 кейсов). **Корпус: Identical
  244→247 (+3)** (collapsible.adoc, sidebars.adoc, release-plan.adoc); blast 8 файлов:
  3 флипа, **0 регрессий**, 5 changed-still-different — почти все существенно ближе:
  assign-id 84→2, example-blocks →2, quote 161→109, add-title 291→252, block 57=57
  (остаток — другой корень: `.Title` на ulist теряется). clippy 0, test --workspace
  зелёное (908: html 345).

- [x] **Checklist `%interactive`: `<input type="checkbox">` вместо NCR-маркеров** —
  СДЕЛАНО (ветка `fix/checklist-rendering`, 2026-06-11; НЕ закоммичено). Корень
  checklist.adoc (49 diff, один корень): `[%interactive]`/`options=interactive` на
  checklist у asciidoctor рендерит `<input type="checkbox" data-item-complete="1"
  checked>` / `data-item-complete="0"` вместо `&#10003;`/`&#10063;` (проба
  /tmp/p_chk1 — байт-в-байт, включая formal-форму и игнор опции на списке без
  чекбоксов). Фикс — только РЕНДЕРЕР: поле `interactive_ulist_stack: Vec<bool>`
  (lib.rs; параллельный стек, push в `start_unordered_list` из `meta.options`,
  pop на `TagEnd::UnorderedList`), arm `Tag::ListItem` (events.rs) — match
  (checked, interactive). Вложенный список опцию НЕ наследует (свой push false).
  Предел (НЕ в корпусе): `:icons: font` + interactive (font-иконки) не поддержан;
  pre-existing (проба p_chk2): `+`-continuation с attrlist+новым `*`-item —
  asciidoctor вливает в один список, мы открываем второй. +1 html-тест
  `test_checklist_interactive_html` (4 кейса). **Корпус: Identical 243→244 (+1)**
  (checklist.adoc); blast ровно 1 файл: 1 флип, **0 регрессий**. clippy 0,
  test --workspace зелёное (907: html 344).

- [x] **anchor:-макрос + xreflabel в `[[id,label]]` + block-attr misdetect + comment
  разделяет списки** — СДЕЛАНО (ветка `fix/inline-anchor-macro-and-xreflabel`,
  2026-06-11; НЕ закоммичено). Четыре корня id.adoc (45 diff), семантика
  верифицирована пробами /tmp/p_id1..9. (1) **ПАРСЕР** `inline.rs::try_anchor_macro` —
  inline-макрос `anchor:id[]`/`anchor:id[xreflabel]` ≡ `[[id]]` → `Tag::Anchor`
  (label НЕ рендерится in place; target с пробелом — литерал, при провале skip всего
  префикса `anchor:` чтобы catch-all не съел `nchor:`; `anchor:` добавлен в NAMES
  escape-листа — `\anchor:x[]` → литерал без backslash). (2) **ПАРСЕР** `[[id,xreflabel]]`
  — label отрезается от id в ОБОИХ формах: inline (`inline.rs::try_anchor` split по
  запятой) и block-anchor (`attributes.rs` legacy-ветка). (3) **ПАРСЕР**
  `scanner.rs::is_block_attribute` ужесточена по BlockAttributeListRx asciidoctor:
  первый символ inner — word char/`{,.#"'%` или пусто; `[[...]]` — отдельная ветка
  BlockAnchorRx (ВСЯ строка = анкор, interior без скобок) → `[[id]]image:...[]`
  теперь параграф с inline-анкором, а не мусорный attrlist. (4) **ПАРСЕР** `block.rs`
  comment-handler: comment-строка ПОСЛЕ blank в list-контексте закрывает списки
  (asciidoctor: «line comment between lists keeps them separate»); comment сразу
  после item (без blank) список НЕ рвёт (probe-verified p_id5/6/7/8). +4 теста
  (inline anchor-macro 4 кейса; scanner block-attr 10 ассертов; attributes legacy
  anchor; block comment-split; html 6-кейсовый). Пределы (НЕ в корпусе для флипа):
  xreflabel НЕ регистрируется как reftext для xref-резолва (`<<id>>` → label;
  родственно lexicon-остатку «reftext из dt-терма» — потребует label в Tag::Anchor +
  регистрацию в XrefResolver). **Корпус: Identical 242→243 (+1)** (id.adoc); blast
  9 файлов: 1 флип, **0 регрессий**, 8 changed-still-different — list-файлы
  существенно ближе к эталону (complex.adoc: ulist-блоков 1→5 при 13 в ref;
  checklist 49=49, revision-information 94→96 — позиционный шум). clippy 0,
  test --workspace зелёное (parser 472, html 343, core 13).

- [x] **Author-атрибуты из attribute-entries + attr-refs в section auto-id** — СДЕЛАНО
  (ветка `fix/author-attr-entries`, 2026-06-11; НЕ закоммичено). Три корня
  reference-author.adoc (37 diff), семантика верифицирована пробами /tmp/p_au1..16
  И чтением источника asciidoctor (parser.rb parse_header_metadata/process_authors,
  document.rb Document#authors).
  (1) **CORE** `Author::from_attribute_value` — names-only дериватор значения
  `:author:` (split ≤3 whitespace-сегментов, 4+ слов → хвост в lastname, `_`→пробел,
  initials = первые символы сегментов, fullname рекомпозируется; email НЕ
  извлекается). (2) **РЕНДЕРЕР** `finish.rs::finalize_header_authors` (зов на
  TagEnd::Header в обоих режимах): если `author`-атрибут ≠ значения от author-line —
  дериватор клоббером пишет firstname/middlename/lastname (даже поверх явных
  entries — rescan asciidoctor), явный `:authorinitials:` (≠ line-derived) выживает,
  authorcount → 1; `render_author_details` — author-спаны attribute-backed
  (`author`/`email` + `author_N`/`email_N` по `authorcount`, как Document#authors):
  attr-entries открывают details без author-line, `:!author:` подавляет details,
  `:email:` без author НЕ открывает. events.rs: Event::Author пишет `authorcount`.
  (3) **ПАРСЕР** `block.rs`: карта `doc_attrs` (lowercase-имена, definition-time
  резолв значений; unset-формы remove; author-line пишет author/firstname/… с
  `_N`-суффиксами) → `resolve_title_attr_refs` перед `generate_id` на всех 4 точках:
  `== About {author}` → `_about_kismet_r_lee` (undefined ref — литерал, скобки
  дропает санация id). Пределы (нет в корпусе): `:authors:` (множественный) не
  поддержан; parser-карта не дериватит firstname из entry для ids. +3 теста
  (html 6-кейсовый, parser section-ids, core дериватор).
  **Корпус: Identical 241→242 (+1)** (reference-author.adoc); blast ровно 1 файл:
  1 флип, **0 регрессий**. clippy 0, test --workspace зелёное (902: parser 469,
  html 342, core 13).

- [x] **subs= trailing-plus + attr-value pass-макрос + guard рекурсии attr-ref** — СДЕЛАНО
  (ветка `fix/subs-trailing-plus-and-attr-pass-macro`, 2026-06-11; НЕ закоммичено).
  Два корня listing.adoc (34 diff) + попутный pre-existing КРАШ.
  (1) **ПАРСЕР** `attributes.rs::parse_subs_value`: trailing `+` (`subs=attributes+` —
  prepend asciidoctor) не детектился → токен игнорировался, set становился NONE
  (XML в листинге не экранировался). Семантика resolve_subs (пробы /tmp/p_subs1..5):
  `+x` append / `x+` prepend / `-x` remove; первый модификатор сидит дефолты, первый
  plain-токен сидит ПУСТОЙ набор (`"quotes,+attributes"` дропает specialchars);
  составные имена (verbatim/normal/none) допустимы как инкрементальные токены
  (+`sub_name_to_flags`). Порядок применения (prepend → значение ре-экранируется)
  bitflag-модели недоступен — только membership; пределы вне корпуса. 2 юнит-теста
  переписаны (probe-verified), +1 trailing-plus.
  (2) **РЕНДЕРЕР** `lib.rs::apply_attr_value_pass_macro`: attr-entry значение —
  full-value `pass:SPEC[content]` (asciidoctor apply_attribute_value_subs) — обёртка
  стрипается; `a|attributes` в SPEC → definition-time резолв `{refs}` (core
  resolve_attr_refs_text, undefined → литерал, при использовании НЕ ре-сканится);
  пустой SPEC не трогается (inline pass-макрос вставит verbatim at use). Бонус:
  `:fn-disclaimer: pass:c,q[footnote:…]` (footnote.adoc) теперь даёт настоящие
  footnote-`<sup>` вместо мусорного custom-macro.
  (3) **РЕНДЕРЕР** guard рекурсии (pre-existing КРАШ `:x: {x}` → stack overflow):
  поле `attr_refs_in_progress`, повторный вход по имени → литерал `{name}`
  (asciidoctor никогда не ре-сканит вставленные значения). +2 html-теста.
  **Корпус: Identical 240→241 (+1)** (listing.adoc); blast 4 файла: 1 флип,
  **0 регрессий**, include 125→124 / subs 92→89 / footnote 245→260 (семантически
  лучше, позиционный шум). clippy 0, test --workspace зелёное (parser 468, html 341).

- [x] **Revision-атрибуты из attribute-entries в `<div class="details">`** — СДЕЛАНО (ветка
  `fix/revision-attrs-from-entries`, 2026-06-11; НЕ закоммичено). Корень
  reference-revision-attributes.adoc (31 diff): revision-спаны рендерились только из
  revision-line (`Event::Revision`), а asciidoctor (html5.rb) — из document-атрибутов
  `revnumber`/`revdate`/`revremark`, как бы они ни были заданы. Семантика (пробы
  /tmp/p_rev1..8, все 8 байт-в-байт после фикса): attr-entry в header достаточен;
  значение verbatim (`:revnumber: v8.3` → «version v8.3» — `v` стрипается ТОЛЬКО при
  парсинге revision-line); attr-entry ПОБЕЖДАЕТ revision-line (later-wins);
  `:!revdate:` снимает спан И запятую после version; set-but-empty `:revnumber:` →
  спан «version »; body-атрибуты в details НЕ попадают (header-final state); автор не
  обязателен — одинокий revdate открывает details. Фикс — только РЕНДЕРЕР
  (`finish.rs::render_author_details`): чтение revnumber/revdate/revremark из
  `document_attrs` (зов на `TagEnd::Header` — снимок ровно header-состояния);
  guard пустоты по авторам+трём атрибутам; поле `HtmlRenderer.revision` удалено
  (events.rs arm Event::Revision лишь вливает attr_entries в document_attrs —
  precedence в порядке стрима). +1 html-тест `test_revision_attrs_from_attribute_entries`
  (4 кейса). **Корпус: Identical 239→240 (+1)** (reference-revision-attributes.adoc);
  blast ровно 1 файл: 1 флип, **0 регрессий**. clippy 0, test --workspace зелёное
  (html 338→339, parser 467).

- [x] **Admonition: block-форма с параграф-обёртками + гейт стиля по типу делимитера** —
  СДЕЛАНО (ветка `fix/admonition-block-paragraph-wrappers`, 2026-06-11; НЕ закоммичено).
  Корень apply-subs-to-blocks.adoc (31 diff): параграфы внутри delimited admonition
  (`[TIP]` + `====`) рендерились голым текстом (как у `TIP: text`), а asciidoctor
  оборачивает их в `<div class="paragraph"><p>` (compound-контент). Семантика
  (пробы /tmp/p_adm1..13): paragraph-форма (`NOTE: text` и `[NOTE]` на параграфе) —
  голый текст в td; block-форма (`[NOTE]` на `====`/`--`) — обычные блок-обёртки;
  admonition-стиль ЧТИТСЯ только на example/open — на listing/literal/sidebar/quote/
  passthrough ИГНОРИРУЕТСЯ (блок остаётся родным, стиль дропается). Фикс:
  **ПАРСЕР** — `Tag::Admonition` +поле `block: bool` (event.rs); paragraph-точки
  (block.rs) → `block: false`; ранний перехват «admonition style on any delimited
  block» УДАЛЁН — гейт `Example|Open` в structural-ветке → `block: true`
  (для verbatim-типов стиль падает в их родную ветку и дропается, как у unknown-style).
  **РЕНДЕРЕР** — параллельный стек `admonition_block_stack: Vec<bool>` (push в
  start_admonition, pop в TagEnd::Admonition); `is_direct_child_of_admonition` подавляет
  `<p>` только при paragraph-форме; в `is_inside_compact_context` arm Admonition
  возвращает компактность только для paragraph-формы (block-форма → полные обёртки).
  Обновлены 2 html-теста под верную семантику + 1 новый
  `test_admonition_block_vs_paragraph_forms`; 2 parser-теста и 1 integration-тест
  обновлены (`block: true/false`); builder.rs — паттерн `{ kind, .. }`. Пробы: 11/12
  байт-в-байт (p_adm12 — pre-existing лишний `</div>` у голого passthrough, не
  admonition-баг). **Корпус: Identical 235→239 (+4)** (header.adoc, icon-macro.adoc,
  apply-subs-to-blocks.adoc, validation.adoc); blast 10 файлов: 4 флипа,
  **0 регрессий**, 6 changed-still-different (ordered 420→232, admonition 223→197,
  cookbook 2604→2582; syntax-quick-reference 2759→2791 — позиционный шум,
  admonition-сегмент проверен байт-в-байт). clippy 0, test --workspace зелёное
  (parser 467, html 337→338).

- [x] **Таблицы: `noheader`-опция + formal `options=`/`opts=`** — СДЕЛАНО (ветка
  `fix/table-noheader-option`, 2026-06-11; НЕ закоммичено). Корень add-header-row.adoc
  (29 diff): `[%noheader]` не подавлял implicit-промоушен первой строки в header
  (первая строка непустая + blank после → `<thead>`). Семантика asciidoctor (пробы
  /tmp/p_nh1..7): `noheader` (shorthand И formal) гасит ТОЛЬКО implicit-ветку; при
  конфликте `%header%noheader` побеждает явный `header`; `opts=` — alias `options=`;
  значение comma-separated (`options="header,footer"`). Попутно закрыт пробел: formal
  `options=header` ВООБЩЕ не работал (в корпусе маскировался implicit-правилом —
  у formal-таблиц blank после первой строки). Фикс — только ПАРСЕР, 3 точки:
  `attributes.rs::parse` — named `options`/`opts` промотируются в вектор `options`
  (split по запятой, тот же путь, что shorthand `%`); `block.rs` — оба места решения
  has_header (psv ~1379 и csv/dsv ~1627): `&& !has_option("noheader")` в implicit-ветке.
  +1 html-тест `test_table_noheader_option_html` (5 кейсов). **Корпус: Identical
  234→235 (+1)** (add-header-row.adoc); blast 2 файла: 1 флип, **0 регрессий**,
  row.adoc 312→310 (нейтрально-лучше, доминирует корень `cols="2*"` multiplier).
  clippy 0, test --workspace зелёное (html 336→337).

- [x] **`[partintro]`-параграф маскируется в open block** — СДЕЛАНО (ветка
  `fix/partintro-paragraph-openblock`, 2026-06-11; НЕ закоммичено). Корень part.adoc
  (18 diff): asciidoctor (PARAGRAPH_STYLES) маскирует параграф со стилем `partintro` в
  open block — `<div class="openblock partintro"><div class="content"><div
  class="paragraph"><p>…` (проба /tmp/p_pi1); у нас оставался параграфом с классом
  (`paragraph partintro`). Фикс — только ПАРСЕР, 2 точки: `attributes.rs::
  block_style_kind` +`"partintro"`; `block.rs::scan_paragraph` — arm
  `quote|example|sidebar` расширен до `|partintro` с kind `DelimitedBlockKind::Open`
  (style НЕ исключается в `emit_block_metadata` → рендерер сам добавляет класс
  `openblock partintro`). `[partintro]` на `--`-блоке уже работал (delimited-маппинг
  имеет `_ => {}` фолбэк) — guard-ассерт в тесте. НЕ реализована валидность asciidoctor
  («только book-part, иначе ERROR + exclude всего блока», проба p_pi2) — в корпусе
  `[partintro]` вне part нет. +1 html-тест
  `test_partintro_paragraph_masquerades_as_open_block`. **Корпус: Identical 233→234
  (+1)** (part.adoc, 0 diffs); blast ровно 1 файл: 1 флип, **0 регрессий**. clippy 0,
  test --workspace зелёное (html 335→336).

- [x] **url-макросы: irc/ftp-схемы, role= → class, mailto subject/body** — СДЕЛАНО (ветка
  `fix/url-macro-irc-role-mailto`, 2026-06-11; НЕ закоммичено). Три корня url.adoc (7 diff),
  все верифицированы пробами (/tmp/p_url1..3, p_u_a..d). (1) **irc:// и ftp:// — автолинк-схемы**
  как http(s): голые → `class="bare"`, с `[text]` — обычная ссылка; ПАРСЕР
  (`inline.rs`): +2 dispatch-арма (`ftp://`, `irc://`) на существующий `try_autolink`.
  (2) **`role=` на link/url/mailto-макросах** → class на `<a>`; при пустом тексте —
  `class="bare green"` (bare первым). `Tag::Link` +поле `role` (event.rs),
  `parse_link_attrs` парсит `role=`, рендерер (`events.rs`) эмитит class сразу ПОСЛЕ
  href (raw-порядок asciidoctor: href, class, target, rel — было class после rel).
  (3) **mailto positional 2/3** → `?subject=&body=` с percent-encode ERB-стиля
  (литеральны только `A-Za-z0-9_.~-`, пробел `%20`, hex UPPERCASE; `url_encode_into` в
  inline.rs); пустые компоненты опускаются (asciidoctor на `[T,,body]` ПАДАЕТ — nil
  conversion, поведение свободно); кавычки вокруг subject/body снимаются. Попутно
  закрыт латентный баг `parse_link_attrs`: named-only attrlist (`[role=green]`,
  `[window=_blank]`) давал text = весь bracket_content → теперь пустой text → bare;
  named-ветка гейтится валидностью имени ключа (`[A-Za-z0-9_-]+`), чтобы quoted-positional
  с `=` внутри не съедался. +1 parser-тест (5 кейсов), +1 html-тест (6 ассертов).
  **Корпус: Identical 232→233 (+1)** (url.adoc); blast ровно 1 файл: 1 флип,
  **0 регрессий**. clippy 0, test --workspace зелёное (parser 467, html 335, всего 893).

- [x] **Multi-author атрибуты: underscore-суффикс `author_2`** — СДЕЛАНО (ветка
  `fix/multi-author-attr-underscore`, 2026-06-11; в master `4c62625`). Корень
  multiple-authors.adoc (4 diff): имена document-атрибутов авторов 2+ у asciidoctor —
  С подчёркиванием (`author_2`, `lastname_2`, `email_3`…), форма `author2` — НЕ атрибут
  (литерал); а span-id в `<div class="details">` — БЕЗ подчёркивания (`id="author2"`,
  `id="email3"`). У нас обе формы были без подчёркивания → attr-refs не резолвились
  (проба /tmp/p_auth1.adoc). Фикс — только CORE (`adoc-render-core/lib.rs`,
  `AuthorRegistry`): `attr_suffix` разделён на `id_suffix` (без сепаратора, для span-id;
  потребитель `finish.rs::render_author_details`) и `name_suffix` (`_2`/`_3`, для
  attr-entries в `add()`). Путь потребления один (`events.rs::Event::Author` →
  `document_attrs`), builder.rs attr-entries не генерирует. Обновлён core-тест,
  +1 html-тест `test_multi_author_attr_names_underscore` (резолв underscore-форм,
  литеральность `{author2}`, span-id без подчёркивания). **Корпус: Identical 231→232
  (+1)** (multiple-authors.adoc, verified 0 diffs); blast ровно 1 файл: 1 флип,
  **0 регрессий**. clippy 0, test --workspace зелёное (html 333→334, всего 891).

- [x] **Email-автолинк без `class="bare"`** — СДЕЛАНО (ветка
  `fix/email-autolink-no-bare-class`, 2026-06-11; в master `05627f9`). Корень header.adoc
  (3 diff): asciidoctor НЕ вешает `class="bare"` на email-автолинки (`user@x.org` →
  `<a href="mailto:…">…</a>` без класса); `bare` — только URL-автолинки и `link:`/URL-макросы
  с пустым текстом (`link:mailto:x@y[]` — bare, `mailto:x@y[]` — НЕ bare; пробы
  /tmp/p_mail1..2.adoc). Фикс — 1 строка, ПАРСЕР (`inline.rs::try_email_autolink`):
  `is_bare: true` → `false` (`try_mailto_macro` уже был верен). Обновлены 5 parser-тестов +
  1 html-тест (`test_email_autolink_html` усилен negative-ассертом). **Корпус: Identical
  230→231 (+1)** (header.adoc); blast 4 файла: 1 флип, **0 регрессий**, 3 улучшены/нейтральны
  (multiple-authors 7→4, url 9→7, reference-author 37→37 — другие корни). clippy 0,
  test --workspace зелёное.

- [x] **version-label в revnumber + attr-entry внутри параграфа — литерал** — СДЕЛАНО (ветка
  `fix/version-label-revnumber`, 2026-06-11; в master `4f72ab3`). Два корня version-label.adoc
  (2 diff). (1) **РЕНДЕРЕР** (`finish.rs::render_author_details`): `<span id="revnumber">`
  — хардкод `version X,` → шаблон asciidoctor `{version-label.downcase} {revnumber}` +
  запятая ТОЛЬКО при наличии revdate; default `version-label: Version` добавлен в
  document_attrs (lib.rs); `:!version-label:` снимает ключ → пустой label, но ведущий
  пробел остаётся (`<span id="revnumber"> 3</span>` — артефакт шаблона, проба подтверждена).
  (2) **ПАРСЕР** (`block.rs`): attribute-entry строка внутри текстового блока — НЕ атрибут.
  Семантика asciidoctor (пробы /tmp/p_vl6..11, все байт-в-байт): в параграфе,
  admonition-параграфе и principal ulist/olist — ЛИТЕРАЛЬНЫЙ текст; в dlist при принципале
  на той же строке wrapped attr-entry ДРОПАЕТСЯ (не литерал И не применяется — причуда,
  верифицирована пробой с предопределённым атрибутом); при голом `term::` attr-entry
  следующей строкой (даже после blank) — литеральный принципал. Фикс: `is_attribute_entry`
  убран из break-условий scan_paragraph/admonition-сборщика/`is_list_continuation_line`;
  в dlist — drop-ветка в wrapped-цикле + attr-entry допущен как принципал. На границе
  блока (после blank) атрибут ПРИМЕНЯЕТСЯ как раньше (guard-тест). +2 теста
  (`test_attribute_entry_inside_paragraph_is_literal` — 5 кейсов;
  `test_revnumber_version_label` — 4 кейса). **Корпус: Identical 229→230 (+1)**
  (version-label.adoc); blast 4 файла: 1 флип, **0 регрессий**, header.adoc 16→3 diff
  (сильное улучшение), metadata/stem changed-still-different. clippy 0, test --workspace
  зелёное (889: parser 466, html 333).

- [x] **toc2/toc-left/toc-right классы: body vs div + header-only гейт** — СДЕЛАНО (ветка
  `fix/toc2-body-class`, 2026-06-11; НЕ закоммичено). Семантика asciidoctor (пробы
  /tmp/p_toc1..6.adoc, все 6 байт-в-байт после фикса): `toc2` в body-классе ТОЛЬКО при
  размещении `left`/`right` (`:toc:`/`preamble`/`macro` → body `article`); при side-размещении
  body = `article toc2 toc-left|toc-right`, а div TOC — просто `class="toc2"` (НЕ
  `toc2 toc-left`); mid-document `:toc: left` НЕ даёт НИЧЕГО (нормализация toc-class/
  toc-position — только header-атрибуты, document.rb). У нас было: toc2 при ЛЮБОМ непустом
  значении `toc` (вкл. mid-document), `toc-left/right` на div вместо body. Фикс — только
  РЕНДЕРЕР, 2 точки (`finish.rs`): `generate_toc` — `left|right` → div `class="toc2"`;
  `write_document_head` — гейт `toc_auto_seen && (left|right)` → ` toc2 toc-{pos}`
  (`toc_auto_seen` = парсер эмитит `Event::Toc` внутри header'а только при header-`:toc:` —
  готовый признак «toc из header»). Обновлены 2 теста (toc_left/right: body+div ассерты,
  standalone), +1 тест `test_toc_mid_document_no_body_class`. **Корпус: Identical 228→229
  (+1)** (toc.adoc); blast ровно 1 файл: 1 флип, **0 регрессий** (base пересобран из master
  `e3dd825`). clippy 0, test --workspace зелёное (html 331→332). **Остаток (НЕ в корпусе)**:
  mid-document смена значения `toc` ПОСЛЕ header-`:toc:` у нас меняет live-размещение
  (asciidoctor морозит на header) — снапшот не делал; кастомный `:toc-class:` не поддержан.

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
