# Различия adoc-parser vs Asciidoctor

Сравнение на реальных документах из `/mnt/c/tmp/adoc-test/` (344 файла).
Дата сравнения: 2026-03-22 (обновлено).

Скрипт сравнения: `asciidoctor -o - -a nofooter <file>` vs `adoc -a nofooter <file>`.
Нормализация: игнорируются `<head>`, `<style>`, атрибут `style=`, порядок атрибутов.

## Статистика

| Статус | Кол-во |
|--------|--------|
| Идентичны | 71 |
| С различиями | 273 |
| Ошибки | 0 |

---

## Список различий

### Блочный уровень

- [x] **1. Footer при `-a nofooter`** (было 131 файл → 0)
  Реализован CLI флаг `-a`/`--attribute` для передачи атрибутов. `adoc -a nofooter` теперь подавляет footer. Атрибуты передаются и в preprocessor (conditionals) и в renderer (HtmlOptions).

- [x] **2. Include — fallback placeholder** (было 101 → 4 остаточных)
  Неразрешимые include выводят `Unresolved directive in <file> - include::path[attrs]`. Формат совпадает с Asciidoctor. Escaped `\include::` — backslash удаляется. Остаточные: 2 Antora-специфичных (`text:`, `pass:` prefix), 2 мелких расхождения в формате.

- [x] **3. Callouts** (было 18 файлов → 0 callout-специфичных)
  Реализована полная поддержка callouts: нумерованные `<N>`, autonumbered `<.>`, XML comment `<!--N-->` и `<!--.-->`. Маркеры удаляются из кода и заменяются на `<b class="conum">(N)</b>`. Callout list (`<div class="colist arabic">`) генерируется корректно. Пробел между множественными callout refs на одной строке совпадает с Asciidoctor.

- [ ] **4. Таблицы — `<caption>` перед `<colgroup>`** (18 файлов)
  Asciidoctor: `<caption class="title">Table N. ...</caption>` идёт перед `<colgroup>`. Наш парсер выводит `<colgroup>` первым, `<caption>` вторым. Порядок должен быть: caption → colgroup.

- [ ] **5. Таблицы — `<colgroup>` расхождения** (7 файлов)
  В некоторых случаях содержимое `<colgroup>` (количество `<col>`, ширины) не совпадает с Asciidoctor. Таблицы без явного `cols=` атрибута могут не получать colgroup.

- [ ] **6. Авторская информация не генерируется** (14 файлов)
  Asciidoctor выводит `<div class="details"><span class="author">...` и `<br>` с revision info в `<div id="header">`. Наш парсер пропускает author/revision metadata.

- [ ] **7. Quote blocks — нет `<div class="attribution">`** (3 файла)
  Атрибуция (`— Author, Source`) выводится как текст внутри blockquote вместо `<div class="attribution">` после `</blockquote>`. Также attribution text попадает в CSS-класс quoteblock.

- [ ] **8. Специальные секции** (3 файла)
  `[abstract]`, `[colophon]`, `[dedication]` добавляются как классы на `sect1` div. Asciidoctor для `doctype=article` не добавляет эти классы.

- [ ] **9. `doctype=book` неверно определяется** (2 файла)
  `<body class="book">` вместо `<body class="article">`. Также лишний `toc2` класс на body.

- [ ] **10. Collapsible blocks (`<details>/<summary>`) не поддерживаются** (1 файл)
  `[%collapsible]` block должен генерировать `<details><summary>`, выводит обычный div.

- [ ] **11. Роли на блоках не применяются** (5+ файлов)
  `[role=screenshot]` на image → `<div class="imageblock">` вместо `<div class="imageblock screenshot">`. Block roles из metadata не попадают на wrapper div.

- [ ] **12. Description lists — некорректный HTML** (1+ файл)
  `<ul>` вместо `<dl>` для description lists в некоторых контекстах. Также `<dd>` vs `<dt>` перепутаны.

### Inline-уровень

- [ ] **13. `class="term"` на `<strong>` в description lists** (24 файла)
  Asciidoctor: `<strong class="term">`. Наш: `<strong>`.

- [ ] **14. Ссылки — лишний `class="bare"` / отсутствуют `target`+`rel`** (19 файлов)
  Лишний `class="bare"` на URL-ссылках. Отсутствуют `target="_blank" rel="noopener"` для `link:` с `window=_blank` или ролями.

- [ ] **15. Entities — ошибочное экранирование backslash** (6 файлов)
  `&sect;` → `\&sect;`, `&lt;` → `\&lt;`, `&#174;` → `\&#174;`, `&#8942;` → `\&#8942;`. Backslash перед entity references не должен выводиться в HTML.

- [ ] **16. `class="path"` на `<em>` для путей** (6 файлов)
  `` `path` `` → Asciidoctor: `<em class="path">`. Наш: `<em>`.

- [ ] **17. Custom inline macros → `<span>` вместо правильного тега** (4 файла)
  `irc://`, `anchor:` → `<a>`. `[.line-through]#text#` → `<del>`. `#text#` → `<mark>`. Наш выдаёт `<span class="custom-macro">`.

- [ ] **18. Image alt — двойное экранирование кавычек** (3 файла)
  `alt=""text""` вместо `alt="text"`. Кавычки внутри alt-текста дублируются.

- [ ] **19. Cross-references — ID не нормализуется** (2 файла)
  `href="#Substitutions"` вместо `href="#_substitutions"`. Авто-генерация ID из заголовка не приводит к lowercase + underscore prefix.

- [ ] **20. Inline anchor — некорректный парсинг `[[id,reftext]]`** (1 файл)
  `[[bookmark-d,last paragraph]]` → `id="bookmark-d,last paragraph"`, должно быть `id="bookmark-d"` с reftext сохранённым отдельно.

- [ ] **21. Hardbreak (`+`) внутри параграфов** (2 файла)
  `+` в конце строки внутри параграфа не генерирует `<br>` в некоторых контекстах.

### HTML-рендеринг

- [ ] **22. Source blocks — отсутствует `class="highlight"` на `<pre>`** (9 файлов)
  Asciidoctor: `<pre class="highlight">`. Наш: `<pre>`.

- [ ] **23. Лишние CSS-классы на listing blocks** (5+ файлов)
  `<div class="listingblock asciidoc">` вместо `<div class="listingblock">`. Язык source block попадает как CSS-класс на wrapper div.

- [ ] **24. ID секций — точки заменяются неверно** (2 файла)
  `0.3.0 Milestone Build` → `_030_milestone_build`, должно быть `_0_3_0_milestone_build`. Точки должны заменяться на `_`, а не удаляться.

- [ ] **25. Audio/Video — потеря атрибутов** (2 файла)
  `autoplay` не передаётся; URL-фрагменты (`#t=60`) теряются.

- [ ] **26. Таблицы — frame/grid атрибуты** (2 файла)
  `frame=ends grid=none` → наш: `frame-all grid-all`. Значения frame/grid из block metadata не применяются.

- [ ] **27. Source block language подстановка** (4 файла)
  `[source]` без языка + `source-language` attribute → Asciidoctor подставляет язык. Также `source` как язык вместо реального языка в data-lang.

- [ ] **28. TOC генерация** (2 файла)
  `:toc:` атрибут генерирует `<div id="toc">` с `<ul class="sectlevel1">`. Наш парсер добавляет `toc2` класс на body и неверно обрабатывает TOC placement.

- [ ] **29. Kbd macro не распознаётся** (3 файла)
  `kbd:[Enter]` не обрабатывается как inline macro, выводится как текст вместо `<kbd>Enter</kbd>`.
