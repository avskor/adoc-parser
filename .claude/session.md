# Session context

## Последняя сессия (2026-06-09, поздняя-2) — Фаза 3: link blank-window `^` (п.14)

п.19 xref-id-normalization уже смержена в master (755b320). Выбран следующий чистый flip
по near-miss на baseline 158 — крупнейший кластер: суффикс `^` в тексте ссылки.

### Ветка `fix/link-blank-window-caret` (от master; НЕ закоммичено)
- **Симптом**: `https://u[text^]` / `link:u[text^]` → мы оставляли `^` в видимом тексте
  (`macro^`) и НЕ добавляли `target="_blank" rel="noopener"`. Asciidoctor: `^` = blank-window
  shorthand → снять каретку, открыть в новом окне.
- **Семантика** (верифицирована пробами): trailing `^` на тексте ссылки → `window=_blank`
  (рендер: `target="_blank" rel="noopener"`), `^` снимается; явный `window=` побеждает каретку;
  работает для bare-URL, `link:`, mailto, `++url++` (все идут через `parse_link_attrs`).
- **Корень**: `attributes.rs::parse_link_attrs` пушил `text` (первый positional) сырым.
- **Фикс** (1 место, централизованно): после `let mut text = positional.first()...` —
  `if let Some(stripped) = text.strip_suffix('^') { text = stripped; if window.is_none()
  { window = Some("_blank"); } }`. Инфраструктура (`Tag::Link.window/nofollow`, рендер
  `target`/`rel` в `adoc-html/lib.rs:1128`) УЖЕ была — фикс минимален. +1 unit-тест
  `test_link_attrs_blank_window_caret` (4 кейса: caret, caret+role, no-caret, explicit-window).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 438→439).
- Корпус `compare_full.py` (release): **Identical 158→162 (+4), Different 182, Errors 0**.
- Blast radius (base-бинарь из master через `git worktree` vs new, `/tmp/check_blast.py`):
  9 файлов изменили вывод; из них **4 FLIP→IDENTICAL** (description, image-format,
  xref-text-and-style, key-concepts), 5 остались Different по НЕ-caret причинам
  (url — link-role `class="external"`; asciidoc-vs-markdown — md-fences; image-svg,
  ts-url-format, bibliography). **0 регрессий** (0 Identical→Different; net +4 точно объяснён;
  caret-ссылки в 5 оставшихся теперь совпадают с Asciidoctor по target/rel). worktree удалён.
- TODO.md: baseline 158→162, п.14 → `[~]` с под-пунктом blank-window `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/link-blank-window-caret` (только по запросу).
- Следующие чистые flip-кандидаты (near-miss на 162):
  - **bibliography-anchor `[pp]`** — `[[[pp]]]`/`<<pp>>` → reftext в скобках `[pp]`
    (1-diff кластер: _crud `[search_and_sort]`, bibliography `[pp]`/`[gang]`, subs/index
    `[table-subs-groups]`, xref.adoc `[anchors]`/`[paragraphs]`, _responses). `[id]` exp vs `id` got.
  - **апостроф `'`→’** в plain-тексте под каким-то контекстом (scope, span-cells, README) —
    остаток п.37; в тексте макроса (xref/link) — отдельно (архитектурно).
  - **`§`/bare char-ref** сохранять как сущность (title-links — остаток п.15).
  - **`// end::para[]` утечка** тег-региона (verse, literal — Asciidoctor КЕЕРS comment в verse).
- Архитектурные (отложены): `{attr-ref}[text]` (порядок subs — icons-font/auto-ids/custom-ids/
  index), link-role `class="external"` (нет поля role в `Tag::Link` — расширение типа).

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). near-miss `/tmp/nearmiss.py`.
  Сравнение семантическое (DOM). LSP для навигации, context7 MCP для доков.

---

## Сессия (2026-06-09, поздняя) — Фаза 3: п.19 xref-id норм. (natural cross reference)

Следующий чистый flip после image-alt-quotes (тот уже смержен+запушен, master == origin/master).

### Ветка `fix/xref-id-normalization` (от master; НЕ закоммичено)
- **Симптом**: `<<Substitutions>>` + секция `== Substitutions` (forward-ссылка) → мы давали
  `href="#Substitutions"`, Asciidoctor — `href="#_substitutions"` (id секции).
- **Семантика Asciidoctor** (верифицирована пробами, НЕ по памяти):
  - target == **заголовок секции** (case-sensitive) → id этой секции (auto `_substitutions`
    ИЛИ явный `[#myid]` → `#myid`);
  - target — зарегистрированный id → остаётся как есть;
  - иначе сырой target (`<<Foo Bar>>`→`#Foo Bar`, `<<substitutions>>` (lower) → не матчит);
  - резолюция href НЕ зависит от наличия текста (`<<T,текст>>` тоже резолвит href).
- **Корень**: `adoc-html/src/lib.rs::start_cross_reference` писал href сразу из сырого target.
  Но это forward-ссылка → нужна ленивая резолюция в `finish()` (как уже для текста xref).
- **Фикс**:
  - новое поле `xref_href_placeholders: Vec<(String,String)>` (placeholder, raw target);
  - в `start_cross_reference` (internal-ветка) вместо `html_escape(target)` пишу
    плейсхолдер `\x00XREFHREF_N\x00` (счётчик `xref_placeholder_counter` переиспользован;
    префикс XREFHREF ≠ XREF → подстроки не пересекаются при `replace`);
  - в `finish()` отдельный блок: `known_ids` (из toc_entries + block_ref_titles),
    `title_to_id` (из toc_entries, first-wins). Резолв: known id → как есть; иначе title→id;
    иначе сырой. `html_escape` + `output.replace`.
- +1 html-тест `test_natural_cross_reference` (5 кейсов: forward, no-match, explicit-id,
  case-sensitive, labeled).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (html 302→303).
- Корпус `compare_full.py` (release): **Identical 157→158 (+1), Different 186, Errors 0**.
- Blast radius — РОВНО 3 файла изменили вывод (проверены поштучно base vs new бинари):
  1 FLIP (positional-and-named-attributes); 2 остались DIFFERENT по НЕ-xref причинам
  (audio-and-video — av-attrs; link-macro-attribute-parsing — link-парсинг), но их href стал
  ВЕРНЫМ (`#_vimeo_and_youtube_videos`, `#_noopener_and_nofollow`, `#_blank_window_shorthand`).
  0 регрессий.
- TODO.md: baseline 157→158, п.19 помечен `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/xref-id-normalization` (только по запросу).
- Следующие чистые flip-кандидаты Фазы 3 (по near-miss на 158):
  - **link `^`+rel/target** для литеральных `link:`/URL (description, xref-text-and-style — по 2 diff);
    NB: `{attr-ref}[text]` — архитектурно (порядок subs).
  - **`// end::para[]` утечка** тег-региона (verse.adoc, literal.adoc).
  - **остаток п.37**: апостроф `'`→’ в display-тексте макроса (xref/link) не проходит REPLACEMENTS.
  - **п.24** (точки в id секций) — родственно п.19, но отдельная нормализация.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки эмпирически.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). Сравнение семантическое (DOM).
  LSP для навигации, context7 MCP для доков.

---

## Сессия (2026-06-09) — Фаза 3: п.18 image alt двойные кавычки

Самый чистый near-miss-кандидат на baseline 153 (предсказан в прошлой session.md).
em-dash и escaped-preprocessor уже смержены в master (dfa0819).

### Ветка `fix/image-alt-quotes` (от master; НЕ закоммичено)
- **Симптом**: `image::set-version-label.png["Byline...",role=screenshot]` →
  `<img alt="&quot;Byline...&quot;">` вместо `alt="Byline...">`.
- **Корень**: `adoc-parser/src/attributes.rs::parse_image_attrs`. Именованные значения
  (`key="v"`) снимали обрамляющие `"` (строки ~436-439), а **позиционные** (alt = positional[0],
  width=[1], height=[2]) пушились сырыми → кавычки доезжали до рендерера → `&quot;`.
- **Фикс**: вынесен хелпер `fn strip_enclosing_quotes(&str)->&str` (снимает ОДНУ пару двойных
  кавычек, согласован со `split_respecting_quotes`, который трекает только `"`). Применён в обеих
  ветках разбора (именованной — рефактор, поведение то же; позиционной — новое). Один источник
  `parse_image_attrs` → покрывает block-image (block.rs:563) и inline-image (inline.rs:1521).
  НЕ трогал общий `BlockAttributes::parse` (строка 199) — чтобы не расширять blast radius.
- +1 тест `test_parse_image_attrs_quoted_alt`.

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 437→438).
- Корпус `compare_full.py` (release): **Identical 153→157 (+4), Different 187, Errors 0**.
- Blast radius — РОВНО 6 файлов с закавыченным позиционным alt (`grep image::?…["`), проверены
  поштучно (`/tmp/check6.py`, normalize из compare_full, base vs new бинари):
  4 FLIP DIFFERENT→IDENTICAL (author-attribute-entries, reference-revision-attributes,
  revision-attribute-entries, version-label); 2 остались DIFFERENT по НЕ-alt причинам
  (image.adoc — обёртка `<a class="image">`/block-vs-inline; revision-line — вне alt). 0 регрессий.
- TODO.md: baseline 153→157, п.18 помечен `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/image-alt-quotes` (только по запросу).
  NB: на master 2 незапушенных коммита (em-dash) — `origin/master` отстаёт на 2.
- Следующие чистые flip-кандидаты Фазы 3 (по near-miss на 157):
  - **xref-id норм.** `#Substitutions`→`#_substitutions` (п.19/24, positional-and-named-attributes).
  - **link `^`+rel/target** для литеральных `link:`/URL (description, xref-text-and-style — по 2 diff);
    NB: `{attr-ref}[text]` — архитектурно (порядок subs).
  - **`// end::para[]` утечка** тег-региона (verse.adoc, literal.adoc).
  - **остаток п.37**: апостроф `'`→’ в display-тексте макроса (xref/link) не проходит REPLACEMENTS.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). near-miss: `/tmp/nearmiss.py`.
  Сравнение семантическое (DOM) — сырой `diff` может «врать» (`’`/`&#8217;`, whitespace в `<code>`).
  LSP для навигации, context7 MCP для доков.

---

## Сессия (2026-05-31, поздняя) — Фаза 3: em-dash границы + ZWSP

Кандидат по near-miss на baseline 149. Кластер «типографика» (п.37) — самый безопасный/выгодный
из чистых flip (link через `{attr-ref}` оказался архитектурным — порядок подстановок, отложен).

### Ветка `fix/em-dash-boundaries` (от master; НЕ закоммичено)
- **Корень**: `inline.rs::apply_typographic_replacements`, bare-`--` арм (строка ~28). Старое
  правило: любой `--` (кроме space-space) → `—`. Это (а) слишком агрессивно (` --dir`→`—dir`,
  `S.S.T.--`→`—`; Asciidoctor оставляет `--`) и (б) без ZWSP (`cases--such`→`—` вместо `—​`).
- **Фикс**: bare `--` → `—`+ZWSP (`—​`) ТОЛЬКО для `\w--\w` (Asciidoctor `(\w)--(?=\w)`,
  `\w`=ASCII alnum+`_`). Иначе → **`None`** (не `Some("--",2)`!): первый `-` остаётся литералом,
  второй переразбирается → `-->` корректно даёт `-→` (asciidoctor: `A --> B`→`A -→ B`, проверено).
  Space-space правило (` -- `→thin-em-thin) не тронуто.
- Тесты: обновлены 2 дубля bare-em-dash (2668 и 3801) под ZWSP; `test_arrow_triple_not_replaced`
  (3763) `A --> B`: было `—>`, стало `-→`. +2 теста (`run --dir`, `For S.S.T.--` остаются).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 435→437).
- Корпус `compare_full.py` (release): **Identical 149→153 (+4), Different 191, Errors 0**.
  Flip: asg/README (`--dir`), dedication (`S.S.T.--`), continuation (ZWSP), callouts (бонус).
  0 регрессий (Different −4 ровно; по регэкспу Asciidoctor наш фикс строго консервативнее).
- Побочно резолвило em-dash-diff в revision-attribute-entries (2→1) и image-format (3→2) —
  не флипнули (остался alt-баг / link-баг соответственно).
- TODO.md: baseline 149→153; п.37 помечен `[~]` с под-пунктом em-dash `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/em-dash-boundaries` (только по запросу).
- Следующие чистые flip-кандидаты (по near-miss на 153):
  - **alt двойная кавычка** (п.18): `<img alt=""...">` — author-attribute-entries (1 diff),
    version-label (2 diff, оба alt), revision-attribute-entries (1 diff, теперь только alt).
    Корень — значение alt в image-макросе сохраняет кавычки. Флипнет ~3 файла. САМЫЙ ЧИСТЫЙ.
  - **xref-id норм.** `#Substitutions`→`#_substitutions` (positional-and-named-attributes, 1 diff).
  - **link `^`+rel/target** (литеральные `link:`/URL): description, xref-text-and-style (по 2 diff).
    NB: `{attr-ref}[text]` (icons-font/auto-ids/custom-ids/ROOT-index) — архитектурно (порядок subs).
  - **`// end::para[]` утечка** тег-региона (verse.adoc, 1 diff) + literal.adoc (`// end::indent[]`).
  - **апостроф в тексте макроса** (остаток п.37): xref/link display-текст не проходит REPLACEMENTS.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). near-miss: `/tmp/nearmiss.py`.
  Сравнение семантическое (DOM). LSP для навигации, context7 MCP для доков.

---

## Сессия (2026-05-31) — Фаза 3: escaped preprocessor-директива

Второй кандидат сессии. Выбор по `/tmp/nearmiss.py`: escaped-директива `\ifdef`/`\endif`
(admonitions, inter-document-xref — «1 diff away»). Preprocessor-слой (не inline).

### Ветка `fix/escaped-preprocessor-directive` (от master; НЕ закоммичено)
- **Корень**: `\ifdef::env-github[]` — backslash экранирует preprocessor-директиву. Asciidoctor
  снимает `\` и выводит `ifdef::...[]` литералом без вычисления; мы сохраняли `\`
  (`parse_conditional` возвращает None из-за `\`, строка падала в обычный output).
- **Фикс** (preprocessor.rs, `preprocess_with_attrs`): новый шаг «0» в начале цикла —
  `if let Some(rest) = line.strip_prefix('\\') && starts_with_conditional_directive(rest)`
  → при `!is_skipping` эмитим `rest` (строку без `\`), `continue`. Хелпер
  `starts_with_conditional_directive` проверяет префиксы `ifdef::`/`ifndef::`/`ifeval::`/`endif::`
  (`::` отсекает слова вроде `ifdefinitely`).
- **КРИТИЧНО — колонка 0**: проверяем СЫРОЙ `line`, НЕ `trimmed`. Asciidoctor распознаёт
  директивы только в начале строки. Первая версия на `trimmed` снимала `\` и при отступе →
  сломала conditionals.adoc (` \ifdef::just-an-example[]` в `[source,indent=0]` листинге, где
  отступ НАМЕРЕННО гасит директиву — это написано в комментарии самого файла). Column-0 чинит:
  indented `\ifdef` остаётся как есть.
- +4 unit-теста (block/inline strip, non-directive kept, indented kept).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 431→435).
- Корпус `compare_full.py` (release): **Identical 145→149 (+4), Different 195, Errors 0**.
  Blast radius — ровно 5 файлов с escaped-директивами (вне их вывод побайтово не менялся):
  admonitions, inter-document-xref, conditionals, ifdef-ifndef, ifeval — ВСЕ 5 теперь Identical
  (net +4, т.к. один был Identical уже на baseline 145). 0 регрессий.
- conditionals.adoc остаётся с сырым diff'ом (` \ifdef` vs `\ifdef` — лишний ведущий пробел от
  несрезанного `[source,indent=0]`), но нормализатор его прощает → Identical. Отдельная бага.
- TODO.md: baseline 145→149; пункт отмечен `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/escaped-preprocessor-directive` (только по запросу).
- Следующие чистые flip-кандидаты Фазы 3 (по near-miss):
  - **xref-id норм.** `#Substitutions`→`#_substitutions` (п.19/24): positional-and-named-attributes.
  - **alt двойная кавычка** (п.18): `<img alt=""…">` — author/revision-attribute-entries.
  - **`// end::para[]` утечка** тег-региона в выводе (verse.adoc) — tagged-region/comment.
  - **`[source,indent=0]`** не срезает общий отступ (conditionals.adoc) — блок-скан.
  - **ОТДЕЛЬНО**: preserve bare char-ref (`&#174;` в обычном тексте → сохранять как сущность,
    не экранировать). НЕ изолированный 1-diff; внутри listing/literal оба экранируют — не трогать.

### Предостережения
- НЕ `cargo fmt` (не fmt-clean). Коммит только по запросу. Верифицировать находки аудита.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release `target/release/adoc`, 344 файла).
  near-miss: `/tmp/nearmiss.py`. Сравнение семантическое (DOM): `’`/`&#8217;` и whitespace внутри
  `<code>` нормализуются → сырой `diff` может «врать». LSP для навигации, context7 MCP для доков.
