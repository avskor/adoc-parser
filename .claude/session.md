# Session context

## Сессия (2026-06-22) — F-AY: макро-autolink после границы `"`/`'`

Запрос «начни следующую задачу из TODO». Ветка `fix/autolink-macro-quote-boundary` (НЕ закоммичена/смержена — по паттерну F-* коммит/merge/push ПО ЗАПРОСУ).

### Сделано
Корень **B** из мульти-root `links.adoc`: макро-форма autolink (`url[…]`) после `"`/`'` не распознавалась.
- **Корень** (verified asciidoctor 2.0.23 `rx.rb InlineLinkRx` + 6 проб): left-boundary допускает `"`/`'`, но они
  открывают ТОЛЬКО макро-форму; bare `"https://x"` остаётся литералом, `"https://x[]"` линкуется.
- **Фикс (2 файла, оба движка):** `subst/macros.rs::try_autolink` + `inline.rs::try_autolink` (legacy) — `quote_boundary`
  как tentative-граница + гейт `if quote_boundary && !bracket_follows → None`.
- **Тесты:** +1 parser (`quoted_boundary_links_macro_form_only_matches_asciidoctor`), +1 html
  (`test_macro_autolink_after_quote_boundary_html`). clippy 0, test --workspace **1272 зелёных** (parser 644, html 516).
- **Верификация:** Гейт **344/344 байт-в-байт** vs master `679b5b7` (gate_check 0 diff). Frontier 250 + adoc2docx 52
  new-vs-base sweep = **ровно 1 файл** (links.adoc, diff 89→62, строка-13 байт-в-байт с asciidoctor), **0 регрессий**.

### Состояние репо
- На ветке `fix/autolink-macro-quote-boundary` (HEAD == master `679b5b7`, изменения НЕ закоммичены).
- Изменены: `adoc-parser/src/subst/macros.rs`, `adoc-parser/src/inline.rs`, `adoc-parser/src/subst/mod.rs` (+тест),
  `adoc-html/src/tests.rs` (+тест), `TODO.md`, `.claude/session.md`.
- master == origin/master == `679b5b7`, чист.

### Остаток `links.adoc` (вне scope этой сессии, документировано в F-AY)
- **Корень A** (вложенные `[]` в тексте ссылки, `[.overline]#…#`): архитектурный — наш движок извлекает macros ДО quotes
  (`run_pipeline_with` стр.243/254), asciidoctor — после (quotes резолвит `[.overline]#…#` в `<span>` → внутренние `[]`
  исчезают до link-regex). Это проект `proj_sequential_quotes_rewrite`.
- **Корень C** (`id=`/`title=` на ссылке сбрасываются): чисто аддитивно, но ~40 сайтов конструирования `Tag::Link`
  (event.rs + оба движка + рендерер + тесты). Отдельная задача, низкий регресс-риск.

### Следующая работа — триаж adoc2docx (новый корпус, не в гейте): 41 identical / 8 clean-div
Малые файлы исчерпаны на не-архитектурном: `images`(1)/`xref`(1495) = xrefstyle:full (нумерованные подписи + full/short/
basic-стиль текста xref — крупная фича); `callouts/xml/source/test` = Rouge syntax-highlighter (`<span class="nb">` и т.п.,
архитектурное); `sections`(55) = глубокая нумерация special-секций (part/colophon/abstract/dedication/preface/appendix/
glossary/index + partnums/sectnums-toggle/signifiers). `links`(62) = остаток корни A (архитектурный) + C (аддитивный id/title).
Логичный следующий неархитектурный кандидат: `links` корень C (id/title) ИЛИ `sections` нумерация.
Методология: `frontier_parity.py /mnt/c/tmp/adoc2docx`, `showdiff.py <file>` (в `/mnt/c/tmp/adoc-test/`), gate_check.py
(база `/tmp/adoc_base` = master), читать исходник asciidoctor 2.0.23 + CLI-пробы, НЕ доверять позиционному differ'у.
