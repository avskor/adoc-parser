# Session context

## Сессия (2026-06-23) — F-BC: нумерация спец-секций (special-section) + abstract-в-book → sections.adoc identical

Запрос «начни следующую задачу». master `e7abf3a` (F-BB смержен; прошлый session.md устарел — говорил
«не закоммичено», фактически смержено `1076083`/`e7abf3a`). Триаж: frontier исчерпан (228 identical; остаток —
manpage-backend out-of-scope, multi-special-ex 87 multi-root, CHANGELOG=substitution-ordering replacements-before-macros,
migration={asciidoctor-version} intrinsic — оба небезопасны). adoc2docx ближайший — `sections.adoc` (20 diff).
Ветка `feat/special-section-numbering` (изменения НЕ закоммичены — паттерн F-*: коммит ПО ЗАПРОСУ).

### Сделано — 2 связанных корня (sections.adoc: 20 diff → 0, identical)
Изучено правило asciidoctor (parser.rb `initialize_section` + section.rb:49 `@special = parent.special` +
abstract_block.rb `assign_numeral` + html5 `convert_section`):

**Корень A — наследование non-numbered у потомков спец-секций.** Asciidoctor наследует `special` детям
(`@special = parent.special`); спец-секция нумеруется только если appendix (или `sectnums=all`, мы не трекаем).
Значит non-appendix спец-секция (preface/colophon/dedication/glossary/bibliography/index) И ВЕСЬ её subtree —
unnumbered: `=== Sub` под `[preface]` без номера, хотя своего стиля не несёт. Раньше мы нумеровали («1.1.»).
- **adoc-html/lib.rs** новое поле `section_unnumbered_stack: Vec<bool>` (+init).
- **adoc-html/blocks.rs** `start_section_div`: push `parent_unnumbered || (is_special && style != appendix)`.
  `start_section_title`: в условие нумерации добавлено `section_unnumbered_stack.last() != Some(&true)`
  (короткозамыкает ДО `number_prefix` → счётчик не бампится). **events.rs** TagEnd::Section: парный pop.

**Корень B — `[abstract]` в book → numbered chapter.** parser.rb:1612 `if book && sect_style=='abstract'` →
`sect_name='chapter', sect_level=1`, теряет special: рендерится как обычный numbered `sect1` (БЕЗ класса
`abstract`) и СЪЕДАЕТ номер главы → сдвигает следующие главы (это чинит ВСЕ offset-диффы 3..6 и xref «Opa 4»).
В article остаётся special (unnumbered). Раньше мы трактовали abstract как special (без номера) во всех doctype.
- **adoc-html/blocks.rs** `start_section_div`: `let book_abstract = doctype_book && style==Some("abstract")`;
  `is_special = !book_abstract && matches!(...)`. Очистка стиля div: `if is_special || book_abstract { m.style=None }`
  (чтобы div был чистый `sect1`). sectname=Chapter получается автоматически (ветка `book && level==2 && !is_special`).

### Верификация
- clippy 0; **test --workspace: 0 упавших** (html 526→528: +test_special_section_subsection_unnumbered_html,
  +test_book_abstract_numbered_chapter_html; parser 645, compat 233, html-compat ok).
- **Гейт 344/344 байт-в-байт** vs master `e7abf3a` (gate_check.py 0 diff; база `/tmp/adoc_base` = свежий master-бинарь,
  рабочее дерево было чистым == master). Ни один гейт-файл не использует спец-нумерацию/abstract-в-book.
- **Sweep base-vs-new (frontier 250 + adoc2docx 52 = 302): РОВНО 1 изменённый** — sections.adoc (целевой).
  0 регрессий. adoc2docx Identical **43→44**; frontier **228 стабильно**.
- CLI-пробы vs asciidoctor 2.0.23 байт-в-байт: preface-subsection unnumbered, abstract-в-book numbered chapter,
  appendix subsection «A.1» numbered (не задет правилом A), article-abstract unnumbered.

### Состояние репо
- Ветка `feat/special-section-numbering` (от master `e7abf3a`, НЕ закоммичена). master чист == origin.
- 4 файла: adoc-html/blocks.rs (+~20), adoc-html/lib.rs (+поле/+init), adoc-html/events.rs (+pop),
  adoc-html/tests.rs (+2 теста, ~55 строк).

### Остаток / следующая работа
- **compat-mode кавычки** (` ``…'' ` вместо `&#8220;…&#8221;`) в xreftext (`@document.compat_mode`) — без корпус-файла.
- **multi-special-ex.adoc** (frontier, 87 diff, _includes-фрагмент, вероятно нужен doctype/multi-root).
- Крупные adoc2docx: test 1105, source 681, xml 291, callouts 195, links 62 — НЕ триажены (вероятно мульти-root).
- frontier single-diffs архитектурны: CHANGELOG (replacements-before-macros: `...`→ellipsis внутри URL-href),
  migration ({asciidoctor-version} intrinsic — матчить «2.0.23» семантически некорректно).
- Методология: `frontier_parity.py /mnt/c/tmp/adoc2docx`, `showdiff.py <file>`, gate_check.py (база `/tmp/adoc_base`),
  sweep base-vs-new (`scratchpad/sweep_bn.py`). Бинарь: `cargo build --release -p adoc-cli`.
  asciidoctor 2.0.23 gem: `/usr/share/rubygems-integration/all/gems/asciidoctor-2.0.23/lib/asciidoctor/`.
