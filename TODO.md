# TODO — adoc-parser

Roadmap по итогам архитектурного аудита. Источник задач совместимости — `COMPAT-DIFF.md`
(числа в скобках — затронутые файлы корпуса `/mnt/c/tmp/adoc-test/`, 344 шт.).

Перед каждым коммитом: `cargo clippy --workspace` (0 warnings) + `cargo test --workspace`
(всё зелёное). Никогда не коммитить прямо в master — сначала ветка (см. CLAUDE.md).

---

## FRONTIER-КОРПУС (2026-06-17, 106-я): 9 классов расхождений на новых репозиториях

Основной гейт (`/mnt/c/tmp/adoc-test/`, 3 репо) ОСТАЁТСЯ чистым **344/344** (регресс-гард). Новые репо вынесены
в **`/mnt/c/tmp/adoc-frontier/`** (НЕ засоряют гейт): `asciidoctor-org` (74) + `asciidoctor` (176) = 250 файлов,
**191 identical, 41 чистых расхождения** (+18 include-шум). Скрипты: `frontier_parity.py`, `showdiff.py <file>`
(в каталоге `/mnt/c/tmp/adoc-test/`). Найденные классы (приоритет по частоте/импакту, верифицированы пробами):

- [x] **F-BD. Именованные атрибуты ссылки `id=`/`title=` на `<a>`** (ветка `fix/link-label-span-and-named-attrs`,
  2026-06-23). Запрос «начни следующую задачу». master `1c85cd2` (F-BC/F-BB смержены, в TODO не записаны — велись
  через session.md). Триаж adoc2docx: ближайший clean-div `links.adoc` (62 позиц. diff). **2 корня** (позиц. differ
  раздут десинком @[58]): **A** = `[.role]#text#` span в метке ссылки (АРХИТЕКТУРНЫЙ, не взят — см. ниже); **B** =
  named `id=`/`title=` НЕ рендерились на `<a>` (строка 30). **Корень B СДЕЛАН.** Правило asciidoctor (`html5.rb
  convert_inline_anchor` тип `:link` + `append_link_constraint_attrs`, эмпирически подтверждено): порядок атрибутов
  = **href, id, class(role), title, target, rel**. **Фикс (6 файлов):** `attributes.rs` `LinkAttrs`+поля id/title,
  `parse_link_attrs` ловит `"id"`/`"title"`; `event.rs` `Tag::Link`+поля id/title (def+clone); `adoc-html/events.rs`
  рендер id (после href) + title (после class) через `write_attr`+`resolve_inline_attr_value`; `subst/macros.rs`
  (движок) `build_link`+параметры, проводка в try_link/try_mailto/autolink-`url[text]` + sentinel-guard на id/title;
  `inline.rs` (legacy) 4 link_attrs-сайта проводят, bare→None, тест-сайты обновлены. +1 тест
  `test_link_id_and_title_attrs_html`. clippy `--workspace` 0; **test --workspace 0 упавших** (html 528→529, parser
  645, compat 233). **Гейт 344/344 байт-в-байт** vs master `1c85cd2` (0 корпусных link id/title). **Sweep
  frontier(250)+adoc2docx(52)=304 new-vs-base: РОВНО 1 файл** (links.adoc строка 30, байт-в-байт == asciidoctor 2.0.23),
  **0 регрессий**. CLI-пробы (link:/bare-URL/mailto/named-only-bare) == asciidoctor. Коммит/merge/push — ПО ЗАПРОСУ.
  - [ ] **links.adoc корень A** — `[.role]#text#` в метке link/url-макроса (строки 17,19). АРХИТЕКТУРНЫЙ: у нас
    macros-ПЕРЕД-quotes (зеркало legacy), asciidoctor — quotes-ДО-macros → `[attrlist]#…#` становится `<span>` до
    захвата метки. Правило фикса ПОНЯТО+подтверждено (граничные `[a [b] c]`/`[label]*next*`/`[a [.role]#span# b]`):
    при поиске закрывающего `]` метки ПРОПУСКАТЬ `]`, который закрывает ВНУТРЕННИЙ `[…]` (unescaped `[` строго между
    скобкой макроса и кандидатом) И сразу за ним идёт quote-маркер, формирующий ВАЛИДНЫЙ span (детекция —
    `quotes.rs constrained_open_close`/`simple_pair_open_close`, уже `pub(super)`, на сыром src в macros.rs).
    Отдельный осторожный инкремент (критичное сканирование скобок макроса). Флипнет links.adoc → Identical (adoc2docx 44→45).

- [x] **F-BA. xrefstyle для CAPTIONED-БЛОКОВ (`AbstractBlock#xreftext`) + listing-caption рендеринг**
  (ветка `feat/xref-block-caption`, 2026-06-22). Запрос «начни следующую задачу». Продолжение F-AZ: слайс block-caption
  (документирован в F-AZ как самый влиятельный остаток). **Баг (verified `abstract_block.rb:345-370` + CLI-пробы):**
  xref на captioned-блок (figure/table/listing/example с title+caption) со стилем full/short мы выдавали голым title;
  asciidoctor — caption-форму. Плюс наши listing/source блоки вообще не рендерили подпись «Listing N.» при
  `:listing-caption:`. **Корень:** отсутствие block-xreftext + listing-caption. **Алгоритм (упрощён, доказано
  эквивалентно для корпуса — `if`/`else` ветки asciidoctor совпадают для нормальных блоков):** гейт = `@title &&
  !@caption.empty`; **full** = `{caption.chomp('. ')}, &#8220;{title_html}&#8221;`; **short** = `{caption.chomp('. ')}`;
  **basic/nil/default** = bare title; reftext выигрывает у всего (style-independent). Listing-caption не сидируется по
  умолчанию (norm = голый title; при set → «Label N. » + общий счётчик listing/source). **Фикс (5 файлов):**
  (1) `adoc-render-core` — `CaptionKind::Listing` + `listing` счётчик (семантика figure/table: bump только при Numbered);
  pub `block_xreftext(caption, title_html, style)` (mirror `section_xreftext`). (2) `adoc-html`: `block_refs: Vec<(id,
  BlockRefMeta{caption,title_html})>`; `render_caption_prefix` (возвращает escaped-строку + bump, заменил
  `push_caption_prefix`) + `register_block_ref`; `emit_listing_title` (caption-aware listing+source); регистрация на
  сайтах table/figure/example/listing/source. (3) `finish.rs` — в xref-loop ветка: full/short + captioned (`!caption.empty`)
  + НЕ в `reftext_ids` (anchor+bibliography) → `block_xreftext`; иначе прежний путь. **Parts + chapter-signifier-в-ЗАГОЛОВКАХ
  ДЕФЕРНУТЫ** (другие корни xref.adoc). clippy 0, **test --workspace 1288 зелёных** (render-core 24→25, html 521→524
  [+3 теста], parser 645, compat 233). **Гейт 344/344 байт-в-байт** vs master `11786f8` (gate_check.py 0 diff — ни один
  гейт-файл не ставит `:listing-caption:`; xref-конструкции не триггерят full/short block-path). **Sweep
  frontier(250)+adoc2docx(52)=302 new-vs-base: РОВНО 3 файла, 0 регрессий**, все = улучшения == asciidoctor: **images.adoc
  1→0** (Identical adoc2docx 41→42), **xref.adoc 833→10** (остаток = chapter-signifier-в-ЗАГОЛОВКАХ ×9 + part-xref ×1,
  out-of-scope), **source.adoc 682→681**. CLI-пробы == asciidoctor 2.0.23 (example full/short/basic, suppressed `caption=`→
  bare title, reftext+caption+full→reftext wins, 6 xref.adoc block-кейсов байт-в-байт). **Остаток xrefstyle (вне scope):**
  chapter/part-signifier в ЗАГОЛОВКАХ (флипнет остаток xref.adoc); parts (@numbered); compat-mode кавычки; нумерация
  спец-секций (sections.adoc 20). Коммит/merge --no-ff/push — ПО ЗАПРОСУ.

- [x] **F-AZ. xrefstyle для СЕКЦИЙ (`Section#xreftext`): basic/short/full + дефолт, signifiers, section reftext**
  (ветка `feat/xref-section-xrefstyle`, 2026-06-22). Запрос «начни следующую задачу из TODO» + выбор пользователя «xrefstyle,
  слайс секции» (корпус достиг хвоста: основной frontier 250 исчерпан = manpage/malformed/3 не-бага; adoc2docx — только
  крупные фичи [xrefstyle/нумерация спец-секций] или Rouge/sequential-quotes). **Баг (3 в одном, verified пробами asciidoctor
  2.0.23):** (1) внутренний auto-text xref на нумерованную секцию мы выдавали с номером (`<a href="#s1">1. First</a>`),
  asciidoctor — голый title `First` (дефолтный xrefstyle = bare title; мы клали `TocEntry.title` с номером как текст ссылки);
  (2) `xref:id[xrefstyle=short]` мы брали буквально как текст ссылки; (3) section `reftext` не honor'или (`1. Titled` вместо
  `My Ref Text`). Все три — следствие отсутствия `Section#xreftext`. **Корень (verified `section.rb:119-157` + 10+ CLI-проб):**
  приоритет reftext→стиль; full=`{refsig} {num}, {quoted}` (sectnum '.', ',' → comma-suffix), short=`{refsig} {num}`,
  basic/nil → bare title (но chapter/appendix → `<em>title</em>`); кавычки chapter/appendix=`<em>`, section=`&#8220;…&#8221;`;
  signifier=`{sectname}-refsig` (дефолты Section/Chapter/Appendix, явный сброс `!` → нет signifier); ненумерованная → basic-стиль;
  стиль = per-xref `xref:id[xrefstyle=X]` поверх документного `:xrefstyle:`. **Фикс (12 файлов):** (1) `adoc-render-core`
  — pub `SectName`/`SectionRefMeta`/`section_xreftext` (чистое форматирование готового HTML) + `SectionNumberer.last_number`
  (bare-номер `1.1`/`A`/roman, заполняется в number/appendix/part_prefix). (2) `event.rs` — `Tag::CrossReference{+xrefstyle}`
  + to_static. (3) `attributes.rs` — pub `extract_xref_attrs` (positional[0]=label + named xrefstyle, заимствованные слайсы),
  вызывается ТОЛЬКО при `=` в bracket (гейт asciidoctor; без `=` весь bracket = label, как раньше). (4) парсинг в ОБОИХ движках
  симметрично (`inline.rs::try_xref_macro`, `macros.rs::try_xref`/`build_cross_reference`); shorthand `<<>>` всегда
  `xrefstyle:None`. (5) `adoc-html`: захват `section_refs: Vec<(id, SectionRefMeta)>` (`start_section_div` → sectname+reftext;
  `start_section_title` → bare-номер + offset rendered-title; `TagEnd::SectionTitle` → срез `output[start..]` как raw_title_html
  ДО `</hN>`); refsig-дефолты засеяны в `document_attrs`; `xref_placeholders` +4-е поле (per-xref style); `finish.rs` — для
  section-target эффективный стиль (per-xref.or(doc)) → `section_xreftext`, иначе прежняя логика (anchors/blocks/bibliography).
  **Parts ДЕФЕРНУТЫ** (тонкость part-`@numbered`: `<<part>>` даёт то bare title то `prt I,…` при идентичных partnums/part-refsig
  — `numbered=None` → bare title, безопасно). `TocEntry.title` НЕ тронут (TOC/natural-xref). +6 render-core +1 parser (форма
  события + parity pipeline==legacy) +5 html (default-bare-фикс, модели section/chapter/appendix, reftext+custom/unset signifier,
  per-xref override). clippy 0, **test --workspace зелёное** (parser 644→645, html 516→521, render-core 18→24, compat 233).
  **Гейт 344/344 байт-в-байт** vs master `99af83a` (`gate_check.py` 0 diff — гейт-файлы с sectnums+xref все inter-doc/явная
  метка, дефолтного бага не задевают). **Sweep frontier(250)+adoc2docx(52) new-vs-base = 0 регрессий**, улучшения: **xref.adoc
  1495→833**, **sections.adoc 55→20**, callouts 196→195, asciidoctor-0-1-4 body-xref на appendix `Appendix A: TL;DR`→`TL;DR`
  (== asciidoctor; TOC сохраняет caption). `images.adoc` НЕ флипнут (figure-xref, не секция — слайс блоков). 8/8 CLI-проб ==
  asciidoctor 2.0.23. **Остаток xrefstyle (вне scope):** parts (@numbered); block-caption figure/table/listing/example (слайс
  блоков → images.adoc); chapter/part-signifier в ЗАГОЛОВКАХ (`My Chapter 1. Section`) → xref.adoc не флипается целиком;
  compat-mode кавычки. Коммит/merge --no-ff/push — ПО ЗАПРОСУ.

- [x] **F-AY. Макро-форма autolink (`url[…]`) после границы `"`/`'` не распознавалась** (ветка
  `fix/autolink-macro-quote-boundary`, 2026-06-22). Запрос «начни следующую задачу из TODO». Master `679b5b7` чист (F-AX
  смержен), открытые `[ ]` синтетические. Триаж `frontier_parity.py /mnt/c/tmp/adoc2docx` (41 identical / 8 clean-div):
  малые файлы — `images`(1)/`xref`(1495) архитектурные (xrefstyle:full = нумерованные подписи + full-style текст),
  `callouts/xml/source/test` упираются в Rouge syntax-highlighter (`<span class="nb">`), `sections`(55) — глубокая
  нумерация special-секций. `links.adoc`(89) — **мульти-root**: (A) вложенные `[]` в тексте ссылки
  (`[.overline]#…#`) = архитектурный (наш движок извлекает macros ДО quotes, asciidoctor — после, см.
  `subst/mod.rs::run_pipeline_with` стр.243/254); (B) макро-autolink после `"`; (C) `id=`/`title=` сбрасываются.
  **Выбран корень B** (первое расхождение, частый реальный паттерн, чисто аддитивно к границе). **Корень (verified
  исходником asciidoctor 2.0.23 `rx.rb` `InlineLinkRx` + 6 CLI-проб):** left-boundary класс
  `(^|link:|#{CG_BLANK}|&lt;|[>\(\)\[\];"'])` допускает `"`/`'`, НО они открывают ТОЛЬКО макро-форму (`url[…]`): bare
  `"https://x"` остаётся литералом (asciidoctor его не линкует), macro `"https://x[]"`/`'https://x[t]'` — линкуется
  (`(` уже линкует и bare). Оба наших движка имели `at_autolink_boundary` без `"`/`'` (только ws/`<>()[];`), поэтому
  `"url[]"` не открывался. **Фикс (2 файла, оба движка симметрично):** (1) `subst/macros.rs::try_autolink` — `quote_boundary
  = prev ∈ {",'}`; при провале `autolink_url_limit` и quote_boundary → limit=len() (tentative), затем гейт
  `if quote_boundary && !bracket_follows → None` (bare-после-кавычки не линкуется). (2) `inline.rs::try_autolink` (legacy)
  — то же: admit `"`/`'` к boundary-проверке + тот же гейт после `bracket_follows`. `"` остаётся литералом в тексте до
  ссылки (flush до scheme-start / байт в буфере). +1 parser (`quoted_boundary_links_macro_form_only_matches_asciidoctor` —
  макро `"`/`'`/`("`/labelled линкуются, bare `"`/`'` нет, `(` bare регресс-гард, pipeline==legacy на всех) +1 html
  (`test_macro_autolink_after_quote_boundary_html` — байт-точный `Type "…[]"`, single-quote+label, bare-в-кавычках
  регресс-гард). clippy 0, **test --workspace 1272 зелёных** (parser 643→644, html 515→516). **Гейт 344/344 байт-в-байт**
  vs master `679b5b7` (`gate_check.py` 0 diff — 0 корпусных `"url[…]"`). **Frontier 250 + adoc2docx 52 new-vs-base sweep =
  РОВНО 1 файл** (links.adoc, diff 89→62: строка-13 `Type "https://asciidoctor.org[]"` теперь байт-в-байт с asciidoctor),
  **0 регрессий**. 6 CLI-проб == asciidoctor 2.0.23 (macro `"`/`'`/`("`; bare `"`/`'` не линкуется; `(` bare линкуется).
  **Остаток links.adoc (вне scope, документировано):** корень A (вложенные `[]` = архитектурный порядок macros/quotes,
  проект `proj_sequential_quotes_rewrite`); корень C (`id=`/`title=` на ссылке — ~40 сайтов конструирования `Tag::Link`,
  чисто аддитивно, отдельная задача). Коммит/merge --no-ff/push — ПО ЗАПРОСУ.

- [x] **F-AX. Experimental UI-макросы: `btn:[…]`-label и `menu:`-сегменты не проходили inline-субституцию**
  (ветка `fix/ui-macro-btn-menu-inline-subst`, 2026-06-22). Запрос «начни следующую задачу из TODO.md». Master `1aed67a`
  чист (F-AW смержен), открытые `[ ]` синтетические. Триаж `frontier_parity.py /mnt/c/tmp/adoc2docx` (40 identical /
  9 clean-div): `menu.adoc` (80 diff, позиционный десинк = single-root). `showdiff`+пробы: `btn:[~_Ok_~]` мы рендерили
  `<b class="button">~_Ok_~</b>` (asciidoctor `<b class="button"><sub><em>Ok</em></sub></b>`), `menu:View[_Zoom_ > Reset]`
  → submenu `_Zoom_` сырой (asciidoctor `<em>Zoom</em>`). **Корень (verified исходником asciidoctor 2.0.23 `substitutors.rb`
  + 5 проб — тот же класс, что F-AW):** весь `sub_macros` идёт в цепочке `normal` ПОСЛЕ specialchars+quotes+replacements,
  поэтому markup в `[…]` UI-макроса субституируется ДО извлечения макроса; оба наших движка (subst `macros.rs::try_btn`/
  `try_menu` И legacy `inline.rs::try_btn_macro`/`try_menu_macro`) отдают контент СЫРЫМ `Text`-событием → рендерер
  (`adoc-html`) эскейпил без субституции. (Квотированный menu `"a > b"` через `build_menuseq` УЖЕ переразбирал сегменты —
  асимметрия с формальным `menu:t[…]`.) **Фикс (3 файла, ЧИСТО РЕНДЕРЕР — единая точка: оба движка эмитят одинаковый
  raw-Text):** (1) `lib.rs` — хелпер `render_ui_macro_inline(value, escape_quotes)`: парсит через `current_subs()`,
  no-markup fast-path → ref-сохраняющий escape (`html_escape_preserving_refs` при `escape_quotes` для menu / `_text_`
  для btn — байт-точность простых label + char-ref `a&#167;b`), иначе `push_event`. (2) `events.rs` — button-ветка Text
  чистит `button_mode` и зовёт хелпер (`escape_quotes=false`). (3) `inline.rs::render_menu` — target + каждая часть через
  хелпер (`escape_quotes=true`); split на `>` остаётся на СЫРЫХ items (разделители литеральные до субституции), затем
  субституция каждой части. kbd НЕ тронут (общий `try_bracket_ui`, но рендерится через `kbd_mode`-split, своя ветка).
  +2 html (`test_btn_inline_subst_html` [`~_Ok_~`→sub/em, `*Bold*`→strong, char-ref `a&#167;b`, литерал `"q"`],
  `test_menu_segment_inline_subst_html` [`_Zoom_`→em, `Save As...`→ellipsis-replacements]). clippy 0, **test --workspace
  1270 зелёных** (html 513→515). **Гейт 344/344 байт-в-байт** vs master `1aed67a` (`gate_check.py` 0 diff — корпусные
  btn/menu без markup/replacements в контенте). **Frontier 250 + adoc2docx 52 new-vs-base sweep = РОВНО 1 файл**
  (menu.adoc, 80→0 diff, байт-в-байт), **0 регрессий**. **adoc2docx 40→41 identical (+1).** 5 CLI-проб == asciidoctor
  2.0.23. **Побочно (улучшение):** menu-item `...`/`--` теперь кёрлятся replacements (semantically == asciidoctor; raw
  UTF-8 `…​` vs его NCR `&#8230;&#8203;` — фоновая typographic-NCR разница, не флипает в одиночку). Коммит/merge --no-ff/
  push — ПО ЗАПРОСУ.

- [x] **F-AW. Font-иконка: именованный `size=X` игнорировался + значение `title` не проходило inline-субституцию**
  (ветка `fix/icon-named-size-and-title-subs`, 2026-06-22). Запрос «начни следующую задачу из TODO.md». Master `419f1af`
  чист (F-AV смержен; session.md устарел — обычная картина), открытые `[ ]` синтетические «0 корпусного выигрыша».
  Триаж `frontier_parity.py /mnt/c/tmp/adoc2docx` (39 identical / 10 clean-div): наименьший системный — `icons.adoc`
  (10 diff). `showdiff` выявил ДВА чистых бага рендерера на каждой из 10 строк: (1) `<i class="fa fa-address-card"
  title="~Title~">` vs asciidoctor `… fa-2x/fa-fw/… title="<sub>Title</sub>">`. **Корни (verified исходником
  asciidoctor 2.0.23 + 7 проб):** (a) `substitutors.rb:419` для icon `posattrs=['size']` → `size` это и первый
  позиционный, и именованный атрибут; наш `render_icon` (`adoc-html/inline.rs`) имел ветку size ТОЛЬКО для позиционного
  (`i==0 && нет '='`), в `match` по именам ветки `"size"` НЕ БЫЛО → `size=2x`/`size=fw`/… молча терялись (класс размера
  отсутствовал во всех 10 строках). (b) весь `sub_macros` идёт в цепочке `normal` ПОСЛЕ `specialcharacters`+`quotes`, поэтому
  `~Title~` внутри `[title=…]` превращается в `<sub>Title</sub>` ещё на проходе quotes — ДО извлечения макроса; наш движок
  отдаёт атрибуты icon сырым `Text`-событием (`subst/macros.rs::try_icon`, by design — leaf-макрос), рендерер html-эскейпил
  title без субституции (`a < b`→`&lt;` срабатывал лишь как attr-escape в `write_attr`, а `~`/`*`/`__` оставались сырыми).
  **Фикс (1 файл, `adoc-html/src/inline.rs::render_icon`, чисто рендерер — как F-AO):** (1) добавлена ветка `"size" =>
  size = Some(...)` в `match key` (аддитивно, позиционный путь не тронут; size+rotate/flip уже эмитились независимо —
  size перед flip/rotate, flip>rotate); (2) title рендерится через `render_inline_value(&mut tmp, t.trim_matches('"'))`
  (de-quote + текущие subs блока = NORMAL в параграфе → quotes/replacements; no-markup fast-path сохраняет байт-точность
  простых title как `title=Info`), вместо сырого `write_attr`. +3 html (`test_icon_named_size_html`,
  `test_icon_size_with_rotate_html` [size+rotate и size+flip], `test_icon_title_inline_subst_html` [`~Title~`→sub,
  `*Bold*`→strong, quoted `"quoted ~sub~ val"`→de-quote+sub]). clippy 0, **test --workspace 1268 зелёных** (html 510→513).
  **Гейт 344/344 байт-в-байт** vs master `419f1af` (`gate_check.py` 0 diff — корпусные `:icons: font`-файлы без
  inline-`icon:` с `size=`/markup-`title`). **Frontier 250 + adoc2docx 52 new-vs-base sweep = РОВНО 1 файл** (icons.adoc,
  clean-div 10→0, байт-в-байт с asciidoctor), **0 регрессий**. **adoc2docx 39→40 identical (+1).** 7 CLI-проб == asciidoctor
  2.0.23 (size=2x/[2x]/size=fw+rotate=270/title=~Sub~/title=*Bold*/role+size+title=__em__/quoted-title). Коммит/merge
  --no-ff/push — ПО ЗАПРОСУ.

- [x] **F-AV. Стиль `[abstract]` (параграф/open-блок) → `quoteblock` с `<blockquote>`** (ветка
  `fix/abstract-block-quoteblock`, 2026-06-22). Запрос «начни следующую задачу из TODO.md». Master `86dfa6f` чист,
  открытые `[ ]` все синтетические «0 корпусного выигрыша». Триаж adoc2docx (`frontier_parity.py /mnt/c/tmp/adoc2docx`):
  `abstract.adoc` 6 diff — `[abstract]` на параграфе мы рендерили `<div class="paragraph abstract"><p>`, на open-блоке
  `<div class="openblock abstract"><div class="content">`; asciidoctor оба — `<div class="quoteblock abstract"><blockquote>`.
  **Корень (verified исходником asciidoctor 2.0.23 `parser.rb`+`html5.rb`):** `PARAGRAPH_STYLES` включает `abstract`;
  параграф `[abstract]` → `build_block(:open,:compound,…)`, но при `terminator.nil?` (это параграф) `content_model`
  понижается до `:simple` → текст ложится в blockquote БЕЗ `<p>`-обёртки; open-блок `--` остаётся `:compound` → дети-
  параграфы внутри blockquote. Конвертер `convert_open` для `style=='abstract'` эмитит ЕДИНУЮ структуру
  `<div{id} class="quoteblock abstract{role}">[<div class="title">…</div>]<blockquote>{content}</blockquote></div>`
  (только содержимое различается — поток событий его и даёт). **Фикс (3 файла, в рендерере):** (1) `blocks.rs` — хелперы
  `start_abstract_block` (общее открытие: `write_meta_attrs(meta,"quoteblock")` даёт `quoteblock abstract {roles}`,
  т.к. style уже в meta; + `emit_pending_block_title` + `<blockquote>`) и `close_abstract_block` (newline-guard +
  `</blockquote></div>` — покрывает обе формы: голый текст параграфа без хвостового `\n`, дитя open-блока с `\n`);
  `start_paragraph` ветвится на `style=="abstract"` (флаг `abstract_para`, без `<p>`); `start_delimited_block` Open ветвится
  (флаг в `delimited_block_stack` tuple — `(Open,true)`=abstract). (2) `events.rs` — `TagEnd::Paragraph` при `abstract_para`
  → `close_abstract_block`+return; `TagEnd::DelimitedBlock` новая ветка `(Open,true)`. (3) `lib.rs` — поле `abstract_para`.
  **Секционный `[abstract]` (`== Title`) и `partintro` НЕ задеты** (другой путь — `start_section_div`/openblock). +4 html
  (`test_abstract_paragraph_quoteblock_html` байт-точный с title, `_no_title_html`, `_open_block_quoteblock_html`
  дети-параграфы, `_block_id_role_html` id+роль на обеих формах). clippy 0, **test --workspace зелёное** (html 506→510,
  parser 643, compat-lab 1, прочее неизменно). **Гейт 344/344 байт-в-байт** vs master `86dfa6f` (`gate_check.py` 0 diff —
  все корпусные `[abstract]` внутри `[source]----` listing-блоков [показ синтаксиса] либо секционный стиль → не рендерятся
  как abstract-блок). **adoc2docx: 37→39 identical (+2):** `abstract.adoc` (clean-div 6→0) и
  `asciidoc/software-development-cookbook.adoc` (inc-div→0, единственным расхождением был abstract) — оба байт-в-байт с
  asciidoctor; `test.adoc` abstract-регион MATCH (прочие diff'ы остались, остаётся clean-div). **new-vs-base sweep
  frontier+adoc2docx = РОВНО 3 файла** (abstract/cookbook/test, все abstract-регионы построчно == asciidoctor), **0
  регрессий** (frontier без `[abstract]` → 0 изменений). **5 CLI-проб == asciidoctor 2.0.23** (параграф id+role+title,
  open id+role, abstract-в-секции, book+doctitle). **Вне scope (ниша, нет в корпусе/frontier):** book БЕЗ doctitle +
  первый блок `[abstract]` — asciidoctor НЕ создаёт преамбулу (`parser.rb:330` `has_header || (book && attributes[1] !=
  'abstract')`) и ИСКЛЮЧАЕТ контент (`convert_open` guard `parent==document && book`); мы создаём преамбулу и рендерим
  (предсуществующее, не регрессия; завязано на стриминговую преамбулу F-AU). Коммит/merge --no-ff/push — ПО ЗАПРОСУ.

- [x] **F-AU. Преамбула (`<div id="preamble">`) для doctype book БЕЗ следующей секции** (ветка
  `fix/book-preamble-no-section`, 2026-06-22). Запрос «начни следующую задачу из TODO.md». Master `a97f782` чист,
  открытые `[ ]` все синтетические «0 корпусного выигрыша». **Расширил триаж нового корпуса** `/mnt/c/tmp/adoc2docx/`
  (52 feature-демо, не в гейте/frontier): был 27 identical / 21 clean-div. **Системный класс (10 файлов сразу):**
  audio/keyboard/video/footnotes/text/example/sidebar/open/checklist/admonitions — все `= Title` + `:doctype: book` +
  тело БЕЗ секций; asciidoctor оборачивает тело в `<div id="preamble"><div class="sectionbody">`, мы — нет.
  **Корень (verified пробами asciidoctor 2.0.23 + `parser.rb` `next_section`):** asciidoctor СОЗДАЁТ преамбулу при
  `has_header || doctype==book`; ОБЁРТКА эмитится если у преамбулы есть контент И (`book` ИЛИ следует секция). Матрица
  (10 проб): article+title — обёртка ТОЛЬКО при следующей секции; **book — обёртка ВСЕГДА** (с/без заголовка, с/без
  секции), кроме пустого тела. Наш рендерер реализовывал ровно article-правило: `finish.rs:73-74` явно «no section
  followed — leave content as-is». **Фикс (2 файла):** (1) `events.rs` — `preamble_start` теперь ставится при
  `has_document_title || doctype_book` (обе точки: standalone и embedded `TagEnd::Header`); общий хелпер `close_preamble`
  (извлечён из section-start пути дословно: split_off + `:toc: preamble` развилка). (2) `finish.rs` — если `preamble_start`
  всё ещё `Some` (секция не встретилась — section-start путь её `take`-ает и оборачивает) И `doctype_book` → `close_preamble`
  (book оборачивает; article роняет — прежнее поведение). **Единый стейт-модель:** section-start путь покрывает «секция
  следует» для обоих doctype; finish добивает book-без-секции. Пустое тело book → preamble_content пуст → нет обёртки
  (== asciidoctor). +4 html (`test_book_preamble_without_section_html` точный байт, `_without_title_html`,
  `_with_section_unchanged_html` [один preamble, секция снаружи], `test_book_no_preamble_for_section_only_html`). clippy 0,
  **test --workspace зелёное** (html 502→506, parser 643, compat 233, прочее неизменно). **Гейт 344/344 байт-в-байт** vs
  master `a97f782` (`gate_check.py` 0 diff — 0 корпусных book-без-секции). **Frontier 250: 228 identical, new-vs-base 0 diff**
  (article-доминантный, book-правило не задевает). **adoc2docx: 27→37 identical (+10)**, у остальных diff сократился
  (links 115→89, menu 87→80, footnotes/example/sidebar/open → 0); 10 флипнутых файлов body-diff=0 байт-в-байт с asciidoctor.
  **10 проб == asciidoctor 2.0.23** (article±секция, book±заголовок±секция±тело, section-only, pre-body+section). **Вне scope
  (предсуществующее, НЕ регрессия):** book+title+пустое-тело — лишняя пустая строка между `<div id="content">` и `</div>`
  (есть и на master, normalize_html = 0 diff). Коммит/merge --no-ff/push — ПО ЗАПРОСУ.

- [x] **F-AT. Порядок CSS-классов таблицы: `stripes-*` ДО width-класса (`stretch`/`fit-content`)** (ветка
  `fix/table-class-stripes-order`, 2026-06-22). Запрос «начни следующую задачу из TODO.md». Master `2ead05d` чист,
  открытые `[ ]` все синтетические «0 корпусного выигрыша», основной frontier (asciidoctor + asciidoctor-org)
  ИСЧЕРПАН на ценных фиксах (228 identical; остаток 5 clean-div = 2 архитектурных [manpage backend 146; partintro-в-
  article на malformed-фикстуре multi-special-ex 87] + 3 non-bug diff=1 [doctime `{localtime}`, CHANGELOG эллипсис F-AP,
  migration `{asciidoctor-version}` intrinsic]). **Расширил корпус новым источником** `/mnt/c/tmp/adoc2docx/` (52
  feature-демо .adoc, не в гейте/frontier): 24 identical, 24 clean-div. Триаж diff=1 (память `compat_corpus_methodology`):
  `tables.adoc`/`tables-fix.adoc` — порядок классов: asciidoctor `stripes-odd stretch`, мы `stretch stripes-odd`.
  **Корень (verified пробами + asciidoctor `html5.rb` convert_table):** каноничный порядок —
  `tableblock frame-X grid-X stripes-X {width-class} {roles}`; `stripes-*` идёт сразу после `grid-X`, ДО width-класса
  (`stretch`/`fit-content`), роли — последними (через `write_meta_attrs`). Наш `start_table` (adoc-html/blocks.rs:149-164)
  эмитил width-класс ПЕРЕД stripes. **Фикс (1 файл):** блок `stripes-` перенесён выше блока width-класса в
  `start_table`. **Тест:** +1 html (`test_table_class_order_stripes_before_width_html` — точный порядок для
  default-width `stripes-even stretch`, autowidth `stripes-odd fit-content`, role `stripes-hover stretch myrole`; старые
  тесты проверяли лишь НАЛИЧИЕ классов, не порядок — баг проскользнул). clippy 0, **test --workspace зелёное**
  (html 501→502, прочее неизменно). **Гейт 344/344 байт-в-байт** vs master `2ead05d` (`gate_check.py` 0 diff — 0
  корпусных таблиц с stripes+width-класс одновременно → нейтрально). **Frontier 250 без регрессий** (228 identical
  стабильно). **adoc2docx: 4 файла IMPROVED** (tables/tables-fix/tables-fix-копия → байт-в-байт с asciidoctor;
  test.adoc table-теги теперь MATCH), **0 регрессий** (new-vs-base sweep: изменились только 4 table-файла, единственная
  строка — порядок класса). **5/5 + 3/3 CLI-проб == asciidoctor 2.0.23** (stripes+width%/autowidth/header/мультироли/
  no-grid+no-frame). Коммит/merge --no-ff/push — ПО ЗАПРОСУ.

- [x] **F-AS. Inter-document xref: расширение/фрагмент в shorthand `<<>>` vs formal `xref:` (#2740)** (ветка
  `fix/xref-interdoc-extension`, 2026-06-22). Запрос «начни следующую задачу из TODO.md» + выбор пользователя
  «xref inter-doc расширение» (frontier исчерпан на флипающих фиксах: 5 clean-div = 2 архитектурных [manpage 146,
  multi-special-ex book-degr 87] + 2 не-бага [doctime `{localtime}`, migration `{asciidoctor-version}` intrinsic] +
  CHANGELOG не-флипающий). Триаж CHANGELOG [4975] через showdiff: `<<target.asciidoc#,text>>` рендерился
  `href="target.asciidoc#"`, asciidoctor `target.html`. **Корень (verified исходником asciidoctor `substitutors.rb`
  стр.760-836 + `html5.rb` convert_inline_anchor):** наш движок резолвил расширение единой `interdoc_xref_href` в
  РЕНДЕРЕРЕ по эвристике `contains('.')`, не зная формы; asciidoctor применяет РАЗНЫЕ правила: shorthand `<<>>` —
  inter-doc только при `#`, любое AsciiDoc-расширение (`.adoc/.asciidoc/.asc/.ad/.txt`) → `.html`, non-AsciiDoc/без-ext
  +`#` → `+.html` (`foo.pdf#`→`foo.pdf.html`), без `#` = ВНУТРЕННИЙ id (`<<file.adoc>>`→`#file.adoc`); formal `xref:` —
  только `.adoc`→`.html`, прочие extname проходят дословно (`foo.asciidoc#sec`); auto-label = path БЕЗ fragment
  (href С fragment). **Архитектурная находка:** `{attr}` в target резолвится в рендерере (`xref:{rel}.adoc[]`), поэтому
  классификация ДОЛЖНА остаться в render-core; из парсера прокинута лишь ФОРМА. **Фикс (6 файлов):** (1) `adoc-render-core`
  — новый `pub fn resolve_xref(target, is_macro) -> XrefResolution{Interdoc{href,text}|Internal{id}}`, точная реплика
  asciidoctor (trailing-`#` chop, `ASCIIDOC_EXTENSIONS`, `has_extname`, `strip_asciidoc_ext`, entity-guard `&#`);
  УДАЛЕНЫ `is_interdoc_xref_target`/`interdoc_xref_href`. (2) `event.rs` — `Tag::CrossReference{…, is_macro: bool}` +
  to_static. (3) `inline.rs` — `try_cross_reference`→false, `try_xref_macro`→true (+11 тест-конструкций). (4) `subst/macros.rs`
  — `build_cross_reference(+is_macro)`, `try_xref`→true, `try_cross_ref`→false. (5) `adoc-html/inline.rs`
  — `start_cross_reference(+is_macro)`: Interdoc → href с fragment напрямую + auto-text=path без fragment через placeholder;
  Internal → XREFHREF lazy-lookup (без изменений). (6) `events.rs` прокидка. +1 render-core тест
  (`resolve_xref_classification`, ~20 форм) +1 parser (`cross_reference_carries_form_for_interdoc_extension_rules`) +1 html
  (`test_interdoc_xref_extension_rules_html`, 11 форм vs asciidoctor). clippy 0, **test --workspace 1256 зелёных**
  (parser 642→643, html 500→501, render-core, compat 233). **Гейт 344/344 байт-в-байт** vs master `107b979`
  (`gate_check.py` 0 diff; рискованные корпусные формы `<<filename.adoc,…>>`/`<<user-manual#back-pass,…>>` все в
  passthrough/comment-контексте → не рендерятся как xref). **Frontier 250 new-vs-base = РОВНО 1 файл** (CHANGELOG
  IMPROVED diff 2→1: [4975] `target.asciidoc#`→`target.html` MATCH asciidoctor; остаток [6137] эллипсис в URL — намеренный
  trade-off F-AP, вне scope), **0 регрессий**. **21/21 CLI-проб == asciidoctor 2.0.23** (shorthand .adoc/.asciidoc/.asc/.ad/.txt/
  .pdf/no-ext + trailing-`#` + auto-label без fragment + обратный `<<file.adoc>>`→internal + `<<#id>>`; formal `.adoc`/.asciidoc/
  .html/plain-word). **Вне scope (не в корпусе/frontier):** [6137] неэскейпленный `...`→`…​` в топ-левел URL (требует
  «replacements только в первом проходе»-флага, F-AP); docname/includes-tracking (src2src внутренний-документ → `#frag`)
  — наш парсер не отслеживает docname, всегда внешняя ветка (корректно для standalone). Коммит/merge --no-ff/push — ПО ЗАПРОСУ.

- [x] **F-AR. Незакрытый `++`/`+++`-run переразбирается как одиночный `+…+` passthrough** (ветка
  `fix/plus-run-single-plus-fallback`, 2026-06-22). Запрос «начни следующую задачу из TODO.md». Master `d053acb` чист
  (все открытые `[ ]` — синтетические «0 корпусного выигрыша»); frontier-триаж diff=1 случаев выявил РЕАЛЬНЫЙ баг (≠ синтетика):
  `mdbasics.adoc` [123-124] `(+*+,\n+++, and +-+)` — мы рендерили `(*, ++, and +-)`, asciidoctor `(*, +, and -)`. **Корень
  (`subst/passthrough.rs`):** asciidoctor извлекает passthrough в ДВА прохода — `InlinePassMacroRx` (`+++…+++`/`++…++`) ПЕРВЫМ,
  затем `InlinePassRx` (одиночный `+…+`); `++`/`+++`-run, не закрывшийся как multi-plus, НЕ passthrough → одиночный `+`
  его забирает (`+++`→`<pass:+>`, `+x++`→`<pass:x>+`). Наш `try_plus` при провале triple+double возвращал `None` (run
  оставался литералом), а `try_single_plus` close-правило имело ЛИШНИЕ ограничения `!preceded_by_plus`/`!followed_by_plus`
  (отсутствуют у asciidoctor: его close требует лишь `\S` перед и `(?!CG_WORD)` после — `rx.rb:585` `InlinePassRx[false]`).
  **Фикс:** (1) `try_plus` — триплет/дубль ветки получили `.or_else(try_single_plus)` (только вне compat; compat по-прежнему
  отдаёт single/double в quotes). (2) `try_single_plus` close-loop: сняты `!preceded_by_plus`/`!followed_by_plus`, добавлен
  пропуск настоящих `++…++`/`+++…+++` регионов (новый `multi_plus_span_len`, triple-then-double как `try_plus`) — зеркало
  двухфазности: одиночный `+` НЕ может закрыться/поглотить `+`, принадлежащий реальному multi-plus (`+x ++y++`→`+x y`).
  +1 parser (`unclosed_plus_run_reparses_as_single_plus_matches_asciidoctor` — движок==asciidoctor на `+++`/`+x++`/`+text++more`/
  `note +++ here`/mdbasics-строке + регресс-гард `+x ++y++`; движок намеренно ≠ legacy, legacy держит `+++` литералом)
  +1 html (`test_unclosed_plus_run_reparses_as_single_plus_html`). clippy 0, **test --workspace зелёное** (parser 641→642,
  html 499→500, compat 233). **Гейт 344/344 байт-в-байт** vs master `d053acb` (`gate_check.py` 0 diff). **Frontier identical
  227→228 (+1):** new-vs-base sweep по всем 250 = РОВНО 1 файл (mdbasics.adoc, IMPROVED → байт-в-байт с asciidoctor),
  **0 регрессий**. 20+ CLI-проб vs asciidoctor 2.0.23 MATCH (`+++`/`+-+`/`+x++`/`+text++more`/`note +++ here`/`a +++ b`,
  регресс-гарды `+x+`/`++bold++`/`+++raw+++`/`C++ and C++`/`a++b`/`word+plus+word`/`+x ++y++`/`+x +++y+++`). **Вне scope
  (предсуществующее, НЕ регрессия, синтетика — нет в корпусе/frontier):** одиночный `+`, охватывающий placeholder уже-
  извлечённого `++…++` (`+a ++b++ c+` → asciidoctor `a b c`, мы `a ++b++ c` — байт-в-байт с base); точное воспроизведение
  требует истинно-двухфазной архитектуры (placeholder в content одиночного `+` восстанавливается raw), наш single-pass
  пропускает регион как литерал. Коммит/merge --no-ff/push — ПО ЗАПРОСУ.

- [x] **F-AQ. Escaped-macro-префикс: схема `file://` в autolink + валидация `anchor:`-id** (ветка
  `fix/escaped-macro-prefix`, 2026-06-21, 151-я). Запрос «начни следующую задачу из TODO.md» + выбор пользователя
  «escaped-macro-префикс» (все F-* закрыты, Фаза 4/D7 сделаны, корпус исчерпан на чистых фиксах — frontier identical 227,
  clean-div 6 нишевые: manpage [146] doctype, multi-special-ex [87] book-деградация, CHANGELOG [4], 3×diff=1 не-баги).
  Триаж CHANGELOG [4] через showdiff: два независимых escaped-класса. **(1) [8085] `\file:/// → file:///`:**
  asciidoctor `InlineLinkRx` схема-группа `(?:https?|file|ftp|irc)://` включает `file` (требует `://`); `\file://…` с
  валидным URL-хвостом = escaped autolink (снимает `\`, plain), unescaped `file://…` = bare-link (`class="bare"`). Наш
  движок поддерживал `http/https/ftp/irc`, но НЕ `file`. **(2) [10276] `\anchor:<id>[<reftext>]` сохраняет `\`:**
  asciidoctor `InlineAnchorRx` требует id = `[CC_ALPHA_:][CC_WORD\-:.]*` (исходник `rx.rb:443`), `<id>` не матчится
  (первый `<` невалиден) → не макрос → `\\?` не срабатывает → backslash сохраняется. Наш `macro_escape_len`/`try_anchor_macro`
  принимали любой non-whitespace target → снимали `\` (и делали `<a id="&lt;id&gt;">` для unescaped — тоже баг). **Корень
  выведён пробами + исходником asciidoctor 2.0.23** (`rx.rb`: `InlineLinkRx` стр.524, `InlineAnchorRx` стр.443). **Фикс:**
  (A) `file://` добавлен в `scheme_at` (subst/macros.rs) + `autolink_scheme_at` (inline.rs) + legacy `b'f'`-arm
  (`ftp://`||`file://`); dispatch-байты `b'h'|b'f'|b'i'` уже покрывали `f`. (B) новый общий `scanner::is_valid_anchor_id`
  (первый символ `is_alphabetic()`/`_`/`:`, остальные `is_alphanumeric()`/`-`/`:`/`.` — Unicode-aware зеркало
  `\p{Alpha}`/`\p{Word}`), применён в 4 точках: `escape::macro_escape_len` + `inline::inline_macro_escape_len` (anchor →
  decline → keep `\`), `subst::try_anchor_macro` + `inline::try_anchor_macro` (invalid id → not anchor → literal). +1 scanner
  тест (`test_is_valid_anchor_id`) +1 parser (`escaped_macro_prefix_file_scheme_and_anchor_id` — event-векторы + pipeline==legacy
  parity на 8 формах) +1 html (`test_escaped_macro_prefix_file_scheme_and_anchor_id`). clippy 0 (`--workspace`; 3
  `--all-targets` warning'а в тест-коде ПРЕДСУЩЕСТВУЮТ на base — concat!/reftext, не мои), test --workspace зелёное
  (parser 633→635, html 489→490, compat 233). **Гейт 344/344 байт-в-байт** vs master `6b831fd` (`gate_check.py` 0 diff;
  в гейте `file://` только в `link:file:///…[…]` = link-макрос, диспатчится ДО autolink-arm; 2 `anchor:`-id (`bookmark-c`,
  `tiger-image`) оба валидны → валидация не трогает; 0 escaped-форм). **Frontier identical 227 (стабильно):** new-vs-base
  sweep по всем 250 = РОВНО 1 файл (CHANGELOG IMPROVED 4→2, обе escaped-строки [8085]/[10276] теперь MATCH asciidoctor),
  **0 регрессий**. CHANGELOG не флипает: остаток [2] = несвязанные классы ([4975] xref-ext `target.asciidoc#`→`target.html`;
  [6137] non-escaped `...`→`…​` в URL — оба вне scope). 16/16 + 9/10 CLI-проб vs asciidoctor 2.0.23 MATCH. **Вне scope:**
  (1) `\file://` БЕЗ URL-хвоста (`\file:// y`) — asciidoctor сохраняет `\` (autolink не матчится без хвоста), наш escaped-
  autolink-arm снимает по scheme+boundary без проверки хвоста — **ПРЕДСУЩЕСТВУЮЩАЯ дивергенция для ВСЕХ схем**
  (`\http://`/`\ftp://`/`\irc://` на base ведут себя так же), не регрессия, корпусные `\file:///root` имеют валидный хвост;
  (2) [4975] xref-расширение, [6137] non-escaped эллипсис в URL (требует «replacements только в первом проходе»-флага).

- [x] **F-AP. Экранированный `\...`-эллипсис (и `\--`/`\(C)`/…) в TARGET inline-ссылки → литерал без `\`** (ветка
  `fix/url-target-replacements`, 2026-06-21, 150-я). Триаж остатка `CHANGELOG.adoc` [17] через showdiff: доминирующий
  класс — 13 строк вида `…/compare/v1.5.6\...v1.5.6.1[full diff]` рендерились с СЫРЫМ `\...` в href (`v1.5.6\...v1.5.6.1`),
  asciidoctor — `v1.5.6...v1.5.6.1` (backslash снят, литеральные точки, БЕЗ эллипсиса). **Корень (ПАРСЕР,
  `subst/macros.rs`):** asciidoctor гоняет `replacements` по ВСЕЙ строке ДО `macros`-прохода (subst-порядок), поэтому TARGET
  ссылки наследует и снятие `\` (`/\\?\.\.\./` → литерал без backslash), и `...`→`…​`. Наш движок извлекает macros ПЕРВЫМ
  (до `replacements`), а `escape::run` запечатывает `\...` как `Literal("...")` ещё раньше → autolink-проход видит sentinel
  в span'е URL и ПАНИКует на legacy (`span_has_sentinel`), а legacy сохраняет сырой `\...`. **Фикс:** новый
  `reconstruct_link_target(work, span)` — пробегает span TARGET'а: plain-куски копирует ВЕРБАТИМ, контент `Literal`-leaf
  (= экранированный паттерн БЕЗ backslash) вклеивает, на любом НЕ-`Literal` sentinel'е (passthrough/attr-ref/char-ref/macro)
  возвращает `None` → punt (как было). Вызван в 3 арм'ах `try_autolink` (angle/URL[text]/bare) и в `try_link` (после
  `passthrough_url`), заменив `span_has_sentinel`-punt; в `try_autolink` URL[text] добавлен отдельный sentinel-гард на
  LABEL (зеркало `try_link`). **Plain-куски НЕ кёрлятся** (намеренно): URL из резолвнутого attr-ref (`{url-repo}/…`)
  ре-парсится ПОСЛЕ того как окружающий текст уже прошёл `replacements` раз — повторный кёрл дал бы двойную субституцию
  (`v2.0.25\...`→`v2.0.25…​`); это сломало бы 9 attr-ref `\...`-строк (v2.0.x, были identical). Цена: топ-левел литеральный
  URL с НЕэкранированным `...` не кёрлится ([6137] `v1.5.6.1...v1.5.6.2`, 1 строка — вне scope). +1 parser
  (`escaped_ellipsis_in_url_target_keeps_literal_dots` — URL[text]/bare/link: формы) +1 html
  (`test_escaped_ellipsis_in_link_target_html` — + attr-ref no-double-curl). clippy 0, test --workspace зелёное (parser
  632→633, html 488→489, compat 233). **Гейт 344/344 байт-в-байт** vs master `b5a31c0` (`gate_check.py` 0 diff; 0 гейт-файлов
  с trigger-в-URL — измерено regex'ом по корпусу → нейтральность ПО КОНСТРУКЦИИ: меняется лишь URL с `Literal`-sentinel =
  экранированный typographic, которых в гейте 0). **Frontier identical 227 (стабильно):** new-vs-base sweep по всем 250 =
  РОВНО 1 файл (CHANGELOG IMPROVED 17→4), 0 регрессий. CHANGELOG не флипает в identical — остаток [4] это несвязанные классы
  (xref `target.asciidoc#`→`.html`; `\file:///`/`\anchor:` escaped-macro-префикс в plain-тексте; [6137] неэскейпленный `...`).
  10/10 CLI-проб vs asciidoctor 2.0.23 MATCH (full-literal/bare/link:/angle/attr-ref `\...`, `\--`/`\->`/`\(C)`, нормальные
  URL с `-`/`?`/`&`). **Вне scope:** (1) неэкранированный `...`→`…​` в топ-левел URL ([6137]) — требует «replacements только
  в первом проходе»-флага (детект attr-ref-репарса хрупок, 1 строка); (2) `\file:///`/`\anchor:` escaped-macro в plain-тексте
  (отдельный класс — escape снимает `\` только перед известными macro-именами; `file:`/`anchor:` сюда не входят, разный
  путь); (3) image/mailto/xref TARGET'ы тем же образом не получают replacements (не в корпусе).

- [x] **F-AO. Font-иконка (`:icons: font`): роль на `<i>` вместо `<span>`, link через `<a class="icon">` вместо
  `<span><a class="image">`, кавычки в `role` не снимались** (ветка `fix/icon-font-role-placement`, 2026-06-21, 149-я).
  Триаж топ clean-div `CHANGELOG.adoc` [75] через showdiff: доминирующий класс — 58 из 75 строк — `:star:
  icon:star[role=red]` (`:icons: font`) рендерился как `<span class="icon"><i class="fa fa-star red">`, asciidoctor —
  `<span class="icon red"><i class="fa fa-star">`. **Корень (ЧИСТО РЕНДЕРЕР, `adoc-html/src/inline.rs::render_icon`,
  font-путь):** (1) `role` добавлялся в class-list внутреннего `<i>`, а asciidoctor `convert_inline_image` (html5.rb:1224-1230)
  кладёт его на внешний `<span class="icon …">` (`class_attr_val = type` + role); (2) link рендерился как внешний
  `<a class="icon" href>` без span, asciidoctor — внешний `<span class="icon …">` + внутренний `<a class="image" href>`
  (строка 1223, вне type-ветки); (3) `role="red big"` не разкавычивался (`&quot;red big&quot;` на `<i>`); (4) при flip+rotate
  оба эмитились, asciidoctor — flip ИЛИ rotate (flip wins, 1191-1194). Наш не-font путь (литеральный `[name]`) УЖЕ делал
  role-на-span и `<a class="image">` правильно — чинился только font-путь (асимметрия). **Фикс:** `<i>` несёт лишь
  `fa fa-NAME` + size + flip/elsif-rotate; `<span class="icon [role]">` обёртка (role через `trim_matches('"')`); link →
  внутренний `<a class="image" href [target/rel]>`. **Гейт-риск нулевой ПО КОНСТРУКЦИИ:** в гейте 4 файла с `:icons: font`
  (admonitions/icons-font/icons/callout) — НИ ОДНОГО рендерящегося inline `icon:`-макроса; `icon-macro.adoc` использует
  icon-макросы, но БЕЗ `:icons:` → литеральный путь (не тронут). +2 html (`test_icon_quoted_multi_role_html`,
  `test_icon_link_role_window_html`) + правка 3 существующих (role/link/combined кодировали баг). clippy 0, test --workspace
  зелёное (html 486→488, parser 632, compat 233). **Гейт 344/344 байт-в-байт** vs master `4a35fc0` (`gate_check.py` 0 diff).
  **Frontier identical 227 (стабильно):** new-vs-base sweep по всем 250 = РОВНО 4 файла, ВСЕ icon-related, КАЖДАЯ изменённая
  строка теперь байт-в-байт с asciidoctor — CHANGELOG 75→17, syntax 448→444 (IMPROVED), asciidoctor-0-1-4 (позиционный
  differ 4752→5596 = АРТЕФАКТ переалайнинга, все 6 icon-строк MATCH asciidoctor), asciidoctor-1-5-0 (1 icon-строка
  фикс, MATCH), **0 контентных регрессий**. 9 CLI-проб vs asciidoctor 2.0.23 MATCH (role/size/link/window/flip-vs-rotate/
  multi-role-quoted/link+role). **Вне scope (остаток CHANGELOG diff=17):** escaped `\...`-эллипсис в URL (backslash не
  снимается), `\file:///` (escaped macro-префикс), `target.asciidoc#` xref-резолюция, non-escaped `...`→`…` в URL-target;
  `:icons:` без значения = image-режим (`<img src>`), наш код трактует любой `icons` как font (предсуществующий, не в корпусе).

- [x] **F-AN. Leaf-блок (`image::`/`video::`/`audio::`, thematic/page break) после list-item НЕ отсоединялся** (ветка
  `fix/dlist-block-detach`, 2026-06-21, 148-я). Триаж debuter [16] через showdiff: `image::…[…,align=center]` после
  dlist-описания (через пустую строку, без `+`) рендерился ВНУТРИ `<dd>`, asciidoctor — сиблингом ПОСЛЕ закрытия
  `</dl>`. Минимальные пробы расширили класс: `image::`/`video::`/`audio::` И thematic break `'''` все ошибочно
  цеплялись к предыдущему dd (и склеивались прямо в строку `<p>`), тогда как делимитед-блоки (`----`/`====`),
  admonition (`NOTE:`), table (`|===`), markdown-fence и обычный параграф — корректно отсоединялись. **Корень:**
  обработчики этих leaf-блоков в `block.rs::scan_block_macros` (image/video/audio) и `scan_leaf_blocks` (thematic/page
  break) НЕ имели гарда закрытия списка, в отличие от admonition/table/comment (`is_directly_in_list_context() &&
  !in_continuation && had_blank_line → close_list_contexts()`). **Поведение asciidoctor выведено пробами:** leaf-блок
  после ПУСТОЙ строки (без `+`) завершает список (сиблинг); БЕЗ пустой строки (`term:: desc` ⏎ `image::…`) или после
  `+`-continuation — прикрепляется к dd. **Фикс:** новый хелпер `BlockScanner::close_list_if_blank_separated()` (зеркало
  inline-гарда admonition/table/comment), вызван в 5 арм'ах: thematic break, page break, block image, block video,
  block audio (до `advance()`). Гард срабатывает и на обычных `ListItem` (ordered/unordered) — проба `* a\n\nimage::`
  тоже MATCH. **Гейт-риск нулевой ПО КОНСТРУКЦИИ:** гард меняет вывод лишь там, где `(список)+(пустая строка)+(leaf-блок)`,
  т.е. где мы УЖЕ расходились с asciidoctor; будь такой случай в гейте — он бы давал diff, но гейт 344/344 чист
  (`gate_check.py` new-vs-base 0 diff подтверждает). +2 parser (`test_block_macro_after_blank_detaches_from_dlist` —
  detach+attach; `test_thematic_break_after_blank_detaches_from_list`) +1 html (`test_block_image_after_dlist_detaches_html`
  — detach+attach байт-в-байт). clippy 0, test --workspace зелёное (parser 630→632, html 485→486). **Гейт 344/344
  байт-в-байт** (new==base на всём гейте). **Frontier identical 226→227 (+1):** debuter флипнул 16→0; new-vs-base sweep
  по всем 250 = РОВНО 2 файла, ОБА IMPROVED (debuter 16→0; asciidoctor-1-5-0 10→3 — остаток = несвязанный класс
  section-id с инлайн-разметкой в заголовке `<h2 id>`), **0 регрессий**. 6 CLI-проб vs asciidoctor 2.0.23 MATCH (detach
  image/video/audio/hr, no-blank attach, `+`-continuation attach, ordered/unordered списки). **Вне scope:** (1) `toc::[]`-
  макрос после списка тем же путём не отсоединяется (тот же гард тривиально добавить, но не в корпусе/редко); (2) остаток
  debuter был ТОЛЬКО этот класс — файл полностью байт-в-байт.

- [x] **F-AM. Двухстрочный (setext / подчёркнутый) заголовок документа и секций + авто-`compat-mode`** (ветка
  `feat/setext-section-titles`, 2026-06-21, 147-я). Триаж топ clean-div `sample.adoc` [152] через showdiff: структура
  полностью смещена с верха — `Document Title` подчёркнутый `===` не распознавался как doctitle (рендерился `<p>`),
  каскад на весь документ. **Setext-форма (легаси AsciiDoc, default-ON в asciidoctor `Compliance.underline_style_section_titles`):**
  строка-подчёркивание однородна, первый символ ∈ `= - ~ ^ +` → уровень (asciidoctor 0-4 = parser 1-5: `=`→1 doctitle,
  `-`→2 … `+`→5); строка-заголовок не с `.`, содержит ≥1 alnum (Unicode-aware, `SetextSectionTitleRx`); `|len₁−len₂|<2`
  по СИМВОЛАМ (rstripped). **setext-doctitle авто-включает `compat-mode`** для всего документа (parser.rb:161 «default to
  compat-mode if document has setext doctitle») → `'subsection'`→`<em>`. **Реализация:** (1) scanner `strip_setext_title(l1,l2)`
  (зеркало `setext_section_title?`); (2) block `scan_header_constructs` — детекция setext-doctitle (level 1), эмит
  `Attribute{compat-mode}` в header_events (до body, до inline-субституции) + `doc_attrs[compat-mode]`; (3) `scan_leaf_blocks` —
  детекция setext-секций; (4) `setext: bool` проброшен в `scan_document_header`/`scan_section`/`scan_discrete_heading`
  (extra `advance()` для строки-подчёркивания). **Порядок проверок зеркалит asciidoctor:** section-title (atx+setext) ловится
  в `next_section` ДО списков/делимитеров (`is_next_line_section?` перед `next_block`) — совпало с текущим размещением atx в
  scan_leaf_blocks. **Три гарда (выведены гейт/тест-регрессиями):** (a) setext НЕ внутри delimited-блока (`!is_inside_delimited_block() ||`
  pending `[discrete]`/float) — asciidoctor обычные секции в блоках не парсит (`next_block` ловит лишь float/discrete),
  иначе `Outer\n====` во вложенном example → `<h1>`; (b) `line_closes_open_delimited_block(next)` — закрывающий делимитер
  блока не подчёркивание (`2.3` в `====`…`====` остаётся `<p>`, иначе → `<h1>`); (c) `is_bracketed_attr_line(line)` —
  `[…]`-образная строка (block-attr/anchor, в т.ч. non-ASCII `[注意]`, что ASCII-only `is_block_attribute` не ловит) =
  метаданные, не setext (иначе `[TIP]\n====` → setext-doctitle «[TIP]», контент исчезал). +5 тестов (scanner
  `strip_setext_title`; block doctitle/section/terminator-guard/bracketed-guard) +3 html (doctitle+compat/section/terminator).
  clippy 0, test --workspace зелёное (parser 625→630, html 485, html_output +3). **Гейт 344/344 байт-в-байт** vs master
  `549ed6c` (0 setext-doctitle в гейте → авто-compat-mode не затрагивает; 26 setext-кандидатов — все внутри listing/example,
  гарды (a)/(b) их отсекают). **Frontier identical 225→226 (+1):** sample.adoc флипнул, new-vs-base sweep по всем 250 = РОВНО
  1 файл изменён (sample.adoc IMPROVED 152→0), **0 регрессий** (промежуточный регресс README-zh_CN `[注意]` пойман sweep'ом и
  закрыт гардом (c)). 13+ CLI-проб vs asciidoctor 2.0.23 MATCH (уровни =-~^+, author/revision, discrete, inline-разметка,
  длина-tolerance, atx-doctitle-без-compat, все три гарда). **Вне scope:** Unicode-block-attr `[注意]`→admonition-стиль
  (asciidoctor парсит как attr-list через Unicode CG_WORD, наш `is_block_attribute` ASCII-only → paragraph; pre-existing,
  гард (c) сохраняет base-поведение); atx-секция/setext внутри example → asciidoctor рендерит literal `<p>`, наш парсер →
  discrete heading (pre-existing, не в гейте, не тронуто).

- [x] **F-AL. Текст сноски `footnote:[…]` получает inline-замены + многострочная склейка** (ветка
  `fix/footnote-inline-subs`, смержена `383b521`, 2026-06-21, 146-я). Триаж четырёх diff=1 frontier-файлов через
  showdiff: github-0.1.4 [747] `I'm`→курлится только в обычном параграфе, в сноске НЕТ. Корень: текст сноски эмитился
  СЫРЫМ (`finish.rs::render_footnotes` делал лишь `html_escape_text`) — ни апостроф (`I'm`/`it's`), ни `*bold*`/`_em_`/
  `` `mono` `` не применялись, хотя asciidoctor субституирует текст footnote-макроса в рамках inline-прохода (NORMAL).
  **Чисто рендерер** (3 файла + render-core doc): (1) новый `HtmlRenderer::render_footnote_text` (lib.rs) — прогон
  через InlineParser с `current_subs()` + `push_event`; fast-path без разметки использует `html_escape_text`
  (text-content, `"` остаётся литералом — НЕ `&quot;`, в отличие от `render_inline_value`/attr-value), что сохраняет
  плоские сноски байт-в-байт; (2) `events.rs::Event::Footnote` рендерит текст в локальный String в define-time
  (как asciidoctor — субституция в точке макроса) и хранит ГОТОВЫЙ HTML; (3) `finish.rs::render_footnotes` эмитит
  `note.text` verbatim (`push_str` вместо `html_escape_text`). **Под-класс (тот же путь):** многострочная сноска
  склеивается в одну строку — rstrip каждой строки + join одним пробелом (newline→разделитель, ведущий отступ
  продолжения сохранён); правило выведено пробами asciidoctor (`a\nb`→`a b`, `a \nb`→`a b`, `a\n b`→`a  b`,
  `a  \n  b`→`a   b`); scoped к сноскам (обычный параграф сохраняет `\n` — гейт байт-в-байт это доказывает). Парсер
  не тронут. +4 html-теста (subs/monospace+specialchars/multiline/plain-unchanged; html 481→485), parser 625, compat
  233; clippy 0. **Гейт 344/344 байт-в-байт** vs master `83caf44` (единственный гейт-файл с разметочной сноской —
  `macros/examples/footnote.adoc` — ЗАТЕНЯЕТ её: id `disclaimer` уже определён ранее как `Opinions are my own.` plain,
  pass:c,q-вариант становится ссылкой, текст игнорируется → output не меняется; gate_check.py 0 diff). **Frontier
  identical 222→225 (+3):** github-0.1.4 (1→0, апостроф), asciidoc-returns-to-github (12→0, footnote-разметка),
  asciidoclet-1.5.0 (12→0, разметка + многострочная склейка) флипнули в identical; new-vs-base sweep по всем 250 =
  РОВНО 4 файла, ВСЕ IMPROVED (writers-guide 3735→3733, остаётся divergent по прочим классам), **0 регрессий, 0
  NUL-sentinel-утечек** (xref/`<<` в сносках корпуса отсутствуют — проверено grep'ом; sentinel-резолв идёт ПЕРЕД
  render_footnotes в обоих путях, но define-time рендер без xref не плодит sentinel). 4+ CLI-пробы vs asciidoctor 2.0.23
  семантически MATCH (`&#8217;` vs литеральный U+2019 — нормализуемое NCR-различие). **Вне scope (остаток среди diff=1
  frontier):** (1) `mdbasics` [169] жадный passthrough `+++, and +-+` (после soft-break парсится как ОДИН `+…+` вместо
  двух минимальных — non-greedy + `(?!\w)` в asciidoctor); (2) `migration` [678] intrinsic `{asciidoctor-version}` не
  резолвится (окружение-специфичный, = `2.0.23` у asciidoctor); (3) `doctime-localtime` [6] разница clock/TZ
  (недетерминированно, не баг).

- [x] **F-AK. Unicode word-char в apostrophe-replacement** (ветка `fix/apostrophe-unicode-word-char`, 2026-06-21, 145-я).
  Триаж топ clean-div через showdiff: `d'éditer`/`l'éditeur`/`d'écrire` НЕ курлились (`d'autres`, `it's` — ASCII-фланг —
  курлились норм). Корень: apostrophe-replacement (`adoc-parser/src/inline.rs::apply_typographic_replacements`, единственная
  реализация — модный движок `subst/replacements.rs` её переиспользует) гейтил оба фланга байтом `is_ascii_alphanumeric()`,
  а asciidoctor REPLACEMENTS `(\p{Alnum})\'(?=\p{Alpha})` Unicode-aware И асимметричен: левый `\p{Alnum}`, правый
  (lookahead) `\p{Alpha}` (БЕЗ цифр). Многобайтная буква `é`/Cyrillic слева/справа → байт `i±1` не ASCII alnum → не курлили.
  **Фикс (1 арм):** декодируем флангующие *символы* (`text[..i].chars().next_back()` / `text[i+1..].chars().next()`),
  левый `char::is_alphanumeric`, правый `char::is_alphabetic` (stdlib-зеркало `\p{Alnum}`/`\p{Alpha}`; правый сужен —
  `5'6` теперь литерал как у asciidoctor, `1'a` курлит). **Гейт-риск нулевой ПО КОНСТРУКЦИИ:** в гейте 0 не-ASCII-фланк
  апострофов и 0 `<alnum>'<digit>` (проверено grep'ом) → расширение влево добавляет матчи только там, где asciidoctor УЖЕ
  курлит (иначе был бы текущий гейт-фейл), сужение вправо снимает матчи только на digit-right (которых в гейте нет).
  +2 parser (`test_apostrophe_unicode_word_char` — право/лево/обе стороны не-ASCII; `test_apostrophe_digit_on_right_stays_literal`)
  +1 html (`test_apostrophe_unicode_word_char_html`); clippy 0, test --workspace зелёное (parser 623→625, html 480→481).
  **Гейт 344/344 байт-в-байт** vs master `917e86e` (base через worktree, `gate_check.py` 0 diff). **Frontier identical 222
  (стабильно), clean-div 11:** new-vs-base по всем 250 = РОВНО 2 файла, оба IMPROVED (README-fr 499→498 — снят body
  `d'écrire`; debuter 18→16 — сняты `d'éditer`+`l'éditeur`), **0 регрессий**, 0 флипов (оба остаются с residual
  структурными/alt-text диффами). 10 CLI-проб vs asciidoctor 2.0.23 семантически MATCH (Unicode/Cyrillic курлят,
  `5'6`/`'twas`/`cats'` — гарды). **Вне scope (отдельные классы, обнаружены при триаже):** (1) replacements НЕ применяются
  к image `alt`-тексту (`d'une` в alt README-fr [162] остаётся прямым — наш рендерер не курлит alt); (2) structural
  divergence README-fr [1271] (`[discrete]`/CHANGELOG-секция → paragraph) и debuter imageblock-ordering; (3) escaped
  Unicode-апостроф `d\'éditer` (escape.rs гейт всё ещё ASCII-only — pre-existing, не во frontier).

- [x] **F-AJ. `:toc: preamble` размещает TOC ВНУТРИ preamble-div** (ветка `fix/toc-preamble-placement`, смержена
  `9433b92`, 2026-06-21, 144-я). Триаж debuter (118) + github-0.1.4 (50, тоже `:toc: preamble`) через showdiff: оба
  каскадили от ОДНОГО сдвига — TOC ставился КАК СОСЕД после полностью закрытого preamble (`</div></div>` + toc), а
  asciidoctor (`convert_preamble`, html5.rb) ставит его МЕЖДУ закрытием sectionbody и закрытием preamble-div:
  `<div id="preamble"><div class="sectionbody">…</div>{toc}</div>`. **Чисто рендерер** (`adoc-html/events.rs`, 2 точки):
  (1) section-start wrap — `</div>\n</div>\n` расщеплён, `toc_insert_position` пишется МЕЖДУ закрытием sectionbody и
  preamble (хелпер `want_preamble_toc`, empty-preamble ветка сохранена); (2) embedded header-путь (`header_suppress_start`) —
  `preamble` отложен на wrap-путь как и `macro` (был исключён только `macro`, асимметрия со standalone-гардом стр. 913) →
  embedded `:toc: preamble` (через `to_html`) тоже корректен. Парсер не тронут. Усилен `test_toc_preamble` (баланс div:
  preamble ещё открыт на момент TOC). clippy 0, test --workspace зелёное (html 479→480, parser 623). **Гейт 344/344
  байт-в-байт** vs master (standalone CLI-путь для корпуса не изменён; `:toc: preamble` в гейте нет → 0 риска). **Frontier
  identical 222 (стабильно), clean-div 11:** debuter **118→18**, github-0.1.4 **50→1** (оба IMPROVED, не флипнули в
  identical из-за ОСТАТОЧНОГО несвязанного класса — Unicode word-char в apostrophe-replacement: `d'éditer`/`I'm`,
  asciidoctor курлит через `\p{Word}`, мы гейтим по `is_ascii_alphanumeric`). new-vs-base по всем 250 = РОВНО 2 файла,
  оба IMPROVED, **0 регрессий**. CLI-пробы vs asciidoctor 2.0.23: preamble basic/nested/custom-title/no-preamble (TOC
  не рендерится без preamble — MATCH) + регресс-гарды auto/macro/left — все MATCH.
  - **Вне scope (pre-existing, не тронуто фиксом):** (1) атрибут `:toc-placement: preamble` (отдельно от значения `:toc:`)
    не читается как директива размещения → TOC не рендерится (base==new, проба `placement_attr` 21→21); (2) Unicode
    word-char apostrophe-replacement (`d'éditer`→`d’éditer`) — остаток debuter (18) и github-0.1.4 (1); **починка этого
    класса флипнет github-0.1.4 в identical** (сильный одно-классовый кандидат); (3) пустая trailing-секция: whitespace
    в `<div class="sectionbody">` (предсуществующее, не TOC-related).

- [x] **F-AI. Indented continuation-строка в `a|`/`l|`-ячейке = literal paragraph** (ветка
  `fix/asciidoc-cell-indented-literal`, смержена `c62a348`, 2026-06-21). Триаж index.adoc (136 diff, каскад с [129]):
  внутри AsciiDoc-ячейки строка с ведущим пробелом ` $ asciidoctor document.adoc` должна стать indented literal
  (`<div class="literalblock"><pre>…</pre>`), но наш `cell_text` (block.rs) делал `s.trim()`, срезая ведущий пробел
  первой контентной строки → `<p>` (paragraph). **Корень:** зеркало asciidoctor `Table::Cell` (table.rb:266-278) —
  rstrip + снятие только ведущих `\n` (БЕЗ lstrip); отступ первой контентной строки значим. Remainder на строке
  сепаратора уже lstripped scanner'ом (scanner.rs:1062), поэтому ведущий пробел в `cell.content` приходит лишь с
  continuation-строк. Фикс (`block.rs::cell_text`): `s.trim()` → `s.trim_end().trim_start_matches('\n')` для
  AsciiDoc/Literal. `a|\n $ cmd`→literalblock, `a| $ cmd`→paragraph (same-line, без изм.). +1 parser
  (`asciidoc_cell_preserves_leading_indentation`) +1 html (`test_asciidoc_cell_indented_literal_html`); clippy 0,
  test --workspace зелёное (parser 622→623, html 479→480). **Гейт 344/344 байт-в-байт** vs master. **Frontier
  identical 221→222** (`index.adoc` 136→0, весь каскад = один класс), clean-div 12→11, new-vs-base = ровно 1 файл
  IMPROVED, 0 регрессий. 7 CLI-проб vs asciidoctor 2.0.23 MATCH. **Вне scope (pre-existing, не во frontier):**
  literal `l| $ cmd` (same-line) — asciidoctor сохраняет ведущий пробел в `<pre>`, наш scanner.rs:1062 его режет.

- [x] **F-U. smart-quote утечка сквозь моноширинные `` `…` ``-спаны** (ветка `fix/smart-quote-monospace-boundary`,
  2026-06-19). Кручёная single/double smart-quote замена «протекала» сквозь границы `` `…` ``-спанов:
  `` `'a'` and `'b'` `` → наш БАГ `<code>'a‘ and ’b'</code>` (схлоп + кручёные апострофы) вместо asciidoctor
  `<code>'a'</code> and <code>'b'</code>`. Корень (`subst/quotes.rs`): `pass_smart_quotes` детектил `quote`+`` ` ``
  открыватель БЕЗ проверки левой границы → ложный `` '` `` после word-char. Asciidoctor modern QUOTE_SUBS
  `:single`/`:double` КОНСТРЕЙНЕД (`(^|[^\w;:}])OPEN(\S|\S.*?\S)CLOSE(?!\w)`). Фикс: делегировал modern smart-quotes
  в УЖЕ существующий констрейнед `pass_compat_curved` (open=`quote`+`` ` ``, close=`` ` ``+`quote`); удалил dead
  `pass_smart_quotes`/`find_smart_quote_close`. Строго субтрактивный (только отклоняет лишние матчи) → 0 риска гейта.
  legacy/compat не тронуты. +1 parser event-vector тест + html-фикстура; clippy 0, test --workspace зелёное
  (parser 604→605). **Гейт 344/344 байт-в-байт** vs master `5678867`. **Frontier identical 213→214 (+1)**,
  api/index.adoc (топ-2, 347) ФЛИПНУЛ в identical; new-vs-base difflib: 1 файл IMPROVED 11→0, 0 регрессий.
  10/10 CLI-проб MATCH asciidoctor. **Вне scope:** `&#8220;`-сущность vs литеральный `“` (схлопывается при норме).
- [x] **F-A. Текст ссылки/autolink режется по первой запятой** (ветка `fix/link-text-comma`, 2026-06-17).
  `parse_link_attrs` получил параметр `LinkKind {Link, Mailto}`: для `Link` bracket парсится как attribute-list
  (сплит по запятой) ТОЛЬКО при наличии named-атрибута (`key=value`, валидный ключ) или ведущей кавычки `"`;
  иначе весь bracket-content = текст (запятые сохранены) — правило asciidoctor 1.5.7+. `Mailto` всегда позиционный
  (subject/body без `=`). Попутно: strip кавычек у `text` в attr-list-режиме (`["A, B",role=x]` → text «A, B»).
  Фикс в общей функции `attributes.rs` покрывает оба движка (inline.rs + subst/macros.rs); 7 call-site обновлено.
  Гейт 344/344 байт-в-байт vs master (0 регрессий), frontier: 4 файла улучшено (`news/_index.adoc` 1→0 identical,
  `what-is-asciidoc.adoc` 3→2, README-de/fr -1), 0 регрессий. +4 юнит-теста, clippy 0, test --workspace зелёное.
- [x] **F-B. `:hide-uri-scheme:`** (ветка `feat/hide-uri-scheme`, 2026-06-17). Новый `adoc_render_core::strip_uri_scheme`
  по правилу asciidoctor `UriSniffRx` (`\A\p{Alpha}[\p{Alnum}.+-]+:/{0,2}`; ручной char-скан, zero-dep). Применяется в
  рендерере (`events.rs`, ветка `bare_link_pending`) ТОЛЬКО к видимому тексту bare-autolink/`link:url[]` при наличии
  document-attr `hide-uri-scheme`; href неизменен; fallback к полному тексту при пустом срезе (голая схема `https://`).
  mailto безопасен (эмитит `is_bare:false` → флаг не ставится, совпадает с asciidoctor). Гейт 344/344 байт-в-байт,
  frontier 191→192 identical (`gradle-plugin` news → identical), 0 регрессий (diff new-vs-master: только текст
  bare-ссылок, href цел). +3 юнит-теста core, +1 html-тест (5 кейсов), clippy 0, test --workspace зелёное.
- [x] **F-G. `:source-language:`** (ветка `feat/source-language-default`, 2026-06-18). Document-attr `source-language`
  теперь даёт язык по умолчанию. Парсер (`block.rs`): хелпер `default_source_language()` (presence-based,
  `doc_attrs["source-language"]`); (1) fallback для `[source]` без явного языка — delimited (2848) и paragraph (2452);
  (2) промоция голого `----` listing → source при `delim==Listing && block_style_kind().is_none() && source-language`
  (явный `[listing]`/`[literal]`, `....` literal — НЕ промотятся; `[source,lang]` явный побеждает; `:source-language!:`
  снимает; пустой атрибут промотит с пустым языком — всё MATCH asciidoctor 2.0.23). Рендерер не тронут. +5 parser
  +1 html тестов; clippy 0, test --workspace зелёное (parser 562→567). Гейт **344/344** байт-в-байт (watch-point
  `verbatim/examples/source.adoc` устоял — мангленный tag-soup, оба движка идентичны). Frontier 192→**193 identical**,
  clean-div 40→39: 8 файлов улучшено (READMEs −20 каждый — bare-listing промоция; CONTRIBUTING −6, writers-guide −8,
  asciidoclet-news −14 — `[source]`-дефолт), **0 регрессий** (new-vs-master: изменились ТОЛЬКО source-language файлы).
- [x] **F-J (НОВЫЙ, обнаружен при F-G). fenced ` ``` ` → source** (ветка `feat/fenced-source`, 2026-06-18).
  `block.rs::scan_markdown_code_fence`: ветки `Some(lang)`/`else` схлопнуты в единую эмиссию `SourceBlock`
  (язык = info-string, иначе `default_source_language()`); спец-кейс пустого тела (`Text("\n")`) удалён — ветка
  `Some` уже доказала, что пустое/`[""]` тело → `<code></code>` (MATCH asciidoctor). Блочный стиль `[source,lang]`/
  `[listing]`/`[literal]` для языка ИГНОРИТСЯ (fence сбрасывает стиль в source; язык только из info/source-language),
  `.Title` сохранён — всё MATCH asciidoctor 2.0.23 (12 проб). Рендерер не тронут (`start_source_block` при
  `language:None` уже даёт `<pre class="highlight"><code>`). +5 parser +1 html тестов (и обновлён старый
  `test_markdown_code_fence_without_language` под source); clippy 0, test --workspace зелёное (parser 567→572,
  html 438→439). Гейт **344/344** байт-в-байт (живые fence в гейте отсутствуют — голые ` ``` ` лежат ВНУТРИ
  `[source,markdown]\n----…----` listing). Frontier identical 193 (без изм. — оба улучшенных файла имеют и другие
  расхождения); diff new-vs-master: **2 файла улучшено** (`asciidoctor-0-1-4-released`, `asciidoc-writers-guide`:
  `<pre>` → `<pre class="highlight"><code>`), **0 регрессий**.
  - [x] **F-J'. `:compat-mode:` → `:language:` алиасит `:source-language:`** (ветка `feat/compat-mode-language`,
    смержена `a910a53`, 2026-06-18). Зеркало asciidoctor `Document#save_attributes` (`document.rb:1216-1217`):
    после парсинга header, при наличии ОБОИХ `compat-mode` И `language` → `source-language := language` (перезапись).
    Header-only (mid-doc не действует), порядок не важен (language всегда побеждает), presence-based (пустой language
    алиасит), `:compat-mode!:` снимает. **Чисто парсер** (`block.rs`): новый хелпер `apply_compat_mode_source_language`
    рядом с `default_source_language`, вызов РОВНО ОДИН раз перед `body_started = true` (переход header→body).
    `default_source_language` и рендерер не тронуты — F-G промоция bare `----`, F-J bare fence, F-K hljs подхватывают
    `source-language` автоматически. +8 parser +1 html теста; clippy 0, test --workspace зелёное (parser 582→590,
    html 451→452). **Гейт 344/344 байт-в-байт** vs master (`:language:` в гейте нет → 0 риска). Frontier identical 202
    (0 регрессий); единственный new-vs-base изменившийся файл `asciidoctor-0-1-4-released.adoc` IMPROVED:
    `language-asciidoc` 0→47 (= эталон), line-diff vs asciidoctor 996→906. 9 CLI-проб vs asciidoctor 2.0.23 MATCH.
    План (проверен независимым агентом): `~/.claude/plans/compat-mode-language-alias-fjp.md`.
    - **Follow-up (вне scope):** setext doctitle авто-включает compat-mode (`parser.rb:160-161`, setext не поддерживаем);
      прочие эффекты compat-mode (single-quote emphasis, `++` passthrough, footnoteref, natural xrefs); `source-language`
      не наследуется вложенными AsciiDoc table-cells (pre-existing, есть и без compat-mode).
  - [x] **F-K. Порядок классов highlightjs (+ `language-none`)** (ветка `fix/hljs-class-order`, 2026-06-18).
    `start_source_block` (`adoc-html/src/blocks.rs`): при активном `:source-highlighter: highlight.js` класс на `<code>`
    теперь эмитится в порядке asciidoctor `class="language-X hljs"` (был `"hljs language-X"`). Заодно покрыт `[source]`
    **без языка** под highlight.js → `class="language-none hljs"` (без `data-lang`) — раньше класса не было вовсе.
    Прочие highlighter'ы (`rouge`/`pygments`/`coderay`) и `highlighter.is_none()` не тронуты. +0 файлов парсера (чисто
    рендерер); тесты: обновлены 3 ассерта (`test_source_block_highlightjs`, `test_markdown_code_fence_with_highlighter`,
    `test_source_block_no_language` — последний ассертил старый баг). clippy 0, test --workspace зелёное (html 442).
    **Гейт 344/344 байт-в-байт** vs master (highlight.js в гейте нет → нулевой риск). Frontier: Identical 198 и clean_div 34
    без изм. (hljs-файлы имеют прочие расхождения), `asciidoc-writers-guide` diff 5373→5366 IMPROVED, **0 регрессий**
    (new-vs-master на hljs-файлах: writers-guide IMPROVED, остальные same). Пробы vs asciidoctor 2.0.23: оба случая MATCH.
- [x] **F-D. `toc::[attrs]`** (ветка `feat/toc-macro-attrs`, 2026-06-18). `scanner::is_toc_macro` расширен с точного
  `== "toc::[]"` на `toc::[…]` (mirror `BlockTocMacroRx`); новый `scanner::toc_macro_attrs` отдаёт содержимое скобок.
  Парсер (`block.rs`) парсит их через `BlockAttributes::parse(...).named["levels"]` → `Event::TocMacro { levels: Option<u8> }`
  (unit-вариант стал struct-вариантом; позиционный `toc::[2]` игнорится — asciidoctor читает только named `levels`).
  Рендерер: поле `toc_macro_levels`, `generate_toc(levels: u8)` — макро-TOC honor'ит per-macro override, auto-TOC берёт
  `:toclevels:`. Вне `:toc: macro` макрос инертен (`<!-- toc disabled -->`, levels игнор). +4 parser +2 html теста,
  clippy 0, test --workspace зелёное (parser 572→574, html 439→441). Гейт **344/344 байт-в-байт** vs master (0 регрессий).
  Frontier: `migration.adoc` каскад **861→571** diff (корень починен); identical 193 без изм. Единственный flag
  (`asciidoctor-0-1-4-released` 5423→5474) — позиционный артефакт differ'а: new эмитит ТОЧНЫЙ маркер asciidoctor
  `<!-- toc disabled -->` (master давал неверный `<p>toc::[levels=1]</p>`), 4→1 строк каскадит на ~4900 строках файла,
  расходящегося из-за необработанного `ifdef::env-site`. Core-проба `toc::[levels=1]` под `:toc: macro` — IDENTICAL asciidoctor.
  **Вне scope (follow-up, нет в корпусе):** `.Title`→toctitle, `[.role]`→class, `[#id]`→id на toc-макросе.
- [x] **F-C. Author / inline `<url>`** (ветка `fix/angle-url`, 2026-06-18). Две независимые подзадачи, обе по правилам
  asciidoctor 2.0.23 (верифицировано пробами). **(1) Строка автора** (`adoc-html`): адрес из `<…>` теперь рендерится
  через `sub_macros`, а не хардкодом `mailto:` — новый `render_inline_value_with_subs(output,value,subs)` (`lib.rs`),
  вызов в `finish.rs` с `SPECIALCHARS|MACROS` (`render_author_details` стал `&mut self`). URL→bare-link
  (`class="bare"`, без `mailto:`), email→`mailto:` (байт-идентично прежнему), `ftp/irc`→bare, `<*bold*>`→литерал
  (quotes НЕ запускается). **(2) Inline `<url>`** (`subst/macros.rs::try_autolink`+вызов, зеркало в legacy
  `inline.rs::try_autolink`): bare-автолинк слева от `<` — при закрытом `>` снимаются ОБА `<`/`>` (трейлинг-пунктуация
  СОХРАНЯЕТСЯ, `>` — жёсткая граница), при незакрытом `<url` — declines (литерал). `<url[text]>` и `<email>` — скобки
  остаются (уже совпадали, не тронуты; описание в старом TODO про `<https://x[@t]>` было неточным). Сигнатура
  `try_autolink` → `Option<(events,end,strip_angle)>`. +2 parser-теста (`angle_bracket_url_matches_asciidoctor` точные
  Event-векторы + 8 кейсов в `reproduces_legacy_on_link_inputs`), +1 html (`test_angle_bracket_url_autolink`),
  +1 author (`author_with_url`); clippy 0, test --workspace зелёное (parser 574→575, html 441→442, author 6→7).
  **Гейт 344/344 байт-в-байт** vs master (0 регрессий). **Frontier identical 193→198 (+5)**, clean_div 39→34;
  diff new-vs-master = 21 файла, ВСЕ IMPROVED, 0 регрессий (5 стали identical: what-is-asciidoc, funding-campaign,
  js-1-5-0, maven-plugin-1-5-0, wow-asciidoc; READMEs −4 каждый — author-URL; release-notes/news −1..−4).
  **Follow-up'ы (вне scope, не в корпус-импакте):** F-C' — `<mailto:x@y.com>` (asciidoctor: plain-text) и charset
  email-локали (`a&b@…`: asciidoctor включает `&` в local part, мы — нет); `<<url>>` ложно матчится как xref.
- [x] **F-F. `menu:X[]`** (ветка `fix/menu-menuref`, 2026-06-18). Одиночная menu-ссылка с пустыми скобками
  (`menu:File[]`, нет `menuitem`/`submenus`) теперь рендерится как asciidoctor `<b class="menuref">File</b>`
  (была `<span class="menu">File</span>`). **Чисто рендерер** (`adoc-html/src/inline.rs::render_menu`, ветка
  `items_str.is_empty()`) — парсер не тронут (Event `Start(Menu{target})`+опц.`Text`+`End` уже корректен; различие
  single/multi решается в рендерере). Мульти-ветка (`menuseq`/`submenu`/`menuitem`) не изменена. Спека верифицирована
  исходником asciidoctor (`convert_inline_menu`: нет menuitem + пустые submenus → `%(<b class="menuref">…)`).
  Тесты: обновлён 1 ассерт (`test_menu_no_items_html` → `<b class="menuref">File</b>`); `test_menu_html`/
  `test_menu_submenus_html` без изм. clippy 0, test --workspace зелёное (html 442, parser 575 — без изм.).
  **Гейт 344/344 байт-в-байт** vs master (0 регрессий). Пробы vs asciidoctor 2.0.23: все 3 кейса (empty/menuitem/
  submenu) MATCH байт-в-байт. **Frontier identical 198→199 (+1)**, clean_div 34→33; new-vs-base = 1 файл IMPROVED
  (`tooling/index.adoc` 175→173, `menu:Packages[]` → `<b class="menuref">`), **0 регрессий**.
  - **Follow-up (вне scope, нет в корпус-импакте):** asciidoctor использует `,` как fallback-разделитель подменю при
    отсутствии `>` (`menu:File[New, Save]` → submenus=[New], menuitem=Save) — мы сплитим только по `>`. Каретка при
    `:icons: font` (`<i class="fa fa-angle-right caret">`) — мы всегда эмитим текстовую.
- [x] **F-E. `[%nowrap]` / `:prewrap!:`** (ветка `fix/nowrap-class`, 2026-06-18). Класс `nowrap` теперь добавляется
  на `<pre>` verbatim-блоков. **Чисто рендерер** (`adoc-html`) + один сид атрибута; парсер не тронут. Спека
  верифицирована исходником asciidoctor 2.0.23 (`html5.rb` convert_listing/convert_literal: `nowrap = option?('nowrap')
  || !attr?('prewrap')`) + пробами. (1) `lib.rs`: сид `prewrap=''` в `HtmlRenderer::new` (зеркало
  `asciidoctor.rb DEFAULT_ATTRIBUTES`), снимается через `:prewrap!:` существующей strip-`!` логикой. (2) `blocks.rs`:
  хелпер `nowrap_active(meta)` (опция `nowrap` ИЛИ prewrap снят); применён в `start_source_block` (push `"nowrap"`
  ПОСЛЕДНИМ классом → `highlight nowrap`, `rouge highlight nowrap`, `highlightjs/CodeRay highlight nowrap`), Listing
  и Literal ветках (`<pre class="nowrap">`). Verse не затронут. NB: опцию nowrap несёт ТОЛЬКО shorthand-форма
  (`[source%nowrap,ruby]`/`[%nowrap]`); comma-форма `[source,ruby,%nowrap]` — 3-й позиционный = `linenums` (MATCH
  asciidoctor, не nowrap). +6 html-тестов; clippy 0, test --workspace зелёное (html 442→448). **Гейт 344/344
  байт-в-байт** vs master (0 регрессий). **Frontier identical 199→200 (+1)**, clean_div 33→32; new-vs-base = 2 файла
  IMPROVED (`wrap.adoc` `[%nowrap,java]`, `asciidoctor-0-1-4-released`: `highlight`→`highlight nowrap`, оба MATCH
  asciidoctor), **0 регрессий**.
  - **Follow-up (вне scope, предсуществующее):** rouge/coderay `linenums` — мы эмитим класс `linenums` на внешнем
    `<pre>` (`rouge highlight linenums`), asciidoctor — нет (`rouge highlight`, нумерация только в табличке). Не введено
    этим фиксом, nowrap добавляется поверх. Отдельная задача.
- [x] **F-I. UTF-8 BOM** (ветка `fix/utf8-bom`, 2026-06-18). Ведущий UTF-8 BOM (`U+FEFF`) теперь срезается
  до любой обработки — зеркало asciidoctor Reader. Новый zero-copy `scanner::strip_bom(input) ->
  input.strip_prefix('\u{feff}').unwrap_or(input)` (только ВЕДУЩИЙ BOM; BOM в середине цел — MATCH asciidoctor).
  Применён в 3 точках входа (идемпотентно): `Parser::new` (главная — закрывает scope через все pipeline:
  library/wasm `to_html` напрямую + CLI, куда BOM доезжает нетронутым через preprocess); defensive в
  `preprocessor::resolve_includes_with_source` и `preprocess_with_attrs` (BOM перед `:attr:`/`include::` в
  1-й строке тоже распознаётся). `BlockScanner::new_nested` не тронут. +4 parser (`scanner::test_strip_bom`,
  `parser::strips_leading_bom`/`keeps_non_leading_bom`, `preprocessor::test_leading_bom_stripped_before_attribute_entry`)
  +1 html (`test_leading_bom_stripped`); clippy 0, test --workspace зелёное (parser 575→579, html 448→449).
  **Гейт 344/344 байт-в-байт** vs master (BOM-файлов в гейте нет → 0 регрессий, прогон для гарантии).
  **Frontier identical 200→201 (+1)**, clean_div 32→31: `asciidoctor/test/fixtures/file-with-utf8-bom.adoc`
  (`﻿= 人`) флипнулся в identical — base давал `<title>Untitled</title>`+`<p>﻿= 人</p>`, new даёт
  `<title>人</title>`+`<h1>人</h1>` (embedded байт-в-байт MATCH asciidoctor). **0 регрессий** (это единственный
  изменившийся new-vs-base файл, изменение = улучшение). Пробы vs asciidoctor 2.0.23: BOM+`= Title`,
  BOM+`:attr:` first-line, BOM+CJK-title — все MATCH.
- [x] **F-H. Thematic break не прерывает открытый параграф** (ветка `fix/thematic-break-paragraph`, 2026-06-18).
  Корень дефолтного расхождения YAML front matter — **не** про front matter как таковой: в asciidoctor 2.0.23
  thematic break (`'''`/`---`/`***`/`___`) распознаётся ТОЛЬКО как самостоятельный блок на границе (после пустой
  строки / начало документа) и НЕ прерывает уже открытый top-level параграф (mid-paragraph это обычный текст —
  `read_paragraph_lines`/`StartOfBlockProc`). Мы ошибочно прерывали. Front matter — частный случай: открывающий
  `---` (граница) → `<hr>`, затем `key:val` + внутренний `---` + `= Title` слипаются в ОДИН параграф до пустой
  строки (section marker уже корректно не прерывал). **Чисто парсер** — убрано `is_thematic_break` из списка
  прерывателей открытого параграфа в 2 точках `block.rs`: `scan_paragraph` и `scan_admonition` (principal-параграф,
  «same break conditions»). Не тронуты: `scan_leaf_blocks` (эмиссия `<hr>` на границе), `is_page_break`,
  list/dlist-continuation (3367/3538). +3 parser-теста (`test_thematic_break_does_not_interrupt_paragraph`,
  `test_thematic_break_at_block_boundary_still_breaks` регресс-гард, `test_yaml_front_matter_collapses_into_paragraph`)
  +2 html (`test_thematic_break_does_not_interrupt_paragraph`, `test_yaml_front_matter`); clippy 0, test --workspace
  зелёное (parser 579→582, html 449→451). **Гейт 344/344 байт-в-байт** vs master (0 регрессий). Frontier identical
  201→**202 (+1)**, clean_div 31→30: единственный new-vs-base изменившийся файл — `asciidoctor/test/fixtures/with-front-matter.adoc`
  (`---\nname: value\n---\ncontent`) флипнулся в MATCH asciidoctor (base прерывал на 2-м `---`), **0 регрессий**.
  Пробы vs asciidoctor 2.0.23 MATCH: полный FM, FM без `=Title`, `---`/`'''` в параграфе, admonition `NOTE:…\n---\n…`,
  регресс-гард границы.
  - **Follow-up (вне scope, предсуществующие, НЕ введены/ухудшены фиксом):** (1) `skip-front-matter` атрибут —
    срез ведущего `---\n…\n---\n` целиком в препроцессоре (мы игнорируем). (2) `***`/`___` отдельной строкой ВНУТРИ
    параграфа → asciidoctor `<strong>*</strong>`/`<em>_</em>`, у нас литерал (inline-движок; фикс УЛУЧШИЛ — убрал
    лишний `<hr>`, но не до MATCH). (3) `- - -` (spaced) внутри параграфа прерывается как unordered list marker
    (`- ` распознаётся раньше thematic, стр. 2389<2396) — отдельное задокументированное list-marker расхождение.
    (4) page break `<<<` внутри параграфа — не проверено (описание F-H только про thematic).
- [x] **F-L. compat-mode `+text+`/`++text++` → monospaced** (ветка `feat/compat-mode-monospace`, 2026-06-18).
  Под `:compat-mode:` asciidoctor меняет QUOTE_SUBS (`substitutors.rb:477-479`): `+text+` (constrained) и
  `++text++` (unconstrained) → `<code>` (monospaced) с полными normal subs; без compat = passthrough (literal).
  Мы всегда трактовали как passthrough. **Чисто парсер** (3 файла): (1) `inline.rs` `InlineOptions` +поле
  `compat_mode` (зеркало `experimental`: `apply_attribute`/`from_attr_lookup`); (2) `subst/passthrough.rs`
  `try_plus` — при compat НЕ извлекать single `+`/double `++` (вернуть None → отдать quotes), `+++` triple
  остаётся raw passthrough (без fallback на `++`); (3) `subst/quotes.rs` `run_all` +`options`, при compat —
  `pass_unconstrained/constrained(b'+', Monospace)` на позиции монospace (после smart-quotes, перед emphasis =
  asciidoctor QUOTE_SUBS[true] порядок); `subst/mod.rs` прокидывает options. Вложенный `*bold*` внутри `+…+`
  работает (strong pass раньше). +3 parser +1 html теста (4 литерала `InlineOptions{experimental}` → `..Default`);
  clippy 0, test --workspace зелёное (parser 590→592, html 452→453). **Гейт 344/344 байт-в-байт** vs master
  (в гейте нет активного compat-mode: `literal-monospace.adoc` имеет `:compat-mode:` только как пример в listing —
  проверено). **Frontier identical 202→208 (+6)**, clean_div 30→24: 6 файлов стали identical (a-new-resource,
  asciidoclet-announcement, java-integration ×3, js-render), 13+ IMPROVED (oscon 602→69, github-0.1.4 543→50,
  enjoy-java 307→59, plain-text-diagrams 293→124). **0 реальных регрессий** (asciidoctor-0-1-2 позиционный diff
  417→432 — ЛОЖНО, LCS-diff 156→142 IMPROVED: многострочный `<code>` сдвигает хвост наивного differ'а). 10+ CLI-проб
  vs asciidoctor 2.0.23 MATCH. План: `~/.claude/plans/compat-mode-plus-monospace-fjp2.md`.
  - **Follow-up'ы (вне scope):** (1) backtick `` `text` `` в compat → literal monospace (no quote-subs/attr-refs;
    `{author}` не резолвится) — отдельный класс через `InlinePassRx[compat]`. (2) single-quote `'text'` → `<em>`
    (compat QUOTE_SUBS insert-3). (3) smart-quotes `` ``…'' ``/`` `…' `` в compat. (4) link-макрос СРАЗУ после
    `+`/`++` без пробела (`+http://x[t]+`) не резолвится — предсуществующий macros-autolink boundary (был скрыт
    non-compat passthrough; `+ http…` и backtick работают). (5) `++++` quad (`<code>+</code>+`), escaped `\++x++`
    edge. (6) CLI `-a compat-mode` не активирует (как и `-a experimental` — header-форма работает, предсуществующее).

**Рекомендация:** F-A…F-AH — ЗАКРЫТЫ (frontier identical **221**, clean-div 12). Дальше — топ clean-div (многоклассовые):
sample 152 (header не распознан), manpage 146 (doctype manpage), index 136 (table-cell literal + др.), debuter 118,
multi-special-ex 87 (partintro/book), CHANGELOG 75 (xref + `\...`-escape ellipsis), github-0.1.4 50 (TOC-позиция),
asciidoclet 12 (multiline footnote: `]` на следующей строке — footnote дропается, #footnotes пуст), asciidoc-returns 12
(footnote attr-ref-значение-link-макрос, остаток после F-AG: 273→12), mdbasics 1 (остаток = `+++`/`+-+` single-plus
passthrough после F-AH). Чистых одно-классовых кандидатов не осталось (enjoy закрыт F-AF, asciidoc-returns-каскад F-AG,
`\'`-escape F-AH). Follow-up'ы: **single-plus `+++`→`+`** (mdbasics — `+++` не матчится как single-plus passthrough с
content `+`; ВЫСОКИЙ риск гейта — `+` повсеместен); F-L (backtick-literal/single-quote в compat); F-H (`skip-front-matter`,
inline `***`/`___`); F-AG (`source-language`/полные doc_attrs во вложенной AsciiDoc-ячейке). Минимальные остатки
(diff=1, intrinsic, не actionable): doctime-localtime (`{localtime}`), migration (`{asciidoctor-version}`).

- [x] **F-M. Экранированная `\]` в bracketed inline-макросах** (ветка `fix/escaped-bracket-macros`, 2026-06-18).
  Внутри `name:target[content]` экранированная `\]` НЕ закрывает макрос (часть content) и разэкранируется `\]`→`]`
  (правило asciidoctor `(.*?[^\\])?\]`; для link — `text.gsub ESC_R_SB, R_SB`). Был наивный `rest.find(']')` →
  обрыв на первой `]`, хвост `\` в content + утечка остатка. **Доминирующий frontier-класс** (asciidoc-py 1645,
  migration 571 — каскады от `pass:[[x-\]…]` / `pass:[icon:fire[\]]`). **Фикс (чисто парсер, 2 файла):**
  (1) `subst/macros.rs` — новый `pub(super)` хелпер `find_macro_close_bracket(s, open)` (escape-aware scan, вынесен из
  `try_stem`) + `unescape_close_bracket(s) -> Cow` (`\]`→`]`, no-alloc без escape); применены к xref/link/mailto/image/
  icon/footnote/kbd/btn/menu/anchor/indexterm/indexterm2 + URL[text]-autolink (наивный `find(']')` заменён). Убран
  старый guard `bracket_end <= bracket_start` — поиск close от `bracket_start` = asciidoctor (target non-greedy до
  первого `[`; `xref:a]b[c]` → `#a]b`, было literal). `try_stem` переведён на хелпер. (2) `subst/passthrough.rs` —
  `try_pass_macro` (bare verbatim) + `try_pass_spec_macro` (`pass:q`… разэкранировать до spec'd-subs) на тех же
  хелперах (импорт из macros). **legacy `inline.rs` не тронут** (sequential — дефолт; legacy лишь fallback при
  decline, escaped-входы не declinе'ятся) → follow-up. Тесты: +2 unit (хелперы, macros.rs `mod tests`),
  +2 html (`test_escaped_close_bracket_in_macros` 10 классов, `..._regression_guards` одиночный `\`/пустой `[]`/plain),
  2 reversed-bracket кейса (`xref:a]b[c]`, `icon:a]b[c]`) перенесены из «reproduces_legacy» в «adopt-asciidoctor».
  clippy 0, test --workspace зелёное (parser lib 592→594, html 453→455). **Гейт 344/344 байт-в-байт** vs master
  (0 регрессий; оба gate-файла с `\]` — неактивный контекст: literal-monospace indented-literal, ui.adoc kbd без
  experimental). **Frontier identical 208→209 (+1)**, clean-div 24→23: new-vs-base ровно 2 файла ОБА IMPROVED, 0 регр.
  (`asciidoc-py` 1645→**0 identical**, `migration` 571→273 — остаток = bare `[x-]` follow-up). 14 CLI-проб vs
  asciidoctor 2.0.23 MATCH. План: `~/.claude/plans/escaped-bracket-macros-fm.md`.
  - **Вне scope (follow-up):** bare `[x-]` local-compat-role (`<code class="x-">` vs `<code>`, migration 647/677 —
    остаток 273); legacy `inline.rs` escaped-паритет (14 сайтов `find(']')` + готовый `parse_bracket_macro_escaped`);
    F-N indented→literal перебивает section-marker (recommended-practices 15); mdbasics escaped `\'`; install-macos
    `menu:` с иконкой; footnote-marker порядок атрибутов (`id`/`class`, предсуществующее, рендерер).
- [x] **F-L′. compat-mode кавычки: `` ``..'' `` → curly «“..”», `'..'` → `<em>`, `` `..' `` → curly ‘..’**
  (ветка `feat/compat-mode-quotes`, 2026-06-18, 121-я; план `~/.claude/plans/compat-mode-quotes-flp.md`).
  Доминирующий остаток compat-кластера (22 файла) после F-L. Три недостающих правила `COMPAT_QUOTE_SUBS`
  (`asciidoctor.rb:469-485`), порядок double→emphasis→single (все constrained). **Чисто парсер** (`subst/quotes.rs`):
  развилка в `run_all` по `options.compat_mode`; новая `pass_compat_curved` (обобщение `pass_smart_quotes` на
  асимметричные многобайтные open/close + `compat_open_boundary` + `find_compat_curved_close`, эмит через
  существующий `smart_quote_sentinel`); emphasis через `pass_constrained(b'\'')`. В compat non-compat smart-quotes
  (`"`..`"`/`'`..`'`) ОТКЛЮЧЕНЫ (asciidoctor их заменяет → `"`page`"`=`"<code>page</code>"`); backtick-monospace
  ОСТАВЛЕН (asciidoctor извлекает `` `code` `` как literal-passthrough ДО QUOTE_SUBS). Гард: апострофы
  (`don't`/`O'Reilly`/`it's`) НЕ → `<em>` (constrained boundary); lazy close → вся строка-в-кавычках `<em>`.
  +3 parser +1 html теста; clippy 0, test --workspace зелёное (parser 594→597, html 455→456). **Гейт 344/344
  байт-в-байт** vs master (нет активного compat в гейте). Frontier identical 209 (стабильно); new-vs-base
  **4 изменились, 2 IMPROVED, 0 регрессий**: `oscon` 69→**4** (остаток = attr-ref `{github-uri}` в image URL),
  `asciidoc-returns-to-github` 420→**273**; 2 NEUTRAL (`0-1-3`/`0-1-4`) спот-чеком = content-IMPROVED
  (`` ``OK'' ``→“OK”; `"`air quotes`"` остался литералом в listing — base ошибочно курлил verbatim). 10 CLI-проб
  vs 2.0.23 (normalize): 9/10 MATCH (10-й = compat-backtick-passthrough, follow-up).
  - **Follow-up (вне scope):** compat-backtick-literal passthrough `` `text` `` (F-L #1; даёт `` `gem` ``→`<code>`,
    новый корень asciidoc-returns 245); `''..''` сдвоенные апострофы (сторона апострофа, патология lazy-close в
    `constrained_open_close`); legacy `inline.rs` compat-паритет; REPLACEMENTS right-single-quote `` `' ``.
- [x] **F-N. `icon:name[]` без `:icons:` → текстовый fallback `[name]`** (ветка `feat/icon-text-fallback`,
  2026-06-18, 122-я; план `~/.claude/plans/jolly-knitting-sparrow.md`). Доминирующий остаточный кластер
  frontier-прохода (209 identical). Корень: `render_icon` (`adoc-html/src/inline.rs`) **никогда не читал
  `:icons:`** — безусловно эмитил FA `<i class="fa fa-NAME">`. Asciidoctor при отсутствии `:icons:` рендерит
  `<span class="icon">[name&#93;</span>` (литерал; закрывающая `]` = NCR). **Чисто рендерер** (1 метод):
  при `!document_attrs.contains_key("icons")` — text-fallback (`alt` заменяет `name`, `role` на span,
  `link`→`<a class="image">`+`window`/`rel=noopener`; size/title/rotate/flip игнор). Font-путь оставлен
  байт-в-байт (захват `alt`/`window` в парс-цикле им игнорируется). +5 новых html-тестов; 8 существующих
  icon-тестов + 2 XSS-кейса мигрированы на префикс `:icons: font`. clippy 0, test --workspace зелёное
  (html unit 455→**461**, parser 597 без изм.). **Гейт 344/344 байт-в-байт** (в гейт-корпусе все `icon:` —
  внутри listing-блоков, реальных inline-иконок 0). Frontier identical **209** (0 регрессий); new-vs-base по
  всем 250 файлам → изменились **ровно 2** icon-файла, ОБА → asciidoctor: `asciidoctorj-1-5-0` 452→**3**
  (IMPROVED), `install-macos` icon-контент совпал (`<i fa>`→`[apple&#93;`; +1 naive — ложная регрессия от
  незакрытого experimental-menu-shorthand `"X > Y"`→menuseq). 8 CLI-проб vs 2.0.23: text-fallback IDENTICAL.
  - **Follow-up (вне scope):** image-режим `:icons:`→`<img>` (0 корпус-импакта); font link/role-паритет
    (`<a class="image">`+role-на-span; CHANGELOG/syntax далеки от identical); experimental-menu-shorthand
    `"Menu > Item"`→menuseq (корень install-macos 476).
- [x] **F-O. attr-ref: хвостовой `[label]` после нерезолвленного/intrinsic-значения прогоняется через
  inline-subs** (ветка `fix/attr-ref-trailing-subs`, 2026-06-18, план `~/.claude/plans/attr-ref-trailing-subs-fo.md`).
  **Крупнейший frontier-каскад `templates.adoc` (1692).** Корень (ЧИСТО РЕНДЕРЕР, `adoc-html/src/events.rs:176`):
  ссылку из `Event::AttributeReference{name,trailing}` строит рендерер; ветка `Document(value)` re-парсит
  `value+trailing` корректно, а ветки **MissingSkip / Intrinsic / Env / Fallback** эмитили хвост через
  `html_escape_text` ЛИТЕРАЛЬНО (backtick/`*` не обрабатывались). asciidoctor (attr-missing=skip) оставляет
  `{name}`/intrinsic-значение, но хвост `[...]` проходит обычные subs (`[\`x\`]`→`[<code>x</code>]`). Templates
  ломался на вложенном нерезолвленном `{url-api-gems}`/`{release-version}` внутри `{apidoc-*}`-значения.
  **Фикс:** в 4 не-Document армах `html_escape_text(&br)` → `self.render_inline_value(output, &br)` (уважает
  `current_subs()` — в verbatim только specialchars, backtick остаётся литералом = 0 риска; рекурсия по
  вложенным attr-ref терминируется — хвост строгий суффикс). Парсер не тронут (рендерер общий → оба движка).
  +1 html-тест `test_attribute_reference_trailing_subs` (undef+code/bold, nested templates-like, intrinsic `{sp}`,
  3 регресс-гарда: defined-URI link / plain хвост / verbatim listing). clippy 0, test --workspace зелёное
  (html unit 461→**462**, parser 597 без изм.). **Гейт 344/344 байт-в-байт** vs master `9d40067` (gate_check.py
  → 0 diff). **Frontier identical 209→210 (+1)**: new-vs-base = **2 файла, ОБА IMPROVED, 0 регрессий** —
  `custom.adoc` 846→**0** (флип в identical), `templates.adoc` 1692→**634** (остаток = conum-класс `<1>`→`<b class="conum">`,
  не F-O). 8 CLI-проб vs asciidoctor 2.0.23 MATCH байт-в-байт. **Ожидает коммита/мержа/пуша по запросу пользователя.**
  - **Follow-up (вне scope):** остаток templates = callout-номера `<N>` в listing → conums (отдельный класс);
    остальные frontier-классы: `[x-]` local-compat (asciidoclet 383, migration 273), experimental-menu-shorthand
    (install-macos 477), multi-backtick mis-pair (api/index 347), compat-backtick passthrough (asciidoc-returns 273).
- [x] **F-P. `[x-]` literal-monospace passthrough-маркер** (ветка `feat/x-literal-monospace`, 2026-06-19,
  план `~/.claude/plans/majestic-mixing-lobster.md`). `[x-]` (и `[<attrs> x-]`) перед inline-monospace — магический
  passthrough-маркер asciidoctor (старое AsciiDoc-поведение, `InlinePassRx[false]` `x-`-ветка, `rx.rb:585` +
  `substitutors.rb:1076-1121`): роль `x-` ОТБРАСЫВАЕТСЯ, content рендерится как `<code>` с OLD behaviour —
  backtick close → **BASIC_SUBS** (specialchars only, `*b*`/`_em_`/`{attr}` ЛИТЕРАЛ), `+` close → **NORMAL_SUBS**
  (`_em_`→`<em>`, attr резолвится); `[<attrs> x-]` сохраняет ведущую роль (`[method x-]+save()+`→`<code class="method">`).
  Был баг: трактовали `x-` как обычную роль + применяли subs (`<code class="x-"><strong>`). **Чисто парсер** (2 файла):
  (1) `subst/quotes.rs` — `parse_attrs` → `pub(super)` (переиспользование). (2) `subst/passthrough.rs` — обработка
  в `extract()` (FIRST pass, до всех subs = зеркало `extract_passthroughs`): новый arm для `[` с open-boundary
  (`x_marker_open_boundary`: preceding ∉ {word,`;`,`:`,`\`}), `try_x_marker` (matchит `[x-]`/`[… x-]` + backtick/`+`,
  строит `[Start(Monospace{id,roles}), content, End]` → `macro_sentinel` опаковый leaf; backtick → `Text` BASIC_SUBS,
  plus → `run_pipeline(content, NORMAL)`), `find_pass_close` (lazy first-valid, зеркало `(\S|\S.*?\S)\7(?!WORD)`).
  Регресс-safe: не-`x-` attrlist (`[x-y]`/`[a-]`/`[foo]`/`[.role]`) → None → fall-through → обычная роль (quotes pass).
  +1 parser (`x_marker_literal_monospace`) +1 html (`test_x_marker_literal_monospace`) тест; clippy 0, test --workspace
  зелёное (parser 597→**598**, html 462→**463**). **Гейт 344/344 байт-в-байт** vs master `c65c14a` (в гейте нет
  `[x-]` → 0 риска). **Frontier identical 210** (стабильно); new-vs-base = **2 файла, ОБА IMPROVED, 0 регрессий** —
  `asciidoclet-1.5.0-released` 383→**12**, `migration` 273→**1** (остатки = intrinsic/custom attr-ref, ДРУГОЙ класс).
  8 CLI-форм vs asciidoctor 2.0.23 (backtick/plus/role/regress×4/edge) MATCH байт-в-байт. **Ожидает коммита/мержа/пуша.**
  - **Follow-up (вне scope):** escaped `\[x-]…` и `[x-]\+text+` (preceding `\` блокирует arm — отсутствует в корпусе,
    не регресс vs master); `[`/`\n` внутри attrlist (отвергаются — single-line упрощение); named-роли `[role=x x-]`
    (`parse_attrs` только позиционный/shorthand). Остаток migration/asciidoclet = intrinsic-attr резолв (`{asciidoctor-version}`).
- [x] **F-Q. Conum (callout `<N>`) в явных `[listing]`/`[literal]` делимитед-блоках** (ветка `feat/listing-literal-callouts`,
  2026-06-19, план `~/.claude/plans/lexical-gliding-snowglobe.md`). Callout-маркеры обрабатывались в голых `----`/`....`
  и `[source]`, но НЕ в блоках с явным позиционным стилем `[listing]`/`[literal]` (`<1>`→литерал `&lt;1&gt;`). Asciidoctor
  назначает subs по `content_model` (`:verbatim`=`[:specialcharacters,:callouts]`), независимо от наличия явного стиля
  (`substitutors.rb:1287-1315`, `block.rb:12-24`); `:verse`→NORMAL, `:pass`/`++++`→`:raw`/NO_SUBS (без callouts). **Чисто
  парсер** (`block.rs`, root-cause рефакторинг). Корень — ДВЕ почти идентичные ветки эмиссии verbatim в `scan_delimited_block`:
  ветка явного стиля пушила сырой `Event::Text` без `resolve_callouts_in_lines`, голая — обрабатывала. Фикс: извлечён общий
  метод `scan_verbatim_delimited_block(kind, delim_type, delim_len, block_attrs, title_events)` = дословный код голой ветки
  (цикл сбора + trailing-trim + single-empty-line + reindent + callouts + эмиссия; `kind` из параметра, может ≠ `delim_type`).
  Голая ветка и явная listing/literal теперь делегируют в него (зеркало существующих `scan_source_block`/`scan_verse_block`);
  `pass` остался на отдельном raw-пути (NONE subs). Унификация попутно подтянула резолв `subs=`/`indent=`/пустого блока к явным
  verbatim (всё MATCH asciidoctor). +4 parser (`test_listing_style_callouts`/`_literal_style_callouts`/`_pass_style_no_callouts`/
  `_listing_style_without_callouts_unchanged`) +3 html (`test_listing_style_callouts_html`/`_literal_*`/`_pass_*`); clippy 0,
  test --workspace зелёное (parser 598→**602**, html 463→**466**). **Гейт 344/344 байт-в-байт** vs master `725ae8d`
  (предпроверено: ~8 гейт-файлов с явными делимитед `[listing]`/`[literal]`, но НИ ОДИН без end-of-line `<N>`/`indent=` →
  0 риска). **Frontier identical 210→211 (+1)**, clean_div 23→22: new-vs-base = **1 файл, IMPROVED, 0 регрессий** —
  `templates.adoc` 634→**0** (флип в identical). 7 CLI-форм vs asciidoctor 2.0.23 (listing/literal/bare/pass/empty×2/
  `subs="+macros"`) MATCH байт-в-байт. **Ожидает коммита/мержа/пуша.**
  - **Follow-up (вне scope, нет корпус-импакта):** (1) формат conum под `:icons: font` (`<i class="conum" data-value="N">`)
    — отдельный класс (plain-text-diagrams 124), рендерер. (2) Параграф-стиль `[listing]`/`[literal]` (без делимитера,
    `scan_paragraph`) с callouts. (3) Пред-существующее: `[listing]` над `====`/`--`/`....` ремапит в listing (наш
    `test_listing_style_on_example_delimiter`), asciidoctor по masq оставляет example — фикс лишь добавил туда conum, тип
    блока не менял. (4) Бар `++++` passthrough даёт `process_callouts=true` (наш pre-existing vs asciidoctor `:raw`) — код
    перенесён дословно, не чинили.
- [x] **F-R. dlist-терм с ведущим двоеточием (`:context::`)** (ветка `feat/colon-prefixed-dlist-term`, 2026-06-19,
  план `~/.claude/plans/fizzy-stirring-dijkstra.md`). Термы definition-list, начинающиеся с `:` (`:context::`,
  `:style::`, `:id::`, `:role::`), НЕ распознавались — `scanner::is_attribute_entry` ошибочно принимала строку как
  document-attribute (`name="context"`, `value=": ..."`) и перехватывала её в `scan_leaf_blocks` (block.rs:901)
  ДО dlist-детекции (`scan_list_constructs`, block.rs:1172) → **весь список терялся** (мусорный параграф). Asciidoctor
  `AttributeEntryRx` (`rx.rb:125`) после разделяющего `:` требует пробел/таб ИЛИ конец строки → `:context::` падает
  в `DescriptionListRx` (`rx.rb:337`, term-группа `([^ \t].*?)` без ограничения на 1-й символ → term `:context`).
  **Чисто парсер, 1 функция (~4 строки):** `scanner::is_attribute_entry` — после разделяющего `:` теперь требуется
  пробел/`\t`/EOL (зеркало value-клаузы `(?:[ \t]+…)?$`); иначе `None` → строка падает в dlist. `:foo:bar::` →
  терм `:foo:bar`; `:key:value` (no-space) → больше не attr (asciidoctor тоже отвергает); `:key: value`/`:toc:`/
  `:author: :smile:` — без изменений. `is_description_list_marker` уже была корректна (возвращала `:context`), не тронута.
  +6 scanner-ассертов (3 в `test_is_attribute_entry`, 2 в `test_is_description_list_marker`) +1 parser
  (`test_colon_prefixed_dlist_term`) +1 html (`test_colon_prefixed_dlist_term_html`); clippy 0, test --workspace
  зелёное (parser 602→**603**, html 466→**467**). **Гейт 344/344 байт-в-байт** vs master `7f8430a` (line-level
  flip-детектор: **0 строк гейта** меняют классификацию). **Frontier identical 211→212 (+1)**, clean_div 22→21:
  new-vs-base = **1 файл, IMPROVED, 0 регрессий** — `find-blocks.adoc` **296→0** (флип в identical). 5 CLI-проб
  vs asciidoctor 2.0.23 (`:context::`/`:foo:bar::`/`:key:value`/`:key: value`/multi-term) MATCH байт-в-байт.
  **Ожидает коммита/мержа/пуша.**
  - **Follow-up (вне scope):** word-char-start-проверка имени атрибута (`:-x:`, asciidoctor отвергает — не нужна для
    бага, добавляет риск); прочие frontier-каскады: experimental-menu (install-macos 477), dlist отступная multiline
    desc (0-1-2 432), multi-backtick (api/index 347), compat-backtick (asciidoc-returns 273).
- [x] **F-S. conum/colist под `:icons:` (font + image)** (ветка `feat/icons-conum-colist`, 2026-06-19,
  план `~/.claude/plans/vectorized-zooming-charm.md`). При активном `icons` Asciidoctor меняет рендеринг callout-ов
  в ДВУХ местах, мы — нет: (A) маркер conum в verbatim, (B) сам colist становится `<table>` вместо `<ol>`.
  **Чисто рендерер** (`adoc-html`), парсер не тронут (события `Event::CalloutRef/XmlCalloutRef`, `Tag::CalloutList/
  CalloutListItem` уже корректны). Спека по `html5.rb:490 convert_colist` + `:1191 convert_inline_callout` +
  `abstract_node.rb:292 icon_uri`. **3 состояния** (зеркало admonition `blocks.rs:49`): `Some("font")`/`Some(_)`
  (image)/`None`. (A) verbatim conum (ИСХОДНЫЙ N, со скобками): font→`<i class="conum" data-value="N"></i><b>(N)</b>`,
  image→`<img src="{iconsdir}/callouts/N.{icontype}" alt="N">`, none→без изм.; под icons у `XmlCalloutRef` обёртка
  `&lt;!--…--&gt;` исчезает. (B) colist при icons set → `<table>` с ПОЗИЦИОННЫМ маркером (счётчик `callout_list_num`,
  игнор исходного `<N>`, БЕЗ скобок), текст ячейки без `<p>`, блоки-продолжения прямо в `<td>`; без icons → `<ol>`
  без изм. Новый хелпер `callout_marker(&self,n,parens)->Option<String>` (events.rs, owned-return снимает borrow-конфликт);
  поле `callout_list_num: u32` (lib.rs). R1: open/close_li_paragraph пропускаются ОБА в table-режиме (баланс стеков
  `li_p_open`+`li_para_count`). +9 html-тестов; clippy 0, test --workspace зелёное (html unit 467→**476**, parser 603).
  **Гейт 344/344 байт-в-байт** vs master `779639f` (ни один гейт-файл не рендерит conum/colist при активных иконках:
  callout.adoc/admonitions.adoc — все callout-события ДО активации `:icons:`; callout.adoc подтверждён). **Frontier
  identical 212 (без изм. — улучшенные файлы имеют остаточные не-conum расхождения)**; new-vs-base (difflib edit-distance,
  не позиционный `nd` — он каскадит на структурном ol→table): **5 файлов, все IMPROVED, 0 регрессий** — syntax 47→31,
  asciidoc-writers-guide 158→56, 0-1-3-released 104→74, 0-1-4-released 306→207, plain-text-diagrams 48→4. CLI-пробы
  vs asciidoctor 2.0.23: font/image × (verbatim/colist/XML/positional/title) MATCH; continuation нормализуется identical.
  **Ожидает мержа/пуша.**
  - **Follow-up (вне scope):** icon-role на inline `icon:name[role=x]` (syntax 316: `<span class="icon blue">` vs наш
    `<i class="fa fa-flag blue">`); checklist `<i class="fa fa-check-square-o">` (syntax 589); прочие топ-каскады:
    experimental-menu (install-macos 477), literal-каскад (0-1-2 432), multi-backtick (api/index 347).
- [x] **F-T. Quoted inline menu `"X > Y"` под `:experimental:`** (ветка `feat/quoted-inline-menu`, 2026-06-19,
  план `~/.claude/plans/lexical-twirling-iverson.md`). Asciidoctor `InlineMenuRx` (`rx.rb:571`): строка в ДВОЙНЫХ
  кавычках, чьё содержимое начинается с `[\w&]` и держит space/newline-обрамлённый `>`, под атрибутом `experimental`
  становится menu-sequence (`"icon:apple[] > Software Update"` → `<span class="menuseq"><b class="menu"><span
  class="icon">[apple&#93;</span></b>&#160;<b class="caret">&#8250;</b> <b class="menuitem">Software Update</b></span>`).
  **Чистая структурная модель событий** (НЕ трогает `Tag::Menu`/`menu:`-макрос и F-F-тесты): новые `MenuPart{Menu,
  Submenu,Item}`, `Tag::MenuSeq`/`Tag::MenuPart{role}`, `TagEnd::MenuSeq/MenuPart` (`event.rs`). Детекция в
  `subst/macros.rs::extract` под `options.experimental`, на ЛИТЕРАЛЬНОМ `>` (specialchars не проход у нас — escape
  только в рендерере; first-char `[\w&]` на до-quotes тексте отклоняет ровно те же случаи, что asciidoctor на
  post-quotes — маркеры `_*\`#^~<[` ∉ `[\w&]`). Хелперы `quoted_menu_span_end`/`has_spaced_gt`/`try_quoted_menu`/
  `build_menuseq`: сплит по ВСЕМ `>`, strip, первый=menu/последний=menuitem/средние=submenu; КАЖДЫЙ сегмент
  inner-reparse через `run_pipeline` (subs БЕЗ изм., MACROS ON) → icon/image/link/quotes рендерятся внутри ЛЮБОГО
  сегмента (проба `"File > *bold*"` → `<strong>` в menuitem MATCH). Ведущий `\"` снимает backslash (литерал, escape.rs
  оставляет `\"` нетронутым). Рендерер stateless (`events.rs` start/end плечи + `inline.rs::menu_caret()` — caret
  `&#160;<b class="caret">&#8250;</b> ` или при `:icons: font` `&#160;<i class="fa fa-angle-right caret"></i> `, эмит
  перед каждой не-`menu` частью). Legacy не тронут (нет `InlineMenuRx`, fallback недостижим). builder.rs +2 фрейма.
  +1 parser-тест (`quoted_inline_menu_matches_asciidoctor`: точные event-векторы 2/3-сегмента, icon-в-menu, link-в-
  menuitem, 5 не-matches, escape, gating) +2 html-фикстуры (`quoted-menu`, `quoted-menu-icons-font`). clippy 0,
  test --workspace зелёное (parser 603→**604**, html unit 476, html-фикстур 70→**72**). **Гейт 344/344 байт-в-байт**
  vs master `a4e7bde` (в гейт-корпусе нет `:experimental:` с ` > `-строками → правило мёртвый код; `Tag::Menu` цел).
  **Frontier identical 212→213 (+1)**, clean_div 21→20: new-vs-base = **1 файл, IMPROVED, 0 регрессий** —
  `install-asciidoctor-macos.adoc` **477→0** (флип в identical, 3× `"icon:apple[] > Software Update"`). 16 CLI-проб
  vs asciidoctor 2.0.23 (icon/image/link/bold × menu/submenu/menuitem, multi-`>`, escape, не-matches, `:icons: font`,
  multiline) MATCH байт-в-байт. **Ожидает мержа/пуша.**
  - **Follow-up (вне scope):** сегмент, чей 1-й символ asciidoctor превратил бы в `<` (отклоняется обоими — OK);
    сентинель внутри `"…"` → деклайн (корпусно недостижимо). Прочие топ-каскады: literal/dlist-indent (0-1-2 432),
    multi-backtick (api/index 347), compat-`+gem+` десинк (asciidoc-returns 273), manpage doctype (manpage 160).
- [x] **F-V. Многострочное отступленное описание dlist схлопывалось в literalblock** (ветка
  `feat/dlist-indented-description`, 2026-06-19). Топ frontier-расхождение (`asciidoctor-0-1-2-released.adoc` 432 diff,
  паттерн ×13). Dlist-термин с пустым inline-desc + отступленный многострочный параграф-описание: asciidoctor собирает
  смежные (без пустой строки) строки в ОДИН `<p>` внутри `<dd>` со срезом общего min-отступа (`adjust_indentation!`,
  indent 0); наш парсер брал 1-ю строку как `<p>`, а остаток (отступленный) делал `literalblock` с нерезолвленными
  `{attr}`. **Корень** (`block.rs::scan_description_list_item`): цикл сбора continuation-строк имел гард
  `!line.starts_with(' ')` → отступленные строки отбрасывались блок-сканеру (→ literal). У ulist/olist гарда нет
  (P2/P3 MATCH); обычный параграф тоже корректен (P1) — баг ТОЛЬКО в dlist. **Фикс (чисто парсер, 1 файл):** (1) снят
  indent-гард (цикл уже прерывается на blank через `is_dlist_continuation_line` → case C «literal после пустой строки»
  сохранён); (2) принципал-из-следующей-строки + continuation объединяются в один блок, общий min-отступ срезается через
  `reindent_verbatim_lines(_, 0)` (zero-copy `&line[n..]`), inline-принципал (`term:: text`, col 0) из min-расчёта
  исключён; (3) per-line `trim_end` (asciidoctor rstrip'ит). Mixed-indent сохраняет относительный отступ (config: 2/6 →
  дедент 2 → «    six.»). +4 parser-теста (empty-desc multiline, inline-desc+indent, common-indent дедент, case-C
  literal-guard) +1 html-фикстура (`dlist-indented-description`, 4 кейса). clippy 0, test --workspace зелёное
  (parser 605→**609**, html-fix 72→73). **Гейт 344/344 байт-в-байт** vs master `f7cd349` (gate_check new-vs-base 0 diff
  — отступленных dlist-continuation в гейте нет). **Frontier identical 214→215 (+1)**, clean-div 19→18: new-vs-base =
  **4 файла, ВСЕ IMPROVED, 0 регрессий** — `0-1-2-released` **432→0** (флип), `manpage` 160→146, man-asciidoctor/
  asciidoctor(man) 864→779. 14 CLI-проб vs asciidoctor 2.0.23 MATCH (inline/empty desc, uniform/mixed/tab indent,
  multi-term, inline-markup, nested, case C, corpus-сниппет с `{issue-ref}`). Смержена в master `8637c9e`.
- [x] **F-W. Экранированная типографская замена в attr-ref trailing → паразитный `0`** (ветка
  `fix/attr-ref-escaped-typographic`, 2026-06-19). Sequential-движок ПРОТЕКАЛ внутреннее sentinel-представление
  в публичный `Event::AttributeReference.trailing_brackets`: для `{attr}/path\X[brackets]` (где `\X` — экранированная
  замена `\...`/`\--`/`\(C)`/`\(R)`/`\(TM)`) `escape::run` запечатывал `\X` в Literal-sentinel, а `attributes::try_attr`
  захватывал `trailing` сырым срезом буфера → sentinel-байты попадали в событие → рендерер ре-парсил
  `value+trailing_brackets` в свежем `Work` с пустой таблицей тегов → `try_parse` отказывал на `TAG_LEAD` → legacy
  парсил сырые control-байты → цифра индекса `0` протекала в href (`\...` → `0`, порча URL). **Фикс (чисто парсер,
  1 файл `subst/attributes.rs::extract`):** десентинелизовать `trailing` против `work.tags` перед сохранением через
  существующую `tokenize::desentinelize` (`Literal("...")`→`...`; ранний возврат no-op для trailing без sentinel).
  URL-значный атрибут формирует ссылку, таргет берётся verbatim → href с литеральным `...` = байт-в-байт asciidoctor
  2.0.23 (у него replacements ДО macros → `\X` литерален к формированию ссылки). Движок намеренно расходится с legacy
  (legacy держит сырой `\...` → неверный href); входы отсутствуют во всех `reproduces_legacy_on_*`. +1 parser
  Event-вектор +1 рендерер +1 HTML-фикстура; clippy --workspace 0, test --workspace зелёное (parser 609→610,
  html 476→477, html-фикстур 17→18). **Гейт 344/344 байт-в-байт** vs master (gate_check new-vs-base 0 diff — конструкция
  в гейте отсутствует). **Frontier identical 215 (без регресса)**, clean-div 18: new-vs-base difflib = 2 файла,
  0 регрессий — `CHANGELOG.adoc` IMPROVED **84→75** (атрибут-префиксная форма `{url-repo}/...\...`, v2.0.x),
  `mdbasics.adoc` NEUTRAL (мусорный `0` устранён: `\'.text'`→`'.text'`). 5/5 CLI-проб MATCH. **Ожидает мержа/пуша.**
  - **Follow-up'ы (вне scope, документировано):** прямой bare-URL link-макрос `\...` (CHANGELOG v1.5.x форма
    `https://…/\...` — legacy сохраняет бэкслеш в таргете, ~13 строк); неэкранированный `...` в attr-trailing
    link-таргете (asciidoctor применяет ellipsis `…`, мы — нет); arrow-escapes `\->`/`\=>`/`\<-`/`\<=`
    (пре-существующая `>`-граница URL-автолинка); `\'`-семантика (asciidoctor держит `\'`, мы дропаем бэкслеш);
    Macro-sentinel в trailing (`desentinelize` дропает).
- [x] **F-X. icon-макрос в text-mode → default alt + section-id** (ветка `fix/icon-macro-id-alt`, 2026-06-19,
  СМЕРЖЕНА). Inline `icon:NAME[]` без `:icons:` (text-mode) расходился с asciidoctor 2.0.23 по двум связанным граням:
  (1) **alt-текст** — `icon:fast-forward[]` → `[fast-forward]` вместо `[fast forward]`: default alt должен быть
  `File.basename(name, extname).tr('_-',' ')` (отброс пути/расширения, дефис/подчёркивание→пробел); (2) **section-id** —
  `== icon:fast-forward[] Migration` → `_iconfast_forward_migration` вместо `_fast_forward_migration` (литерал `icon`
  тёк в id). Корень: `strip_urls_for_id`/`generate_id` (scanner.rs) обрабатывали `link:`/`http(s)://`, но не `icon:`;
  `render_icon` (adoc-html) брал сырой `name` при отсутствии явного `alt=`. **Фикс (4 файла):** новый общий
  `scanner::icon_default_alt` (zero-dep, re-export `adoc_parser::icon_default_alt`) — используется и id-генератором,
  и рендерером; ветка `icon:` в `strip_urls_for_id` (подстановка alt, fall-through для невалидных скобок); `render_icon`
  default alt через хелпер в text-mode (явный `alt=` и font/image-mode не тронуты). +1 unit `icon_default_alt`,
  +8 `generate_id`-кейсов (parser 609→611), +1 html default-alt (html 477→478), +2 HTML-фикстуры (18). clippy 0,
  test --workspace зелёное. **Гейт 344/344 байт-в-байт** vs master (gate_check 0 diff — icon-имена гейта простые →
  identity alt; icon-в-заголовке в гейте 0). **Frontier identical 215→216** (`asciidoctorj-1-5-0-released` флип),
  clean-div 18→17, 0 регрессий (new-vs-base: 4 строки только в asciidoctorj, IMPROVED). 14 CLI-проб MATCH (вкл.
  путь/расширение/malformed `icon:noclose`/quoted `alt`).
  - **Follow-up (вне scope, документировано):** снятие кавычек у named-атрибутов icon (`alt`/`role`) —
    `icon:home[role="a b"]` → у нас `class="icon "a b""`, у asciidoctor `class="icon a b"`. Пре-существующий латентный
    баг в attr-цикле `render_icon` (quote-blind `split(',')`); затрагивает ГЕЙТ (quoted `role=` в `asciidoc-lang/subs/*`)
    → отдельная задача с ручной верификацией. Техника-зеркало: `adoc-html/src/media.rs:36-55`.
- [x] **F-Y. Вложенный attr-ref в значении атрибута резолвится в момент ОПРЕДЕЛЕНИЯ** (ветка
  `feat/attr-ref-resolve-at-definition`, 2026-06-19). `:dan-uri: {github-uri}/mojavelinux` (где `:github-uri:` уже
  определён) у asciidoctor хранит `https://github.com/mojavelinux` (резолв в момент определения по уже определённым
  атрибутам — `Document#apply_attribute_value_subs`/`set_attribute`, document.rb). У нас рендерер строил `document_attrs`
  из СЫРЫХ `Event::Attribute` и разворачивал лишь один уровень при использовании → в macro-атрибуте (`link="{dan-uri}"`)
  литерал `{github-uri}` тёк в href. **Корень:** `record_attribute_entry` (block.rs) уже резолвил значение в `doc_attrs`
  (для section-id), но эмитимый `Event::Attribute` нёс сырое значение → таблица рендерера рассинхронизирована с block-уровнем.
  **Фикс (чисто парсер, block.rs):** `record_attribute_entry` теперь возвращает `Option<String>` (Some только когда `{ref}`
  реально развернулся — Cow::Owned; None для unset-форм и значений без изменений → caller держит zero-copy slice). Все
  4 точки эмиссии `Event::Attribute` (body `scan_leaf_blocks` + 3 header-пути) эмитят резолвленное значение. Произвольная
  глубина вложенности работает автоматически: каждый атрибут уже полностью резолвлен на момент своего определения, поэтому
  один уровень резолва на определение достаточен (зеркало asciidoctor). Forward-ref (`:b: {a}/x` до `:a:`) остаётся
  литералом `{a}/x` (default `attribute-missing=skip`), не back-patch'ится. **Вне scope (follow-up):** specialchars-проход
  по значению в момент определения (asciidoctor: `HEADER_SUBS=[:specialcharacters,:attributes]` — `<>&` экранируются
  один раз при определении); `pass:[]`/`pass:subs[]` форма значения атрибута (`AttributeEntryPassMacroRx` — bypass header
  subs). Оба не встречаются в текущих расхождениях. +3 parser integration-теста (resolved-at-definition / deep-nesting /
  forward-ref-literal), +1 HTML-фикстура `inline/attr-ref-resolve-at-definition` (эталон `asciidoctor -e`, 18→19). clippy 0,
  test --workspace зелёное (1197 passed). **Гейт 344/344 байт-в-байт** vs master (gate_check 0 diff — gate-определения с
  вложенными ref почти все литеральные примеры в `[source]----`; `image.adoc` реальные, но значение после reset не
  рендерится). **Frontier identical 216→217** (oscon-2013 флип 4→0), clean-div 17→16; new-vs-base: 6 файлов ВСЕ IMPROVED,
  0 регрессий (oscon 4→0; README de/jp/fr/zh/en −1..−143 — взаимоссылающиеся `{uri-*}`). Проба + фикстура MATCH asciidoctor 2.0.23.
- [x] **F-Z. callout-guard (комментарный префикс) под `:icons: font`** (ветка `fix/callout-guard-icons-font`, 2026-06-19).
  В verbatim-блоке conum может стоять за line-comment'ом: `require 'a' # <1>`. Asciidoctor трактует префикс
  (`//`/`#`/`--`/`;;` + опц. один пробел) как *guard* конума: `sub_callouts` (substitutors.rb:920) матчит `CalloutSourceRx`
  (группа `((?://|#|--|;;) ?)?`) и кладёт её в атрибут `guard`; `convert_inline_callout` (html5.rb:1159) при text-иконках
  (default) **ре-вставляет** guard (`#{guard}<b class="conum">…`), при `:icons: font`/image — **отбрасывает**. У нас guard
  оставался в `Event::Text` перед `Event::CalloutRef` → для text-иконок случайно совпадал (gate так и проходил), но под
  `:icons: font` мы оставляли `# ` там, где asciidoctor его снимает. **Фикс:** guard вынесен из текста НА событие
  (`CalloutRef(u32)` → `CalloutRef { num, guard: CowStr }`), решение «снять/оставить» принимает рендерер (знает `:icons:`).
  Новый pure-хелпер `scanner::callout_guard_offset` (зеркало правила: один опц. пробел; два пробела `#  <1>` → не guard);
  сплит в `push_callout_events_resolved` (block.rs) — только для первого Standard-маркера (XmlComment `<!--N-->` не тронут,
  свой guard в рендерере). Рендерер (events.rs): `Some(marker)` (font/image) → конум без guard; `None` (text) → `guard`+conum
  (guard HTML-safe). +1 unit `test_callout_guard_offset`, +2 parser event-vector (`guard_captured`/`only_first_marker`),
  +2 html-фикстуры (`block/callout-guard-icons-font` + `…-text-icons`, эталоны `asciidoctor -e`). clippy `--workspace` 0,
  test --workspace зелёное (parser 614, html 478, html-фикстур 78→80). **Гейт 344/344 байт-в-байт** vs master (gate_check 0 diff —
  все gate-callout'ы text-иконочные → guard ре-вставляется идентично). **Frontier identical 217** (без изм.), clean-div 16;
  new-vs-base: `plain-text-diagrams` IMPROVED 4→1 (3 callout-диффа сняты; остаток `''`-префикс на `.Title` — др. класс),
  3 include-noise файла (syntax/writers-guide/0-1-4-released) NEUTRAL (guard снят верно — spot-check MATCH asciidoctor,
  но позиционный метрик маскирован нерезолвленными `include::`), **0 регрессий**. 10 CLI-проб MATCH asciidoctor 2.0.23
  (font снят / text сохранён / `#<1>` без пробела / `#  <1>` два пробела не-guard / multi-callout guard только на первом).
  **Вне scope (follow-up):** `[source,…,line-comment=%]`/`line-comment=` (кастомный/пустой коммент, `CalloutSourceRxMap`);
  `# <!--1-->` (line-comment перед XML-comment callout) — оба вне корпуса.
- [x] **F-AA. single-quoted значение именованного block-атрибута снимает кавычки (`[caption='']`)** (ветка
  `fix/single-quoted-named-attr`, 2026-06-20). Парсер именованных block-атрибутов (`BlockAttributes::parse`,
  attributes.rs) снимал только ДВОЙНЫЕ кавычки (`"…"`); single-quoted значение текло литералом. `[caption='']`
  → у нас `''The PlantUML…` вместо asciidoctor `The PlantUML…` (пустая caption = нет префикса). Asciidoctor
  снимает кавычки для ОБОИХ видов (различие — лишь в подстановках: single-quoted доп. получают normal subs).
  **Фикс (1 файл):** `strip_enclosing_quotes` обобщён на оба вида кавычек (byte-скан: первый байт `"`/`'`, последний
  равен первому; mismatched `'x"`, одиночные `'x`/`x'` — не трогаются), инлайновый double-only strip в `parse`
  заменён вызовом хелпера. После анкования `caption=''`→`""` рендерер сам даёт `CaptionPrefix::None` (уже было).
  Подстановки для single-quoted named-значений НЕ применяются (caption рендерится через html_escape; задокументировано
  как follow-up — вне корпуса). +2 unit (`…single_quoted_value_unquoted`, `strip_enclosing_quotes_both_forms`),
  +1 html-фикстура `block/image-caption-empty-single-quote` (oscon-зеркало plain-text-diagrams, эталон asciidoctor).
  clippy `--workspace` 0, test --workspace зелёное (parser 614→616, html-фикстур +1). **Гейт 344/344 байт-в-байт**
  vs master (gate_check 0 diff — single-quoted named block-атрибутов в гейте 0 вхождений → нулевой риск).
  **Frontier identical 217→218 (+1)**, clean-div 16→15: `plain-text-diagrams` ФЛИП 1→0 (единственный изменившийся
  new-vs-base файл — IMPROVED, **0 регрессий**; во всём frontier single-quoted named block-атрибут только этот один).
  7 CLI-проб MATCH asciidoctor 2.0.23 (empty/non-empty single caption, double-guard, single/double role, single id,
  table caption). **Вне scope (follow-up):** normal subs для single-quoted named-значений (`caption='*x*'` → bold);
  `pass:[]`/`pass:subs[]` форма значения; single-quoted значения в inline `[attrlist]`.
- [x] **F-AB. Отступленный section/heading-маркер → literal-параграф (колонка 0)** (ветка `feat/indented-section-literal`,
  2026-06-20). Класс из «recommended-practices→literal», явно перечисленный в рекомендации (стр. 214). Asciidoctor
  `SectionTitleRx` (`/^=={0,5}[ \t]+(\S.*?)[ \t]*$/`) якорится в колонке 0 — ведущий пробел/таб дисквалифицирует строку
  как заголовок, она падает в literal-параграф. Markdown-ATX (`## …`) — то же правило. Мы же `strip_section_marker`/
  `strip_markdown_heading` делали `line.trim_start()` ПЕРЕД подсчётом `=`/`#` → отступленный ` == Foo` ошибочно
  становился секцией (а `{counter}`/attr-ref в нём резолвились вместо литерала). **Чисто парсер, 1 файл (`scanner.rs`):**
  убран `trim_start()` в обеих функциях — работаем по `line` напрямую (ведущий пробел → `count_leading` даёт 0 → `None` →
  строка уходит в literal-параграф, который мы уже корректно рендерим). Субтрактивно: распознаём МЕНЬШЕ секций. Гарды
  continuation (`is_dlist/list_continuation_line` через `strip_any_section_marker`) — отступленный `==` теперь None
  (корректнее: asciidoctor никогда не считает его секцией). +6 scanner-ассертов (3 фн: section/markdown/any), обновлён
  тест `  ## Indented` (был `Some`→теперь `None`); +1 parser integration (`test_indented_section_marker_is_literal_paragraph`:
  точный event-вектор + markdown + col-0 регресс-гард); +1 html-фикстура `block/indented-section-marker-literal`
  (3 формы, эталон `asciidoctor -e`). clippy `--workspace` 0, test --workspace зелёное (parser integration 28→29,
  html-фикстур 81→82). **Гейт 344/344 байт-в-байт** vs master `a521930` (gate_check 0 diff — отступленных section-маркеров
  в гейте 0 вхождений → нулевой риск). **Frontier identical 218** (без флипа — counter-in-literal остался отдельным
  классом), clean-div 15: new-vs-base = **1 файл, IMPROVED, 0 регрессий** — `asciidoc-recommended-practices` **15→1**
  (14 section-диффов сняты; остаток = `{counter:cnt-step}` в literal-блоке, отдельный класс). 8 CLI-проб vs asciidoctor
  2.0.23 (indented `==`/`== ==`/`##`/tab; col-0 `==`/`##`/`=`-doctitle регресс-гарды; multi-line literal) MATCH.
  **Вне scope (follow-up):** attribute/counter subs ВНУТРИ literal-блока (asciidoctor `[:specialcharacters]` без
  `:attributes` → `{counter}` остаётся литералом; мы резолвим в препроцессоре независимо от контекста — известный остаток
  «счётчики в verbatim»); отступленные block-делимитеры/list-маркеры (тот же column-0-класс, не в текущих расхождениях).
- [x] **F-AC. Indented literal-параграф не резолвит counter/attr** (ветка `feat/indented-literal-no-counter`,
  2026-06-20). Прямой остаток F-AB (recommended-practices = 1: ` {counter:cnt-step}` после blank). Asciidoctor даёт
  literal-блокам только specialchars-subs (без `:attributes`), а препроцессор резолвил `{counter:...}`/attribute-entry/
  attrlist-ref независимо от блок-контекста. **Чисто парсер, 1 файл (`preprocessor.rs`):** новое состояние
  `at_boundary`/`in_indented_literal`; блок 4c ПЕРЕД expand_counters эмитит строки indented literal-параграфа untouched.
  Правило (probe-verified): indented (пробел/таб) непустая строка НА ГРАНИЦЕ БЛОКА (старт документа / после blank /
  после закрытия delimited/fence-блока) открывает literal-параграф до следующей blank; continuation (indented сразу за
  непустой строкой) — обычный текст, counter резолвится. `at_boundary` обновляется во всех точках эмиссии; reader-level
  строки (conditionals) его не трогают. +1 unit (7 кейсов: after-blank/continuation/attr-entry/after-fence/multi-line/
  col-0-guard/tab). clippy 0, test --workspace зелёное (parser lib 616→617). **Гейт 344/344 байт-в-байт** vs master
  `a7609cf` (gate_check 0 diff). **Frontier identical 218→219:** new-vs-base = **1 файл, IMPROVED 1→0, 0 регрессий**
  (`asciidoc-recommended-practices` флип в identical; во всём frontier изменился ровно этот файл). 10/10 CLI-проб vs
  asciidoctor 2.0.23 MATCH. **Вне scope (follow-up):** html-compat harness тестирует только `to_html` БЕЗ препроцессинга
  (counter резолвится лишь в CLI-пути) → фича покрыта препроцессор-юнитами; `[normal]`-стиль на indented (рендерер уже
  обрабатывает); indented attr-entry внутри списка как attached literal block (предсуществующее).
- [x] **F-AD. attrlist-constrained span откатывается от удвоенного маркера** (ветка `fix/attrlist-constrained-doubled-marker`,
  2026-06-20). Топ clean-div кандидат `static-awe` (41, весь diff — позиционный каскад от ОДНОГО корня). `[.path]__config/site.yml_`
  (attrlist + constrained `_`, **содержимое начинается с маркера**) у нас тёк литералом, у asciitoctor → `<em class="path">_config/site.yml</em>`.
  **Чисто парсер, 1 файл (`subst/quotes.rs`):** в `attrlist_constrained` убрана отбраковка удвоенного маркера
  (`bytes[marker_pos+1] == marker`). Зеркало asciitoctor pass-order: unconstrained-пасс (`__…__`) бежит ПЕРВЫМ и забирает
  все настоящие `[attr]__…__`; до constrained-пасса доживают только формы без закрывающего `__`, и `_(\S|\S.*?\S)_` матчит
  их с фолдингом второго маркера в content. Generic по маркеру (`*`/`` ` ``/`#`/`_`). Одиночная форма `[.path]_/images_`
  и закрытая `[.r]__closed__` (unconstrained) не тронуты. +1 unit (`attrlist_constrained_falls_back_from_doubled_marker`,
  4 маркера + closed-гард) + 1 html-фикстура `inline/attrlist-constrained-doubled-marker`. clippy 0, test --workspace
  зелёное (parser lib 617→618, html-compat фикстур +1). **Гейт 344/344 байт-в-байт** vs master `892ce90` (gate_check 0 diff;
  в гейте 0 файлов с `]__` → нулевой риск по конструкции). **Frontier identical 219→220:** new-vs-base = static-awe IMPROVED
  41→0 (флип в identical), clean-div 14→13, 0 регрессий. 5/5 CLI-проб (все маркеры) vs asciidoctor 2.0.23 MATCH.
  **Вне scope (follow-up):** БЕЗ attrlist (`__leading_` голый) — `constrained_open_close` (стр. 232) тоже отбраковывает
  удвоенный маркер; asciitoctor даёт `<em>_leading</em>`. Не в текущих расхождениях, шире по импакту (используется в
  escape/macros-арках) → отдельная задача с гейт-проверкой.
- [x] **F-AE. callout-маркеры `<N>` в markdown ```-fence + escape `\<N>`** (ветка `feat/markdown-fence-callouts`,
  смержена `1daa0eb`, 2026-06-21). Markdown ```-fence = verbatim source-блок (content_model `:verbatim` =
  `[:specialcharacters,:callouts]`), но `scan_markdown_code_fence` пушил сырой `Event::Text` без `resolve_callouts_in_lines`
  → `<N>` оставался литералом `&lt;N&gt;`. **Чисто парсер, 2 файла.** (1) `block.rs::scan_markdown_code_fence` — добавлен
  `resolve_callouts_in_lines` + `push_callout_events_resolved` (зеркало `scan_source_block`; `process_callouts` из
  VERBATIM-subs, `[subs=-callouts]` отключает; reindent НЕ добавлен — не было в исходной ветке). (2) **ROOT-CAUSE фикс
  escape** `scanner.rs::strip_callout_markers`: экранированный `\<N>`/`\<!--N-->` НЕ conum — backslash дропается, маркер
  литерал (asciidoctor `CalloutSourceRx` escape-группа). Был ПРЕДСУЩЕСТВУЮЩИЙ баг и в `scan_source_block` (`----`/`[source]`
  тоже неверно курлили `\<N>` в conum). Сигнатура → `Cow` (escape требует аллокации); `resolve_callouts_in_lines`
  обновлён (Borrowed делегирует напрямую, Owned реюзает аллокацию только при `len==len`). Escape слева от реального
  (`\<1> <2>`): run стопится на escape (правый conum, левый литерал). +1 parser event-vector
  (`test_markdown_fence_callouts`) + 5 escape-кейсов в `test_strip_callout_markers` + 1 html-фикстура
  `block/markdown-fence-callouts` (эталон `asciidoctor -s`). clippy 0, test --workspace зелёное (parser 618→**619**,
  html-compat +1). **Гейт 344/344 байт-в-байт** vs master `688a5bd` (в гейте 0 файлов с fence+callout И 0 с `\<N>` в
  verbatim → нулевой риск по конструкции). **Frontier identical 220 (стабильно), clean-div 13 (стабильно)** — 0 флипов в
  обе стороны; new-vs-base = 3 файла, ВСЕ верифицированы байт-в-байт vs asciidoctor 2.0.23: `enjoy` (clean) `<1>`→conum
  (59→227 — позиционный артефакт differ'а, единственное контент-изменение = 1 строка, остаток = implicit-table-header);
  `0-1-4` (noise) IMPROVED 5053→**4752** (escape + non-escaped conum + front-matter `--- <1>`); `writers-guide` (noise)
  2× `\<N>`→литерал (+10 позиционный артефакт, контент IMPROVED). 6/6 CLI-проб MATCH (std/autonum `<.>`/guard `# <1>`/
  escape `\<N>`/XML `\<!--N-->`/escape-в-listing). **Вне scope (follow-up):** pathological `<1> \<2>` (escaped справа от
  реального — run стопится, левый не conum, нет в корпусе); `enjoy` implicit-table-header (первый ряд → `<thead>`).
- [x] **F-AF. Пробел после quoted-значения разделяет атрибуты в attrlist** (ветка `feat/implicit-table-header`,
  смержена `c0fd8c5`, 2026-06-21). Запрос «начни следующую задачу из TODO.md». Кандидат `enjoy` (227) при триаже
  оказался НЕ implicit-table-header (как предполагала рекомендация 139-й), а другим классом: таблица имеет ЯВНЫЙ
  `[cols="1m,2" options="header"]`, где атрибуты разделены **пробелом**, не запятой. `split_respecting_quotes` делил
  только по запятой → `options="header"` склеивался с `cols`-значением и терялся → `<thead>` пропадал. **Правило
  asciidoctor** (зонды + исходник `attribute_list.rb`): закрывающая кавычка quoted-значения завершает атрибут, поэтому
  следующий пробел действует как разделитель; для UNQUOTED-значений и shorthand (`.cls`) значение тянется до запятой
  (`scan_to_delimiter`) — пробел НЕ разделяет (`[cols=2 options=header]` и `[.cls options="header"]` корректно БЕЗ
  header, MATCH asciidoctor). **Чисто парсер, 1 функция** (`attributes.rs::split_respecting_quotes`): флаг
  `after_close_quote` — пробел сразу после закрывающей кавычки даёт split; аддитивен к comma-split. Общая функция →
  покрывает block-, image- и link-attrlist. +1 unit (`whitespace_after_quote_splits_attributes`, 5 кейсов вкл. 2
  negative) +1 html-фикстура `block/table-header-space-attrlist` (эталон `asciidoctor -e`, байт-в-байт). clippy 0,
  test --workspace зелёное (parser 619→**620**, html-compat +1). **Гейт 344/344 байт-в-байт** vs master (`gate_check`
  0 diff; в гейте 0 attrlist'ов с риск-паттерном — единственное grep-совпадение лежало внутри `[source]`-листинга,
  не парсится). **Frontier identical 220→221 (+1):** `enjoy` флипнул в identical (`showdiff` пуст, байт-в-байт
  asciidoctor), clean-div 13→12, **0 регрессий** (new-vs-base = только enjoy). Зонды vs asciidoctor 2.0.23: space/comma/
  опции-порядок/multi-space/tab — все MATCH.
- [x] **F-AG. Вложенная AsciiDoc-ячейка (`a|`) наследует `:compat-mode:`/`:experimental:`** (ветка
  `feat/asciidoc-cell-inline-options`, смержена `ba9a11f`, ЗАПУШЕНО, ветка удалена, 2026-06-21). Запрос «начни следующую задачу из TODO.md». Топ clean-div
  `asciidoc-returns` (273) рекомендация помечала «compat-backtick passthrough» — **при триаже showdiff оказалось иначе**
  (урок [[feedback_frontier_triage]]): корень = `+gem+`/`+yum+` под `:compat-mode:` ВНУТРИ AsciiDoc-ячейки таблицы. Ячейка
  рендерилась через свежий `Parser::new(&raw)`, не наследовавший inline-влияющие doc-attrs внешнего документа → `+text+`
  трактовался как passthrough (литерал) вместо monospaced `<code>` (F-L работает только на top-level). Asciidoctor парсит
  AsciiDoc-ячейку как inner document, наследующий атрибуты родителя. **Фикс (2 файла):** (1) `parser.rs` — новый
  `Parser::new_with_inline_options(input, options)` seed'ит `InlineOptions` (остаётся изменяемым локальными attribute-entries
  ячейки); (2) `adoc-html/events.rs` (`CellStyle::AsciiDoc` арм) — выводит опции из накопленного `document_attrs`
  (`InlineOptions::from_attr_lookup`) и пробрасывает во вложенный cell-парсер. Покрывает compat-mode (`+text+`/`++text++`/
  `'em'`) И experimental (`kbd:`/`btn:`/`menu:`). **Гейт 344/344 байт-в-байт** vs master `3e4e63d` (нет активного
  compat/experimental в реальных header'ах гейта — единственный `:compat-mode:` лежит в listing-примере
  `literal-monospace.adoc`, не парсится → нулевой риск по конструкции). **Frontier identical 221 (стабильно)**; new-vs-base
  по всем 250 = **1 файл, IMPROVED, 0 регрессий**: `asciidoc-returns-to-github` **273→12** (остаток = footnote с attr-ref
  `{git-man-pages}`-значением-link-макросом, ДРУГОЙ класс — top-level, не cell). 7 CLI-проб vs asciidoctor 2.0.23 MATCH
  (compat `+`/`++`/`'em'`, experimental `kbd:`, 3 регресс-гарда: no-compat литерал, experimental-off литерал, локальный
  `:compat-mode!:` override). +1 parser (`seeds_compat_mode_inline_options`) +1 html (`test_asciidoc_cell_inherits_inline_options_html`,
  4 кейса) теста; clippy 0, test --workspace зелёное (parser 620→621, html unit 478→479).
  - **Follow-up (вне scope):** (1) `source-language` не наследуется вложенной ячейкой (block-уровень, не InlineOptions —
    отдельный seed `doc_attrs`; предсущ. F-J' follow-up, нет в cell-импакте этого файла). (2) Footnote attr-ref, чьё значение
    = link-макрос (`{git-man-pages}`=`http://…[text]`), не резолвится в footnote-тексте (остаток asciidoc-returns 12,
    top-level класс). (3) Полное наследование doc_attrs ячейкой (sectnums/counters) — асциидоктор наследует, мы только
    inline-флаги.
- [x] **F-AH. `\'` escape снимается только в word-flanked контексте (apostrophe-replacement)** (ветка
  `fix/escaped-apostrophe-non-word-flanked`, СМЕРЖЕНА в master `d57324a`, ЗАПУШЕНО, ветка удалена, 2026-06-21). Запрос «начни следующую задачу из TODO.md». Триаж мелких
  clean-div через showdiff (урок [[feedback_frontier_triage]]): mdbasics (3) = 2 класса (`\'`-escape + single-plus
  passthrough); footnote-кандидаты (asciidoclet/asciidoc-returns) — широкий/рискованный класс (порядок subst attributes
  ДО macros рекурсивно меняет границы footnote-скобки). Взят чистый `\'`-класс. **Корень:** asciidoctor снимает `\` перед
  `'` ТОЛЬКО где правило apostrophe-replacement `(\w)\\?'(?=\w)` реально матчит (апостроф между word-char) → `it\'s`→`it's`
  (литерал, не кручёный); вне этого `\'` остаётся литералом (`\'.text'`, `\'word'`, `\'>'`). Наш `subst/escape.rs`
  безусловно запечатывал `\'` как литерал `'` (как `\{`/`\[`/`\<`) → дропал `\` ВЕЗДЕ. **Фикс (чисто парсер,
  1 файл `subst/escape.rs`):** `'` вынесен из generic-арма; новый арм запечатывает `'` как `Literal` ТОЛЬКО при
  `bytes[i-1].is_ascii_alphanumeric() && bytes[i+2].is_ascii_alphanumeric()` (точное word-flank-зеркало
  apostrophe-replacement в `apply_typographic_replacements`); иначе → blanket-арм (backslash литерал). Compat-mode
  `\'em'` отрабатывает в `pass_constrained` (escape-арм quotes-прохода дропает `\` при формировании span'а); compat
  `it\'s` — word-flank-гейтом. **legacy `inline.rs` не тронут** (sequential — дефолт; legacy лишь fallback, escaped-входы
  не declinе'ятся) → документированная divergence. Тесты: +1 parser (`escaped_apostrophe_matches_asciidoctor`, 4
  non-word-flanked exact-vector + 2 word-flanked boundary-contrast), +1 html-фикстура (`inline/escaped-apostrophe`,
  эталон `asciidoctor -s`). clippy 0, test --workspace зелёное (parser lib 621→622, html unit 479). **Гейт 344/344
  байт-в-байт** vs master `db28ca6` (gate_check 0 diff; 4 gate-файла с `\'` — word-flanked / внутри listing → не изм.).
  **Frontier identical 221 (стабильно), clean-div 12:** new-vs-base ровно 1 файл, IMPROVED, 0 регрессий
  (`mdbasics` **3→1**; остаток = `+++`/`+-+` single-plus passthrough — отдельный класс). 6 CLI-проб vs asciidoctor 2.0.23 MATCH.
  - **Follow-up (вне scope):** (1) **single-plus `+++`→`+`** (mdbasics остаток: `+++` не матчится как single-plus
    passthrough с content `+`; `+-+`→`-` работает) — ВЫСОКИЙ риск гейта (`+` повсеместен), отдельная сессия. (2) legacy
    `inline.rs:1035` всё ещё дропает `\'` безусловно (fallback-only, не в корпус-импакте).

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

## АКТУАЛЬНО (2026-06-17, 103-я сессия): ХВОСТЫ ФАЗЫ 3 — doc-свип «gate» + оценка legacy-quotes (ветка `chore/subst-phase3-doc-sweep`, НЕ закоммичено)

Запрос «продолжи» → AskUserQuestion «закрыть хвосты Фазы 3». Корпус **344/344 байт-идентичны asciidoctor**
(перемерено на master `80a6b39`). Находка: native-конверсия passthrough-в-макросе = **0 корпусного выигрыша**
(таких punt-кейсов в 344 файлах нет) — остаётся опциональным edge-correctness.
- [x] **Doc-свип stale «gate»-терминологии** (8 файлов subst, СТРОГО doc-only): убраны present-tense ссылки на
  снятый differential-gate, заменены на decline-механизм/фактическое поведение. attributes/escape/passthrough/
  post_replacements/replacements/tokenize + macros (×8 present-tense + правка смысла macros.rs:283) + mod.rs (×2
  test-докстринга). ОСТАВЛЕНО историческое «the gate was removed / with the gate removed the engine ADOPTS» (mod.rs
  History, macros.rs:84) и «Gated on MACROS/QUOTES» (subs-flag).
- [x] **Оценка мёртвого legacy-quotes** → НИЧЕГО НЕ УДАЛЕНО (обосновано + проба бинарём). `parse_legacy` — живой
  fallback при !QUOTES / sentinel-байт / `flag_decline`; в двух последних legacy-QUOTES исполняется (вход несёт
  quote-маркеры). `a \++x++ and *bold*`→`<strong>`, `a \x01b *bold*`→`<strong>`. Удаление сломало бы fallback —
  заблокировано на native-конверсии + редизайне sentinel-байт-fallback. Gate-скаффолдинг уже снят в Фазе 3.
- Верификация: **clippy 0; test --workspace зелёное** (parser 558, html 434, compat 233, doc-tests); **parity 344/344**.

---

## (2026-06-17, 102-я сессия): РЕРАЙТ inline — **ФАЗА 3: СНЯТ GATE** (sequential-движок дефолт; СМЕРЖЕНА в master `80a6b39`)

Корпус **343 → 344** (FLIP `outline.adoc` 4813→0 vs asciidoctor). Фаза 2 (1-32/N) смержена в master (`45d0e56`).
**(Фаза 3) СМЕРЖЕНА** в master (`f3dd1e2` → merge `80a6b39`); session.md 102-й писался ДО коммита.
- [x] **Снять differential-gate, движок sequential — ДЕФОЛТ, env-флаги удалены.**
  - `inline.rs`: убран guard `subst::enabled()`; движок — первая попытка, `parse_legacy` — fallback при decline.
  - `subst/mod.rs`: удалены `enabled()`/`force()`/`env_true()` + env-флаги `ADOC_QUOTES_SEQUENTIAL`/`ADOC_SUBST_FORCE`.
    `try_parse` без гейта (оставлены decline-проверки: нет QUOTES / sentinel-байт).
- [x] **Явный per-construct decline→legacy** (инкрементальный safe-baseline вместо гейта; решение пользователя).
  - Shared `thread_local DECLINED` + `flag_decline()`/`take_decline()` в `subst/mod.rs`; `try_parse` → None при флаге.
  - `macros.rs`: `span_has_sentinel` (17 punt-сайтов) + 2 punt'а `try_link` → `flag_decline` (passthrough в target/
    label макроса → legacy, корректно). `passthrough.rs`: новый arm `\++`/`\+++` (отложенная форма) → `flag_decline`.
- [x] **Тесты:** 6 групп `try_parse(...).is_none()` (divergent-correct) → adopt; `link:[++pass++]` остаётся decline
    (punt). `inline.rs` хелперы `parse`/`parse_experimental` → `parse_legacy` (тестируют live legacy-fallback;
    11 escape/replacement тестов падали лишь на форме event-потока, HTML идентичен).
  - clippy 0; **test --workspace зелёное** (parser 558, html 434, html_output 41, **compat 233/233**, прочее).
    gate_check = 1 diff (outline). **blast: Identical 343→344, [FLIP] outline 4813→0, 0 REGR.**
- [ ] **СЛЕДУЮЩЕЕ (ОПЦИОНАЛЬНО, 0 корпусного выигрыша):** native-конверсия passthrough-в-макросе по семействам
    (seed-tags re-parse label xref/link/mailto; verbatim-строка image-alt/icon/UI; footnote parser↔renderer) —
    снимает punt'ы, делает new==asciidoctor на синтетических edge. Затем удаление legacy-quotes (требует ещё
    редизайн sentinel-байт-fallback). [Doc-свип «gate»-терминологии — СДЕЛАНО в 103-й, см. выше.]
  - [x] **ШАГ A: движок покрывает не-QUOTES subs-наборы, снят QUOTES-гейт** (ветка `refactor/engine-non-quotes-subs`,
    2026-06-22, off master `fe6c0d1`, НЕ закоммичена). Запрос «начни следующую задачу из TODO.md» + выбор пользователя
    «Удаление legacy (шаг A)» (единственная `- [ ]` опциональная; 3 native-фазы выше СМЕРЖЕНЫ в master). **Цель:**
    снять условие decline №1 (`try_parse → None` при `!subs.has(QUOTES)`), заставив движок обрабатывать subs-наборы
    БЕЗ quotes (`[subs=attributes]`, `[subs=+macros]`, `[subs=attributes+]`); инкрементальный шаг к удалению legacy
    (decline №2 sentinel-байт и №3 `DECLINED` остаются для будущих шагов B/C). **Корень дивергенции (verified
    чтением кода):** legacy escape катч-олл (inline.rs:1043-1050) снимает `\` с `\* \_ \` \# \^ \~ \{ \[ \< \\ \'`
    БЕЗУСЛОВНО; движок (`escape::run` blanket-арм) откладывал quote-marker escapes (`\*`/`\_`/`` \` ``/`\#`/`\^`/`\~`)
    в QUOTES-пасс → при QUOTES off этот пасс не бежит → движок держал `\*` литералом, legacy давал `*`. **Фикс
    (2 точки):** (A) `subst/escape.rs` — новый арм перед blanket-`else`, гейт `!quotes_on`, matches
    `\* \_ \` \# \^ \~ \'`, emit `literal_sentinel` (coalescing, как legacy катч-олл). **КРИТИЧНО:** гейт `!quotes_on`
    сохраняет QUOTES-on путь байт-в-байт — движок там НАМЕРЕННО расходится с legacy (фикс legacy-бага под asciidoctor:
    `\#nospan` сохраняет `\`, тесты `marker_escape_matches_asciidoctor`/`escaped_apostrophe_matches_asciidoctor`).
    (B) `subst/mod.rs:try_parse` — `!subs.has(QUOTES)` → `!subs.needs_inline_parsing()` (зеркало гарда вызывающего
    parser.rs:151; VERBATIM/NONE по-прежнему отступают). +doc-свип (mod.rs:5-8/80-92/187-189, escape.rs модульный +
    blanket комменты). **Тест:** `reproduces_legacy_on_non_quotes_subs` (mod.rs) — матрица 9 не-QUOTES subs-наборов ×
    ~50 входов, сравнение через `coalesce()`-нормализацию (смежные `Text` мёрджатся: legacy typographic-escape арм
    эмитит `\--`/`\(C)` отдельным `Text`, движок коалесцирует — HTML-идентично, предсуществующая структурная разница);
    ловит любой контент/тег/структуру, толерантен к HTML-нейтральному re-split. +хелперы `legacy_subs`/`pipeline_subs`;
    `try_parse_declines_without_quotes` расширен (VERBATIM/NONE→None, MACROS-only/ATTRIBUTES-only→Some). clippy 0,
    **test --workspace зелёное** (parser 638→639, html/compat неизменны). **Гейт 344/344 байт-в-байт** vs master
    `fe6c0d1` (`gate_check.py` 0 diff; non-QUOTES блоки `pass.adoc`/`attribute-entry-substitutions.adoc` теперь идут
    через движок). **Frontier 250 new-vs-base 0 diff** (`fsweep.py` в scratchpad). CLI-пробы: `[subs=attributes]`/
    `[subs=+macros]`/`[subs=normal]` все NEW==BASE. **Эмпирически доказано (флип арма off→gate+frontier всё ещё 0):**
    в корпусе/frontier 0 marker-escapes под non-QUOTES subs → фикс нужен НЕ для нейтральности, а для
    СОГЛАСОВАННОСТИ (движок единообразно воспроизводит legacy = чистый drop-in для будущего удаления legacy) + для
    сильного checked-in differential-теста (engine==legacy). **НАХОДКА (вне scope, см. ниже):** legacy (и движок)
    под non-QUOTES снимают `\*`/`\--`/`\link:`/`\{` там, где asciidoctor СОХРАНЯЕТ (нет соотв. subst-пасса) —
    предсуществующий legacy-баг, base имел его до шага A (0 регрессий). Коммит/merge --no-ff/push — ПО ЗАПРОСУ.
  - [x] **decline №3 (char-ref в URL link/autolink) — native + entity-preserving href в рендерере** (ветка
    `fix/char-ref-in-url`, 2026-06-22, off master `039105d`, НЕ закоммичена). Запрос «начни следующую задачу из TODO.md»
    + выбор пользователя: направление «удаление legacy → decline №3 (char-ref/`\++` native)» (шаг B/decline №2
    отвергнут — корректное снятие требует правки ГОРЯЧЕГО passthrough-пути ради 0x01/0x02, которых в реальных `.adoc`
    нет; 0 файлов гейта/frontier). **Находка (пробы asciidoctor 2.0.23 + текущий движок):** при char-ref в URL/alt движок
    (= legacy fallback, т.к. `flag_decline`) расходился с asciidoctor — `link:a&#167;b[text]` давал `href="a&amp;#167;b"`
    вместо `href="a&#167;b"`; корень — РЕНДЕРЕР двойно экранировал валидный entity (общий для engine и legacy баг).
    Документированный паттерн (`asciidoc-lang/.../link-macro.adoc:82` `link:My&#32;Documents/...` — но в listing-блоке,
    0 гейт-выигрыша). **Правило asciidoctor для href/alt:** экранировать `&`→`&amp;`, КРОМЕ начала валидного char-ref
    (`&#N;`/`&#xH;`/`&name;`/`&amp;`) → сохранить дословно. **Изменения (4 файла):** (1) `adoc-parser/src/lib.rs`
    публичный `char_ref_len(&[u8],usize)` (обёртка `subst::char_ref_len`, поднят `pub(super)`→`pub(crate)` + ре-экспорт
    в `subst/mod.rs`) — переиспользование валидатора рендерером. (2) `adoc-html/src/escape.rs` новый `html_escape_href`
    (entity-preserving) + `write_attr_href`; применены в `events.rs` (link href), `media.rs` (inline image src/alt/link-href).
    (3) `adoc-parser/src/subst/macros.rs` `reconstruct_link_target` — арм `CharRef{raw:true}`→splice (снимает punt'ы
    `try_link`:698 + autolink:1628/1657 на char-ref-в-URL; escaped `raw:false` остаётся punt); `build_link` bare-ветка →
    `push_bare_link_text` сегментирует видимый текст на `Text`/`InlinePassthrough` по границам char_ref_len (entity в
    видимом тексте bare-форм; no-char-ref → один `Text`, байт-в-байт). **Движок намеренно ≠ legacy на bare-форме**
    (сегментация vs один escaped `Text`) — стандартный паттерн parity; explicit-text форма == legacy по событиям
    (расхождение лишь в общем рендерере). **Тесты:** +1 parser (`native_char_ref_in_link_url` — no-punt + explicit==legacy
    + bare-сегментация + голый `&` как legacy) +4 html (`test_char_ref_in_link_url_href`/`_autolink_char_ref_href_and_text`/
    `_image_alt_and_src_char_ref`/`_bare_ampersand_in_url_still_escaped`). **Верификация (AIRTIGHT):** clippy 0
    (`--workspace`); test --workspace зелёное (parser 639→640, html 493→497); **гейт 344/344 байт-в-байт** vs `/tmp/adoc_base`
    (=master `039105d`; `gate_check.py` 0 diff — 0 корпусных исполняемых char-ref-в-URL/alt, голый `&`→`&amp;` неизменно);
    **frontier 250 new-vs-base 0 diff**; **13/13 CLI-проб == asciidoctor 2.0.23** (char-ref href/alt, документированный
    `My&#32;`, bare autolink href+текст, `&copy;`/`&#x2026;`, голый `&`→`&amp;`, `&amp;` сохранён, плюс регресс-гарды
    plain link/image). Коммит/merge --no-ff/push — ПО ЗАПРОСУ.
    - [x] **DEFERRED (a) — char-ref в VERBATIM-target + stem** (ветка `fix/char-ref-verbatim-target-stem`, 2026-06-22,
      off master `e493ba1`, НЕ закоммичена). Запрос «начни следующую задачу из TODO.md» + выбор пользователя «char-ref в
      verbatim-target + stem». **Находка (пробы asciidoctor 2.0.23 + текущий движок = legacy fallback, т.к. restore_verbatim
      пантил на char-ref):** char-ref в verbatim-контенте макроса расходился семейно-зависимо. **(1) Preserve-семейства**
      (image alt/target — уже корректны через `write_attr_href`; icon class, kbd/btn/menu, indexterm2) — движок ПЕРЕэкранировал
      (`a&amp;#167;b` vs asciidoctor `a&#167;b`); корень — РЕНДЕРЕР гнал verbatim-контент через `html_escape`/`html_escape_text`,
      двойно экранируя валидный entity (как до decline №3 для href/alt). **(2) stem (ОБРАТНОЕ направление)** — движок
      НЕДОэкранировал: `render_inline_stem`/`render_stem_block` пушили контент СЫРЫМ (вообще без escape) → `stem:[a < b & c]`
      давал `\$a < b & c\$` вместо asciidoctor `\$a &lt; b &amp; c\$` (предсуществующий баг ШИРЕ char-ref — никакие
      specialchars не экранировались). **Правило asciidoctor:** preserve-семейства держат survived char-ref дословно
      (already-formed entity); stem применяет `specialcharacters` (экранирует `<>&`, char-ref тоже → `&amp;#167;`).
      **Изменения (6 файлов):** (A) `adoc-html/escape.rs` — `html_escape_href`→`html_escape_preserving_refs` (rename, attr-flavor,
      escapes `"`) + новый `html_escape_text_preserving_refs` (text-flavor, без `"`); общее ядро `escape_preserving_refs(quotes)`.
      (B) `adoc-html/inline.rs` — stem inline+block: `html_escape_text` (specialchars); kbd/menu/icon-class/icon-literal-alt:
      `html_escape`→`html_escape_preserving_refs` (`"` байт-идентично). (C) `adoc-html/events.rs` + `lib.rs` — новый флаг
      `button_mode` (btn-контент через `html_escape_text_preserving_refs`); IndexTerm event → `html_escape_text_preserving_refs`.
      (D) `adoc-parser/subst/macros.rs` `restore_verbatim` — арм `CharRef{raw:true}`→splice (снят punt для ВСЕХ verbatim-
      семейств); escaped `raw:false` + structural по-прежнему пантят. **Движок==legacy на raw:true** (оба держат литеральный
      char-ref в verbatim-строке; рендерер решает preserve/re-escape). **Тесты:** +1 parser
      (`native_char_ref_in_verbatim_macros_reproduces_legacy` — 10 форм image/icon/indexterm2/stem/(((…)))/kbd/btn/menu ==legacy;
      + переписан `verbatim_macro_passthrough_reconstructed_natively`: char-ref splice+==legacy, escaped пантит) +2 html
      (`test_char_ref_in_verbatim_macros_html`, `test_stem_block_specialchars_escaped_html`; + переписан старый
      `test_stem_no_escape_html`→`test_stem_specialchars_escaped_html`, кодировал БАГ). clippy 0, **test --workspace зелёное**
      (parser 640→641, html 497→499, compat 233). **Гейт 344/344 байт-в-байт** vs master `e493ba1` (`gate_check.py` 0 diff;
      0 корпусных char-ref в verbatim-макросах). **Frontier 250 new-vs-base 0 diff** (`fsweep.py`; реальные menu char-ref-кейсы
      `menu:File[Save As&#8230;]`/`menu:Tools[…&gt;…]` сидят в `substitutions_test.rb` = не парсится как `.adoc`, но дали
      эталонный вывод). **18/18 CLI-проб == asciidoctor 2.0.23** (image alt/target, icon font/literal, kbd single/seq, btn, menu
      item/target, indexterm2, stem asciimath/latexmath/block/specialchars, + регресс-гарды plain/lone-`&`). **Вне scope
      (предсуществующее, документировано):** escaped `\&#…;` (raw:false) в string-capture семействах (kbd/btn/menu/icon) —
      asciidoctor снимает `\` и экранирует `&`, движок (legacy fallback + entity-preserving рендерер) держит `\` и сохраняет
      entity; синтетический edge, gate/frontier-нейтрально, неустраним без escape-pass-aware рендеринга (= DEFERRED (b)/(c));
      menu split-делимитер (`,`/`&gt;`) + font-caret под `:icons: font` — отдельные menu-баги (НЕ char-ref); anchor reftext
      (xref label) УЖЕ корректен. Коммит/merge --no-ff/push — ПО ЗАПРОСУ.
    - [ ] **DEFERRED (follow-up, отдельные задачи):** (b) escaped `\&#…;` (raw:false) в URL — остаётся punt (escape-семантика
      отлична); (c) `\++`/`\+++` (passthrough.rs:102) — asciidoctor сам непоследователен (ASG≠HTML), reproduce-legacy либо ждать.
  - [ ] **(asciidoctor-parity, отдельная связная задача, 0 корпусного выигрыша):** под non-QUOTES subs движок снимает
    `\`-escape БЕЗУСЛОВНО (через legacy-зеркальные арм'ы escape.rs: `\{`/`\[`/`\<`/typographic + новый `\*`-арм), тогда
    как asciidoctor снимает `\` ТОЛЬКО если соответствующий subst-пасс активен (`\{name}`→`{name}` лишь при
    ATTRIBUTES; `\*`/`\--`/`\link:` сохраняются без QUOTES/REPLACEMENTS/MACROS). Сделать per-subst-гейтинг escape целиком
    (не частично) → new==asciidoctor на синтетических non-QUOTES edge. Цена: рассогласование с legacy (legacy-баг),
    differential-тест шага A надо переписать на asciidoctor-expected векторы. Верифицировано пробой:
    `[subs=attributes]\n\*x*` → asciidoctor `\*x*`, наш движок/base `*x*`.
  - [x] **ФАЗА 1: seed-tags re-parse label (link/mailto/autolink/xref/`<<>>`)** (ветка
    `refactor/macro-native-sentinel`, 2026-06-21, 152-я). Sentinel в РЕ-ПАРСИМОМ лейбле макроса больше не пантит:
    `reparse_label` гоняет лейбл через `run_pipeline_seeded` (внутренний `Work.tags` = клон внешней таблицы → seeded
    sentinel'ы разрешаются против тех же passthrough/`Literal`/char-ref листьев, пассы их пропускают, внутренний
    tokenize восстанавливает — зеркало asciidoctor: placeholder выживает `subs.without(:macros)` и восстанавливается
    глобально). Реализация: `TagToken`/`PassPiece` derive `Clone`; `Work::with_tags`; `run_pipeline_seeded` +
    общий `run_pipeline_with`; `build_cross_reference`/`build_link`/`push_label` берут `seed: &[TagToken]`; matcher'ы
    `try_xref`/`try_cross_ref`/`try_mailto` получили `work: &Work`. Whole-span `span_has_sentinel`-punt в этих
    matcher'ах заменён на ТОЧЕЧНЫЙ punt: `target_has_sentinel` (id/url/email verbatim) + `attr_has_sentinel`
    (role/window/subject/body verbatim) — лейбл идёт в seeded-репарс. Поле `label` тега `CrossReference`
    десентинелизируется (рендер читает только `is_none()`, но убираем управляющие байты `\x01..\x02` из Cow-поля;
    no-sentinel байт-в-байт через fast-path). +1 parser (`reproduces_legacy_on_xref_label_seeded` — сравнение по
    модулю render-мёртвого поля) + расширен `reproduces_legacy_on_link_passthrough_url_inputs` (формы link/mailto/
    autolink label-passthrough == legacy ТОЧНО) +1 html (`test_macro_label_passthrough_seeded_reparse_html`). clippy
    0, test --workspace 1239 зелёных. **Гейт 344/344 байт-в-байт** + **frontier new-vs-base 0 diff (250)** =
    HTML-нейтрально по ПОСТРОЕНИЮ (native==legacy на этих формах; в корпусе их и нет). **10/10 CLI-проб == asciidoctor
    2.0.23** (link/xref/`<<>>`/mailto/autolink label с passthrough/escape/char-ref). Снято: 2 `span_has_sentinel`-punt
    (xref/mailto) + 2 label `flag_punt` (try_link/try_autolink).
  - [x] **ФАЗА 2: verbatim-реконструкция (image/icon/stem/kbd/btn/menu/quoted-menu/anchor/index-term)** (та же ветка,
    152-я). Sentinel в VERBATIM-контенте leaf-макроса больше не пантит: новый `restore_verbatim(work, content)` вклеивает
    защищённый контент passthrough'а и экранированный `Literal` обратно (= что оставляет глобальный restore asciidoctor),
    char-ref → punt (его verbatim-vs-escaped трактовка семейство-зависима, редок). Любой split по делимитеру (`,` для
    anchor/index, `>` для меню) идёт по ИСХОДНИКУ (sentinel не содержит делимитеров) → защищённый passthrough'ом делимитер
    остаётся внутри одной части; quoted-menu сегменты ре-парсятся seeded (`reparse_seeded`, MACROS ON). Все matcher'ы
    получили `work: &Work`; `span_has_sentinel`-punt снят с 9 семейств (остался ТОЛЬКО footnote). Исправляет реальные баги:
    `kbd:[++Ctrl++]` (база ломала в `<kbd></kbd>+<kbd></kbd>+...`), `image:i.png[++a b++]`→`alt="a b"` (база `alt="++a b++"`),
    `stem:[++x++]`, `btn:[++OK__x++]`. +1 parser (`verbatim_macro_passthrough_reconstructed_natively`, вкл. char-ref punt)
    +1 html (`test_verbatim_macro_passthrough_reconstruction_html`). clippy 0, test --workspace 1241 зелёных. **Гейт 344/344**
    + **frontier 0-diff (250)** — в корпусе 0 таких конструктов (только в `.rb` файле frontier, не парсится). **CLI-пробы ==
    asciidoctor 2.0.23** для passthrough/escape во всех семействах; остаточные дивергенции = ПРЕДСУЩЕСТВУЮЩИЕ рендерер-issues
    (escape `\*` в alt не снимается; URL-кодирование пробела в image-src `a%20b`; bibliography-anchor рендерит `[label]`
    инлайн vs asciidoctor `[<a>]`) — все НЕ связаны с passthrough, new ≥ base везде, 0 регрессий.
  - [x] **ФАЗА 3 (footnote) — СНЯТ последний span-sentinel punt** (ветка `refactor/footnote-native-events`, 2026-06-22,
    off master `358bfa6`, НЕ закоммичена). Запрос «начни следующую задачу из TODO.md» + выбор пользователя «делать Фазу 3
    footnote». **Дизайн — инкрементальный additive-вариант** (НЕ unified «Footnote всегда несёт события», как изначально
    предполагал TODO): новый `Event::FootnoteParsed { id, events }` эмитится native-движком ТОЛЬКО когда тело футноута несёт
    sentinel (passthrough/escape); общий путь (без sentinel) остаётся прежним `Event::Footnote { text }` + ре-парс рендерером
    — **байт-в-байт неизменен, нулевой риск общему случаю**. Legacy-движок не тронут (sentinel'ов не производит). **Корень
    решения:** рендерер `render_footnote_text` РЕ-ПАРСИТ сырой текст, поэтому тело с поднятыми passthrough-маркерами надо
    распарсить В ПАРСЕРЕ (до потери маркеров) и отдать готовые события. **Реализация (`subst/macros.rs::try_footnote`):**
    принимает `work/subs/options`; `restore_verbatim` для id (verbatim-anchor, fast-path для реального id без sentinel); при
    sentinel в теле — `collapse_footnote_newlines` (зеркало рендерер-склейки многострочного тела) + `reparse_seeded(&work.tags,
    full subs, MACROS ON)` → `FootnoteParsed`; иначе прежний `Event::Footnote`. Снят `span_has_sentinel`-punt (footnote был
    последним юзером → функция УДАЛЕНА как мёртвый код; 2 doc-ссылки на неё депортированы). Рендерер (`adoc-html/events.rs`):
    handler `FootnoteParsed` рендерит события напрямую (без ре-парса), `<sup>`-маркер вынесен в общий `emit_footnote_def`
    (переиспользован обоими handler'ами, байт-в-байт). `into_static`-arm + ASG-builder no-op-группа + 1 exhaustive-match.
    +1 parser (`footnote_with_sentinel_body_parses_natively` — `++…++`→Text, `+++…+++`→InlinePassthrough, named, `pass:[]`,
    typo-escape `\--`/`\(C)`, + общий путь неизменен) +1 html (`test_footnote_passthrough_body_native_html`). clippy 0,
    **test --workspace зелёное** (parser 637→638, html 492→493, compat 233). **Гейт 344/344 байт-в-байт** vs master `358bfa6`
    (`gate_check.py` 0 diff — в корпусе 0 footnote-с-passthrough). **Frontier 250 new-vs-base 0 diff** (7 footnote-файлов без
    таких конструктов). **7/7 CLI-проб == asciidoctor 2.0.23** (`++__x__++`/`+++<b>raw</b>+++`/named/plain/typo-escape/
    multiline+pass). **Реальный фикс (синтетический edge): `footnote:[pass:[<i>x</i>] y]`** — base даёт literal
    `pass:[&lt;i&gt;…]` (расходится с asciidoctor), new даёт `<i>x</i> y` == asciidoctor (именно «делает new==asciidoctor на
    синтетических edge»). **Вне scope (предсуществующее, new==base):** (1) порядок атрибутов в `<sup>`-маркере определения
    (`<a id=… class=…>` у asciidoctor vs `<a class=… id=…>` у нас) — рендерер, не связан с footnote; (2) footnote-скобки
    режут по первому `]` без nesting (`footnote:[a [nested]` → cut), когда внутр. `[…]` НЕ извлёкся в sentinel; (3)
    passthrough с внутренним `\n` в footnote — native сохраняет `\n` в leaf, punt-путь склеил бы (не в корпусе). **Остаток:**
    удаление мёртвого legacy-quotes-кода (требует редизайна sentinel-байт-fallback — отд. крупная задача). Коммит/merge
    в master --no-ff — ПО ЗАПРОСУ.

---

## (2026-06-17, 101-я сессия): РЕРАЙТ inline — Фаза 2 ПАРИТЕТ (32/N) `render_kbd_keys` сплит по `,` + trailing-delim (ветка `feat/subst-phase2-parity-32`)

Корпус неизменен **344/344** (рендерер-фикс корпус-невидим). Phase 2 (1-31/N) СМЕРЖЕНА в master
(`9c8fe5d`). **(32/N) НЕ закоммичена** (рабочее дерево; коммит/мерж/пуш — по запросу пользователя).
base-бинарь `/tmp/adoc_base` ПЕРЕСОБРАН чисто из master `9c8fe5d`.
- [x] **(32/N) `render_kbd_keys` паритет сплита** — документированный остаток с 25/N (предсуществующий БАГ
  рендерера: сплит ТОЛЬКО по `+`, не по `,`; trailing-delim давал пустые `<kbd></kbd>`). Завершает Фазу 2
  паритет: после 29/30/31 (attr-ref во всех target макросов) это последний документированный остаток-кандидат.
  - **Корень:** `render_kbd_keys` (adoc-html/src/inline.rs) сплитил `text.split('+')` → `kbd:[Ctrl,T]`
    давал `<kbd>Ctrl,T</kbd>`, `kbd:[Ctrl++]` → пустые ключи. Парсер УЖЕ эмитит сырой Text внутри `Keyboard`
    (фикс рендерер-only, как 27-31). Реальный алгоритм asciidoctor верифицирован по `substitutors.rb:357-369`
    + `html5.rb:1237` И эмпирически (`asciidoctor -s`).
  - **Фикс (рендерер, 1 файл inline.rs):** delim = первое вхождение `,` или `+` на char-позиции ≥1 (что раньше;
    leading-delim на поз.0 — литеральный ключ, `kbd:[+]`→`<kbd>+</kbd>`). Trailing-delim спец-случай
    (`Ctrl++`/`Ctrl,,`): chop + split + вернуть delim последнему trimmed-ключу. Per-key trim. Рендер ВСЕГДА
    join по `+` (1 ключ→`<kbd>X</kbd>`, иначе keyseq). `Vec<Cow>` (owned только для trailing-last). +import Cow.
  - **КЛЮЧЕВОЕ (first-delimiter-wins):** `kbd:[Ctrl, T+X]`→`<kbd>Ctrl</kbd>+<kbd>T+X</kbd>` (split ТОЛЬКО по
    запятой, внутр. `+` литерал). `kbd:[a, b ,]`→last `b,` (пробел перед хвостовым delim trim, потом append).
  - **Тест:** `test_kbd_comma_and_delimiter_parity_html` (tests.rs): comma=`+` / per-key trim / first-delim-wins /
    trailing `++`,`,,` / leading-delim literal / single trailing (`a+`) / ws-before-trailing.
  - clippy 0; test --workspace зелёное (html unit 433→434, parser 558, прочее неизменно). **gate_check toggle
    off/on 344/0** (airtight base≡new — единств. корпус kbd-с-запятой сидит ВНУТРИ `[source]`-листинга →
    verbatim, не парсится макросом). **blast_force Identical 344→344** (0 REGR). e2e (p32_kbd/edge/ws/more):
    ВСЕ формы (comma, trailing-delim, leading-delim, ws, html-escape) == `asciidoctor -s` байт-в-байт.

---

## (2026-06-17, 100-я сессия): РЕРАЙТ inline — Фаза 2 ПАРИТЕТ (31/N) attribute-ref в xref TARGET (ветка `feat/subst-phase2-parity-31`)

Корпус неизменен **344/344** (рендерер-фикс корпус-невидим). Phase 2 (1-30/N) СМЕРЖЕНА в master
(`b330983`). **(31/N) СМЕРЖЕНА в master `9c8fe5d`** (Merge `9c8fe5d`).
base-бинарь `/tmp/adoc_base` ПЕРЕСОБРАН чисто из master `b330983`.
- [x] **(31/N) attr-ref в xref TARGET** `xref:{rel}.adoc[]`/`xref:{frag}[]`/`<<{id}>>` → asciidoctor.
  Завершение «attr-ref во ВСЕХ target макросов»: документированный остаток 29/30 (xref `{rel}` target — отд.
  механизм xref-резолвера). Альтернатива render_kbd_keys (другая тема) отложена.
  - **Корень (те же, что 29/30):** macros-пасс ДО attributes → `{rel}`/`{frag}` доживают литералом в
    `Tag::CrossReference.target`. asciidoctor резолвит attributes ДО macros → `href="intro.html"`/`#section-one`,
    наш base `href="{rel}.html"`/`#{frag}`. **NB:** фикс рендерер-side (attr-refs эмитятся парсером литералом —
    docstring subst/attributes.rs).
  - **Фикс (рендерер, 1 файл inline.rs):** `start_cross_reference` — `let resolved = resolve_inline_attr_value(target);
    let target: &str = resolved.as_ref();` ДО is_interdoc_xref_target/interdoc_xref_href и сохранения internal
    href-placeholder/fallback. Резолвнутый target драйвит ВСЁ: interdoc/internal-классификацию, `.adoc`→`.html`,
    lookup id, bracketed fallback. Cow из аргумента (не self) → безопасно из `&mut self`.
  - **КЛЮЧЕВОЕ:** резолв ДО is_interdoc — `{rel}.adoc`→interdoc, `{frag}`→internal (natural-xref по резолвнутому
    id подхватывает заголовок секции). undefined→keep-literal (`{undef}.adoc`→`{undef}.html`, `{undef}`→`#{undef}`).
  - **Скоуп:** ВСЕ target макросов теперь резолвят attr-refs (link/image 29/30 + xref 31). ОТЛОЖЕНО (остаток):
    render_kbd_keys сплит по `,` (`kbd:[Ctrl,T]`→split; рендерер, pre-existing).
  - **Тест:** `test_attr_ref_in_xref_target_resolves` (html_output.rs): interdoc `{rel}.adoc` / interdoc+`#{frag}` /
    internal `{frag}`→href+fallback / резолв-id→реальная секция / angle `<<{secid}>>` / undefined keep-literal /
    no-sentinel-leak. Работает под ОБОИМИ движками (рендерер shared).
  - clippy 0; test --workspace зелёное (html_output 40→41). **gate_check toggle off/on 344/0** (airtight base≡new —
    единств. корпус-хит `xref:chain-{chapter}[]` сидит ВНУТРИ `[source]`-листинга → verbatim, не парсится макросом).
    **blast_force Identical 344→344** (0 REGR). e2e (p31_xref/p31_edge): все in-scope формы == asciidoctor
    байт-в-байт (единств. остаток-diff p31_edge — pre-existing trailing-blank после секции, есть и на base, НЕ про xref).

---

## (2026-06-17, 99-я сессия): РЕРАЙТ inline — Фаза 2 ПАРИТЕТ (30/N) attribute-ref в image alt + БЛОЧНЫЙ image target/alt/link (ветка `feat/subst-phase2-parity-30`)

Корпус неизменен **344/344** (рендерер-фикс корпус-невидим). Phase 2 (1-29/N) СМЕРЖЕНА в master
(`96f04ae`). **(30/N) НЕ закоммичена** (рабочее дерево; коммит/мерж/пуш — по запросу пользователя).
base-бинарь `/tmp/adoc_base` ПЕРЕСОБРАН чисто из master `96f04ae`.
- [x] **(30/N) attr-ref в image alt + БЛОЧНЫЙ image (target/alt/link href/auto-alt)** → asciidoctor.
  Завершение «image attr-ref паритета»: документированный остаток 29/N (блочный image + inline image `alt`).
  - **Корень (те же, что 29/N):** macros-пасс ДО attributes → `{p}`/`{a}` доживают литералом в полях
    `Tag::Image`/`BlockMeta`. **inline image** (29/N резолвил src/role/link, но НЕ `alt`, и auto-alt брал СЫРОЙ
    target → `image:{p}[]`→`alt="{p}"`); **block image** (`start_block_image`) НЕ резолвил НИЧЕГО
    (`src="img/{p}"`, `alt="{a}"`, `href="{u}"`). asciidoctor резолвит attributes ДО macros. **NB:** фикс
    рендерер-side (attr-refs эмитятся парсером литералом — docstring subst/attributes.rs).
  - **КЛЮЧЕВОЕ:** `auto_alt_from_target` ОБЯЗАН получать РЕЗОЛВНУТЫЙ target (asciidoctor `image::{p}[]`→`alt="tiger"`).
  - **Фикс (рендерер, 1 файл media.rs):** `start_block_image` — `let resolved_target = resolve_inline_attr_value(target)`
    ДО image_uri/auto-alt; link href, non-empty alt (`into_owned`), img-src + interactive `data`/fallback src
    через резолв. `start_inline_image` — лифт `resolved_target` (auto-alt из него), non-empty alt через резолв.
    `resolve_inline_attr_value` возвращает Cow из аргумента (не self) → безопасно из `&mut self`-метода.
  - **Скоуп:** ВСЕ поля inline+block image (target/alt/link href/auto-alt/interactive). ОТЛОЖЕНО (остаток):
    xref `{rel}` target (`xref:{rel}.adoc`→`intro.html`/`#intro`; xref-резолвер — отд. механизм); render_kbd_keys
    сплит по `,` (`kbd:[Ctrl,T]`→split; рендерер, pre-existing). inline `link:` уже покрыт 29/N.
  - **Тест:** `test_attr_ref_in_image_target_alt_link_resolves` (html_output.rs): block target+alt / block auto-alt
    из резолв-basename / block `link={u}`+target / inline alt / inline auto-alt / undefined target+alt keep-literal
    +imagesdir / no-sentinel-leak. Работает под ОБОИМИ движками (рендерер shared).
  - clippy 0; test --workspace зелёное (html_output 39→40). **gate_check toggle off/on 344/0** (airtight base≡new —
    единств. корпус-хиты `image::{imagesdir}/…`/`image::image.jpg[{half-width}]` сидят ВНУТРИ `[source]`-листингов
    → verbatim, не парсятся макросом). **blast_force Identical 344→344** (0 REGR). e2e (p30_full/p30_edge): все
    in-scope формы == asciidoctor байт-в-байт (abs/URL target → imagesdir не применяется; undefined keep-literal).

---

## (2026-06-17, 98-я сессия): РЕРАЙТ inline — Фаза 2 ПАРИТЕТ (29/N) attribute-ref в TARGET inline link/image (ветка `feat/subst-phase2-parity-29`)

Корпус неизменен **344/344** (рендерер-фикс корпус-невидим). Phase 2 (1-28/N) СМЕРЖЕНА в master
(`b8d95b7`). **(29/N) НЕ закоммичена** (рабочее дерево; коммит/мерж/пуш — по запросу пользователя).
base-бинарь `/tmp/adoc_base` ПЕРЕСОБРАН чисто из master `b8d95b7`.
- [x] **(29/N) attribute-ref в TARGET inline link/image** `link:{u}[…]`/`image:{p}[…]` → asciidoctor.
  Прямое продолжение 28/N (документированный остаток: «`href={u}` в target макроса НЕ резолвится»).
  - **Корень:** macros-пасс ДО attributes → `{u}` доживает литералом в `Tag::Link.url`/image `target`;
    арм Link (`write_attr(output,"href",url)`) и `start_inline_image` (`image_uri(target)` + `link=` href)
    писали target как есть, минуя `resolve_inline_attr_value`. asciidoctor резолвит attributes ДО macros →
    `href="https://example.com"`. **NB:** attr-refs не резолвятся в ПАРСЕРЕ ни одним движком (docstring
    subst/attributes.rs) — только в рендерере → фикс рендерер-side (как 27/28).
  - **Фикс (рендерер, 3 файла):** events.rs Link-арм — `href` через `resolve_inline_attr_value`; bare-ссылка
    (`link:{u}[]`) ставит флаг `bare_link_pending` (lib.rs struct) → Text-обработчик резолвит видимый текст
    (он повторяет target) тем же путём, что href; media.rs `start_inline_image` — `target` (ДО `image_uri`,
    т.е. до imagesdir) и `link=` href через `resolve_inline_attr_value`. undefined→литерал, no-`{`→borrow (no-op).
  - **Скоуп:** ТОЛЬКО inline `link:`/`image:` (target+bare-текст+image `link=`). ОТЛОЖЕНО (остаток): блочный
    image `image::{p}` (BlockMeta/write_meta_attrs — отд. механизм); xref `{rel}` target (xref-резолвер,
    отд. механизм — `xref:{rel}[]`→`docs/intro.html`); inline image `alt` attr-ref.
  - **Тест:** `test_attr_ref_in_link_and_image_target_resolves` (html_output.rs): link `home`, link+path
    `/issues`, image+imagesdir `img/tiger.png`, image `link=` href, bare `link:{u}[]` (href+текст), undefined
    `{undef}`, no-sentinel-leak. Работает под ОБОИМИ движками (рендерер shared) — `to_html` (legacy default).
  - clippy 0; test --workspace зелёное (html_output 38→39). **gate_check toggle off/on 344/0** (airtight
    base≡new — 0 файлов корпуса ставят `link:{`/`image:{` с defined-attr: единств. `{site-url}` undefined →
    литерал в обоих; блочный `image::{imagesdir}` вне inline-пути). **blast_force Identical 344→344** (0 REGR).
    e2e (p29/p29b/p29c/p29d): все in-scope формы == asciidoctor байт-в-байт (xref+блочный image — отложенный остаток).

---

## (2026-06-17, 97-я сессия): РЕРАЙТ inline — Фаза 2 ПАРИТЕТ (28/N) attribute-ref в named-role link/inline-image (ветка `feat/subst-phase2-parity-28`)

Корпус неизменен **344/344** (рендерер-фикс корпус-невидим). Phase 2 (1-27/N) уже СМЕРЖЕНА в master
(`3dd9b8e`). **(28/N) ЗАКОММИЧЕНА на ветке** (`3ecc059`), **ОЖИДАЕТ авторизации** на `git merge --no-ff`
+ `git push` + удаление ветки. base-бинарь `/tmp/adoc_base` ПЕРЕСОБРАН чисто из master `3dd9b8e`
(stash→build adoc-cli→cp→pop).
- [x] **(28/N) attribute-ref в named-role link/inline-image** `link:u[t,role={r}]`/`image:p[a,role={r}]`
  → asciidoctor. Прямое продолжение 27/N (там роли InlineSpan/Strong/Em/Mono научились резолвить attr-refs;
  собств. арм-рендереры Link/Image остались литералом `class="{r}"`).
  - **Корень:** macros-пасс ДО attributes → `{r}` доживает литералом в `Tag::Link.role`/`InlineImage.role`;
    арм Link (events.rs:630 `html_escape(output,r)`) и `start_inline_image` (media.rs:370 `push_str(r)`)
    писали роль как есть, минуя `resolve_inline_attr_value`. base==new gated (валидный HTML, но НЕ резолв).
  - **Фикс (рендерер, 3 строки):** `resolve_inline_attr_value` поднят до `pub(crate)` (inline.rs); Link-арм
    и `start_inline_image` гонят роль через него перед html_escape/push. defined→value, undefined→литерал
    (`attribute-missing=skip`), quoted мульти-роль (`role="{r} external"`)→резолв ref на месте.
  - **Скоуп:** ТОЛЬКО inline link + inline image (документированная задача). Блочный image (роль из `BlockMeta`,
    отд. механизм) и icon-макрос (pre-existing рендерер-расхождение font-icons) НЕ тронуты. `href={u}` в target
    макроса НЕ резолвится — ОТДЕЛЬНЫЙ pre-existing баг (attr-ref в target), вне скоупа.
  - **Тест:** `test_attr_ref_in_link_and_image_role_resolves` (html_output.rs): link defined `fancy`,
    image `image fancy`, undefined `{undef}`, quoted мульти-роль `fancy external`, no-sentinel-leak.
  - clippy 0; test --workspace зелёное (html_output 37→38). **gate_check toggle off/on 344/0** (airtight
    base≡new — рендерер shared обоими движками, но 0 файлов корпуса ставят `role={attr}`). **blast_force
    Identical 344→344** (0 REGR). e2e: класс link/image == asciidoctor байт-в-байт.

---

## (2026-06-17, 96-я сессия): РЕРАЙТ inline — Фаза 2 ПАРИТЕТ (27/N) attribute-ref в inline `[attrlist]` роли/id (ветка `feat/subst-phase2-parity-27`)

Корпус неизменен **344/344** (гейт держит). Phase 2 контентно ЗАВЕРШЕНА (FORCE==asciidoctor 344/344) →
пользователь выбрал (AskUserQuestion) **parity-харднинг** = охота на sub-file edge-кейсы new≠legacy.
Phase 2 (1-26/N) уже СМЕРЖЕНА в master (`91447d6`). **(27/N) ЗАКОММИЧЕНА на ветке, ОЖИДАЕТ авторизации**
на `git merge --no-ff` + `git push` + удаление ветки:
- [x] **(27/N) attribute-ref в inline `[attrlist]`** `[{role}]*x*`/`[.{role}]_y_`/`[#{id}]`z`` → asciidoctor.
  Был **класс B баг**: сырой сентинель `\x01N\x02` утекал в `class="^A0^B"` (сломанный HTML).
  - **Разведка:** фаззер pipeline vs legacy (~370 форм) нашёл 11 расхождений: 10 класс A (new==asciidoctor,
    legacy баг — улучшения, НЕ чинятся) + 1 класс B (этот баг).
  - **Корень:** наш движок гоняет `attributes` ДО `quotes` (лифтит `{role}` в сентинель) → `quotes`
    захватывает сентинель в роль → утечка. asciidoctor: `quotes` ДО `attributes`, роль держит литерал `{role}`,
    глобальный attr-пасс резолвит позже. Рендерер роли вообще не резолвил attr-refs.
  - **Парсер:** `desentinelize(tags, s)` (tokenize.rs) — сентинель→литерал-исходник (`AttrRef`→`{name}`+trailing,
    leaf-токены→текст, структурные→drop); `attrlist_unconstrained`/`attrlist_constrained` (quotes.rs) зовут его
    перед `parse_attrs`.
  - **Рендерер:** `resolve_inline_attr_value(&self,v)->Cow` (inline.rs, fast-path no-`{`→borrow, иначе
    `resolve_attr_refs_text`); `push_inline_id_class` стал методом `&self`, резолвит id/роли; events.rs
    Strong/Em/Mono `Self::`→`self.`, InlineSpan-арм схлопнут в метод.
  - **Гейт:** SHORTHAND `[.{a}]`/`[#{a}]` теперь БАЙТ-РАВНЫ legacy → **ADOPT** (корректны и в default-сборке);
    POSITIONAL `[{a}]` legacy не парсит → DECLINE → исправлены под FORCE. Корпус не затронут.
  - **Тесты:** `force_resolves_attr_ref_sentinel_in_inline_attrlist` (mod.rs, 5 векторов+no-leak+gate-decisions);
    `test_attr_ref_in_inline_role_resolves` (html_output.rs, defined/undefined/no-leak).
  - clippy 0; test --workspace зелёное (parser 557→558, html_output 36→37). **gate_check toggle off/on 344/0**
    (airtight). **blast_force 344→344** (0 REGR). e2e: класс `[{attr}]` == asciidoctor байт-в-байт.
- [x] **(28/N) Сиблинг-гап macro named-role attr-ref** `link:u[t,role={r}]`/`image:p[a,role={r}]` →
  СМЕРЖЕН/ЗАКОММИЧЕН на ветке `feat/subst-phase2-parity-28` (см. секцию 28/N ниже).
- **Дальше:** render_kbd_keys по `,`; класс-A улучшения (new уже корректен); **ФИНАЛ (Фаза 3): снять gate**.

---

## (2026-06-17, 95-я сессия): РЕРАЙТ inline — Фаза 2 (26/N) doubled-backslash escape (ветка `feat/subst-phase2-parity-26`)

Корпус неизменен **344/344** (гейт держит). Фаза 2 контентно завершена (FORCE 344/344) → задачу выбрал
пользователь (AskUserQuestion): **ПАРИТЕТ new≡legacy** на sub-file edge-кейсах = предусловие чистого снятия
гейта. Phase 2 (1-25/N) уже СМЕРЖЕНА в master (`f1e572f`). **(26/N) НЕ закоммичена** (рабочее дерево;
коммит/мерж по запросу пользователя):
- [x] **(26/N) doubled-backslash escape** `\\*bold*`→`\*bold*`, `\\pass:`→`\pass:`, `\\+plus+`→`\+plus+`,
  `\\^sup^`→`\^sup^` (и каскад N→N-1 `\`). Снят избыточный guard `(i==0 || bytes[i-1] != b'\\')` в 4 escape-армах.
  - **Разведка:** doubled-backslash сломан в ОБОИХ движках. asciidoctor: N backslash → (N-1)+конструкт-литерал
    (ровно ОДИН смежный `\` поглощается). Класс A (new корректнее legacy: `\\ bare`/`\\https`/`*x*--`) — улучшения;
    класс B (new ошибочен: `\\*marker*`/`\\pass:` мусор) — баги, чинятся.
  - **Корень:** guard отсекал второй `\` (смежный с конструктом) → escape не срабатывал → оба `\`+рендер. Guard
    ИЗБЫТОЧЕН — армы уже гейтятся на «конструкт сформировался бы» (`constrained_open_close`/`simple_pair_open_close`/
    `pass_escape_prefix_len`/`try_single_plus`). Снятие → escape на последнем `\`, каскад точен. Nested-защита
    БЕСПЛАТНА: `_\\*a*_`→`<em>\\*a*</em>` (внутренний `*a*` не закрывается перед `_` → None → escape не срабатывает).
  - **Файлы:** quotes.rs (pass_constrained:348, pass_simple_pair:~567), passthrough.rs (single-plus:63, pass:85).
- [x] **«добей» — macro/char-ref/index/double-plus doubled-backslash** (расширение по запросу пользователя):
  `\\image:`/`\\xref:`/`\\mailto:`/`\\link:foo.html`→`\image:…`, `\\&copy;`→`\&amp;copy;`, `\\((term))`→`\((term))`,
  `\\++pp++`→`++pp++`, `\\++*x*++`→`++<strong>x</strong>++`.
  - **escape.rs**: арм `Some(b'\\')` else-ветка `"\\\\"; i+=2` → `'\\'; i+=1` (advance-by-1: первый `\` литералом,
    ре-вход на втором → одиночный escape поглощает смежный `\`). Маркеры/bare `\\`/`\\https` не задеты.
  - **passthrough.rs**: новый арм `\\++…++` (перед single-plus) — оба `\` дропнуты, `++` как Macro-leaf, content RAW
    flows (зеркало `doubled_marker_escape` для `**`). Triple `\\+++` исключён (`bytes[i+4] != '+'`).
  - **mod.rs тест** `force_handles_doubled_backslash_macro_index_and_double_plus` (6 векторов + 4 gate-decline).
  - clippy 0; test --workspace зелёное (parser 555→557 (+2)). **gate_check toggle 344, 0** (airtight). **blast_force
    Identical 344→344** (0 REGR). e2e (p_fin/p_dp/p_q1/q2/p_db): NEW(FORCE)==asciidoctor байт-в-байт на ВСЕХ формах.
- [x] **Остаток — 2 патологии САМОГО asciidoctor — WONTFIX** (вне корпуса; asciidoctor сам непоследователен): `\\link:URL[text]` с автолинк-URL
  (asciidoctor рендерит ссылку; `\\link:foo.html` — литерал, как и у нас); `\\+++…+++` (triple-plus, asciidoctor сам
  непоследователен `+++…+`). Воспроизводить не стоит.
- **Дальше:** cross-span `*x*-- y` (new УЖЕ корректен — улучшение); render_kbd_keys по `,`; **ФИНАЛ (Фаза 3): снять gate**.

---

## (2026-06-17, 94-я сессия): РЕРАЙТ inline — Фаза 2 (25/N) UI-макросы kbd/btn/menu (ветка `feat/subst-phase2-next-25`)

Корпус неизменен **344/344** (гейт держит). Фаза 2 = перенести оставшиеся пассы пайплайна asciidoctor
в `adoc-parser/src/subst/`, довести FORCE-движок до байт-идентичности, в финале снять gate.
Phase 2 (1-24/N) уже СМЕРЖЕНА в master (`43c2eeb`). **(25/N) ЗАКОММИЧЕНА на ветке, ОЖИДАЕТ
авторизации** на `git merge --no-ff` + `git push` + удаление ветки:
- [x] **(25/N) UI-макросы `kbd:`/`btn:`/`menu:`** (macros-пасс, за `:experimental:`). Последний
  непортированный inline-конструкт; ПРЕДУСЛОВИЕ снятия гейта (Фаза 3). 0 FORCE-diff в корпусе (ни один
  файл не ставит `:experimental:`) → чисто аддитивно, проверяется парсер-тестами + e2e-пробой.
  - **Корень:** новый движок не имел kbd/btn/menu И не пробрасывал `InlineOptions` в `run_pipeline`.
    Легаси-дисп (inline.rs:497-543) гейтит арм на `options.experimental`; контент эмитится СЫРЫМ `Text`
    (рендерер `render_kbd_keys`/`menu_target` сам сплитит по `+`/`,`/`>`) — НЕ репарсится → leaf.
  - **Threading (4 файла):** `run_pipeline(text,subs,options)` + проброс `options` во ВСЕ inner-reparse
    (`macros::extract`, `passthrough::extract`→`pass_spec_events`, `push_label`, `build_link`,
    `build_cross_reference` + `try_xref/cross_ref/link/mailto/autolink/email`). Зеркалит легаси
    `push_macro_label`/inner `InlineState::new(_,_,self.options)` — experimental доходит до вложенных меток.
  - **Фикс (macros.rs):** `try_bracket_ui(prefix_len,open,close)` (kbd/btn, prefix 4, `[` сразу после,
    контент до первого `]`, пусто→decline) → `try_kbd`/`try_btn`; `try_menu` (prefix 5, target до `[` непуст,
    items до первого `]`, пусто-items→без Text) — зеркала `try_kbd_macro`/`try_btn_macro`/`try_menu_macro`
    (+`parse_bracket_macro`/`parse_target_bracket_macro`). `span_has_sentinel` guard на всех. Dispatch-армы
    `options.experimental && b'k'|b'b'|b'm'` перед `<<`-армом; при experimental OFF армы НЕ срабатывают →
    байты текут как текст (== asciidoctor, у которого макросы не зарегистрированы без `:experimental:`).
  - **mod.rs:** тест `reproduces_legacy_on_ui_macro_inputs` (хелперы `pipeline_exp`/`legacy_exp`): 27 кейсов
    `pipeline_exp==legacy_exp` (формы, span-вложенность, mid-word, invalid/empty) + 4 event-вектора +
    OFF-vs-ON контраст. Кейсы с passthrough/escape/char-ref внутри скобок ИСКЛЮЧЕНЫ (сентинель→decline→fallback).
  - clippy 0; test --workspace зелёное (parser 554→555). **gate_check toggle ON: 344, 0 различий base≡new**
    (airtight, нет утечки гейта); blast_toggle 343→343 (0 changed); **blast_force 344→344 (0 REGR/FLIP/FARTHER).**
  - **e2e-проба** (`:experimental:` doc, kbd+`,`/btn/menu+`>`/span): new(force) == legacy байт-в-байт; == asciidoctor
    КРОМЕ `kbd:[Ctrl,T]` — **предсуществующий БАГ РЕНДЕРЕРА** `render_kbd_keys` (сплит только по `+`, не по `,`;
    `adoc-html/src/inline.rs:71`), общий для обоих движков, вне зоны парсер-порта. → отдельная задача ниже.
- [x] **РЕНДЕРЕР: `render_kbd_keys` сплит по `,` (не только `+`)** — СДЕЛАНО в **(32/N)** (см. блок АКТУАЛЬНО).
  asciidoctor сплитит kbd-ключи по первому из `,`/`+` (поз.≥1), trailing-delim спец-случай, join всегда `+`.
  Семантика сверена по `substitutors.rb:357-369`+`html5.rb:1237` И эмпирически. gate airtight 344/0, blast 344→344.
- **Дальше — Фаза 2 контентно ЗАВЕРШЕНА, ВСЕ inline-макросы портированы.** cross-span остатки (gated, корректны):
  `*x*-- y` em-dash после close; escape `\\`/`\\pass:`/`\\https` doubled. **ФИНАЛ (Фаза 3): снять gate** (swap
  дефолта `ADOC_QUOTES_SEQUENTIAL`→on, удалить legacy quotes+edge-флаги) → outline флип в DEFAULT. UI-макросы
  больше не блокируют — предусловие выполнено.

---

## (2026-06-16, 93-я сессия): РЕРАЙТ inline — Фаза 2 (24/N) footnote-макрос → FORCE 344/344 (ветка `feat/subst-phase2-next-24`)

Корпус неизменен **344/344** (гейт держит). Фаза 2 = перенести оставшиеся пассы пайплайна asciidoctor
в `adoc-parser/src/subst/`, довести FORCE-движок до байт-идентичности, в финале снять gate.
Phase 2 (1-23/N) уже СМЕРЖЕНА в master (`0b58a8c`). **(24/N) ЗАКОММИЧЕНА на ветке, ОЖИДАЕТ
авторизации** на `git merge --no-ff` + `git push` + удаление ветки:
- [x] **(24/N) footnote-макрос** (macros-пасс). Выбор по nearmiss под FORCE: остались 2 Different —
  footnote.adoc(283) и include.adoc(375). footnote — «STATEFUL» из заметок ЛОЖНЫЙ страх: реестр/нумерация/
  foot-список в РЕНДЕРЕРЕ (`FootnoteRegistry`, R7-5), общем для движков; для inline footnote = чистый **leaf**.
  - **Корень:** legacy `try_footnote_macro` (inline.rs:1954) эмитит `Event::Footnote{id,text:raw}`/`FootnoteRef`
    — текст СЫРОЙ (НЕ репарсится; рендерер `render_footnotes` лишь `html_escape_text`). Новый движок не имел
    макроса → под FORCE `footnote:[...]` ЛИТЕРАЛ + нет `<div id="footnotes">` → каскад 283. legacy(default)
    рендерит footnote.adoc байт-в-байт == asciidoctor → порт new==legacy даёт и gate, и FORCE-флип.
  - **Фикс (macros.rs):** `try_footnote(src,start)` — зеркало legacy. id = всё до первого `[` (named) / None;
    content до первого `]`; `(Some id, пусто)`→`FootnoteRef`, иначе `Footnote`. `end=start+9+id_len+1+bracket_end`.
    `span_has_sentinel` guard. Dispatch `b'f'`+`starts_with("footnote:")` перед bare-`ftp://`-армом.
  - **mod.rs:** тест `reproduces_legacy_on_footnote_inputs` (15 кейсов + 3 event-вектора). Расхождения с
    `InlineFootnoteMacroRx` (id `[\p{Word}-]+`, `\]`-escape, `footnoteref:`) задокументированы — корпус не юзает.
  - clippy 0; test --workspace зелёное (parser 553→554). **gate_check toggle 344, 0 различий base≡new** (airtight).
    **FORCE: Identical 342→344 (+2 FLIP: footnote 283→0, include 375→0 — include имел 1 footnote в прозе),
    0 REGR, 0 FARTHER.** 🎯 **ВЕСЬ КОРПУС 344/344 под FORCE.**
- **Дальше — Фаза 2 контентно ЗАВЕРШЕНА.** Единственный непортированный inline-конструкт: UI kbd|btn|menu
  (нужен `InlineOptions.experimental` через pipeline + рекурсивные push_label — НЕ leaf). НИ ОДИН файл корпуса
  не ставит `:experimental:` → 0 FORCE-diff, НО для снятия гейта ОБЯЗАТЕЛЬНЫ. cross-span остатки (gated, корректны):
  `*x*-- y` em-dash после close; escape `\\`/`\\pass:`/`\\https` doubled. **ФИНАЛ (Фаза 3): снять gate** (swap
  дефолта, удалить legacy quotes+edge-флаги) → outline флип в DEFAULT; ПРЕДУСЛОВИЕ — портировать UI-макросы.

---

## (2026-06-16, 92-я сессия): РЕРАЙТ inline — Фаза 2 (23/N) bare-autolink внутри constrained-спана (ветка `feat/subst-phase2-next-23`)

Корпус неизменен **344/344** (гейт держит). Фаза 2 = перенести оставшиеся пассы пайплайна asciidoctor
в `adoc-parser/src/subst/`, довести FORCE-движок до байт-идентичности, в финале снять gate.
Phase 2 (1-22/N) уже СМЕРЖЕНА в master (`7647483`). **(23/N) ЗАКОММИЧЕНА на ветке, ОЖИДАЕТ
авторизации** на `git merge --no-ff` + `git push` + удаление ветки:
- [x] **(23/N) bare-autolink внутри constrained-спана** (macros-пасс). Выбор по nearmiss под FORCE: кластер
  cookbook java/index(183)+sdc(183)+java/monitoring(185) — общий корень monitoring.adoc:37
  `` `http://localhost:8080/actuator` `` (bare URL внутри monospace).
  - **Корень:** в asciidoctor subs-порядок quotes→macros: `` `url` `` → `<code>url</code>`, затем macros-пасс
    автолинкует URL (левая граница — `>` от `<code>`, правая — `<` от `</code>`). Наш движок гонит macros ДО
    quotes, поэтому: (1) `try_autolink` проверял только `at_autolink_boundary` (предыдущий байт), backtick не
    граница → НЕ линковал; (2) даже залинковав, URL-скан жадно съедал ещё-литеральный закрывающий маркер.
    legacy корректен (рекурсивный reparse контента спана).
  - **Фикс (macros.rs):** `escaped_autolink_boundary`→`autolink_open_boundary` (обёртка), новая
    `autolink_url_limit(work,bytes,i)` → `Some(close)` при открытии constrained-спана (стенд-ин для `<` от
    `</code>`), `Some(len)` при обычной границе, `None` иначе. `try_autolink` берёт `work`+лимит, скан URL
    капается `(limit-start).min(rest.len())`. Escaped-арм зовёт ту же обёртку (поведение неизменно).
  - **mod.rs**: `reproduces_legacy_on_bare_autolink_in_span_inputs` — 10 кейсов (URL=весь спан / `` ` ``*_#^~ /
    URL в середине спана / trailing-punct / `` word`url` `` не-спан). pipeline()==legacy() (гейт принимает).
  - **КЛЮЧЕВОЕ:** new теперь СОВПАДАЕТ с legacy → gate ПРИНИМАЕТ (вывод и так был верен через фоллбэк, 0 changed);
    FORCE==asciidoctor → флип. Замечание: bare-email внутри `<code>` — legacy И FORCE линкуют, asciidoctor нет
    (предсуществующее расхождение legacy↔asciidoctor, путь `try_email` не трогался).
- **Гейт:** gate_check toggle **344 файла, 0 различий base≡new** (airtight); blast_toggle 344→344.
  **FORCE: Identical 339→342 (+3 FLIP: monitoring 185→0, java/index 183→0, sdc 183→0), 0 REGR, 0 FARTHER, 0 паник.**
- clippy --workspace 0; test --workspace зелёное (parser 552→553, html 433, render-core 15, parsing-lab 1,
  integration 25, html-tests 6/6/1, html_output 36).
- **⚠ ИНФРА:** fmt-гейт `rust-quality-gates` ОТКЛЮЧЁН (решение пользователя 2026-06-16); clippy-гейт активен.
- **Дальше Фаза 2:** escape `\\` bare/`\\pass:`/`\\https` doubled (pre-existing); macros (N+) UI/footnote(STATEFUL);
  cross-span `*x*-- y` em-dash после close-span; **снять gate**. nearmiss на 342: footnote(283 — STATEFUL),
  include(375). bare-autolink-in-mono ВЫРОВНЕН.

### (АРХИВ 91-й) Phase 2 (22/N) constrained-close ищет валидный маркер циклом — СМЕРЖЕНА в master `7647483` (ветка `feat/subst-phase2-next-22`)

Phase 2 (1-21/N) уже СМЕРЖЕНА в master (`2d3ef70`). **(22/N) смержена `7647483`:**
- [x] **(22/N) constrained-close: цикл до валидного маркера** (quotes-пасс). Выбор по nearmiss под FORCE:
  ближайший outline.adoc (2 diff, len_delta=0) — финальный cross-span остаток, строка 877
  `` ** *SDR* … table `head` or `header; `foot` or `footer`. `` (7 бэктиков, нечёт).
  - **Корень:** asciidoctor `monospaced constrained: …`(\S|\S.*?\S)`(?![\p{Word}"'`])` — lazy `.*?` (где `.`
    матчит backtick) ПОГЛОЩАЕТ маркер, который не может закрыть (контент кончился бы пробелом / close за
    word/`"'`backtick) и ищет следующий валидный. Наш `find_closing_constrained`+один `constrained_close_ok`
    брал ПЕРВЫЙ маркер, при провале — отменял спан (как и legacy: легаси-баг, asciidoctor нет). На
    `` `header; `foot` ``: бэктик после `header; ` за пробелом → невалид → у нас спан рвётся (`` `header; `` лит +
    `<code>foot</code>`); asciidoctor поглощает → `<code>header; \`foot</code>`.
  - **Фикс (quotes.rs):** хелпер `find_valid_close_constrained(bytes,marker,content_start,mono_extra)` — цикл над
    `find_closing_constrained`, пропускающий кандидаты, проваливающие `constrained_close_ok` (`from=pos`, строго
    растёт). `constrained_open_close` и `attrlist_constrained` зовут его. Doc-модуль обновлён.
  - **mod.rs**: `constrained_close_search_matches_asciidoctor` — (1) raw `pipeline()`==asciidoctor event-векторы
    (пробел-перед-close ×2, word-после-close `` `a`b` ``→`<code>a`b</code>`); (2) `try_parse().is_none()` gate-decline;
    (3) regression-цикл `pipeline()==legacy()`.
  - **КЛЮЧЕВОЕ:** движок СТРОЖЕ/вернее legacy здесь → new≠legacy → gate-фоллбэк (0 changed); FORCE==asciidoctor.
- **Гейт:** gate_check toggle **344 файла, 0 различий base≡new** (airtight); blast_toggle 344→344.
  **FORCE: Identical 338→339 (+1 FLIP outline.adoc 2→0), 0 REGR, 0 FARTHER, 0 паник.** outline ПОЛНОСТЬЮ выровнен.
- clippy 0; test --workspace зелёное (parser 551→552, html 433, render-core 15, parsing-lab 1, integration 25,
  html-tests 6/6/1, html_output 36).
- **⚠ ИНФРА:** fmt-гейт `rust-quality-gates` (`pre-commit-cargo.sh`, `cargo fmt --all -- --check`) ОТКЛЮЧЁН (решение
  пользователя): сам master не проходит (1169 блоков, ~35 файлов hand-formatted, rustfmt 1.9.0). Fmt-блок
  закомментирован в ОБЕИХ копиях скрипта; clippy-гейт активен. `git --no-verify` не обходит (хук харнесса).
- **Дальше Фаза 2:** escape `\\` bare/`\\pass:`/`\\https` doubled (pre-existing); macros (N+) UI/footnote(STATEFUL);
  cross-span em-dash/autolink-in-mono; **снять gate**. nearmiss на 339: java/index(183), sdc(183),
  java/monitoring(185), footnote(283), include(375). outline под FORCE → 0.

### (АРХИВ 90-й) Phase 2 (21/N) xref-target первый-символ `[\p{Word}#/.:{]` — СМЕРЖЕНА в master `2d3ef70` (ветка `feat/subst-phase2-next-21`)

Корпус неизменен **343/344** (гейт держит). Фаза 2 = перенести оставшиеся пассы пайплайна asciidoctor
в `adoc-parser/src/subst/`, довести FORCE-движок до байт-идентичности, в финале снять gate → flip outline.
Phase 2 (1-20/N) уже СМЕРЖЕНА в master (`377850c`). **(21/N) ЗАКОММИЧЕНА на ветке, ОЖИДАЕТ авторизации** на
`git merge --no-ff` + `git push` + удаление ветки:
- [x] **(21/N) xref-target первый-символ `[\p{Word}#/.:{]`** (macros-пасс). Выбор по nearmiss под FORCE:
  ближайший page-breaks.adoc (88 diff, len_delta=4). Корень — строка 3 `` `<<<`, shown in <<ex-page-break>> ``:
  плоский macros-пасс (бежит ДО quotes) видел `<<` в `<<<` и жадно матчил `>>` из реального `<<ex-page-break>>`
  → ложный гигантский xref, каскад 88. Legacy корректен (рекурсия: backtick поглощает `<<<` ДО проверки `<<`).
  - **Корень:** `macros.rs::try_cross_ref` НЕ проверял первый символ (как и legacy). asciidoctor
    `InlineXrefMacroRx` (`ruby -e 'require"asciidoctor";puts Asciidoctor::InlineXrefMacroRx.source'`):
    `&lt;&lt;([\p{Word}#/.:{].*?)&gt;&gt;` — первый символ цели ОБЯЗАН `[\p{Word}#/.:{]` (НЕ `[\w":]`:
    `"` НЕвалиден; `.`/`/`/`{` валидны). Пробы: `<<<`→нет, `<<#foo>>`→да, `<<"a">>`→нет, `<<<b>>`→`&lt;`+`#b`.
  - **Фикс (macros.rs):** хелпер `xref_target_start_ok` (`is_alphanumeric()||c∈{_#/.:{}`); guard в `try_cross_ref`
    после empty-check. Диспетчер на `None` сдвигает на ОДИН `<` → `<<<b>>` матчит на втором `<<` (`<` лит + `#b`).
  - **mod.rs**: `reproduces_legacy_on_cross_reference_inputs` — `<< id , the label >>`(пробел, расходится)
    → `<<id , the label >>`; +reproduction `<<<`/`` `<<<` ``; +decline-блок (`try_parse==None`): пробел/`-`/`"`/`<<<b>>`.
  - **КЛЮЧЕВОЕ:** ограничение СТРОЖЕ legacy (legacy линкует `<< foo>>`/`<<"a">>`/`<<-y>>`) → на пермиссивных
    формах new≠legacy → gate-фоллбэк (0 changed); под FORCE new==asciidoctor (флипает page-breaks). 0 регрессий
    by construction (asciidoctor использует то же ограничение → движение К нему).
- **Гейт:** gate_check toggle **344 файла, 0 различий base≡new** (airtight); blast_toggle 343→343.
  **FORCE: Identical 337→338 (+1 FLIP page-breaks 88→0), 0 REGR, 0 FARTHER, 0 паник. БОНУС: outline 5487→2**
  (каскад `<<<` коллапсировал — cross-span финал-файл почти выровнен; остаток 2 diff @5268, строка 877
  `` `head` or `header; `foot` `` — alternating backtick boundary, отдельный корень).
- clippy 0; test --workspace зелёное (parser 551, html 433, render-core 15, parsing-lab 1, integration 25,
  html-tests 6/6/1, html_output 36). Пробы FORCE==asciidoctor байт-в-байт (`` x `<<<` y ``/`<<<b>>`/`<<#anchor>>`).
- **Дальше Фаза 2:** outline остаток (backtick boundary @877); escape `\\` bare/`\\pass:`/`\\https` doubled
  (pre-existing); macros (N+) UI/footnote(STATEFUL); cross-span em-dash/autolink-in-mono; снять gate.
  nearmiss на 338: java/index(183), sdc(183), java/monitoring(185), footnote(283), include(375).

### (АРХИВ 89-й) Phase 2 (20/N) escape `\((…))` index-term + `\\MM…MM` doubled-marker — СМЕРЖЕНА в master `377850c`

Phase 2 (1-19/N) в master (`408bae9`). **(20/N) смержена `377850c`:**
- [x] **(20/N) escape `\((…))` index-term shorthand + `\\MM…MM` doubled-marker** (escape-пасс). Выбор по
  nearmiss под FORCE: ближайший subs.adoc (86 diff, len_delta=-4). ДВА корня escape, оба уникальны в корпусе
  (`\((` только subs:20; `\\**` в outline:1487 — внутри passthrough `+…+`, escape-пасс не видит):
  - строка 20 `\((DD AND CC) OR (DD AND EE))` — 1 diff @36; строка 27 `\\__func__` — каскад +4 (@46+).
  - **Корень:** обе формы числились Deferred в `subst/escape.rs`. FORCE: `\((…))`→`\DD…EE`; `\\__func__`→
    `\\<em>func</em>`. **legacy** (`inline.rs::handle_inline_escape`): index-арм (~876) `\((`+`index_term_close`
    (первый `))`, жадно поглощает trailing `)`) → non-concealed `Text("((…))")`, `\(((…)))` concealed →
    `Text("("),IndexTerm,Text(")")`; doubled-арм (~917) `\\MM`+`find_closing_unconstrained` → `Text("MM")`,
    inner reparse, `Text("MM")` (оба `\` дропаются, контент течёт). **asciidoctor** == legacy (пробы p1/p2/p5).
  - **Фикс (escape.rs):** index-арм в `Some(m)` → `index_escape()` → `Macro`-leaf (своё событие, НЕ
    коалесцирующий `Literal` — как legacy отдельный Text; декл при sentinel в контенте). `Some(b'\\')`-арм →
    `doubled_marker_escape()` → open-`MM` `Macro`-leaf + RAW inner в `out` (течёт через char_refs/macros/
    attributes/quotes/replacements) + close-`MM` `Macro`-leaf; иначе старый `\\`-литерал fallback. Порт
    `index_term_close`/`find_closing_unconstrained` (над escape-буфером — passthrough уже сентинели, скип
    `sentinel_end`). КЛЮЧЕВОЕ: `Macro`-leaf даёт точное совпадение событий с legacy для plain-inner → гейт
    АДОПТИТ subs.adoc (не просто fallback). Обе формы перенесены Deferred→Handled в doc-модуле.
  - **mod.rs**: +тест `reproduces_legacy_on_index_and_doubled_marker_escape_inputs` (16 кейсов).
- **Гейт:** blast_toggle **343→343, 0 изменённых** (airtight). **FORCE: 336→337 (+1 FLIP subs.adoc 86→0,
  byte-identical 128=128), 0 REGR, 0 FARTHER, 0 паник.** Пробы p_esc: p1/p2/p5 asciidoctor==FORCE, гейт==legacy.
- clippy 0; test --workspace зелёное (parser 550→551, html 433, compat 233, render-core 15, integration 25).
- **Дальше Фаза 2:** escape `\\` bare / `\\pass:`/`\\https` doubled (pre-existing deferred); macros (N+) UI
  (experimental-проброс)/footnote(STATEFUL); cross-span close-span em-dash; A1 bare-autolink-in-mono; снять
  gate → flip outline. nearmiss на 337: page-breaks(88), java/index(183), footnote(283), include(375).

### (АРХИВ 88-й) Phase 2 (19/N) passthrough-защищённый URL `link:++url++[…]` — СМЕРЖЕНА в master `408bae9`

Phase 2 (1-18/N) в master (`ba712cd`). **(19/N) смержена `408bae9`:**
- [x] **(19/N) passthrough-защищённый URL `link:++url++[…]`** (macros-пасс). Выбор по nearmiss под FORCE:
  ближайший url.adoc (21 diff) — позиционный каскад от утечки строки 80
  `link:++https://example.org/?q=[a b]++[…]` (единственный «живой» `link:++…++` в корпусе; прочие 3 — в
  `[source]`/`----` verbatim).
  - **Корень:** `try_link` намеренно отклонял passthrough-в-URL — к macros-времени `++url++` уже
    passthrough-сентинель (`Passthrough{raw:false}`), старый `span_has_sentinel` → `None`. Под gate откат на
    legacy (корректный), под FORCE отката нет → `link:…[…]` течёт литералом.
  - **legacy** (`try_link_macro` ~2067): спец-кейс `rest.strip_prefix("++")` → URL = вербатим-текст между
    `++…++`, emitted `Cow::Borrowed`; label reparse `push_macro_label`. **asciidoctor:** общий
    `extract_passthroughs`→placeholder→`link:placeholder[…]`→restore (legacy = узкое приближение).
  - **Фикс (macros.rs):** `try_link` получил `work: &Work`; whole-span guard → точечный: sentinel в LABEL →
    decline (как раньше), URL-часть = ровно 1 passthrough-сентинель → `passthrough_url(work, url_part)`
    реконструирует вербатим-URL из пьес leaf'а (`Cow::Owned`), иначе `Cow::Borrowed(plain)`. КЛЮЧЕВОЕ:
    URL-сентинель РЕЗОЛВИТСЯ (не decline) — events == legacy → gate adopts; generalize `++`-only на любую
    passthrough-форму (= asciidoctor), прочие plus-формы под gate расходятся → fallback.
  - **mod.rs**: +тест `reproduces_legacy_on_link_passthrough_url_inputs` (8 reproduction-кейсов + gate-decline
    ассерта для passthrough-в-LABEL).
- **Гейт:** blast_toggle **343→343, 0 изменённых** (airtight). **FORCE: 335→336 (+1 FLIP url.adoc 21→0,
  diffone 216=216), 0 REGR, 0 FARTHER, 0 паник.** Пробы p_url1/p_url2 asciidoctor==FORCE байт-в-байт.
- clippy 0; test --workspace зелёное (parser 549→550, html 433, compat 233, render-core 15).
- **Дальше Фаза 2:** escape `\((…))` index-term shorthand/`\\`/`\\MM` doubled-marker; macros (6/N+) UI
  (experimental-проброс)/footnote(STATEFUL); cross-span close-span em-dash; A1 bare-autolink-in-mono; снять
  gate → flip outline. nearmiss на 336: subs(86), page-breaks(88), java/index(183), footnote(283), include(375).

### (АРХИВ 87-й) Phase 2 (18/N) spec'd pass-макрос `pass:SPEC[…]` — СМЕРЖЕНА в master `ba712cd`

Phase 2 (1-17/N) в master (`a16596b`). **(18/N) смержена `ba712cd`:**
- [x] **(18/N) spec'd pass-макрос `pass:SPEC[…]`** (passthrough-пасс). Выбор по nearmiss под FORCE: ближайший
  format-column-content.adoc (8 diff, утечка `pass:q[` вокруг `[cols=…]`). Один корень — флипает 4 файла.
  - **Корень:** `try_pass_macro` обрабатывал только bare `pass:[…]` (`spec_len==0`); spec'd `pass:SPEC[…]` был
    отложен (`return None`) → обёртка `pass:q[`/`]` течёт литералом, `#e#` ловит quotes-пасс → `pass:q[<mark>e</mark>]`.
  - **asciidoctor:** `extract_passthroughs` извлекает `pass:SPEC[text]` первым пассом, применяет к контенту
    spec'd субституции, запечатывает результат. **legacy** `push_pass_spec_content`: inner reparse + `Text→
    InlinePassthrough` когда `!set.has(SPECIALCHARS)`.
  - **Фикс (passthrough.rs):** dispatch-арм после bare-формы; `try_pass_spec_macro` (parse + spec_len!=0 +
    контент до первого `]`; `spec→pass_spec_to_subs`); `pass_spec_events` (inner `run_pipeline(content,set)` +
    `Text→InlinePassthrough` без SPECIALCHARS; пустой контент → `Vec::new()`). Seal через `macro_sentinel`
    (атомарный leaf). spec'd pass ВСТАВЛЯЕТ сентинель = flush-граница в обоих движках → mid-run/in-span
    совпадают (в отличие от escaped `\pass:` 17/N).
  - **mod.rs**: +тест `reproduces_legacy_on_pass_spec_macro_inputs` (19 кейсов).
- **Гейт:** blast_toggle **343→343, 0 изменённых** (airtight). **FORCE: 331→335 (+4 FLIP), 0 REGR, 0 FARTHER,
  0 паник.** Флипы: revision-line 220→0, pass 135→0, align-by-column 20→0, format-column-content 8→0 (диффы —
  позиционный каскад от утёкшей обёртки). 11 проб asciidoctor==FORCE==legacy.
- clippy 0; test --workspace зелёное (parser 548→549, html 433, compat 233, render-core 15).
- **edge-case (НЕ в корпусе):** `pass:r[--]` (`--` на краю контента) — inner run_pipeline гонит replacements
  `(true,true)` → em-dash, legacy `edges=false` → литерал → gate declines (safe). Нужен проброс edge-флага если встретится.
- **Дальше Фаза 2:** escape `\((…))` index-term shorthand/`\\`/`\\MM` doubled-marker; macros (6/N+) UI
  (experimental-проброс)/footnote(STATEFUL); cross-span close-span em-dash; A1 bare-autolink-in-mono; снять
  gate → flip outline. nearmiss на 335: url(21), subs(86), page-breaks(88), footnote(283), include(375).

### (АРХИВ 86-й) Phase 2 (17/N) em-dash на границе attr-ref/attr-set сентинеля — СМЕРЖЕНА в master `a16596b`

Корпус неизменен **343/344** (гейт держит). Phase 2 (1-16/N) в master (`e25f45c`). **(17/N) смержена `a16596b`:**
- [x] **(17/N) em-dash на границе attr-ref/attr-set сентинеля** (FORCE-fix replacements-пасса). Выбор по
  nearmiss под FORCE: ближайший 1-diff = subs-symbol-repl.adoc (`@125 exp='—' got='--'`, `|{empty}--{empty}`).
  - **Корень:** asciidoctor резолвит `{empty}`→"" в attributes ДО replacements → `--` на границах → spaced
    em-dash. Наш движок эмитит `AttributeReference`-сентинель (резолв в рендерере); replacements гонялся по
    ВСЕМУ буферу `(true,true)` → `--` окружён сентинель-байтами (как `<>`) → не граница → НЕТ em-dash под FORCE.
  - **Legacy:** attr-ref = отд. событие, разбивает Text-ран; край разрыва = граница (`start!=0`/`end<len`).
    Quote-контент — изолированный репарс `edges=false` → не граница. Различает контекст рекурсией, не типом.
  - **Фикс (subst/replacements.rs):** разбить буфер на сегменты по **AttrRef/AttrSet** сентинелям, применить
    `apply_typographic_replacements(seg,true,true)` посегментно, сентинели сохранить вербатим между сегментами.
    Края сегментов у attr-ref → реальные `^`/`$` → em-dash. Quote/passthrough/macro-сентинели остаются ВНУТРИ
    сегмента → не-граница цела (`*--*`/`*--*{empty}` → `--` литерал). Fast-path (нет attr-ref = весь буфер 1
    сегмент `(true,true)`) → байт-в-байт прежнее, без лишней аллокации. REUSE legacy-функции (split, не флаг —
    consume-логика `copy_end=i-1` дропнула бы сентинель-байт).
  - **mod.rs**: +тест `reproduces_legacy_on_attr_ref_emdash_boundary_inputs` (19 кейсов).
- **Гейт:** blast_toggle **343→343, 0 изменённых** (airtight). **FORCE: 330→331 (+1 FLIP subs-symbol-repl.adoc
  1→0 байт-в-байт 295=295), 0 REGR, 0 FARTHER, 0 паник.** subs.adoc 86 без изменений (его diffs — callouts).
- clippy 0 (pre-existing `concat!` в adoc-html lib-тесте — не мой файл); test --workspace зелёное (parser
  547→548, html 433, compat 233, render-core 15).
- **Cross-span НЕ в скоупе:** `*x*-- y` (legacy формирует em-dash после close-span; у нас close-сентинель не-
  граница) — pre-existing divergence (была и до фикса), gate DECLINES → 0 REGR. Дом — open vs close различение.
- **Дальше Фаза 2:** escape `\((…))` index-term shorthand (leaf, 1 кейс subs.adoc:20)/`\\`/`\\MM` doubled-marker;
  macros (6/N+) UI(experimental-проброс)/footnote(STATEFUL); cross-span close-span em-dash; A1 bare-autolink-
  in-mono; снять gate → flip outline. nearmiss на 331: format-column-content(8), align-by-column(20), url(21).

### (АРХИВ 85-й) Phase 2 (16/N) escaped autolink `\http://…` — СМЕРЖЕНА в master `e25f45c`

Корпус неизменен **343/344** (гейт держит). Phase 2 (1-15/N) в master (`05454b4`). **(16/N) смержена `e25f45c`:**
- [x] **(16/N) escaped autolink `\http://…`** (also https/ftp/irc; порт легаси `handle_inline_escape` арма).
  Выбор по nearmiss под FORCE: ближайший 1-diff = links.adoc (`` `\https://…` `` in-backtick, строка 17).
  - **Дом — MACROS-пасс** (НЕ escape.rs): порядок тут passthrough→escape→char_refs→**macros→attributes→
    quotes** (macros ДО quotes). Автолинк живёт в macros; escape.rs blanket оставляет `\http` литералом.
  - **Механизм (зеркало легаси):** дропнуть `\` (не копировать в out), ОСТАВИТЬ в src → autolink-арм на
    scheme видит `\` в src через `at_autolink_boundary` → не линкует → URL течёт литералом.
  - **Boundary `escaped_autolink_boundary`:** дроп когда (1) real boundary (start/ws/`<>()[];`) ИЛИ
    (2) `bytes[i-1]`=маркер `` ` ``/`*`/`_`/`#`/`^`/`~` И спан СФОРМИРУЕТСЯ (`quotes::constrained_open_close`/
    `simple_pair_open_close` reused, `pub(super)`). (2) = pre-quotes стенд-ин asciidoctor `>`-после-`<code>`.
    Спан-чек = нет over-fire на `a*\http`/`` a`\http `` (маркер не открывает спан).
  - **Исключены:** `\\http` (легаси дропает один `\`, asciidoctor оба → gate declines), mid-run после текста
    (`before \http` — легаси 2 Text, flat-движок 1 → событийно расходится, HTML идентичен, gate declines).
  - **mod.rs**: +тест `reproduces_legacy_on_autolink_escape_inputs` (15 кейсов). escape.rs: doc-перенос.
- **Гейт:** blast_toggle **343→343, 0 изменённых** (airtight). **FORCE: 329→330 (+1 FLIP links.adoc 1→0
  байт-в-байт), subs.adoc closer 87→86, 0 REGR, 0 FARTHER, 0 паник.**
- clippy 0, test --workspace зелёное (parser 546→547, html 433, compat 233, render-core 15).
- **A1 pre-existing gap (НЕ в скоупе):** bare autolink В монопространстве (`` `http://x` `` без `\`) не
  автолинкуется (macros до quotes) → `<code>http://x</code>` vs asciidoctor `<code><a>`. Дом — autolink ПОСЛЕ
  quotes (реордер). Крупнее одного арма.
- **Дальше Фаза 2:** escape `\((…))` index-term shorthand (leaf, 1 кейс subs.adoc:20)/`\\`/`\\MM`
  doubled-marker (дом — quote-пассы); A1 bare-autolink-in-mono; macros (6/N+) UI(experimental-проброс)/
  footnote(STATEFUL); снять gate → flip outline.

### (АРХИВ 84-й) Phase 2 (15/N) escape `\pass:SPEC[…]` — СМЕРЖЕНА в master `05454b4`

Корпус неизменен **343/344** (гейт держит). Фаза 2 = перенести оставшиеся пассы пайплайна asciidoctor
в `adoc-parser/src/subst/`, довести FORCE-движок до байт-идентичности, в финале снять gate → flip outline.
Phase 2 (1-14/N) уже СМЕРЖЕНА в master (`b9f03ff`). **(15/N) НЕ закоммичена, ОЖИДАЕТ авторизации** на
commit + `git merge --no-ff` + `git push` + удаление ветки:
- [x] **(15/N) escape `\pass:SPEC[…]`** (порт `pass_escape_prefix_len`). Выбор по nearmiss под FORCE: 3 из
  пяти 1-diff файлов — escaped pass в монопространстве (attribute-entry-substitutions/footnote/
  literal-monospace). Баг под FORCE: passthrough извлекал `pass:[]` → голый `\` → `<code>\</code>`.
  - **Дом — PASSTHROUGH-пасс** (НЕ escape.rs): escaped pass — НЕ плейн-литерал. Легаси дропает `\`,
    `pass:SPEC[` литерал, содержимое `[...]`+`]` ТЕЧЁТ через остальные subs (`\pass:c[*b*]`→
    `pass:c[<strong>b</strong>]`). Passthrough бежит первым и иначе извлёк бы `pass:[]` целиком.
  - **passthrough.rs**: новый арм в `extract()` (после `\+`, перед `+`-passthroughs): `b==\\` + guard
    `(i==0 || bytes[i-1]!=\\)` + `pass_escape_prefix_len(src,i+1)` → `out.push_str(pass:SPEC[)`, advance.
    Helper `pass_escape_prefix_len` (порт: `pass:`+опц.lowercase spec+`[`, len=`5+spec_len+1`; reuse
    `scanner::pass_spec_len`). `\\pass:` doubled ОТЛОЖЕН (guard гасит второй `\`; поведение не изменилось).
  - **gate-эквивалентность:** tokenize коалесцирует текст до сентинеля, `\pass:` сентинель не вставляет →
    непрерывный Text; легаси делает flush_text у `\`. Совпадает event-в-event на ГРАНИЦЕ flush (начало
    input/край спана). Все 3 корпус-кейса in-backtick (escape в начале спана) → gate ADOPTS. Bare mid-run
    (`before \pass:[x]`) → 1 vs 2 Text → decline+fallback (HTML идентичен). escape.rs: только doc-перенос.
  - **mod.rs**: +тест `reproduces_legacy_on_pass_escape_inputs` (14 boundary-кейсов; mid-run исключён).
- **Гейт:** blast_toggle **343→343, 0 изменённых** (airtight). **FORCE: 326→329 (+3 FLIP), 0 REGR, 0
  FARTHER, 0 паник.** Флипы: attribute-entry-substitutions/footnote/literal-monospace 1→0. 4 пробы байт-в-байт.
- clippy 0, test --workspace зелёное (parser 545→546, html 433, compat 233; 24 subst).
- **Дальше Фаза 2:** escape `\https://` autolink (in-backtick: seal URL-экстент+left-boundary, дом —
  macros-пасс после quotes; links.adoc расходится именно на in-backtick)/`\((` index-term shorthand (leaf,
  1 кейс)/`\\`/`\\MM` doubled-marker (дом — quote-пассы); macros (6/N+) UI(experimental-проброс)/
  footnote(STATEFUL); снять gate → flip outline.

### (АРХИВ 83-й) Phase 2 (14/N) escape `\macro` — СМЕРЖЕНА в master `b9f03ff`

Phase 2 (1-13/N) уже СМЕРЖЕНА в master (`9c6a219`). **(14/N) СМЕРЖЕНА** (`b9f03ff`):
- [x] **(14/N) escape `\name:target[…]`** (порт `inline_macro_escape_len`, дешёвый FORCE-win — escaped =
  литерал, impl-движок не нужен). Скоуп = только 12 именованных макросов; `\https://`/`\((`/`\pass:` —
  отд. code-path'ы, ОТЛОЖЕНЫ. **`subst/escape.rs`**:
  - `run(work)` → `run(work, subs)` (+`macros_on`); новый арм `mlen>0` в `Some(m)` (после typographic,
    перед cref — триггеры s/l/a/x/m/i/f не пересекаются с прочими): drop `\`, запечатать форму как leaf.
  - `macro_escape_len(bytes,p)` — порт легаси (12 NAMES, reject `name::` блок-форма, target=non-ws до `[`,
    скан до `]` inclusive; **+sentinel-guard**: TAG_LEAD/TAG_TAIL в скане → return 0 (decline, gate fb),
    т.к. escape бежит после passthrough и target/content мог уже содержать сентинель).
  - **некоалесцирующий leaf:** легаси-macro-escape пушит ОТДЕЛЬНЫЙ `Text` (`\link:u[t] more` → 2 события,
    эмпирич. подтверждено). → `macro_sentinel(vec![Text(Owned)])` (атомарный), НЕ `literal_sentinel`
    (коалесцирует → разошёлся бы на хвосте). Переиспользую `Macro`-токен.
  - **mod.rs**: вызов `escape::run(&mut work, subs)`; doc run_pipeline + escape.rs модуль-doc; +тест
    `reproduces_legacy_on_macro_escape_inputs` (24 кейса).
- **Гейт:** blast_toggle **343→343, 0 изменённых** (airtight). **FORCE: 325→326 (+1 FLIP), 0 REGR, 0
  FARTHER, 0 паник.** FLIP `user-index.adoc` 4→0 (diffone FORCE = 0 diffs, 294=294). База = master-движок
  `9c6a219` под FORCE (blast_* пробрасывает env во все subprocess'ы) → дельта чисто моя.
- clippy 0 (фикс explicit_auto_deref), test --workspace зелёное (parser 544→545, html 433; 23 subst).
- **Дальше Фаза 2:** escape `\https://` autolink (seal URL-экстент+boundary)/`\((` shorthand/`\pass:`/
  `\\`-doubled/doubled-marker; macros (6/N+) UI(experimental-проброс)/footnote(STATEFUL); снять gate →
  flip outline.

### (АРХИВ 82-й) Phase 2 (13/N) macros (5/N) anchor + index-term — СМЕРЖЕНА в master `9c6a219`

Phase 2 (1-12/N) уже СМЕРЖЕНА в master (`f1226b6`). **(13/N) СМЕРЖЕНА** (`9c6a219`):
- [x] **(13/N) macros (5/N) — anchor + index-term** (обе семьи leaf: id/label/term verbatim, БЕЗ
  re-parse, `subs` не нужен; объединены как icon+STEM в 12/N). **`subst/macros.rs`**:
  - **anchor:** `try_anchor` (`[[id]]`/`[[id,label]]` — comma: id.trim_end / label.trim_start, пустой
    label дроп), `try_bibliography_anchor` (`[[[id]]]` — оба .trim(), пустой label ОСТАЁТСЯ `Some` —
    отличие от plain, зеркалю донор), `try_anchor_macro` (`anchor:id[label]` — target `\S+`,
    whitespace/empty→decline). Диспетч `[`: фаерит ТОЛЬКО при следующем `[` (одиночный `[` = quotes
    attrlist, отд. пасс ПОЗЖЕ — macros не трогает); `[[[` (bib) проверяется ПЕРЕД `[[`.
  - **index-term:** `try_index_term` (`((…))`; `index_term_close` non-greedy `(.+?)\)\)(?!\))` — `))` с
    последующим `)` сползает на 1; форма по enclosing-скобкам: both→`ConcealedIndexTerm`, leading-only→
    `Text("(")`+flow `IndexTerm`, trailing-only→flow+`Text(")")`, neither→flow), `try_indexterm`
    (`indexterm:[p,s,t]`→Concealed), `try_indexterm2` (`indexterm2:[term]`→flow). Helper
    `concealed_index_term` (splitn(3,',') trim). Литеральный `(`/`)` = свой `Text` в Macro-leaf
    (токенайзер НЕ коалесцирует события macro-leaf → ≡ legacy flush_text+push).
  - **span_has_sentinel guard** на всех 6 (как image/icon/stem). Tag-поля `Cow::Owned` (== Borrowed, gate
    ОК). Failure-advance `+1` (как легаси; anchor_macro донор +7, но эквивалентно — внутри «anchor:» нет
    macro-старта).
  - **mod.rs**: doc обновлён; +2 теста `reproduces_legacy_on_anchor_inputs` (19) /
    `reproduces_legacy_on_index_term_inputs` (18).
- **Гейт:** blast_toggle **343→343, 0 изменённых файлов** (airtight). **FORCE (base чистый master):
  Identical 313→325 (+12 FLIP), 0 REGR, 0 паник.** Флипы: document-attributes-ref 5751→0, lexicon 498→0,
  span-cells 275→0, id 113→0, custom-attributes 82→0, bibliography 19→0 + add-columns/add-cells/pass-macro/
  CONTRIBUTING/release-and-progress/attribute-terms. **outline FARTHER 4797→5487** — ЭКСПЕКТЕД каскад
  (anchor/index извлекаются, прочие отложенные фичи расходятся; gate отклоняет).
- clippy 0, test --workspace зелёное (parser 544 = +2, html 433; 22 subst-теста).
- **Дальше Фаза 2:** macros (6/N+) UI(kbd/btn/menu — проброс experimental, НЕ leaf)/footnote(STATEFUL);
  escape `\macro` (порт `inline_macro_escape_len`); marker-escape ВНУТРИ пассов (отложено 8/N);
  снять gate → flip outline.

### (АРХИВ 81-й) Phase 2 (12/N) macros (4/N) leaf-макросы icon + STEM — СМЕРЖЕНА в master `f1226b6`
- [x] **(12/N) macros (4/N) — leaf-макросы icon + STEM** (оба leaf как image: НЕТ label re-parse, НЕТ
  options). **`subst/macros.rs`**:
- [x] **(12/N) macros (4/N) — leaf-макросы icon + STEM** (оба leaf как image: НЕТ label re-parse, НЕТ
  options). **`subst/macros.rs`**:
  - `try_icon` (зеркало `try_icon_macro`+`parse_target_bracket_macro`): триггер `i`+`icon:`, `name`→
    `Tag::Icon`, attrlist (если непуст)→ОДИН raw `Text`. Empty-name → decline; `]` = первый после `[`.
  - `try_stem(src,start,prefix_len,variant)` (зеркало `try_stem_macro`+`parse_bracket_macro_escaped`):
    3 написания `stem:[`/`latexmath:[`/`asciimath:[` (триггеры `s`/`l`/`a`, `[` сразу после `:` → target
    пуст), variant→`Tag::Stem`, content→ОДИН raw `Text`. **`\]`-escape**: `]` за `\` не закрывает, все
    `\]`→`]`. escape-пасс `\]` НЕ трогает (blanket-арм) → escaped-bracket доживает до macros.
  - **span_has_sentinel guard** на обоих (escape/passthrough/char-ref лифт изнутри → decline, gate
    fallback). Tag-поля `Cow::Owned` (== Borrowed легаси, gate ОК). НЕТ left-boundary (как у легаси):
    `prefixicon:x[]` матчит icon в середине слова — ОБА движка одинаково.
  - **mod.rs**: doc обновлён; +тест `reproduces_legacy_on_leaf_macro_inputs` (22 кейса).
- **Гейт:** blast_toggle **343→343, 0 изменённых файлов** (airtight). **FORCE (base чистый master):
  Identical 312→313, FLIP stem.adoc 5→0 (байт-в-байт с asciidoctor), 0 REGR, 0 FARTHER, 0 паник.**
  icon-macro.adoc НЕ флипнул — пред-существующее РЕНДЕРЕР-расхождение (font icons vs текст без
  `:icons: font`), к subst не относится; события icon ≡ legacy (unit-тест + 0 REGR).
- clippy 0, test --workspace зелёное (20 subst-тестов, +1 leaf).
- **UI kbd|btn|menu ОТЛОЖЕН:** нужен проброс `InlineOptions.experimental` через `run_pipeline`/`extract`
  + рекурсивные `push_label`/`build_cross_reference` (рефактор сигнатуры) — отд. инкремент. При
  experimental=off (дефолт) UI и так литерал → gate не страдает.
- **Дальше Фаза 2:** macros (5/N+) UI(kbd/btn/menu, см. выше)/anchor(`[[id]]`/`[[[bib]]]`)/
  index-term(`((…))`/`indexterm:`/`indexterm2:`)/footnote(STATEFUL); escape `\macro` (порт
  `inline_macro_escape_len`); marker-escape ВНУТРИ пассов (отложено 8/N); снять gate → flip outline.

### (АРХИВ 80-й) Phase 2 (11/N) macros (3/N) inline image — СМЕРЖЕНА в master `a0c56a6`
- [x] **(11/N) macros (3/N) — inline image** (`image:target[attrs]` → `InlineImage`-тег). Самый
  большой clean-FORCE кандидат (image.adoc 100 diff = чисто литеральный макрос). **`subst/macros.rs`**:
  - `try_image` (зеркало `try_inline_image`): find `[`/`]`, `bracket_end>bracket_start`, target БЕЗ
    empty-guard (донор без него — `image:[alt]` матчится), `parse_image_attrs(content)` (reuse из
    attributes.rs). **Leaf без label re-parse** — alt/width/height/align/float/link/role/title =
    строковые поля тега, не события; `Start(InlineImage)`+`End` строятся напрямую. Хелпер `owned(&str)`.
  - **Триггер `i` + guard `!starts_with("image::")`** (зеркало dispatch'а: `image::` = блочный, инлайн
    оставляет литералом). span-guard declined при сентинеле (`image:x[+raw+]`). Tag-поля = `Cow::Owned`
    (== Borrowed легаси, gate ОК). Failed → advance 1 байт.
  - **mod.rs**: doc обновлён; +тест `reproduces_legacy_on_image_inputs` (19 кейсов: bare/explicit/
    quoted alt, positional w/h, named attrs, attr-ref target литерал, `image::` блок-форма, invalid, span).
- **Гейт:** blast_toggle **343→343, 0 изменённых файлов** (airtight). **FORCE (base `3739f30`-legacy):
  Identical 311→312, FLIP image.adoc 100→0 (байт-в-байт с asciidoctor), closer id.adoc 115→113,
  0 REGR, 0 FARTHER.** force_nearmiss 33→32.
- clippy 0, test --workspace зелёное (parser 540→541, html 433, render-core 15), parsing-lab 233/233
  (19 subst-тестов, +1 image).

### (АРХИВ 79-й) Phase 2 (10/N) macros (2/N) link-семейство — СМЕРЖЕНА в master `3739f30`
- [x] **(10/N) macros (2/N) — link-семейство** (`link:url[attrs]`, `mailto:email[attrs]`, bare
  URL-автолинк `http`/`https`/`ftp`/`irc` (+`[label]` форма), email-автолинк `user@host.tld`). Reuse
  инфры cross-ref (`TagToken::Macro` + label-reparse). **`subst/macros.rs`**:
  - `try_link` (зеркало `try_link_macro`, plain-форма), `try_mailto` (зеркало `try_mailto_macro` +
    `?subject=&body=` query-encode через `url_encode_into` — открыл `pub(crate)` в inline.rs),
    `try_autolink` (зеркало `try_autolink` + `at_autolink_boundary` по предыдущему байту + trailing-punct
    strip + `[label]`), `try_email` (зеркало `try_email_autolink`: backward-scan local part, возвращает
    `local_start` → caller **truncate`out`** на уже-скопированную local part перед splice сентинеля).
  - **`build_link`/`push_label`** общие; `parse_link_attrs` (reuse из attributes.rs) для `^`-blank-window/
    role/window/nofollow/subject/body. Tag::Link поля = `Cow::Owned` (== Borrowed легаси, gate ОК).
  - **`link:++url++[]` (passthrough-in-URL) ОТЛОЖЕНА** — к macros-time `++url++` уже сентинель →
    span-guard declined → gate fb на legacy. Триггеры `l`/`m`/`h`/`f`/`i`/`@` добавлены в `extract`.
  - **mod.rs**: doc обновлён; +тест `reproduces_legacy_on_link_inputs` (40 кейсов).
- **Гейт:** blast_toggle **343→343, 0 изменённых файлов** (airtight). **FORCE (base `4a69fc7`-legacy):
  Identical 111→311 (+200 от cross-ref baseline 254; link даёт +57), 200 FLIP, 21 closer, 4 FARTHER,
  0 REGR.** FARTHER: 3 файла БЕЗ link-триггеров (каскад cross-ref `` `<<<` ``/отложенного, мой код их не
  трогал) + pass-macro.adoc (отложенный `link:++url++[]`). force_nearmiss 90→33.
- clippy 0, test --workspace зелёное (parser 539→540, html 433, render-core 15), parsing-lab 233/233
  (18 subst-тестов, +1 link).
- **Дальше Фаза 2:** macros (3/N+) image/footnote/icon/UI/stem/anchor(`[[id]]`)/index-term; escape
  `\macro` (порт `inline_macro_escape_len`); marker-escape ВНУТРИ пассов (отложено 8/N); снять gate → flip outline.

### (АРХИВ 78-й) Phase 2 (9/N) macros (1/N) cross-reference `xref:`+`<<>>` — СМЕРЖЕНА в master `4a69fc7`
- [x] **(9/N) macros (1/N) — cross-reference (`xref:target[label]` + `<<target>>`/`<<target,label>>`)** —
  первый срез macros (САМОЕ большое семейство, multi-session). Строит ВСЮ инфраструктуру macros:
  - **`TagToken::Macro(Vec<Event<'static>>)`** (tokenize.rs) — leaf держит Start+label-события+End как ОДНУ
    owned-последовательность, в tokenize разворачивается (flush+push клонов). АТОМАРЕН — НЕ участвует в
    cross-span overlap (в отличие от Open/Close span). `macro_sentinel`. `Event<'static>`→`Event<'a>` ковар.
  - **`subst/macros.rs`** (НОВЫЙ) — `extract(work,subs)` скан L→R skip-сентинели; `try_xref`/`try_cross_ref`
    (зеркала `try_xref_macro`/`try_cross_reference`: find `[`/`]`/`>>`, `#`-strip, comma trim, non-empty);
    `build_cross_reference` (Start + label + End); failed-макрос advance 1 байт (легаси `pos+=1`).
  - **Label re-parse = `push_macro_label`:** `super::run_pipeline(l, subs.without(MACROS))` (рекурсия конечна).
    Пустой label → `Text(target)` (no-label, рендереру для unlabeled-xref placeholder); `<<a,>>` пустой
    explicit → НЕТ событий (guard `!l.is_empty()`).
  - **Порядок: macros ПЕРЕД attributes** (легаси потребляет макрос целиком → `{x}` в target литерал, НЕ
    AttrRef), ПОСЛЕ passthrough/escape. **Sentinel-free span guard** (`xref:x[+raw+]` → declined → gate fb).
  - **mod.rs**: `mod macros;` + вызов после char_refs гейт MACROS; +тест
    `reproduces_legacy_on_cross_reference_inputs` (28 кейсов).
- **Гейт:** toggle-on **343→343, 0 изменённых файлов** (airtight; nav-кластер УЖЕ identical под base —
  легаси xref верен; gate адаптирует совпадающие события). **FORCE (base `713d62b`): Identical 111→254
  (+143!), 143 FLIP, 37 closer, 10 FARTHER, 0 REGR** (xref/`<<>>` пронизывают весь корпус). FARTHER —
  каскад отложенных макросов (faq.adoc: URL-макрос, не xref). force_nearmiss 233→90.
- clippy 0, test --workspace зелёное (parser 538→539, html 433, render-core 15), parsing-lab 233/233
  (17 subst-тестов, +1 cross-reference).
- **Дальше Фаза 2:** macros (2/N) link/url/mailto/autolink/email (донор `try_link_macro` 2059,
  `try_autolink` 2480, `parse_link_attrs`); (3/N+) image/footnote/icon/UI/stem/anchor(`[[id]]`)/index-term;
  escape `\macro` (порт `inline_macro_escape_len`); затем снять gate → flip outline.

### (АРХИВ 77-й) Phase 2 (8/N) marker escape + `\+` span-aware — СМЕРЖЕНА в master `8db6fcc` (коммит `f143140`)
- [x] **(8/N) escape маркеров `\*`/`\_`/`` \` ``/`\#`/`\^`/`\~` + `\+` (span-aware, ВНУТРИ пассов)** —
  модель asciidoctor `\\?`: backslash роняется ТОЛЬКО если на этой позиции образовался бы валидный
  спан/passthrough (drop → литеральные маркеры, контент проходит остальные пассы:
  `\*_em_*`→`*<em>em</em>*`, `\+*b*+`→`+<strong>b</strong>+`), иначе `\marker` остаётся литералом.
  `open_boundary` удовлетворяется самим `\` (работает `word\*bold*`). **quotes.rs**: хелперы
  `constrained_open_close`/`simple_pair_open_close` (детект-половина, без сентинелей); escape-ветки в
  `pass_constrained` (`* _ ` #`) и `pass_simple_pair` (`^ ~`); bare-ветки отрефакторены на хелперы.
  **passthrough.rs**: `\+…+` (валидный single-plus) → drop `\`, emit `+` литералом, контент через
  нормальные субституции. **escape.rs/mod.rs**: docs (маркеры/`\+` теперь в своих пассах, не deferred).
  **Гейт ВНУТРИ пассов обязателен** (НЕ escape-first): `\` внутри открытого спана (`` `\` ``) — контент,
  escape-first спрятал бы закрывающий маркер → рвал спан (`` (`\`) and (`]`) ``).
- **Исправлен баг legacy:** asciidoctor сохраняет `\#`/`\^`/`\~`/`\+` при отсутствии спана, legacy
  ОШИБОЧНО ронял backslash — новый движок матчит asciidoctor (FORCE closer; gate отклоняет на transition,
  т.к. событийно ≠ legacy). Также `\`+marker КОАЛЕСЦИРУЕТ литерал в один Text (legacy флашит порознь) —
  HTML тот же, события ≠ → gate fallback (безвреден).
- **ОТЛОЖЕНО:** doubled-формы (`\**`/`\##`/`\++`/`\+++`), `\\MM` — расходятся, редки (guard
  `bytes[i+2]!=marker`/`!='+'` и `bytes[i-1]!='\\'`). Пре-существующее: `a\*b*c` (asciidoctor роняет,
  движок сохраняет — close-assertion subtlety, НЕ тронуто моим изменением).
- **Гейт:** toggle-on **343→343, 0 изменённых файлов** (airtight, нулевая регрессия корпуса).
  **FORCE: subs.adoc 122→87** (closer −35, `\*Stars*`→`*Stars*` и др.), Identical 111→111, **0 flips**
  (ровно как прогноз 76-й: marker-escape без near-miss). **span-cells 271→274 (+3) — АРТЕФАКТ
  выравнивания ndiffs** (строка 18 `` (`\+`) `` теперь даёт корректный `<code>+</code>` == asciidoctor,
  но +4 токена в файле, рассинхронизированном неподдержанным `[[id]]`-anchor, сдвигают позиционную
  метрику; контент строго улучшен, проверено изолированно байт-в-байт). Единственная контент-правка —
  одна строка, в плюс. НЕ контент-регрессия.
- clippy 0, test --workspace зелёное (parser 536→538, html 433, render-core 15), parsing-lab 233/233
  (+2 subst-теста `reproduces_legacy_on_marker_escape_inputs`/`marker_escape_matches_asciidoctor`,
  renamed `escape_marker_left_untouched`→`marker_escape_does_not_tear_spans`; 18 subst всего).
- **Дальше:** **macros** (САМОЕ большое — 9-diff кластер ~13 nav-файлов = xref/link/image/footnote/
  icon/kbd/btn/menu/stem/anchor/autolink/email + `[[id]]` + `((…))`; leaf-токен `Vec<Event>`, RAW-
  подстроки label, recursive sub-pipeline; донор `handle_inline_macro` inline.rs ~416; анализ 74-й) →
  снять gate → flip outline.

### (АРХИВ 76-й) Phase 2 (7/N) char-refs — СМЕРЖЕНА в master `18aaacf`
(имя ветки историческое — пивот marker-escape→char-refs по FORCE-данным, как 74-я macros→curved-quotes):
- [x] **(7/N) char-refs survival + escape `\&#…;`** (`subst/char_refs.rs` НОВЫЙ) — валидный `&…;`
  (named/decimal/hex, порт `char_ref_len_at`) → `TagToken::CharRef{text,raw}`. **survival**
  (`&#167;`/`&copy;`, raw=true) → `InlinePassthrough` (рендерер НЕ экранирует `&`); **escape**
  (`\&#174;`, в escape.rs, raw=false) → `Text` (drop `\`, рендерер экранирует) + печать `&` от
  survival-пасса. ОБА — отдельные события (флашат pending, НЕ коалесцируют как `Literal`). Гейт
  `SPECIALCHARS && REPLACEMENTS` (= legacy `preserve_char_refs`). **char-refs ДО quotes** —
  иначе `#` внутри `&#167;` берётся mark-пассом за маркер (legacy потребляет ref атомарно).
  **Открытие:** `apply_typographic_replacements` выдаёт ЛИТЕРАЛЬНЫЕ Unicode (`\u{2019}`), не entity
  — потому source-char-refs требовали отдельного survival-пасса.
- **ОТЛОЖЕНО (известное расхождение, гейт ловит fallback'ом):** патологический `#&#167;#` (mark
  смежен с десятичным ref) — legacy хватает внутренний `#`, extract-first даёт целый ref. Редко.
- **Гейт:** toggle-on **343→343**, **0 регрессий, 0 flips** (airtight). **FORCE 108 → 111** raw-идентичных,
  **3 FLIP** (title-links 2→0, ui 1→0, toc-ref 1→0), 2 closer (subs-symbol-repl 3→1,
  document-attributes-ref 6434→6433), **0 FARTHER, 0 REGR** (airtight). Остаток subs-symbol-repl @125 =
  `{empty}--{empty}` (deferred attr-resolution, не char-ref).
- clippy 0, test --workspace зелёное (parser 535→536, html 433, render-core 15), parsing-lab 233/233
  (+1 subst-тест `reproduces_legacy_on_char_ref_inputs`, 16 subst всего).
- **Дальше:** escape маркеров+`\+` span-aware (без near-miss, нужен для финала) → macros (САМОЕ
  большое, 9-diff кластер ~13 nav-файлов = xref/link) → снять gate → flip outline.

### (АРХИВ 75-й) Phase 2 (6/N) non-marker escape `\` — СМЕРЖЕНА в master `3fdb828`
- [x] **(6/N) escape `\` (НЕ-маркерный)** (`subst/escape.rs` НОВЫЙ) — дроп backslash + `Literal`-сентинел
  для: типографики (`\--`/`\->`/`\=>`/`\<-`/`\<=`/`\...`/`\(C)`/`\(R)`/`\(TM)`), smart-quote openers
  (`\"`​`` ` ``/`\'`​`` ` ``), `\{`/`\[`/`\<`/`\'`. **tokenize.rs**: `TagToken::Literal(String)` —
  коалесцирующий токен (флашит предыдущий ран, СИДИТ pending → escaped char мержится со следующим в
  ОДИН Text, зеркалит legacy «дроп `\`, char в next flush»). Токенизатор переделан на `pending`-буфер
  (`flush_pending`); НЕ-Literal токены флашат перед эмитом (поведение прежних сохранено). **Порядок:
  passthrough ПЕРВЫМ, escape ВТОРЫМ** (как asciidoctor — passthrough защищает контент до субституций;
  `\` в буфере всегда top-level).
- **ОТЛОЖЕНО (контекстные баги escape-flat-scan, найдены blast'ом):** маркеры `\*`/`\_`/`` \` ``/`\#`/
  `\^`/`\~` (промах `` (`\`) `` — пряли закрывающий маркер span'а → рвали span; их `\\?` принадлежит
  ВНУТРЬ quote-пассов, span-aware); `\+` (требует `\\?` в passthrough-пассе); `\\`/macro escape.
  (char-ref escape `\&#…;` СДЕЛАН в 7/N.)
- **Гейт:** toggle-off **343→343** (legacy не тронут), toggle-on **343→343**, **0 регрессий, 0 flips**
  (airtight). **FORCE-верность 107 → 108** raw-идентичных, **unresolved-references FLIP 1→0**, 3 closer
  (bibliography 12→11, subs 123→122, subs-symbol-repl 4→3), **0 FARTHER, 0 REGR** (airtight).
- clippy 0, test --workspace зелёное (parser 533→535, html 433, render-core 15), parsing-lab 233/233
  (+2 subst-теста `reproduces_legacy_on_escape_inputs`/`escape_marker_left_untouched`, 15 subst всего).

### (АРХИВ 74-й) Phase 2 (5/N) curved smart quotes — в master `5421e0e`
**(5/N) СМЕРЖЕНА `--no-ff` + ЗАПУШЕНА (master `7995142`), ветка удалена; коммиты `7d13f7c`+`01391c2`:**
- [x] **(5/N) curved smart quotes `:double`/`:single`** (`subst/quotes.rs`) — пассы `"`​`…`​`"`→`“…”`
  и `'`​`…`​`'`→`‘…’`, идут ПОСЛЕ strong, ДО monospace (слот QUOTE_SUBS). Curly-символ — leaf-Text
  сентинель (`TagToken::SmartQuote{text,opening}`, литерал-char как legacy, НЕ `&#8220;`-entity) →
  раздельные Text-события (open/inner/close), как у legacy. **Leading-edge подавление** mono/em/mark:
  strong уже отработал (до пасса) → exempt; mono/em/mark идут после → constrained `` ` ``/`_`/`#` НЕ
  открывается, если непосредственно перед ним SmartQuote-OPEN сентинель (`smart_quote_leading_edge`
  + `sentinel_index_before`) — флаг legacy воспроизведён ПОРЯДКОМ пассов, не полем парсера.
  `find_smart_quote_close` скипает сентинели; нет open-boundary/attrlist (паритет с legacy
  `try_smart_quotes`, не широкий asciidoctor-regexp). Escaped `\"`​`…`​`"` — отложенный escape-пасс
  (gate отклоняет). Зеркало всех legacy smart-quote-тестов (double/single, formatting, double-backtick-
  literal, edge-emphasis/mark suppression, leading-only, nested, unclosed/empty).
- **Гейт:** toggle-off **343→343** (legacy не тронут), toggle-on **343→343**, **0 регрессий, 0 flips**
  (airtight). **FORCE-верность 97 → 107** raw-идентичных, **0 REGR, 0 FARTHER**, 10 FLIP, 11 closer
  (near-miss image-position 2→0 FLIP, unresolved-references 2→1 — остаток = отложенный `\{name}` escape).
- clippy 0, test --workspace зелёное (parser 532→533, html 433), parsing-lab 233/233 (+1 subst-тест
  `reproduces_legacy_on_smart_quote_inputs`, 12 subst всего).

### (АРХИВ 73-й) Phase 2 (4/N) attributes — в master `e9ce613`
- [x] **(4/N) attributes `{name}`/`{set:}` extract** (`subst/attributes.rs`) — после passthrough,
  ДО quotes. Legacy НЕ резолвит `{name}` — эмитит `Event::AttributeReference` (резолв в рендерере);
  порт: `{name}` (+опц. trailing `[brackets]`/`/path[brackets]`) → `TagToken::AttrRef` →
  `Event::AttributeReference{fallback:None}`; `{set:name:value}`/`{set:name}`/`{set:name!}` →
  `TagToken::AttrSet` → `Event::Attribute`. ДО quotes (вопреки порядку asciidoctor quotes→attributes):
  захваченный trailing-bracket защищён от quotes-attrlist (`{a}[.role]*x*` → AttrRef(trailing=`[.role]`)
  + голый strong, = legacy). Граничные байты для quotes идентичны (`{`/`}`/сентинел — non-word).
  **UTF-8 баг (поймал FORCE на кириллице):** fall-through copy через `utf8_char_len`+`push_str`
  (побайтовый `push(b as char)` трактует continuation-байт как Latin-1 → порча многобайтового).
  Зеркало `try_attribute_reference`/`try_inline_set` (`fallback` всегда None — нет синтаксиса
  `{name:fallback}`). Escape `\{` — отдельный отложенный пасс (gate отклоняет).
- **Гейт:** toggle-off 343, toggle-on 343, **0 регрессий, 0 flips** (airtight). **FORCE-верность
  92 → 97** raw-идентичных, **0 REGR** (ни один ранее-идеальный файл не сломан); 5 FLIP, 12 closer,
  2 FARTHER (footnote/replacements — каскады отложенных macros/char-refs, подтверждено diffone).
- clippy 0, test --workspace зелёное (parser 532, html 433), parsing-lab 233/233 (+1 subst-тест
  `reproduces_legacy_on_attribute_inputs`, 11 subst всего).

### (АРХИВ 72-й) Phase 2 (3/N) passthrough — в master `967dcd4`
- [x] **(3/N) passthrough extract/restore** (`subst/passthrough.rs`) — FIRST в пайплайне.
  `+++/++/+/bare pass:[]` → `TagToken::Passthrough(Vec<PassPiece{text,raw}>)` (sentinel), токенизатор
  восстанавливает: raw→InlinePassthrough, !raw→Text (escaped). Контракт зеркалит legacy try_*_passthrough
  (triple=raw, double=specialchars-Text, single=literal+embedded pass, bare pass=raw). `++++`→пусто
  (run_pipeline теперь зеркалит empty-guard parse_legacy → [Text(input)]). **Hard-break guard**:
  ` +\n` НЕ захватывается single-plus (legacy ест на пробеле post_replacements'ом ДО `+`) — гейт на
  POST_REPLACEMENTS, оставляется post_replacements-пассу (был корень каскада image-ref). Spec'd
  `pass:SPEC[]` (re-runs subs → non-leaf) + char-refs ОТЛОЖЕНЫ (gate отклоняет). Рефактор:
  `pass_spec_to_subs` вынесена в `pub(crate) fn` (DRY с single-plus embedded-pass).
- **Гейт:** toggle-off 343, toggle-on 343, **0 регрессий, 0 flips** (airtight). **FORCE-верность
  85 → 92** raw-идентичных, **0 REGR** (ни один ранее-идеальный файл не сломан); 6 FORCE-FARTHER —
  каскады отложенных macros/attr (footnote `<<xref>>`, id anchors, outline), gate их отклоняет.
- clippy 0, test --workspace зелёное (parser 531, html 433), parsing-lab 233/233 (+1 subst-тест, 10 всего).
- [x] **ОСТАЛОСЬ Фаза 2 — ЗАВЕРШЕНО** (Фаза 2 смержена 1-32/N + Фаза 3 снятие gate смержена; корпус 344/344;
  FORCE-методология устарела — gate снят). Историч. детали ниже: (FORCE 107/344; near-miss кандидаты на флип под FORCE: unresolved-references
  1-diff = escape `\{name}`; assignment-precedence/comments/discrete-headings/separating и др. уже
  FLIP'нули): **escape** `\*`/`\_`/`` \` ``/`\{`/`\pass:`/`\"` (донор `handle_inline_escape` inline.rs
  ~821; самый дешёвый следующий — 1-diff unresolved-references + escaped smart-quote `\"`​`…`​`"`),
  **char-refs** (`&#167;` survival — legacy InlinePassthrough при specialchars+replacements, донор
  `char_ref_len_at` inline.rs 1122), **macros** (link/xref/image/footnote/icon/kbd/btn/menu/stem/anchor/
  autolink/email + inline-anchor `[[id]]` + concealed index-term `((…))` — overhaul токенизатора, нужны
  leaf-токены с произвольными `Vec<Event>`; САМОЕ большое; донор `handle_inline_macro` inline.rs ~416;
  макросы несут RAW-подстроки (label/alt/footnote-текст), уже стёртые ранними пассами — нужен extract
  с recursive-вычислением label-событий, см. session.md 74-я анализ), spec'd `pass:SPEC[]`.
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

- [x] **`:icons:`-машинерия для callout-списков** — СДЕЛАНО (ВЕРИФИЦ. 2026-06-17: icons-image/icons-font байт-идентичны asciidoctor). (icons-image.adoc, icons-font.adoc,
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
- [x] **п.38 Ссылки: текст вместо URL** (25) — СДЕЛАНО (ВЕРИФИЦ. 2026-06-17: link-текст в dlist-term ==
  asciidoctor). Было: в description-list terms и сложных inline-контекстах не парсился текст ссылки.
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
- [x] **Точечные** — ВСЕ закрыты (ВЕРИФИЦ. 2026-06-17 vs asciidoctor; корпус 344/344). Остаточные вне-скоупные
  edge (предсуществующие, вне корпуса): п.26 `|a|b`-ячейка в одной строке; `:toc: left/right` в embeddable → `toc2`.
  - [x] п.17 (`[.line-through]#`→`<del>`, `#`→`<mark>`; inline-роль на `_`/`*`/`` ` `` в п.13/16) — MATCH
  - [x] п.24 (точки в id секций) — MATCH
  - [x] п.25 (audio/video attrs) — MATCH
  - [x] п.26 (frame/grid классы) — MATCH (отдельный edge `|a|b`-ячейки вне корпуса, не относится к frame/grid)
  - [x] п.36 (`{counter}` в таблицах) — MATCH
  - [x] п.29 `kbd:` / п.39 `btn:`/`menu:` — СДЕЛАНЫ (гейтинг за `:experimental:`)
  - [x] п.20 (`[[id,reftext]]`) — СДЕЛАНО (ветка `feat/block-anchor-reftext`): block-anchor reftext + named
    `[reftext=…]` регистрируются для `<<id>>`; explicit reftext > block title. Корень: attributes.rs дропал
    label после запятой → теперь в `named["reftext"]`; рендерер регистрирует в `anchor_reftexts`. +4 теста, parity 344/344
  - [x] п.28 (`toc::[]` macro) — СДЕЛАНО (ветка `feat/toc-macro-mode`): новый `Event::TocMacro`; рендерит TOC
    только при `:toc: macro` (сентинель-плейсхолдер на месте макроса → preamble-обёртка + порядок), `toctitle
    class="title"`; иначе `<!-- toc disabled -->` (asciidoctor-faithful). +3 теста, parity 344/344. (Предсуществующее
    вне скоупа: `:toc: left/right` в embeddable-режиме даёт `toc2` вместо `toc` — есть и на master.)

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
