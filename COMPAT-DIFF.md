# Различия adoc-parser vs Asciidoctor

Сравнение на реальных документах из `/mnt/c/tmp/adoc-test/` (344 файла).
Дата сравнения: 2026-03-22.

## Статистика

| Статус | Кол-во |
|--------|--------|
| Идентичны | 0 |
| С различиями | 243 |
| Ошибки (include) | 101 |

---

## Список различий

### Блочный уровень

- [x] **1. Footer генерируется при `-a nofooter`** (89 файлов)
  Asciidoctor с `-a nofooter` не выводит footer. Наш парсер всегда генерирует `<div id="footer">` с `<div id="footer-text">Last updated ...`.

- [ ] **2. Include — фатальная ошибка вместо fallback** (101 файл)
  `include::` с Antora-синтаксисом (`example$`, `partial$`, `text:`) вызывает ошибку. Asciidoctor выводит placeholder. Также `include::` внутри listing-блоков экранируется как `\include::` вместо отображения как есть.

- [ ] **3. Таблицы — отсутствует `<colgroup>/<col>`** (16+ файлов)
  Asciidoctor: `<colgroup><col style="width:...%"></colgroup>`. Наш парсер пропускает colgroup и сразу выводит `<tr>/<th>`.

- [ ] **4. Таблицы — отсутствует `<caption>`** (14+ файлов)
  Asciidoctor: `<caption class="title">Table N. ...</caption>`. Наш парсер не генерирует `<caption>` для таблиц (auto-counter + title).

- [ ] **5. Callouts не поддерживаются** (14 файлов)
  Asciidoctor: `<b class="conum">(1)</b>` для source callouts. Маркеры `<1>`, `<2>` не распознаются.

- [ ] **6. Авторская информация (`<div class="details">`) не генерируется** (6 файлов)
  Asciidoctor выводит `<div class="details"><span class="author">...` в body. Наш парсер пропускает author/revision metadata.

- [ ] **7. Quote blocks — отсутствует `<div class="attribution">`** (2 файла)
  Атрибуция (`— Author`) должна быть в отдельном `<div class="attribution">`, а не как параграф.

- [ ] **8. Специальные секции — неверная обработка** (3 файла)
  `[abstract]`, `[colophon]`, `[dedication]` добавляются как классы на `<div class="sect1 abstract">`, Asciidoctor обрабатывает их иначе.

- [ ] **9. `doctype=book` неверно определяется** (2 файла)
  `<body class="book">` вместо `<body class="article">` для обычных документов.

### Inline-уровень

- [ ] **10. `class="term"` на `<strong>` в description lists** (11 файлов)
  Asciidoctor: `<strong class="term">`. Наш: `<strong>`.

- [ ] **11. `class="path"` на `<em>` для путей** (5 файлов)
  Asciidoctor: `<em class="path">`. Наш: `<em>`.

- [ ] **12. Ссылки — лишний `class="bare"` / отсутствуют `target`+`rel`** (11 файлов)
  Лишний `class="bare"` на URL-ссылках без текста. Отсутствуют `target="_blank" rel="noopener"` для `link:` с `window=_blank`.

- [ ] **13. Entities — ошибочное экранирование backslash** (4 файла)
  `&sect;` → `\&sect;`, `&lt;` → `\&lt;`. Backslash перед entity references не должен выводиться.

- [ ] **14. Inline anchor — некорректный парсинг `[[id,reftext]]`** (1 файл)
  `[[bookmark-d,last paragraph]]` → наш: `id="bookmark-d,last paragraph"`, должно быть `id="bookmark-d"`.

- [ ] **15. Image alt — двойное экранирование кавычек** (1 файл)
  `alt=""Mesa Verde Sunset, by JAVH""` вместо `alt="Mesa Verde Sunset, by JAVH"`.

- [ ] **16. Cross-references — ID не нормализуется** (1 файл)
  `href="#Substitutions"` вместо `href="#_substitutions"`.

- [ ] **17. Custom inline macros → `<span>` вместо `<a>`** (2 файла)
  `irc://` URL и `anchor:` макрос должны генерировать `<a>`, выдают `<span class="custom-macro">`.

### HTML-рендеринг

- [ ] **18. Source blocks — отсутствует `class="highlight"` на `<pre>`** (4 файла)
  Asciidoctor: `<pre class="highlight">`. Наш: `<pre>`.

- [ ] **19. Лишние CSS-классы на listing blocks** (3+ файла)
  `<div class="listingblock asciidoc">` вместо `<div class="listingblock">`. Язык source block попадает как CSS-класс на wrapper div.

- [ ] **20. ID секций — точки удаляются** (2 файла)
  `0.3.0 Milestone Build` → наш: `_030_milestone_build`, должно быть `_0_3_0_milestone_build`.

- [ ] **21. Audio/Video — потеря атрибутов** (2 файла)
  `autoplay` не передаётся; URL-фрагменты (`#t=60`) теряются.
