# Session context

## Сессия (2026-06-12, тридцать вторая) — Фаза 3: blank после `|===` гасит implicit header

Запрос «продолжи». Ветка **`fix/add-columns-nearmiss`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 282, master `43f7ab1`
(base-бинарь /tmp/adoc_base пересобран с master).

### Выбор задачи
nearmiss: replacements.adoc (4 diff) — известный NCR-кластер, скип;
**add-columns.adoc (40 diff)** — один корень.

### Семантика asciidoctor (пробы /tmp/p_ac/p1..p8, t1 — все IDENTICAL)
- Blank-строка (одна или несколько) МЕЖДУ `|===` и первой data-строкой
  гасит implicit header promotion (p1/p3); явный `[%header]` всё равно
  промоутит (p4); colcount по-прежнему из первой строки (p3, 2 колонки).
- Comment-строка прозрачна: `|===`+comment+row+blank → header ЕСТЬ (p6);
  но blank до/после comment (до первой data-строки) — гасит (p7/p8).

### Что сделано (ПАРСЕР block.rs scan_table)
- Флаг `blank_before_first_data` — взводится на blank при
  `first_data_idx.is_none()`; добавлен в гейт `implicit_header` (`&& !…`).
- +1 html-тест `test_table_leading_blank_suppresses_implicit_header_html`
  (6 кейсов: blank/несколько blank/comment+blank/только comment/явный
  %header/colcount).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 485, html 366).
- Пробы p1..p8 и add-columns.adoc IDENTICAL.
- **Корпус: Identical 282→284 (+2)**; blast (base 43f7ab1): 4 файла —
  2 флипа (add-columns 40→0, column.adoc 172→0), cell.adoc 975→965 ближе,
  table.adoc 556→597 — позиционный шум поверх pre-existing корня
  (`|=== <1>` в параграфе → у нас colist; изолированная таблица из файла
  сверена: thead у обоих 0, BASE был неправ), **0 семантических регрессий**.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 284: replacements (4 — NCR-кластер, в одиночку бесполезен),
  footnote examples (70), bibliography (72), subs (76), ordered (90),
  part-with-special-sections (103), multipart-book (109), quote (109 —
  `-- Author` attribution), metadata (111), apply-subs-to-text (115).
- Кандидаты-корни прошлых сессий: `cols=2;2;3;3` `;`-разделитель
  (image-ref, image-svg); `l|`-ячейка → `<div class="literal"><pre>`
  (image-svg); `[frame=ends,grid=none]` (image-svg); НОВЫЙ: `|=== <1>` в
  параграфе не должен открывать colist (table.adoc — крупный позиционный
  корень).
- Pre-existing из прошлых сессий: ячейка `a|` nested-парсинг, nested-список
  с другим маркером в li, `[square]`-класс, компактный colist-`<li><p>`,
  `== heading` не прерывает параграф, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, label block-anchor `[[id,label]]` над блоком не
  побеждает `.Title`, `\\https://…` двойной backslash, blank в ячейке →
  второй `<p>`, CSV drop incomplete row.

---

## Сессия (2026-06-12, тридцать первая) — Фаза 3: таблицы — открытая модель ячейки (continuation/пустые/дупликация/спек-цепочки/drop-row/comments)

Запрос «продолжи». Ветка **`fix/align-by-column`** — ЗАКОММИЧЕНА (`0fe6e49`),
смержена в master (`0c5418a`), запушена, локальная ветка удалена.
Baseline: Identical 267, master `4099d62` (прошлая ветка смержена;
base-бинарь /tmp/adoc_base пересобран с master).

### Выбор задачи
nearmiss: **align-by-column.adoc (7 diff)** — один видимый корень
(continuation-строки ячеек), но фикс вскрыл кластер psv-семантики, добит
целиком (6 подкорней).

### Семантика asciidoctor (пробы /tmp/p_abc/p1..p17)
- **Continuation**: текст до первого `|` строки (или строка без `|` вовсе) —
  продолжение последней ячейки предыдущей строки, join `\n` в ОДНОМ
  `<p class="tableblock">` (p1/p2/p6); спек между текстом и `|` — спек
  следующей ячейки (`tail 2+|wide`, p8); без предыдущей ячейки текст
  открывает собственную (p3/p7).
- **Header**: implicit header ТОЛЬКО если blank сразу после первой строки И
  следующая non-blank строка начинается с ячейки — continuation до (p5) или
  после (p9) blank гасит промоушн.
- **Colcount**: имплицитное число колонок = ячейки первой строки, пока та
  «открыта»: ячейка, открытая mid-line на continuation-строке, считается
  (p6: `|a` + `mid |late` → 2 колонки; p1: `|cell two` с новой строки → 1).
- **Drop incomplete row**: ячейки неполной последней строки дропаются
  («dropping cells from incomplete row detected end of table», p7/p10);
  CSV-путь у asciidoctor тоже дропает (p11) — у нас НЕТ (предел).
- **Пустые ячейки**: `|a |` → 2 ячейки, `|a | |c` → mid-пустая; рендер —
  голый `<td></td>` без `<p>` (p12/p13).
- **Дупликация/цепочки**: `2*>m|x` → ячейка ×2 right+mono; `.2+^.>s|` —
  span+align+style цепочкой (CellSpecRx: factor, align, style; спек требует
  пробельной границы слева); копии дупликации несут ПОЛНЫЙ контент включая
  continuation-строки (p15, cell.adoc).
- **Comments**: line-comment в таблице невидим — дроп из контента ячейки, не
  влияет на header/colcount (p17; закрыл style-operators 1 diff и section-ref).
- Blank внутри ячейки → ВТОРОЙ `<p>` в той же ячейке (p9/p16) — НЕ сделано
  (у нас join `\n`), задокументированный предел.

### Что сделано
- **ПАРСЕР** scanner.rs: `parse_table_cells` → `Option<TableLineCells
  { continuation: Option<&str>, cells }>`; `CellSpec.content: Cow<str>`
  (+ поле `duplication: u8`, раскрывается потребителем); НОВЫЙ
  `parse_cell_spec_exact(s) -> Option<ExactCellSpec>` (вся строка = спек;
  префикс и whitespace-отделённый токен в non-last частях); пустые части
  всегда пушатся как ячейки; legacy-суффикс-цепочка осталась fallback'ом
  (квирк `x2+` без пробела сохранён).
- **ПАРСЕР** block.rs scan_table: цикл сбора — скип comment-строк,
  `append_cell_continuation` (join `\n`, в пустую — без `\n`, без ячеек —
  новая), first_row_width пока строка открыта (×duplication), header-гейт
  (blank at first+1 && post_blank_line_starts_cell && width==num_cols);
  экспансия дупликации ПОСЛЕ сбора (`repeat_n`); build_table_rows: последняя
  строка пушится только если заполняет грид (trailing rowspan-occupancy
  учитывается).
- **ПАРСЕР** parser.rs: арм `Event::Text(Cow::Owned)` → inline-парсинг с
  into_static (раньше Owned-текст шёл сырым: слитые ячейки и CSV-поля не
  получали typographic/quotes — отсюда флипы subs-файлов).
- **РЕНДЕРЕР** lib.rs/blocks.rs/events.rs: `cell_p_start_stack` — позиция
  после открывающего `<p class="tableblock">`; на TagEnd::TableCell пустая
  ячейка → truncate `<p>` → `<td></td>` (как asciidoctor).
- Тесты: +2 scanner (exact-spec, duplication unexpanded), +3 html
  (continuation 6 кейсов + comments, пустые ячейки, дупликация/цепочки),
  обновлены `| A | B |` (теперь пустая 3-я ячейка) и trailing-spec ассерты;
  тестовые вызовы переведены на хелпер `line_cells`.

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (941 passed).
- Пробы p1..p17 IDENTICAL, кроме пределов: p9/p16 (второй `<p>` после blank
  в ячейке), p11 (CSV drop incomplete row), p14 (`a|` nested-рендер — давний).
- **Корпус: Identical 267→282 (+15)**; blast (base 4099d62): 37 файлов —
  15 флипов (align-by-column 7→0, build-a-basic-table, add-cells-and-rows,
  row, style-operators 126→0, section-ref 626→0, header-ref, audio-and-video,
  link-macro-ref, unresolved-references, toc-ref, subs/attributes,
  post-replacements, quotes, special-characters), **0 регрессий**, остальные
  в основном ближе (table 612→556, subs-symbol-repl 226→165, replacements
  148→4 — остаток NCR-кластер, document-attributes-ref 6672→6538, ordered
  232→227); рост image-ref 659→746 / image-svg / cell / table-ref —
  позиционный шум поверх pre-existing корней, новые фрагменты сверены с
  эталоном (слитые ячейки = asciidoctor).
- Закоммичено (`0fe6e49`), смержено в master (`0c5418a`), запушено; локальная ветка удалена.

### Известные пределы (вне корпуса)
- Blank в ячейке → второй `<p class="tableblock">` (у нас один `<p>` с `\n`).
- CSV: неполная последняя строка не дропается (отдельный путь
  scan_delimited_format_table).
- `a|`-ячейка: нет nested-парсинга в `<div class="content">` (давний).
- `|e|x` без пробела: у нас `e` — спек (asciidoctor: контент, нужен
  whitespace перед спеком) — legacy-квирк, сохранён сознательно.

### Что дальше
- nearmiss на 282: **add-columns (40)**, footnote examples (70),
  bibliography (72), subs (76), ordered (90), part-with-special-sections
  (103), multipart-book (109), quote (109 — `-- Author` attribution),
  metadata (111), apply-subs-to-text (115).
- Новые кандидаты-корни из этой сессии: `cols=2;2;3;3` — `;`-разделитель
  cols не парсится (image-ref, image-svg); `l|`-ячейка должна рендериться
  `<div class="literal"><pre>` (image-svg); `[frame=ends,grid=none]` на
  таблице (image-svg); NCR-в-monospace кластер (replacements 4 diff —
  по памяти бесполезен в одиночку).
- Pre-existing из прошлых сессий: ячейка `a|` nested-парсинг, nested-список
  с другим маркером в li, `[square]`-класс, компактный colist-`<li><p>`,
  `== heading` не прерывает параграф, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, label block-anchor `[[id,label]]` над блоком не
  побеждает `.Title`, `\\https://…` двойной backslash.

---

## Сессия (2026-06-12, тридцатая) — Фаза 3: footnotes вне #content + merge стопки attrlist + cols-multiplier + trailing cell-spec + счётчики в verbatim

Запрос «продолжи». Ветка **`fix/pages-include-nearmiss`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 262, master `313a275`
(base-бинарь /tmp/adoc_base пересобран с master — ветка стартовала с него же).

### Выбор задачи
nearmiss: **pages/include.adoc (8 diff)** — один корень (footnotes); затем
**customize-title-label.adoc (66 diff)** — три корня, по дороге вскрыт и закрыт
четвёртый (pre-existing trailing cell-spec).

### Семантика asciidoctor (пробы /tmp/p_fn/p1, /tmp/p_ctl/p1..p11, m*, n*)
- **A (footnotes)**: `<div id="footnotes">` идёт ПОСЛЕ закрытия `</div>`
  `#content`, ПЕРЕД `<div id="footer">` (p_fn/p1).
- **B (стопка attrlist-строк)**: метаданные НАКАПЛИВАЮТСЯ, не заменяются:
  named — override по ключу; id — последний побеждает; roles/options —
  аккумулируются (`[#id1.r1]`+`[#id2.r2]` → id2, r1 r2, p8); позиционные —
  послотно: `[quote,Author]`+`[verse]` → verse + attribution (p9); пустой
  слот 1 не затирает стиль: `[source,ruby]`+`[,python]` → python (p10).
- **C (cols multiplier)**: `3*` → 3 колонки (33.3333/33.3333/33.3334 —
  последняя получает остаток), `2*1,3` → 20/20/60, `2*<.^2,>1` → 40/40/20
  со спеком на обеих (p2). caption= на таблице: verbatim-префикс, счётчик НЕ
  бампится (`Table A.`), пустой `[caption=]`/`[caption=""]` → голый title (p3).
- **D (trailing cell-spec)**: спек ячейки привязан к СЛЕДУЮЩЕМУ `|` — в конце
  строки это контент: `|a` → ячейка «a» (не AsciiDoc-style), `|d |e` хранит
  «e»; в середине строки `|one a|two` — спек следующей (проба n4).
- **E (счётчики в verbatim)**: include/conditionals — уровень READER (работают
  в listing!), счётчики `{counter:}`/attr-entries — уровень substitutions/блоков
  (в listing/literal/pass/comment/markdown-fence НЕ работают).

### Что сделано (5 точек)
- **РЕНДЕРЕР** finish.rs: render_footnotes гейтится `!standalone`; lib.rs run():
  footnotes эмитятся после `</div>` content, перед footer.
- **ПАРСЕР** attributes.rs: `BlockAttributes::merge(older, newer)` (id
  last-wins, roles/options extend, named override, позиционные послотно,
  выравнивание implied_source_lang при смешанных формах); block.rs ~615 —
  attrlist-арм мержит вместо замены.
- **РЕНДЕРЕР** blocks.rs `parse_col_widths`: multiplier `N*` раскрывается
  (зеркало parse_col_spec парсера, который уже умел).
- **ПАРСЕР** scanner.rs `parse_table_cells`: спек-суффиксы (style/span/align)
  парсятся только для НЕ-последней части строки (pre-existing: `|a` терял
  ячейку целиком, `|d |e` терял «e», `<.>` в конце строки ячейки съедался —
  этим был сломан и ряд corpus-таблиц).
- **ПАРСЕР** preprocessor.rs: трекинг verbatim-фенсов (`----`/`....`/`++++`/
  `////` с точной длиной закрытия + markdown ```) — внутри: счётчики не
  раскрываются, attr-entries не потребляются; conditionals/endif работают
  по-прежнему (reader-level, обрабатываются до фенс-проверки).
- Тесты: +1 html (footnotes вне content), +1 scanner (trailing cell-spec),
  +1 attributes (merge, 5 сценариев), +1 preprocessor (verbatim-фенсы,
  4 сценария), +3 html (стопка attrlist, multiplier-ширины, `|a`-контент),
  +1 html (counter literal в listing через preprocess).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (935 passed).
- Все пробы IDENTICAL кроме n4 — pre-existing предел: ячейка `a|` (AsciiDoc
  style) не рендерится как вложенный content-div (требует nested-парсинга).
- **Корпус: Identical 262→267 (+5)**; blast (base 313a275): 17 файлов —
  5 флипов (pages/include 8→0, customize-title-label 66→0, subs-group-table
  ×2, image-position), **0 регрессий**, 10 ближе (align-by-column 617→7!,
  row 310→81, add-columns 211→40, footnote 101→70, image-svg 312→263,
  pass-macro 249→241), column 168→172 и table 560→612 — позиционный шум,
  точечно сверено с эталоном (новые фрагменты = asciidoctor: 50/50-колонки,
  `<.>`-текст в ячейке сохранён).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Известные пределы (вне корпуса)
- Ячейка `a|` (AsciiDoc style): спек парсится, но рендерер не оборачивает в
  `<div class="content">` с nested-парсингом (давний архитектурный, n4).
- `[subs="+attributes"]` на listing: asciidoctor раскрыл бы счётчик при
  рендере — наш препроцессор внутрь фенса не заходит вовсе.
- merge позиционных при экзотике (newer со слотами 3+ без стиля) — послотное
  выравнивание приближённое (наша модель не хранит сырые слоты).

### Что дальше
- nearmiss на 267: **align-by-column (7 diff!)** — почти флип, разведать
  первым; add-columns (40), footnote (70), subs (76), bibliography (77),
  row (81), ordered (90), part-with-special-sections (103),
  multipart-book/quote (109 — quote: `-- Author` attribution не реализован),
  metadata (111 — позиционный шум).
- Pre-existing из прошлых сессий: ячейка `a|` nested-парсинг, nested-список
  с другим маркером в li, `[square]`-класс, компактный colist-`<li><p>`,
  `== heading` не прерывает параграф, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, label block-anchor `[[id,label]]` над блоком не
  побеждает `.Title`, `\\https://…` двойной backslash.

---

## Сессия (2026-06-12, двадцать девятая) — Фаза 3: include.adoc examples + links.adoc (форма include-директивы + comment в параграфах + autolink-границы/escape)

Запрос «продолжи». Ветка **`fix/include-directive-shape-and-mid-paragraph-comments`** —
НЕ закоммичена (рабочее дерево). Baseline: Identical 259, master `248d240`
(base-бинарь /tmp/adoc_base пересобран с master — тот же HEAD).

### Выбор задачи
nearmiss: **examples/include.adoc (52 diff)** — три корня (третий обнажился
по ходу); после него добит links.adoc (оставался 1 diff — escaped autolink).

### Семантика asciidoctor (пробы /tmp/p_inc: p1..p11, pA..pE, q1..q13, r1..r4)
- **A (include-shape)**: IncludeDirectiveRx заякорен — `include::` с колонки 0
  (индент → литерал/literal-блок, p9), `]` — ПОСЛЕДНИЙ символ строки (rstrip;
  `include::core.rb[tag=parse] <.>` → НЕ директива: сырой текст + conum, p1/p2);
  trailing-пробелы ок (p7); пробел ВНУТРИ target ок, на краях — нет.
  `\include::…[] tail` — не directive-shaped → НЕ escape, backslash остаётся (p10).
- **B (comment в параграфе)**: line-comment в середине параграфа дропается,
  строки сливаются в один `<p>` (p3/p5) — то же в admonition (pA), ulist (pB),
  dlist dd (pC), olist (pD); в verse/verbatim — контент (pE); comment+blank
  завершает параграф (p4); `////` рвёт (p6); «comment после blank рвёт списки»
  не затронуто.
- **C (autolink-границы)**: bare-URL линкуется только после старта строки,
  пробела или `<>()[];` — `:` (q1! — отсюда литеральная `include::https://…[]`
  линковалась), `-`(q3), `=`(q5), `,`(q6), straight `"`(q8/q9) блокируют;
  `'` у asciidoctor линкует НЕ из-за кавычки, а из-за `;` NCR `&#8217;` (q10).
  Trailing `)` никогда не входит в bare-URL — стрипаются ВСЕ (r1/r4, даже от
  `foo(bar)`), `;`/`:` тоже (r2/r3); но форма `URL[text]` — ДРУГОЙ альтернат
  regex: URL до `[` целиком, `)` сохраняется.

### Что сделано (ПАРСЕР, 4 файла)
- scanner.rs `is_include_directive`: без leading-trim, `strip_suffix(']')` после
  rstrip, path без краевых пробелов (по построению без `[`).
- preprocessor.rs: escaped-ветка — `strip_prefix('\\')` +
  `is_include_directive(rest)`-гейт (вместо безусловного starts_with).
- block.rs: skip-арм `is_line_comment` (advance+continue) в scan_paragraph
  (гейт `!verbatim_paragraph`, при пустом para_lines — break как раньше),
  scan_admonition, 3 цикла wrapped-строк (ulist/olist/colist, replace_all),
  dd-цикл dlist.
- inline.rs `try_autolink`: boundary-check prev-символа (старт/whitespace/
  `<>()[];`, хелперы `at_autolink_boundary`/`autolink_scheme_at`);
  trailing-стрип получил `)` и гейтится `!bracket_follows`
  (форма `URL[text]` идёт нестрипнутой — фикс регрессии key-concepts.adoc 0→3,
  пойманной первым blast'ом).
- inline.rs: НОВЫЙ escape-арм `\https://…` (handle_inline_escape) — backslash
  дропается, URL литерален; гейт: MACROS + autolink_scheme_at + валидная
  граница ПЕРЕД `\` (s-пробы: `word-\https` и `\\https` хранят backslash;
  сам URL не линкуется, т.к. prev для него — оставшийся в input `\`).
  Закрыл links.adoc (232→0, кейс `` `\https://…` `` в monospace).
- Тесты: test_line_comment_skipped переписан (фиксировал разрыв параграфа);
  +5 ассертов в test_is_include_directive; +1 preprocessor
  (non-directive verbatim, indent); +1 parser (comment в ulist-item);
  +2 html (merge параграф/admonition/dd/olist/verse/blank-негатив;
  autolink-границы + trailing-paren + escaped-autolink 3 кейса).

### Статус (верифицировано)
- clippy --workspace 0 (после touch — не кэш); cargo test --workspace зелёное
  (parser 480, html 356).
- Все 20+ проб IDENTICAL (нормализация compare_full), кроме s5 — известный
  pre-existing предел; examples/include.adoc 52→0.
- **Корпус: Identical 259→262 (+3)**; blast (base 248d240): 11 файлов —
  3 флипа (examples/include.adoc, document-attributes.adoc 284→0 — corpus-файл
  с массой comment-в-параграфах, links.adoc 232→0), **0 регрессий**, 5 ближе:
  pages/include.adoc 75→8, image-ref 686→659, subs.adoc 89→76, image 126→125,
  sdr-005 377→372; metadata.adoc 108→111 и outline.adoc 8664→8681 — позиционный
  сдвиговый шум, точечно сверено с эталоном (новый вывод = asciidoctor).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Известные пределы (вне корпуса)
- `a'https://…`: asciidoctor линкует (boundary = `;` от NCR `&#8217;` после
  replacements), мы — нет (сырой UTF-8 `'`).
- URL сразу после inline-спана (`*b*https://…`): asciidoctor линкует (`>` от
  `</strong>` в substituted-тексте), у нас prev=`*` → литерал (chunk-граница,
  родственно em-dash-пределу 28-й сессии).
- `\\https://…` (s5): asciidoctor хранит ОБА backslash; наш eager `\\`-escape
  съедает первый (pre-existing escape-модель, упоминалась в 23-й сессии).

### Что дальше
- nearmiss на 262: **pages/include.adoc (8 diff!)** — почти флип, разведать
  первым; customize-title-label (66), bibliography (77), subs (76),
  subs-group-table (90), ordered (90), footnote (101),
  part-with-special-sections (103), metadata (111 — позиционный шум, реально
  ближе). Кандидат-корень quote.adoc: `-- Author` attribution не реализован.
- Pre-existing из прошлых сессий: nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `cols="2*"` multiplier, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, label block-anchor `[[id,label]]` над блоком не
  побеждает `.Title`. («comment в середине dd-параграфа» — ЗАКРЫТ этой сессией.)

---

## Сессия (2026-06-12, двадцать восьмая) — Фаза 3: source.adoc (em-dash правила + include-строка = текст)

Запрос «продолжи». Ветка **`fix/source-block-nearmiss`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 258, master `6c5d1a3`
(base-бинарь /tmp/adoc_base пересобран через временный worktree).

### Выбор задачи
nearmiss: **source.adoc (63 diff)** — два корня. В файле `---- <.>` — не
делимитер → ВСЕ пары `----` смещены (asciidoctor так же! warning «unterminated
listing block»); расходились только: (A) em-dash в параграфе `---- <.>`,
(B) escaped-include строки.

### Семантика asciidoctor (пробы /tmp/p_src/p1..p7, все IDENTICAL после фикса)
- **A (em-dash)**: правила ровно `(\w)--(?=\w)` → em+ZWSP и
  `(^|\n| |\\)--( |\n|$)` → thin+em+thin (граничный пробел/`\n` ПОГЛОЩАЕТСЯ —
  строки сливаются; gsub: в `a -- -- b` второй `--` литерал). Правила `---`
  НЕТ: `a---b`, `g --- h`, `e----f`, `----` — литералы. `\--` — escape только
  там, где матчился бы unescaped (`\---` → backslash ОСТАЁТСЯ литералом).
- **B (include)**: include резолвится ТОЛЬКО в reader (наш препроцессор);
  строка `include::…[]`, дошедшая до парсера (от escaped `\include::`), —
  обычный ТЕКСТ: параграф, не рвёт параграфы/списки (пробы p5/p6).

### Что сделано (ПАРСЕР, 2 файла)
- inline.rs `apply_typographic_replacements`: арм `---` УДАЛЁН; spaced-арм —
  границы `^`/`\n`/пробел/конец с обеих сторон, граничный символ поглощается,
  guard `i > copied_up_to` (gsub-семантика); word-арм без изменений.
  `typographic_escape_len`: `\--` валиден только при (word-before+word-after)
  или (пробел/`\n`/EOL после) — `\---` больше не эскейпится.
  Пределы (вне корпуса): chunk-границы после inline-конструкций считаются
  line-границами (`*b*-- x` заменили бы, asciidoctor нет); merge строк через
  SoftBreak не делается (EOL `--` даёт em-dash, но `\n` остаётся).
- block.rs: include-арм УДАЛЁН из scan_directives + 4 break-условия
  (`is_include_directive`) из paragraph/list-сканов. Event::Include в enum
  остаётся (API), арм рендерера `<!-- include:: -->` — мёртвый, не тронут.
  scanner::is_include_directive жив (препроцессор).
- Тесты: 5 переписаны (2 block include → plain-text/не-рвёт; inline
  `hello---world`×2, mixed, `\---`-escape — фиксировали самодельную
  семантику), +кейсы в test_typographic_em_dash/spaced (literals, границы
  строк, gsub `a -- -- b` — probe-verified).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 478, html 354).
- Пробы p1..p7 IDENTICAL (нормализованные токены); source.adoc 63→0 diff.
- **Корпус: Identical 258→259 (+1)**; blast (base 6c5d1a3): 7 файлов —
  1 флип (source.adoc), **0 регрессий**, include.adoc 124→52 (сильно ближе),
  остальные 5 — равный счётчик (subs-symbol-repl/delimited: em-dash токены
  стали INREF; quote/data — noref-шум, другие корни).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 259: **include.adoc examples (52** — Unresolved-directive
  семантика?), customize-title-label (66), include pages (75),
  bibliography (77), subs (89), subs-group-table (90), ordered (90),
  footnote (101), part-with-special-sections (103), metadata (108).
- Замечен кандидат-корень quote.adoc (109): строка `-- Author` после
  кавычки-параграфа — attribution quote-блока не реализован.
- Pre-existing из прошлых сессий: nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `cols="2*"` multiplier, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, comment в середине dd-параграфа, label block-anchor
  `[[id,label]]` над блоком не побеждает `.Title`.

---

## Сессия (2026-06-12, двадцать седьмая) — Фаза 3: stem (4 корня: stem-эскейпы, block-macro catch-all, ++++, {n!})

Запрос «продолжи». Ветка **`fix/stem-block-macro-and-escapes`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 257, master `df05b5f`
(base-бинарь /tmp/adoc_base пересобран через временный worktree).

### Выбор задачи
nearmiss: **stem.adoc (56 diff)** — давно откладывался как «3-4 корня», но все
корни оказались малыми и хорошо локализуемыми.

### Семантика asciidoctor (пробы /tmp/p_st/p1..p5, все после фикса байт-в-байт)
- **A**: `stem:[…]` — `\]` НЕ закрывает макрос и unescape'ится в контенте
  (`stem:[[[a,b\],[c,d\]\]((n),(k))]` → `\$[[a,b],[c,d]]((n),(k))\$`;
  правило InlineStemMacroRx `(.*?[^\\])?\]`).
- **B**: блочные макросы матчатся ТОЛЬКО по зарегистрированным именам —
  `stem::[…]`, `foo::bar[baz]`, `chart::data.csv[w=100]` → литеральный параграф
  (`.Title` прикрепляется к нему); зеркало inline-правила 23-й сессии.
- **C**: `++++` в тексте = ПУСТОЙ `++`-passthrough (`++`+`++`) → рендерится в
  ничто; regex asciidoctor `(\+\+\+?)(.*?)\1` бэктрекает с `+++` на `++` с той
  же позиции.
- **D**: имя attr-ref — строго `\w[\w-]*`: `{n!}`/`{x!}`/`{name!fallback}` —
  НЕ референс, литерал (даже если `n` определён). Синтаксиса `!fallback` у
  asciidoctor НЕТ — был самодельный.

### Что сделано (ПАРСЕР, 4 точки)
- inline.rs: `parse_bracket_macro_escaped` (скан `]` с пропуском `\]`, unescape
  через Cow) — используется ТОЛЬКО в `try_stem_macro` (stem/latexmath/asciimath).
- block.rs: арм `scanner::is_custom_block_macro` УДАЛЁН из scan_block_macros;
  scanner.rs: `is_custom_block_macro`/`is_known_block_macro`/`is_valid_macro_name`
  удалены. Tag::CustomBlockMacro в enum остаётся (API), армы рендерера/compat —
  мёртвые, не тронуты.
- inline.rs: triple-plus-арм при провале пробует double-plus с ТОЙ ЖЕ позиции;
  в `try_double_plus_passthrough` close==0 разрешён (пустой → без события).
  Попутно `+++x++` теперь матчится как `++`+`+x`+`++` (бэктрек как asciidoctor).
- inline.rs: `!`-split в attr-ref удалён — content с `!` не парсится как реф;
  поле `fallback` в Event::AttributeReference остаётся (API), парсер всегда
  эмитит None; плюмбинг рендерера не тронут.
- Тесты: 2 parser + 4 html переписаны (фиксировали самодельную семантику
  fallback/custom-block-macro); +2 parser (stem-эскейпы; пустой `++++` +
  литеральный `stem::[…]`), +3 html (unknown block macro + title;
  stem-эскейпы; пустой `++++`).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (924: parser 479, html 354).
- Пробы p1..p5 байт-в-байт; stem.adoc 56→0 diff.
- **Корпус: Identical 257→258 (+1)**; blast (base df05b5f): ровно 1 файл
  изменился — 1 флип (stem.adoc), **0 регрессий** (удаление fallback и
  block-catch-all больше нигде в корпусе не стреляло).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 258: **source (63** — include-«Unresolved directive»-параграфы?,
  `----`→`—-` em-dash в callout-строке листинга), customize-title-label (66),
  include (75), bibliography (77), subs (89), subs-group-table (90),
  ordered (90), footnote (101), part-with-special-sections (103), metadata (108).
- Pre-existing из прошлых сессий: nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `cols="2*"` multiplier, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, comment в середине dd-параграфа должен слить строки
  в один `<p>`; label block-anchor `[[id,label]]` над блоком не побеждает
  `.Title`.

---

## Сессия (2026-06-12, двадцать шестая) — Фаза 3: lexicon (xreflabel/dt-терм → reftext)

Запрос «продолжи». Ветка **`fix/xreflabel-reftext-resolution`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 256, master `f2133db`
(base-бинарь /tmp/adoc_base пересобран через временный worktree).

### Выбор задачи
nearmiss: **lexicon.adoc (34 diff)** — давний кандидат-кластер «xreflabel →
reftext», один корень: все 34 diff'а — нерезолвленные `<<id>>` (у нас fallback
`[id]`, у asciidoctor — текст dt-терма / label).

### Семантика asciidoctor (пробы /tmp/p_xl/p1..7)
- `[[id]]term:: def` → `<<id>>` = текст терма; reftext по умолчанию даёт ТОЛЬКО
  leading-анкер dlist-терма. В параграфах и ulist-item'ах `[[id]]` без label —
  fallback `[id]`. Mid-term анкер (`middle [[jj]]term::`) — тоже fallback.
- `[[id,label]]` / `anchor:id[label]` → label побеждает терм; label
  форматируется при использовании (`label with *bold*` → `<strong>`).
- reftext — разметка: `[[hh]]term with *bold*::` → ссылка содержит
  `term with <strong>bold</strong>`.
- Forward-ref работает (резолв отложен до конца документа).
- Block-anchor `[[id,label]]` НАД блоком: label побеждает `.Title` (p4) —
  НЕ реализовано (предел, в корпусе нет; требует Event::BlockMetadata.reftext).

### Что сделано
- **ПАРСЕР** event.rs: `Tag::Anchor { id, label: Option<CowStr> }` (+into_static);
  inline.rs: `try_anchor` — label из `[[id,label]]` (trim_start, пустой → None),
  `try_anchor_macro` — label из bracket-контента. Тесты обновлены
  (test_anchor_with_reftext_still_works теперь ожидает label).
- **РЕНДЕРЕР** lib.rs: поля `anchor_reftexts: Vec<(String,String)>`,
  `dt_term_start: Option<usize>`, `pending_term_anchor: Option<(String,usize)>`.
  events.rs: Tag::DescriptionTerm — `dt_term_start = output.len()` после
  открывающей разметки (все 3 стиля); арм Anchor — label рендерится через
  render_inline_value → anchor_reftexts; leading-анкер в dt без label →
  pending_term_anchor (id, позиция после `</a>`); TagEnd::DescriptionTerm —
  захват `output[pos..]` как Markup-reftext, сброс dt_term_start.
  finish.rs: цикл `ctx.add_block(id, RefText::Markup)` после bibliography
  (add_block = or_insert, first-wins — секции/блоки/biblio выигрывают).
- +1 html-тест `test_anchor_reftext_xref_resolution` (7 кейсов: dt-терм,
  bold в терме, label с форматированием, anchor-макрос, forward-ref,
  негативы mid-term/параграф).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (923 total).
- Пробы p1..p7 сходятся (кроме документированного предела p4 `<<ee>>`).
- **Корпус: Identical 256→257 (+1)**; blast (base f2133db): ровно 1 файл —
  1 флип (lexicon.adoc 34→0), **0 регрессий**.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 257: stem (56 — 3-4 корня: `\$`-эскейп, `stem::`-макрос literal,
  `++++`+callout, `{n!}`), source (63), customize-title-label (66), include (75),
  bibliography (77), subs (89), subs-group-table (90), ordered (90),
  footnote (101).
- Новый известный предел: label block-anchor-строки `[[id,label]]` над блоком
  не побеждает `.Title` (нужен reftext в Event::BlockMetadata + BlockMeta +
  приоритет над block_ref_titles в finish).
- Pre-existing из прошлых сессий: nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `cols="2*"` multiplier, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, comment в середине dd-параграфа должен слить строки
  в один `<p>`.

---

## Сессия (2026-06-12, двадцать пятая) — Фаза 3: revision-information (had_blank_line не сбрасывался в dlist/colist-сканах)

Запрос «продолжи». Ветка **`fix/revision-information`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 255, master `8edb60d`
(base-бинарь /tmp/adoc_base пересобран через временный worktree).

### Выбор задачи
nearmiss: **revision-information.adoc (24 diff)** — один корень.

### Семантика asciidoctor (пробы /tmp/p_ri1..15)
- Comment-строка СРАЗУ после текста item'а (без blank перед ней) список НЕ рвёт —
  даже если ПЕРЕД этим item'ом была blank-строка (между entries одного dlist).
  Comment ПОСЛЕ blank — рвёт (поведение 18-й сессии, верно).
- Минимальный репро (p_ri13): `a:: x\n\nb:: y\n//c\n\nc:: z` → у asciidoctor
  ОДИН dlist; у нас был раскол после b. То же для colist (p_ri15).

### Корень
`scan_description_list_item` и `scan_callout_list_item` НЕ сбрасывали
`had_blank_line` (в отличие от scan_unordered/ordered_list_item). Blank перед
`b::` оставлял флаг взведённым → comment-handler (block.rs ~870, правило
«comment после blank разделяет списки») ошибочно закрывал список.

### Что сделано (ПАРСЕР block.rs, 2 строки)
- `self.had_blank_line = false` в конце `scan_description_list_item` (~2939) и
  `scan_callout_list_item` (~3161) — зеркало строки 3034 (unordered).
- +1 parser-тест `test_comment_after_dlist_entry_does_not_split_list`
  (позитив + негатив «после blank рвёт»), +1 html-тест
  `test_comment_after_list_entry_keeps_single_list` (dlist, colist, негатив).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 478, html 353).
- Пробы p_ri1..15 все сходятся; revision-information.adoc 24→0 diff.
- **Корпус: Identical 255→256 (+1)**; blast (base 8edb60d): ровно 2 файла —
  1 флип (revision-information.adoc), lexicon.adoc 376→34 (тот же корень рвал
  dlist по всему файлу), **0 регрессий**.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 256: **lexicon (34!** — остаток: xreflabel → reftext для
  xref-резолва, давний кандидат-кластер: label в Tag::Anchor + регистрация в
  XrefResolver + reftext из dt-терма), stem (56 — 3-4 корня: `\$`-эскейп,
  `stem::`-макрос literal, `++++`+callout, `{n!}`), source (63),
  customize-title-label (66), include (75), bibliography (77), subs (89),
  subs-group-table/ordered (90), footnote (101).
- Pre-existing из прошлых сессий: nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `cols="2*"` multiplier, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, comment в СЕРЕДИНЕ dd-параграфа должен слить строки в
  один `<p>` (p_ri4: asciidoctor «text a\nstill a», у нас два блока).

---

## Сессия (2026-06-12, двадцать четвёртая) — Фаза 3: pass.adoc + revision-line (passthrough-`</div>` + doc-интринсики)

Запрос «продолжи». Ветка **`fix/passthrough-stray-div-and-doc-intrinsics`** —
НЕ закоммичена (рабочее дерево). Baseline: Identical 253, master `99fab03`
(base-бинарь /tmp/adoc_base пересобран через временный worktree).

### Выбор задачи
nearmiss: **pass.adoc (18 diff)** — два корня; попутно закрыт
revision-line-with-version-prefix (1 diff — `{docdate}`, ранее скипался,
оказался того же семейства doc-интринсиков).

### Семантика asciidoctor (пробы /tmp/p_pt1..3, /tmp/probedir/p_doc1..2, p_rev2..4)
- Standalone passthrough (`++++` и `[pass]`-параграф) — контент ГОЛЫЙ, без
  обёртки вовсе (нечего закрывать).
- Интринсики от входного файла: `docname` (stem), `docfile` (abs path),
  `docdir`, `docfilesuffix`; `docdate`/`doctime`/`docdatetime` из **mtime**
  (`%F`, `%T %Z` → `14:30:45 +0300`); `localdate`/… = now. При stdin:
  docname/docfile undefined, docdir=cwd, docdate=now. Header-entry
  ПЕРЕОПРЕДЕЛЯЕТ docdate, но НЕ docname (locked).
- Attr-refs в revision-line резолвятся при ЧТЕНИИ строки (read-time):
  атрибут, определённый ПОЗЖЕ в header, — литерал; undefined — литерал;
  `v{docname}` → strip `v` идёт по уже резолвленному значению.

### Что сделано
- **РЕНДЕРЕР** events.rs TagEnd::DelimitedBlock: армы Passthrough (только
  newline-guard, БЕЗ `</div>`) и Comment (ничего) вместо catch-all `</div>`
  (каждый `++++`-блок и `[pass]`-параграф оставлял лишний `</div>`).
- **CLI** main.rs: сидинг интринсиков в initial_attrs (препроцессор) +
  html_attrs (рендерер); явные `-a` (и unset-формы) не перетираются
  (`cli_attr_names`-guard).
- **РЕНДЕРЕР** finish.rs::render_author_details: resolve_attr_refs_text на
  revnumber/revdate/revremark (теперь Option<String>, if-let по ссылке).
  Резолв в арме Event::Revision НЕ работает — парсер следом эмитит
  дублирующие Event::Attribute с сырыми значениями (перетирают); поэтому
  резолв на точке рендера.
- +2 html-теста: test_revision_attr_refs_resolved_in_details (LPR-префикс
  стрипается → «version 55»), test_passthrough_block_bare_content_no_stray_div.

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (920: parser 477, html 352).
- Пробы p_pt1/2 и p_doc1 байт-в-байт; corpus-файлы — чисто (кроме NCR-шума).
- **Корпус: Identical 253→255 (+2)**; blast (base 99fab03): 3 файла — 2 флипа
  (pass.adoc, revision-line-with-version-prefix.adoc), **0 регрессий**,
  stem 56=56 (нейтрально).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Известные пределы (задокументированы, в корпусе нет)
- Резолв revision-refs по header-FINAL state (asciidoctor — read-time): ref на
  атрибут, определённый позже в header, у нас резолвится, у него — литерал.
- v-strip по сырому значению: `v{docname}` → «vp_rev» (asciidoctor «p_rev»).
- docname/docfile/docdir у asciidoctor locked от header-entry — у нас
  переопределяются.
- `outfilesuffix`/`filetype` не сеются (слой рендерера); Ruby `%Z` vs chrono
  `%z` может разойтись в TZ с именованной зоной (UTC и т.п.).
- Pre-existing (НЕ тронуто, base тоже): author-line после attr-entry в header
  не распознаётся вовсе (нет details).

### Что дальше
- nearmiss на 255: **revision-information (24)**, stem (56 — 3-4 корня:
  `\$`-эскейп, `stem::`-макрос literal, `++++`+callout, `{n!}`), source (63),
  customize-title-label (66), include (75), bibliography (77), subs (89),
  subs-group-table/ordered (90), footnote (101).
- Кандидат-кластер: xreflabel → reftext для xref-резолва (label в Tag::Anchor +
  регистрация в XrefResolver; p_id1/2/3 + lexicon-остаток).
- Pre-existing из прошлых сессий: nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `cols="2*"` multiplier, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header.

---

## Сессия (2026-06-12, двадцать третья) — Фаза 3: literal-monospace (pass:SPEC + удаление custom-macro catch-all)

Запрос «продолжи». Ветка **`fix/inline-pass-spec-and-custom-macro-removal`** —
НЕ закоммичена (рабочее дерево). Baseline: Identical 250, master `7f05b9d`
(base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
stem (56 — 3-4 корня, снова отложен) → **literal-monospace (59 diff)**, один
корень: `` `\pass:c[]` `` → у нас `\p` + мусорный `<span custom-macro macro-ass>`.

### Семантика asciidoctor (пробы /tmp/p_ep1..5)
- `pass:SPEC[content]` — SPEC: одночар-алиасы `a c m n p q r v` + полные имена
  (`quotes`, `normal`…); перечисленные subs применяются к контенту
  (`pass:c[<b>]` → escaped, `pass:q[*b*]` → bold БЕЗ экранирования, `pass:n` —
  полный normal-набор). Без `[` после спека — НЕ макрос, литерал (`pass:c here`).
- `\pass:SPEC[…]` — backslash дропается, `pass:SPEC[` литерал, контент и
  хвостовой `]` идут через обычные subs (`\pass:c[*b*]` → `pass:c[<strong>b</strong>]`).
- `\\pass:SPEC[…]` — в escape участвует только ОДИН backslash, первый остаётся
  литералом (`\pass:c[abc]`).
- **Неизвестные inline-макросы НЕ матчатся вовсе** — литеральный текст,
  внутренность скобок идёт через обычные subs (`foo:bar[*b*]` →
  `foo:bar[<strong>b</strong>]`; `chart:sales[Q1,Q2]` — литерал).

### Что сделано (ПАРСЕР inline.rs + attributes.rs)
- `try_pass_macro`: optional spec (`pass_spec_len` — [a-z,_-]-ран строго до `[`;
  невалидный/без скобки → не макрос); `pass_spec_to_subs` (алиасы +
  `attributes::sub_name_to_flags`, теперь pub(crate)); `push_pass_spec_content` —
  ре-парс контента со спекнутым набором, Text→InlinePassthrough когда нет
  SPECIALCHARS (рендерер экранирует Text безусловно).
- Escape-армы: `\pass:SPEC[` (расширен с `pass:[`) + НОВЫЙ арм `\\pass:SPEC[`.
- `pass_macro_span_len` spec-aware (скип границ в constrained-спанах);
  `push_single_plus_content` — spec-aware границы, c-спек → Text (экранируется).
- **Catch-all custom-macro УДАЛЁН** (try_custom_inline_macro + dispatch-арм +
  scanner::is_known_inline_macro): был кошмарно жадный (target до `[` без
  ограничений — «Mono with content: `+abc+` [x]» матчился как макрос `content:`!).
  Tag::CustomInlineMacro остаётся в enum (API), блочный `name::` не тронут.
- Тесты: 3 html-теста переписаны (фиксировали неверную семантику custom-macro),
  +2 html (pass-spec 8 кейсов; escaped-pass 3 кейса), +1 parser (events).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (918: parser 477, html 350).
- Пробы p_ep1/2/4/5 байт-в-байт, кроме двух документированных пределов (ниже).
- **Корпус: Identical 250→253 (+3)**; blast (base 7f05b9d): 11 файлов — 3 флипа
  (literal-monospace, attribute-entries, revision-line), **0 регрессий**,
  8 changed-still-different — ВСЕ ближе: pass 133→18(!), footnote 260→101,
  revision-information 96→24, align-by-column 637→617, format-column-content
  218→198, apply-subs-to-text 119→115, syntax-quick-reference 2791→2735,
  outline 8718→8664.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Известные пределы (задокументированы в коде, в корпусе нет)
- `pass:c,q[…]`: asciidoctor гоняет q ПО уже экранированному тексту (`;` блокирует
  constrained-открытие) — bitflag-модель только membership, у нас `*x*` болдится.
- Spec'нутый pass внутри `+…+`: форматирующие subs не перегоняются (статик-хелпер),
  чтится только membership SPECIALCHARS.
- `foo:b\`ar[baz]`: наш eager-escape съедает backslash (asciidoctor хранит) —
  pre-existing разница escape-модели, не от этого фикса.

### Что дальше
- nearmiss на 253: **pass (18 diff!)**, **revision-information (24!)**, stem (56 —
  3-4 корня: `\$`-эскейп, `stem::`-макрос literal, `++++`+callout, `{n!}`),
  source (63), customize-title-label (66), include (75), bibliography (77),
  subs (89), subs-group-table/ordered (90), footnote (101);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
- Кандидат-кластер: xreflabel → reftext для xref-резолва (label в Tag::Anchor +
  регистрация в XrefResolver; p_id1/2/3 + lexicon-остаток).
- Pre-existing из прошлых сессий: nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `cols="2*"` multiplier, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, лишний `</div>` у standalone passthrough,
  unknown-style в class на quote/sidebar, list-merge через continuation-attrlist.

---

## Сессия (2026-06-12, двадцать вторая) — Фаза 3: block.adoc (`.Title` на списках)

Запрос «продолжи». Ветка **`fix/list-block-title`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 249, master `0e6808c` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
stem (56 — 3-4 независимых корня: `\$`-эскейп, `stem::`-макрос, `++++`+callout,
`{n!}`; отложен) → **block.adoc (57 diff)**, один корень: `.Title` на ulist.

### Семантика asciidoctor (пробы /tmp/p_lt1..6)
- `.Title` на списке → `<div class="title">` ВНУТРИ обёртки, ПЕРЕД
  `<ul>`/`<ol>`/`<dl>`/`<table>` (все формы: ulist/olist/dlist/horizontal/qanda/colist).
- `.Title` ПОСЛЕ blank в list-контексте закрывает списки (как block-attr/comment);
  title вешается на следующий блок. Двойной title — последний побеждает.
- `.Title`-строка БЕЗ blank внутри item/dd/параграфа/admonition-параграфа —
  обычный wrapped-текст (slurp): титулы НИКОГДА не прерывают параграф
  (прерывают attr-строки и делимитеры; `== heading` тоже НЕ прерывает — у нас
  прерывает, pre-existing, не тронуто).

### Что сделано
- **ПАРСЕР** block.rs: (1) `.Title`-handler в scan_block_metadata — close_list_contexts
  при had_blank_line в list-контексте (зеркало block-attr-ветки); (2) исключение
  `is_block_title` УБРАНО из `is_list_continuation_line`, `is_dlist_continuation_line`,
  break-условий `scan_paragraph` и `scan_admonition` (slurp как у asciidoctor).
- **РЕНДЕРЕР**: `emit_pending_block_title` после открытия обёртки в
  `start_unordered_list` (обе ветки), `start_ordered_list`, `start_description_list`
  (3 арма) — blocks.rs; arm `Tag::CalloutList` — events.rs.
- +3 теста: parser `test_block_title_after_blank_separates_lists` (2 кейса),
  parser `test_block_title_line_does_not_interrupt_paragraph`,
  html `test_list_block_title_html` (7 кейсов).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 476, html 348).
- Пробы p_lt1 байт-в-байт; p_lt2/4/5/6 — остатки только pre-existing другие корни
  (вложение списка с другим маркером внутрь li, `[square]`-класс на `<ul>`,
  компактный colist-`<li><p>`, heading не slurp'ится в параграф).
- **Корпус: Identical 249→250 (+1)**; blast (base 0e6808c): 6 файлов — 1 флип
  (block.adoc), **0 регрессий**, 5 changed-still-different — все ближе:
  ordered 223→90, unordered 298→145, release-and-progress-reviews 409→406,
  outline 8735→8718, admonition 197=197 (len ближе).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 250: stem (56 — 3-4 корня: инлайн `\$[[...]]`-эскейп ломает текст,
  `stem::[...]` должен остаться литеральным параграфом а не custom-macro,
  `++++ <.>` в callout-листинге, `{n!}` дропается в latexmath-параграфе),
  literal-monospace (59), source (63), customize-title-label (66), include (75),
  bibliography (77), subs (89), ordered (90 — стало ближе);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
- Кандидат-кластер: **xreflabel → reftext для xref-резолва** (label в Tag::Anchor
  + регистрация в XrefResolver; закрыл бы p_id1/2/3-строки и lexicon-остаток).
- Новые pre-existing находки (НЕ в корпусе как флип): `* x` после blank внутри
  `- y`-списка должен вкладываться как nested ulist в li (у нас — sibling);
  `[square]`-стиль не даёт класс на `<ul>`; colist-`<li><p>` компактен (нет
  переносов); `== heading` не прерывает параграф у asciidoctor (у нас прерывает).
- Прочее: `cols="2*"` multiplier (row.adoc), `[abstract]`-параграф → quoteblock,
  `:icons:`-colist (TODO), кластер `m`/`e`/`s` стиля колонок; pre-existing: лишний
  `</div>` у standalone passthrough, unknown-style течёт в class на quote/sidebar,
  пустые строки в пустых sectionbody, list-merge через continuation-attrlist (p_chk2).

---

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
  берём ближайший к флипу. (revision-line-with-version-prefix закрыт в 24-й
  сессии — CLI сидит `docdate` из mtime файла.)
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
