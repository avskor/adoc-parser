# Session context

## Сессия (2026-06-22) — F-AU: преамбула для doctype book без секции (ветка `fix/book-preamble-no-section`, off master `a97f782`, НЕ закоммичена)

Запрос «начни следующую задачу из TODO.md». Master `a97f782` чист. Открытые `[ ]` (TODO:1213/1305/1307) ВСЕ синтетические
«0 корпусного выигрыша». Основной frontier ИСЧЕРПАН (228 identical, 5 clean-div нишевые). Продолжил триаж нового корпуса
`/mnt/c/tmp/adoc2docx/` (52 feature-демо, не в гейте/frontier): был 27 identical / 21 clean-div.

### Задача (найдена триажем adoc2docx — системный класс, 10 файлов сразу)
`showdiff.py` по audio/keyboard/video/footnotes/text/example/sidebar/open/checklist/admonitions показал общий паттерн:
```
[7] ADOC: <div id="preamble">      [7] OUR : <div class="paragraph">
[8] ADOC: <div class="sectionbody">
```
Все файлы = `= Title` + `:doctype: book` + тело БЕЗ секций. asciidoctor оборачивает тело в
`<div id="preamble"><div class="sectionbody">`, мы — нет.

### Корень (verified пробами asciidoctor 2.0.23 + `parser.rb` next_section)
asciidoctor СОЗДАЁТ преамбулу при `has_header || doctype==book`; ОБЁРТКА эмитится если у преамбулы есть контент И
(`book` ИЛИ следует секция). Матрица 10 проб:
- article+title: обёртка ТОЛЬКО при следующей секции
- **book: обёртка ВСЕГДА** (с/без заголовка, с/без секции), кроме пустого тела
- section-only / no-pre-body: нет обёртки

Наш рендерер реализовывал ровно article-правило: `finish.rs` явно «no section followed — leave content as-is».

### Реализация (2 файла)
- `adoc-html/src/events.rs`: (1) `preamble_start` ставится при `has_document_title || doctype_book` (обе точки —
  standalone и embedded в `TagEnd::Header`). (2) Новый хелпер `close_preamble(output, start)` — извлечён дословно из
  section-start пути (split_off + `:toc: preamble` развилка); section-start теперь зовёт его.
- `adoc-html/src/finish.rs`: если `preamble_start` всё ещё `Some` (секция не встретилась — section-start путь её `take`-ает)
  И `doctype_book` → `close_preamble`. article роняет (прежнее поведение). Пустое тело → preamble_content пуст → нет обёртки.

### Тесты (+4 html, src/tests.rs после test_no_preamble_without_section_html)
`test_book_preamble_without_section_html` (точный байт embedded-вывода), `_without_title_html`,
`_with_section_unchanged_html` (один preamble, секция снаружи), `test_book_no_preamble_for_section_only_html`.

### Верификация
- clippy 0; **test --workspace зелёное** (html 502→506, parser 643, compat 233, render-core 18).
- **Гейт 344/344 байт-в-байт** vs master `a97f782` (`gate_check.py` 0 diff — 0 корпусных book-без-секции).
- **Frontier 250: 228 identical, new-vs-base 0 diff** (article-доминантный).
- **adoc2docx: 27→37 identical (+10)**; остальные diff сократились (links 115→89, menu 87→80). 10 флипнутых body-diff=0.
- **10/10 CLI-проб == asciidoctor 2.0.23**.

### Вне scope (предсуществующее, НЕ регрессия)
book+title+пустое-тело — лишняя пустая строка между `<div id="content">` и `</div>` (есть и на master; normalize_html=0 diff).

### Состояние
Закоммичено? НЕТ. Коммит/merge --no-ff/push — ПО ЗАПРОСУ пользователя. TODO.md обновлён (F-AU добавлен в начало
FRONTIER-секции).

### Остаток adoc2docx clean-div (11, для будущего триажа)
xref(1495 — xrefstyle:full реф-лейблы фигур), test(1111), source(682), xml(291), callouts(196), links(89), menu(80),
sections(55), icons(10 — title с inline-форматированием в `<i title=…>`), abstract(6), images(1 — xrefstyle:full
реф-текст фигуры `Test caption: , "Tux title"`).
