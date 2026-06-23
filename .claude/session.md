# Session context

## Сессия (2026-06-23, 11-я) — F-BN: Unicode-маркер `•` (U+2022) не распознавался как unordered-список

Запрос «начни следующую задачу». master `5b9026c` (F-BM смержен). session.md прошлой сессии устарел (писался до мержа F-BM).

### Триаж (notes-корпус, метрика семантического DOM)
- Скрипты в `/mnt/c/tmp/adoc-test/` (frontier_parity.py, showdiff.py, gate_check.py, refcache.py и др.). Бинарь:
  `cargo build --release -p adoc-cli` → `target/release/adoc`.
- `frontier_parity.py /mnt/c/Work/docs/notes/modules` → 76 identical, 5 чистых расхождений.
- НЕ доверял меткам прошлой сессии — прогнал showdiff по ВСЕМ 5, классифицировал каждый.
  Взял `plan` (230) — самое чистое ПРАВИЛО (не самый маленький diff): один корень дал весь каскад.

### Сделано — F-BN (ветка `fix/bullet-unordered-marker` от master `5b9026c`)
**Баг:** `plan.adoc` — строки `• Сократить … / • Получить … / …` у нас оставались литералом в одном `<p>`,
у asciidoctor → `<div class="ulist"><ul><li>`.
**Корень (verified `rx.rb` + пробы asciidoctor 2.0.23):** asciidoctor `UnorderedListRx = /^[ \t]*(-|\*\**|•)[ \t]+…/` —
`•` (U+2022) равноправный unordered-маркер наряду с `-`/`*`. Наш `is_list_marker_unordered` (scanner.rs) принимал
только `-` и `*`. `•` — ОТДЕЛЬНАЯ семья маркеров: ровно ОДИН `•` (`••` НЕ маркер); нестится НЕЗАВИСИМО от `-`/`*`
(пробы `* a`+`• b` → нест и наоборот).

**Фикс (1 файл, adoc-parser/src/scanner.rs, ~5 строк логики):**
- в `is_list_marker_unordered` после hyphen-блока добавлен bail на `trimmed.strip_prefix('\u{2022}')` +
  `marker_content(rest)` (хелпер уже обеспечивает `[ \t]+`, отсекает `••` и `•no-space`) → возвращает идентичность
  **255** (out of band ВЫШЕ star-счётчиков; зеркало hyphen=0 out of band ниже).
- depth для unordered `ListItem { checked, .. }` ОТБРАСЫВАЕТСЯ в adoc-html (events.rs) → значение 255 не утекает в HTML.
- `close_to_parent_list`/`is_in_list_at_depth` — чистое равенство `*depth == target_depth`, нет диапазонной арифметики
  → сентинел 255 безопасен (матчится только с другими `•` = siblings).

**Тесты:** +4 assert'а в `test_is_list_marker_unordered` (scanner: `• item`/`•\titem`→Some(255); `•• double`/`•no-space`
→None), +1 html-тест `test_unordered_bullet_marker_html` (плоский `•`-список → `<ul>`; нестинг `* a`+`• b`; regression
`••`→параграф).

### Верификация
- clippy `--workspace` **0**. **test --workspace 0 упавших** (html 538→**539**, parser 648, compat 233, render-core 25,
  integration 29, html-compat 1).
- **БАЙТ-НЕЙТРАЛЬНО на старых корпусах:** база `/tmp/adoc_base` ПЕРЕСОБРАНА от текущего master `5b9026c`
  (stash→checkout master→clean+build→cp→checkout branch→pop→rebuild).
  - gate 344 — `gate_check.py` **0 diff**.
  - frontier(250)+adoc2docx(52)=302 — `/tmp/sweep_bvn.py` **0 diff**.
  - `•`-маркер в старых корпусах не встречается.
- **notes Identical 76→77** (plan → identical, выпал из расхождений).
- 7 CLI-проб (`•`-список / `* a`+`• b` / `• a`+`* b` / `••` / `•no-space` / `•\t` / `text • mid`) == asciidoctor 2.0.23
  байт-в-байт (тело, `--no-standalone` у нас = `-s` у asciidoctor).

### Состояние репо
- Ветка `fix/bullet-unordered-marker` от master `5b9026c`, НЕ закоммичена (ждёт запроса коммит/merge/push).
- Изменено: `adoc-parser/src/scanner.rs` (bail на `•` в `is_list_marker_unordered` + 4 assert'а), `adoc-html/src/tests.rs`
  (+1 тест), TODO.md (+F-BN), session.md.
- `/tmp/adoc_base` = бинарь master `5b9026c` (актуальная база регресс-гарда этой сессии).

### Остаток notes (4 чистых расхождения — классы верифицированы showdiff, кандидаты на след. сессии)
По убыванию diff. Классы РАЗНЫЕ:
- **sbertech/index (134)** — ordered-list `. Выполняем раздел "Подготовка DTL".` десинк (item выпадает/неверная
  вложенность; содержимое с кавычками `"…"`).
- **wsl (95)** — автолинк URL внутри backtick inline-кода захватывает ЗАКРЫВАЮЩИЙ backtick в href
  (`<a href="https://rubygems.org\`">`), граница code неверная.
- **keycloak/index (52)** — автолинк URL `http://<host>:<port>/…` (со спецсимволами `<>`) внутри inline-кода: asciidoctor
  делает `<a class="bare">` внутри `<code>`, мы НЕ автолинкуем (URL остаётся текстом).
- **tips (41)** — `[source,yaml]` примыкает к list-item БЕЗ пустой строки → asciidoctor ТЕРЯЕТ source-роль (plain `<pre>`),
  у нас остаётся `<pre class="highlight"><code class="language-yaml">` (корень изолирован прошлой сессией пробами).

### Методология (без изменений)
`frontier_parity.py <roots>` / `showdiff.py <file>` (семантический DOM, ПРАВИЛЬНАЯ метрика для не-verbatim — байт только
ВНУТРИ `<pre>`, см. [[feedback_html_byte_parity_scope]]). `gate_check.py` + `/tmp/sweep_bvn.py` — байт регресс-гард
(база `/tmp/adoc_base` пересобирать от текущего master). Бинарь: `cargo build --release -p adoc-cli`.
⚠ **mtime на /mnt/c НЕ обновляется надёжно** — надёжно только `cargo clean --release -p adoc-parser` перед build
(см. [[feedback_wsl_build_staleness]]). asciidoctor 2.0.23 для проб (наш `--no-standalone` ≈ asciidoctor `-s`).
⚠ **inline разбор идёт через subst/ (string-rewriting), inline.rs InlineState — LEGACY/мёртвый путь** для дефолтного
режима (см. [[proj_sequential_quotes_rewrite]]). НЕ доверять метке прошлой сессии — showdiff каждый кандидат
(см. [[feedback_frontier_triage]]). Источник реальных корпусов: `/mnt/c/Work/docs/notes/modules/` (81 .adoc).
asciidoctor list-регулярки: `rx.rb` в gem (`UnorderedListRx`/`OrderedListRx`/`AnyListRx`).
