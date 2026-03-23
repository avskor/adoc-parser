# Различия adoc-parser vs Asciidoctor

Сравнение на реальных документах из `/mnt/c/tmp/adoc-test/` (344 файла).
Дата сравнения: 2026-03-22 (повторный прогон, числа уточнены).

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

- [x] **4. Таблицы — `<caption>` перед `<colgroup>`** (было 23 файла → 0)
  Порядок исправлен: caption теперь выводится перед colgroup, как требует HTML5 спец и Asciidoctor.

- [x] **5. Таблицы — `<colgroup>` расхождения** (было 24+ файлов → ~8 остаточных)
  Таблицы без явного `cols=` теперь генерируют `<colgroup>` — парсер синтезирует `cols=N` из первой строки данных. Поддержка `%autowidth` (`<col>` без style). Точность процентов колонок — 4 знака (`33.3333%`) с остатком до 100% на последней колонке, как в Asciidoctor. Остаточные расхождения: таблицы внутри неподдерживаемых блоков (open, sidebar), hdlist-таблицы, multiplier-синтаксис в `cols` (`3*`).

- [x] **6. Авторская информация не генерируется** (было 14 файлов → 0)
  Реализован рендеринг author/revision details в `<div id="header">`. Заголовок документа выводится корректно.

- [x] **7. Quote blocks — нет `<div class="attribution">`** (было 3 файла → 0)
  Attribution (`— Author, Source`) теперь выводится в `<div class="attribution">` после `</blockquote>`. Поддержка attribution и citetitle для quote и verse блоков.

- [x] **8. Специальные секции** (было 3 файла → 0)
  Специальные стили секций (`abstract`, `colophon`, `dedication`, `appendix`, `glossary`, `preface`, `index`) больше не добавляются как CSS-классы на sect div. Asciidoctor никогда не добавляет эти стили в class — поведение теперь совпадает.

- [ ] **9. `doctype=book` неверно определяется** (2 файла)
  `<body class="book">` вместо `<body class="article">`. Также лишний `toc2` класс на body.

- [x] **10. Collapsible blocks (`<details>/<summary>`)** (было 2 файла → 0)
  `[%collapsible]` block теперь генерирует `<details><summary>` корректно.

- [ ] **11. Роли на блоках не применяются** (61 файл)
  `[role=screenshot]` на image → `<div class="imageblock">` вместо `<div class="imageblock screenshot">`. `[.lead]` на параграф → `<div class="paragraph">` вместо `<div class="paragraph lead">`. Block roles из metadata не попадают на wrapper div.

- [x] **12. Description lists** (было 2 файла → 0)
  Description lists теперь генерируют корректный `<dl>/<dt>/<dd>` HTML.

- [ ] **30. Open blocks — лишний класс `open`** (12 файлов)
  `[open]` / `--` delimited block — содержимое оборачивается в `<div class="openblock"><div class="content">` корректно, но добавляется лишний CSS-класс `open` на wrapper div: `class="openblock open"` вместо `class="openblock"`.

- [x] **31. Example blocks** (было 7 файлов → 0)
  `====` delimited example block теперь оборачивается в `<div class="exampleblock"><div class="content">` корректно.

- [x] **32. Sidebar blocks** (было 3 файлов → 0)
  `****` delimited sidebar block теперь генерирует `<div class="sidebarblock"><div class="content">` корректно.

- [ ] **33. Admonition blocks — вложенный контент** (5 файлов)
  Содержимое compound admonition blocks (с `====` delimiters) не оборачивается правильно. Вложенные списки и параграфы теряют структуру.

- [x] **34. Вложенные списки** (было 13 файлов → 0)
  3+ уровни вложенности списков теперь корректно сохраняют структуру `<li>/<ul>/<ol>`.

- [x] **35. Checklist (`[x]`/`[ ]`)** (было 4 файла → 0)
  `[*]`/`[x]`/`[ ]` в начале list item теперь преобразуются в `<input type="checkbox">`.

- [ ] **36. Счётчики (`{counter:...}`) не подставляются** (2 файла)
  `{counter:table-number}` выводится как текст вместо инкрементируемого числа.

- [ ] **37. Типографские замены не применяются** (10 файлов, пересекается с п.38)
  `--` не заменяется на `—` (em dash). `'` не заменяется на правую одинарную кавычку `'`. `...` не заменяется на `…`. `->` не заменяется на `→`.

- [ ] **38. Ссылки — текст ссылки вместо URL** (200 файлов, пересекается с п.37 и п.40)
  URL-макросы `https://example.com[Link Text]` — в некоторых контекстах текст ссылки не парсится, выводится URL вместо текста. Особенно проявляется в description list terms и complex inline contexts. Самая массовая категория `text_content_diff` — 200 файлов (включает также п.37 и п.40).

### Inline-уровень

- [ ] **13. `class="term"` на `<strong>` в description lists** (25 файлов)
  Asciidoctor: `<strong class="term">`. Наш: `<strong>`.

- [ ] **14. Ссылки — лишний `class="bare"` / отсутствуют `target`+`rel`** (35 файлов)
  Лишний `class="bare"` на URL-ссылках где Asciidoctor его не ставит. Отсутствуют `target="_blank" rel="noopener"` для `link:` с `window=_blank` или ролями. Число файлов выросло (было 19).

- [ ] **15. Entities — ошибочное экранирование backslash** (8 файлов)
  `&sect;` → `\&sect;`, `&lt;` → `\&lt;`, `&#174;` → `\&#174;`, `&#8942;` → `\&#8942;`. Backslash перед entity references не должен выводиться в HTML. Также `§` (entity) иногда заменяется на `&#167;`.

- [ ] **16. `class="path"` на `<em>` для путей** (8 файлов)
  `` `path` `` → Asciidoctor: `<em class="path">`. Наш: `<em>`.

- [ ] **17. Custom inline macros → `<span>` вместо правильного тега** (5 файлов)
  `irc://` → `<a>`. `anchor:id[]` → `<a id="...">`. `[.line-through]#text#` → `<del>`. `#text#` → `<mark>`. Наш выдаёт `<span class="custom-macro">`.

- [ ] **18. Image alt — двойное экранирование кавычек** (5 файлов)
  `alt=""text""` вместо `alt="text"`. Кавычки внутри alt-текста дублируются.

- [ ] **19. Cross-references — ID не нормализуется** (2 файла)
  `href="#Substitutions"` вместо `href="#_substitutions"`. Авто-генерация ID из заголовка не приводит к lowercase + underscore prefix.

- [ ] **20. Inline anchor — некорректный парсинг `[[id,reftext]]`** (1 файл)
  `[[bookmark-d,last paragraph]]` → `id="bookmark-d,last paragraph"`, должно быть `id="bookmark-d"` с reftext сохранённым отдельно.

- [ ] **21. Hardbreak (`+`) внутри параграфов** (9 файлов)
  `+` в конце строки внутри параграфа не генерирует `<br>` в некоторых контекстах. Число выросло с 2 до 9.

- [ ] **29. Kbd macro не распознаётся** (5 файлов)
  `kbd:[Enter]` не обрабатывается как inline macro, выводится как текст вместо `<kbd>Enter</kbd>`. Нарушает структуру окружающих блоков (listing, ordered list).

- [ ] **39. Btn/Menu macros не распознаются** (1 файл)
  `btn:[Save]` → `<b class="button">Save</b>`. `menu:File[New]` → должен генерировать `<span class="menuseq">`. Выводятся как текст.

### HTML-рендеринг

- [x] **22. Source blocks — `class="highlight"` на `<pre>`** (было 11 файлов → 0)
  `<pre class="highlight">` теперь генерируется корректно для source blocks.

- [x] **23. Лишние CSS-классы на listing blocks** (было 61 файл → 0)
  Язык source block больше не попадает как CSS-класс на wrapper div.

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

- [ ] **40. Attribute substitution в контенте** (10 файлов, пересекается с п.38)
  Документ-атрибуты (`:url-project:`, `:name:` и т.д.) не подставляются в тексте параграфов и блоков. `{attribute-name}` остаётся как есть или выводится с фигурными скобками.
