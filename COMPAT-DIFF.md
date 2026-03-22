# Различия adoc-parser vs Asciidoctor

Сравнение на реальных документах из `/mnt/c/tmp/adoc-test/` (344 файла).
Дата сравнения: 2026-03-22 (обновлено).

Скрипт сравнения: `asciidoctor -o - -a nofooter <file>` vs `adoc -a nofooter <file>`.
Нормализация: игнорируются `<head>`, `<style>`, атрибут `style=`, порядок атрибутов.

## Статистика

| Статус | Кол-во |
|--------|--------|
| Идентичны | 74 |
| С различиями | 270 |
| Ошибки | 0 |

---

## Список различий

### Блочный уровень

- [x] **1. Footer при `-a nofooter`** (было 131 файл → 0)
  Реализован CLI флаг `-a`/`--attribute` для передачи атрибутов. `adoc -a nofooter` теперь подавляет footer. Атрибуты передаются и в preprocessor (conditionals) и в renderer (HtmlOptions).

- [x] **2. Include — fallback placeholder** (было 101 → 3 остаточных)
  Неразрешимые include выводят `Unresolved directive in <file> - include::path[attrs]`. Формат совпадает с Asciidoctor. Escaped `\include::` — backslash удаляется. Остаточные: 2 Antora-специфичных (`text:`, `pass:` prefix), 1 мелкое расхождение в формате.

- [x] **3. Callouts** (было 18 файлов → 0 callout-специфичных)
  Реализована полная поддержка callouts: нумерованные `<N>`, autonumbered `<.>`, XML comment `<!--N-->` и `<!--.-->`. Маркеры удаляются из кода и заменяются на `<b class="conum">(N)</b>`. Callout list (`<div class="colist arabic">`) генерируется корректно. Пробел между множественными callout refs на одной строке совпадает с Asciidoctor.

- [ ] **4. Таблицы — `<caption>` перед `<colgroup>`** (23 файла)
  Asciidoctor: `<caption class="title">Table N. ...</caption>` идёт перед `<colgroup>`. Наш парсер выводит `<colgroup>` первым, `<caption>` вторым. Порядок должен быть: caption → colgroup.

- [ ] **5. Таблицы — `<colgroup>` расхождения** (24+ файла)
  Таблицы без явного `cols=` атрибута не генерируют `<colgroup>` вообще — сразу `<thead>` (19 файлов). Содержимое `<colgroup>` (количество `<col>`, ширины) не совпадает (24 файла). Также `<colgroup>` иногда содержит лишние/недостающие `<col>`.

- [ ] **6. Авторская информация не генерируется** (14 файлов)
  Asciidoctor выводит `<div class="details"><span class="author">...` и `<br>` с revision info в `<div id="header">`. Наш парсер пропускает author/revision metadata. Заголовок документа (h1) выводится в `<div class="sect0">` вместо `<div id="header">`.

- [ ] **7. Quote blocks — нет `<div class="attribution">`** (3 файла)
  Атрибуция (`— Author, Source`) выводится как текст внутри blockquote вместо `<div class="attribution">` после `</blockquote>`. Также attribution text попадает в CSS-класс quoteblock.

- [ ] **8. Специальные секции** (3 файла)
  `[abstract]`, `[colophon]`, `[dedication]` добавляются как классы на `sect1` div. Asciidoctor для `doctype=article` не добавляет эти классы.

- [ ] **9. `doctype=book` неверно определяется** (2 файла)
  `<body class="book">` вместо `<body class="article">`. Также лишний `toc2` класс на body.

- [ ] **10. Collapsible blocks (`<details>/<summary>`) не поддерживаются** (2 файла)
  `[%collapsible]` block должен генерировать `<details><summary>`, выводит обычный div.

- [ ] **11. Роли на блоках не применяются** (14+ файлов)
  `[role=screenshot]` на image → `<div class="imageblock">` вместо `<div class="imageblock screenshot">`. `[.lead]` на параграф → `<div class="paragraph">` вместо `<div class="paragraph lead">`. Block roles из metadata не попадают на wrapper div.

- [ ] **12. Description lists — некорректный HTML** (2+ файла)
  `<ul>` вместо `<dl>` для description lists (1 файл). `<dd>` vs `<dt>` перепутаны (1 файл). Horizontal description lists (`[horizontal]`) генерируют `<table class="hdlist">` вместо правильной разметки.

- [ ] **30. Open blocks — содержимое не оборачивается** (12+ файлов)
  `[open]` block / `--` delimited block — содержимое не оборачивается в `<div class="openblock"><div class="content">`. Вложенные блоки внутри open block выводятся плоско.

- [ ] **31. Example blocks — содержимое не оборачивается** (7+ файлов)
  `====` delimited example block — содержимое выводится как простой параграф вместо `<div class="exampleblock"><div class="content">` с правильной структурой.

- [ ] **32. Sidebar blocks — содержимое не оборачивается** (3+ файла)
  `****` delimited sidebar block — аналогичная проблема: не генерируется `<div class="sidebarblock"><div class="content">`.

- [ ] **33. Admonition blocks — вложенный контент** (5+ файлов)
  Содержимое compound admonition blocks (с `====` delimiters) не оборачивается правильно. Вложенные списки и параграфы теряют структуру.

- [ ] **34. Вложенные списки — потеря вложенности** (13+ файлов)
  При 3+ уровнях вложенности списков теряется правильная структура `<li>/<ul>/<ol>`. Элементы выводятся на неправильном уровне.

- [ ] **35. Checklist (`[x]`/`[ ]`) — маркеры не обрабатываются** (4+ файла)
  `[*]`/`[x]`/`[ ]` в начале list item не преобразуются в `<input type="checkbox">`. Маркеры выводятся как текст.

- [ ] **36. Счётчики (`{counter:...}`) не подставляются** (2 файла)
  `{counter:table-number}` выводится как текст вместо инкрементируемого числа.

- [ ] **37. Типографские замены не применяются** (10+ файлов)
  `--` не заменяется на `—` (em dash). `'` не заменяется на правую одинарную кавычку `'`. `...` не заменяется на `…`. `->` не заменяется на `→`.

- [ ] **38. Ссылки — текст ссылки вместо URL** (30+ файлов)
  URL-макросы `https://example.com[Link Text]` — в некоторых контекстах текст ссылки не парсится, выводится URL вместо текста. Особенно проявляется в description list terms и complex inline contexts.

### Inline-уровень

- [ ] **13. `class="term"` на `<strong>` в description lists** (25 файлов)
  Asciidoctor: `<strong class="term">`. Наш: `<strong>`.

- [ ] **14. Ссылки — лишний `class="bare"` / отсутствуют `target`+`rel`** (35 файлов)
  Лишний `class="bare"` на URL-ссылках где Asciidoctor его не ставит. Отсутствуют `target="_blank" rel="noopener"` для `link:` с `window=_blank` или ролями. Число файлов выросло (было 19).

- [ ] **15. Entities — ошибочное экранирование backslash** (7 файлов)
  `&sect;` → `\&sect;`, `&lt;` → `\&lt;`, `&#174;` → `\&#174;`, `&#8942;` → `\&#8942;`. Backslash перед entity references не должен выводиться в HTML. Также `§` (entity) иногда заменяется на `&#167;`.

- [ ] **16. `class="path"` на `<em>` для путей** (7 файлов)
  `` `path` `` → Asciidoctor: `<em class="path">`. Наш: `<em>`.

- [ ] **17. Custom inline macros → `<span>` вместо правильного тега** (5+ файлов)
  `irc://` → `<a>`. `anchor:id[]` → `<a id="...">`. `[.line-through]#text#` → `<del>`. `#text#` → `<mark>`. Наш выдаёт `<span class="custom-macro">`.

- [ ] **18. Image alt — двойное экранирование кавычек** (5 файлов)
  `alt=""text""` вместо `alt="text"`. Кавычки внутри alt-текста дублируются.

- [ ] **19. Cross-references — ID не нормализуется** (2 файла)
  `href="#Substitutions"` вместо `href="#_substitutions"`. Авто-генерация ID из заголовка не приводит к lowercase + underscore prefix.

- [ ] **20. Inline anchor — некорректный парсинг `[[id,reftext]]`** (1 файл)
  `[[bookmark-d,last paragraph]]` → `id="bookmark-d,last paragraph"`, должно быть `id="bookmark-d"` с reftext сохранённым отдельно.

- [ ] **21. Hardbreak (`+`) внутри параграфов** (9 файлов)
  `+` в конце строки внутри параграфа не генерирует `<br>` в некоторых контекстах. Число выросло с 2 до 9.

- [ ] **29. Kbd macro не распознаётся** (5+ файлов)
  `kbd:[Enter]` не обрабатывается как inline macro, выводится как текст вместо `<kbd>Enter</kbd>`. Нарушает структуру окружающих блоков (listing, ordered list).

- [ ] **39. Btn/Menu macros не распознаются** (1+ файл)
  `btn:[Save]` → `<b class="button">Save</b>`. `menu:File[New]` → должен генерировать `<span class="menuseq">`. Выводятся как текст.

### HTML-рендеринг

- [ ] **22. Source blocks — отсутствует `class="highlight"` на `<pre>`** (11 файлов)
  Asciidoctor: `<pre class="highlight">`. Наш: `<pre>`. Число выросло (было 9).

- [ ] **23. Лишние CSS-классы на listing blocks** (14+ файлов)
  `<div class="listingblock asciidoc">` вместо `<div class="listingblock">`. Язык source block попадает как CSS-класс на wrapper div.

- [ ] **24. ID секций — точки заменяются неверно** (4 файла)
  `0.3.0 Milestone Build` → `_030_milestone_build`, должно быть `_0_3_0_milestone_build`. Точки должны заменяться на `_`, а не удаляться. Число выросло (было 2, найдено ещё в h3).

- [ ] **25. Audio/Video — потеря атрибутов** (2 файла)
  `autoplay` не передаётся; URL-фрагменты (`#t=60`) теряются.

- [ ] **26. Таблицы — frame/grid атрибуты** (2 файла)
  `frame=ends grid=none` → наш: `frame-all grid-all`. Значения frame/grid из block metadata не применяются.

- [ ] **27. Source block language подстановка** (7 файлов)
  `[source]` без языка + `source-language` attribute → Asciidoctor подставляет язык. `source` как язык вместо реального языка в `data-lang`. Число выросло (было 4).

- [ ] **28. TOC генерация** (2 файла)
  `:toc:` атрибут генерирует `<div id="toc">` с `<ul class="sectlevel1">`. Наш парсер добавляет `toc2` класс на body и неверно обрабатывает TOC placement.

- [ ] **40. Attribute substitution в контенте** (10+ файлов)
  Документ-атрибуты (`:url-project:`, `:name:` и т.д.) не подставляются в тексте параграфов и блоков. `{attribute-name}` остаётся как есть или выводится с фигурными скобками.
