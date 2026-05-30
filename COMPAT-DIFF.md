# Различия adoc-parser vs Asciidoctor

Сравнение на реальных документах из `/mnt/c/tmp/adoc-test/` (344 файла).
Дата сравнения: 2026-03-23 (третий прогон, уточнены числа и регрессии).

Скрипт сравнения: `asciidoctor -o - -a nofooter <file>` vs `adoc -a nofooter <file>`.
Нормализация: игнорируются `<head>`, `<style>`, атрибут `style=`, порядок атрибутов.

## Статистика

| Статус | Кол-во | Изменение |
|--------|--------|-----------|
| Идентичны | 71 | −3 |
| С различиями | 273 | +3 |
| Ошибки | 0 | 0 |

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
  Таблицы без явного `cols=` теперь генерируют `<colgroup>` — парсер синтезирует `cols=N` из первой строки данных. Поддержка `%autowidth` (`<col>` без style). Точность процентов колонок — 4 знака (`33.3333%`) с остатком до 100% на последней колонке, как в Asciидоctor. Остаточные расхождения: таблицы внутри неподдерживаемых блоков (open, sidebar), hdlist-таблицы, multiplier-синтаксис в `cols` (`3*`).

- [x] **6. Авторская информация не генерируется** (было 14 файлов → 0)
  Реализован рендеринг author/revision details в `<div id="header">`. Заголовок документа выводится корректно. Остаточные 8 файлов — следствие п.41 (комментарии перед заголовком).

- [x] **7. Quote blocks — нет `<div class="attribution">`** (было 3 файла → 0)
  Attribution (`— Author, Source`) теперь выводится в `<div class="attribution">` после `</blockquote>`. Поддержка attribution и citetitle для quote и verse блоков.

- [x] **8. Специальные секции** (было 3 файла → 0)
  Специальные стили секций (`abstract`, `colophon`, `dedication`, `appendix`, `glossary`, `preface`, `index`) больше не добавляются как CSS-классы на sect div. Asciidoctor никогда не добавляет эти стили в class — поведение теперь совпадает.

- [x] **9. `doctype=book` неверно определяется** (было 2 файла → 0)
  Парсер теперь игнорирует `:doctype:` в теле документа (после header). Только header-атрибуты влияют на doctype, как в Asciidoctor.

- [x] **10. Collapsible blocks (`<details>/<summary>`)** (было 2 файла → 0)
  `[%collapsible]` block теперь генерирует `<details><summary>` корректно.

- [ ] **11. Роли на блоках не применяются** (25 файлов, было 61)
  `[role=screenshot]` на image → `<div class="imageblock">` вместо `<div class="imageblock screenshot">`. `[.lead]` на параграф → `<div class="paragraph">` вместо `<div class="paragraph lead">`. Block roles из metadata не попадают на wrapper div. Число снизилось (с 61 до 25), но проблема не устранена.

- [x] **12. Description lists** (было 2 файла → 0)
  Description lists теперь генерируют корректный `<dl>/<dt>/<dd>` HTML.

- [ ] **30. Open blocks — лишний класс `open`** (0 файлов, было 12 → **исправлено**)
  Ранее `class="openblock open"` вместо `class="openblock"`. Текущий прогон не выявил этой проблемы — можно отметить как исправленное.

- [x] **31. Example blocks** (было 7 файлов → 0)
  `====` delimited example block теперь оборачивается в `<div class="exampleblock"><div class="content">` корректно.

- [x] **32. Sidebar blocks** (было 3 файлов → 0)
  `****` delimited sidebar block теперь генерирует `<div class="sidebarblock"><div class="content">` корректно.

- [ ] **33. Admonition blocks — вложенный контент** (1 файл, было 5)
  Содержимое compound admonition blocks в одном файле (`lists/examples/complex.adoc`) теряет `<td class="icon">`. Остальные 4 файла исправлены.

- [x] **34. Вложенные списки** (было 13 файлов → 0)
  3+ уровни вложенности списков теперь корректно сохраняют структуру `<li>/<ul>/<ol>`.

- [x] **35. Checklist (`[x]`/`[ ]`)** (было 4 файла → 0)
  `[*]`/`[x]`/`[ ]` в начале list item теперь преобразуются в `<input type="checkbox">`.

- [ ] **36. Счётчики (`{counter:...}`) не подставляются** (2 файла)
  `{counter:table-number}` выводится как текст вместо инкрементируемого числа.

- [ ] **37. Типографские замены не применяются** (~10 файлов, пересекается с п.38)
  `--` не заменяется на `—` (em dash). `'` не заменяется на правую одинарную кавычку `'`. `...` не заменяется на `…`. `->` не заменяется на `→`. Точное число файлов определить сложно, т.к. пересекается с другими категориями text_content_diff.

- [ ] **38. Ссылки — текст ссылки вместо URL** (25 файлов, было ~200)
  URL-макросы `https://example.com[Link Text]` — в некоторых контекстах текст ссылки не парсится, выводится URL вместо текста. Число значительно снизилось (с ~200 до 25 чистых случаев). Остаётся в description list terms и complex inline contexts.

### Inline-уровень

- [ ] **13. `class="term"` на `<strong>` в description lists** (28 файлов, было 25)
  Asciidoctor: `<strong class="term">`. Наш: `<strong>`. Число незначительно выросло.

- [ ] **14. Ссылки — лишний `class="bare"` / отсутствуют `target`+`rel`** (23 файла, было 35)
  Лишний `class="bare"` на URL-ссылках где Asciidoctor его не ставит. Отсутствуют `target="_blank" rel="noopener"` для `link:` с `window=_blank` или ролями. Число снизилось.

- [ ] **15. Entities — ошибочное экранирование backslash** (10 файлов, было 8)
  `&sect;` → `\&sect;`, `&lt;` → `\&lt;`, `&#174;` → `\&#174;`, `&#8942;` → `\&#8942;`. Backslash перед entity references не должен выводиться в HTML. Число незначительно выросло.

- [ ] **16. `class="path"` на `<em>` для путей** (7 файлов, было 8)
  `` `path` `` → Asciidoctor: `<em class="path">`. Наш: `<em>`.

- [ ] **17. Custom inline macros → `<span>` вместо правильного тега** (5 файлов)
  `irc://` → `<a>`. `anchor:id[]` → `<a id="...">`. `[.line-through]#text#` → `<del>`. `#text#` → `<mark>`. Наш выдаёт `<span class="custom-macro">`.

- [ ] **18. Image alt — двойное экранирование кавычек** (5 файлов)
  `alt=""text""` вместо `alt="text"`. Кавычки внутри alt-текста дублируются.

- [ ] **19. Cross-references — ID не нормализуется** (2 файла)
  `href="#Substitutions"` вместо `href="#_substitutions"`. Авто-генерация ID из заголовка не приводит к lowercase + underscore prefix.

- [ ] **20. Inline anchor — некорректный парсинг `[[id,reftext]]`** (1 файл)
  `[[bookmark-d,last paragraph]]` → `id="bookmark-d,last paragraph"`, должно быть `id="bookmark-d"` с reftext сохранённым отдельно.

- [x] **21. Hardbreak (`+`) внутри параграфов** (было 9 файлов → 0)
  `+` в конце строки внутри параграфа теперь корректно генерирует `<br>`.

- [ ] **29. Kbd macro не распознаётся** (5 файлов)
  `kbd:[Enter]` не обрабатывается как inline macro, выводится как текст вместо `<kbd>Enter</kbd>`. Нарушает структуру окружающих блоков (listing, ordered list).

- [ ] **39. Btn/Menu macros не распознаются** (1 файл)
  `btn:[Save]` → `<b class="button">Save</b>`. `menu:File[New]` → должен генерировать `<span class="menuseq">`. Выводятся как текст.

### HTML-рендеринг

- [ ] **22. Source blocks — `class="highlight"` на `<pre>`** (26 файлов, было 11 → 0 → **регрессия**)
  `<pre class="highlight">` отсутствует в 26 файлах. Проблема проявляется когда source block находится внутри admonition, после include-placeholder, или в определённых вложенных контекстах. Чистые source blocks верхнего уровня работают корректно.

- [ ] **23. Лишние CSS-классы на listing blocks** (24 файла, было 61 → 0 → **регрессия**)
  Язык source block снова попадает как CSS-класс на wrapper div: `class="listingblock asciidoc"` вместо `class="listingblock"`. Затронуты файлы с `[source,язык]` синтаксисом. Ранее было исправлено, но регрессировало (вероятно при изменениях в source block rendering).

- [ ] **24. ID секций — точки заменяются неверно** (1+ файл, было 4)
  `0.3.0 Milestone Build` → `_030_milestone_build`, должно быть `_0_3_0_milestone_build`. Точки должны заменяться на `_`, а не удаляться.

- [ ] **25. Audio/Video — потеря атрибутов** (2 файла)
  `autoplay` не передаётся; URL-фрагменты (`#t=60`) теряются.

- [ ] **26. Таблицы — frame/grid атрибуты** (2 файла)
  `frame=ends grid=none` → наш: `frame-all grid-all`. Значения frame/grid из block metadata не применяются.

- [ ] **27. Source block language подстановка** (7 файлов)
  `[source]` без языка + `source-language` attribute → Asciidoctor подставляет язык. `source` как язык вместо реального языка в `data-lang`.

- [ ] **28. TOC генерация** (1 файл, было 2)
  `:toc:` атрибут — расхождения в генерации TOC markup.

- [ ] **40. Attribute substitution в контенте** (13 файлов, было 10)
  Документ-атрибуты (`:url-project:`, `:name:` и т.д.) не подставляются в тексте параграфов и блоков. `{attribute-name}` остаётся как есть или выводится с фигурными скобками. Число незначительно выросло.

- [ ] **41. Document header не распознаётся после комментариев** (8 файлов, **новое**)
  Если перед `= Title` идут строки комментариев (`// tag::...`, `// comment`), парсер не распознаёт document header. В результате `<h1>`, author details и revision info не попадают в `<div id="header">`. Asciidoctor корректно игнорирует комментарии перед header.

---

## Сводка по статусу

| Категория | Исправлено | Открыто | Регрессия |
|-----------|-----------|---------|-----------|
| Блочный уровень | 15 | 6 | 0 |
| Inline-уровень | 1 | 9 | 0 |
| HTML-рендеринг | 0 | 10 | 2 (п.22, 23) |
| **Итого** | **16** | **25** | **2** |

### Топ проблем по числу затронутых файлов

1. **п.13** — `class="term"` на `<strong>` — 28 файлов
2. **п.22** — Missing `<pre class="highlight">` — 26 файлов (регрессия)
3. **п.38** — Ссылки: текст вместо URL — 25 файлов
4. **п.11** — Роли на блоках — 25 файлов
5. **п.23** — Язык как CSS-класс на listingblock — 24 файлов (регрессия)
6. **п.14** — Лишний `class="bare"` — 23 файла
7. **п.40** — Attribute substitution — 13 файлов
8. **п.15** — Entity backslash — 10 файлов
