# Session context

## Сессия (2026-06-23, 9-я) — F-BL: TAB-разделитель после list-маркера + `.\t…` ловился как block-title

Запрос «начни следующую задачу». master `c9a9b58` (F-BK смержен).

### Триаж (notes-корпус, метрика семантического DOM)
- `frontier_parity.py /mnt/c/Work/docs/notes/modules` → 74 identical, 7 чистых расхождений (по убыванию diff):
  plan(230)/qwen(194)/sbertech-index(134)/wsl(95)/keycloak(52)/tips(41)/**synapse(23)**.
- Взял самое чистое — `sbertech/synapse.adoc` (23 token-diff). НЕ доверял метке прошлой сессии,
  проверил исходник + showdiff: ordered-список с маркером `.` + **TAB** тихо пропадал.

### Сделано — F-BL (ветка `fix/list-marker-tab-separator` от master `c9a9b58`)
**Баг:** `synapse.adoc` — `.\tПолучение данных из SOAP сервиса` (маркер `.` + TAB) у asciidoctor → `<ol class="arabic">`,
у нас весь список исчезал (выдавали только ведущий параграф, затем закрывали секцию).
**Корень (2 места в scanner.rs, verified `rx.rb` + пробы asciidoctor 2.0.23):**
1. asciidoctor `AnyListRx` разделяет маркер и текст через `[ \t]` (пробел ИЛИ таб). Наши
   `is_list_marker_unordered`/`is_list_marker_ordered` принимали ТОЛЬКО пробел (`!rest.starts_with(' ')` → None для TAB).
2. `is_block_title` (`BlockTitleRx ^\.([^\s.].*)$`) исключал лишь пробел после `.` → `.\t…` ловился как block-title в
   `scan_header_constructs` (раньше list-детектора). Поэтому `.\tItem` в НАЧАЛЕ документа исчезал полностью, тогда как
   `*\t`/`-\t`/`1.\t` доходили до list-детектора (но тоже не распознавались из-за п.1).
   В synapse `.\t` шёл после параграфа — block-title тоже срабатывал, но эффект тот же (список терялся).

**Фикс (1 файл, ТОЛЬКО adoc-parser/scanner.rs):**
- новый хелпер `marker_content(rest)` = `strip_prefix(' ').or_else(strip_prefix('\t'))` → `trim_start` → non-empty
  (зеркало `[ \t]`); обе list-функции переведены на него (`is_list_marker_unordered`: hyphen-арм через `strip_prefix('-')`
  + `marker_content`, star-арм через `marker_content`; `is_list_marker_ordered`: numbered-арм `find('.')`+digits+
  `marker_content`, dotted-арм `marker_content`).
- `is_block_title` переписан под `[^\s.]`: `strip_prefix('.')` → первый символ не `.` и не `is_whitespace()`.

**Тесты:** scanner.rs — +6 assert'ов в `test_is_list_marker_unordered`/`_ordered`/`test_is_block_title` (TAB-кейсы +
regression `.text`→None как block-title). adoc-html/src/tests.rs — +1 `test_list_marker_tab_separator_html`
(`.\t`/`1.\t`/`*\t`/`-\t` дают списки + regression `.My Title` остаётся `<div class="title">`).

### Верификация
- clippy `--workspace` **0**. **test --workspace 0 упавших** (html 537→**538**, parser 647, compat 233, render-core 25,
  integration 29, html-compat 1).
- **БАЙТ-НЕЙТРАЛЬНО на старых корпусах** (паттерн «маркер+TAB» отсутствует — `grep -rlP '^\s*([.*-]+|\d+\.)\t'` гейт = 0):
  - gate 344 — `gate_check.py` **0 diff** vs base `/tmp/adoc_base` (ПЕРЕСОБРАН от master `c9a9b58`).
  - frontier(250)+adoc2docx(52)=302 — `/tmp/sweep_bvn.py` **0 diff**.
- **notes Identical 74→75** (synapse → identical, выпал из списка расхождений).
- 5 CLI-проб тела (`/tmp/p_ot|p_ut|p_ht|p_nt|p_ddt.adoc`) == asciidoctor 2.0.23 байт-в-байт.

### Состояние репо
- Ветка `fix/list-marker-tab-separator` от master `c9a9b58`, НЕ закоммичена (ждёт запроса коммит/merge/push).
- Изменено: `adoc-parser/src/scanner.rs` (2 функции переписаны + хелпер + 6 test-assert'ов),
  `adoc-html/src/tests.rs` (+1 тест), TODO.md (+F-BL), session.md.
- `/tmp/adoc_base` = бинарь master `c9a9b58` (актуальная база регресс-гарда этой сессии).

### Остаток notes (6 чистых расхождений — кандидаты на следующие сессии)
plan(230)/qwen(194)/sbertech-index(134)/wsl(95)/keycloak(52)/tips(41). По убыванию diff; tips(41) самый чистый из
оставшихся. Известные классы: ordered-list десинк, ложная Rouge-подсветка `[source,yaml]` без `:source-highlighter:`,
admonition-в-списке.

### Методология (без изменений)
`frontier_parity.py <roots>` / `showdiff.py <file>` (семантический DOM, ПРАВИЛЬНАЯ метрика для не-verbatim — байт только
ВНУТРИ `<pre>`, см. [[feedback_html_byte_parity_scope]]). `gate_check.py` + `/tmp/sweep_bvn.py` — байт регресс-гард
(база `/tmp/adoc_base` пересобирать от текущего master: stash → build → cp → pop → rebuild). Бинарь:
`cargo build --release -p adoc-cli`. ⚠ mtime на /mnt/c НЕ всегда обновляется — если сборка не подхватила правку, форсировать
`cargo test -p adoc-parser <name>` (перекомпилит) или touch. asciidoctor 2.0.23 для проб. НЕ доверять метке прошлой
сессии — bisect-матрица проб (см. [[feedback_frontier_triage]]). Источник реальных корпусов:
`/mnt/c/Work/docs/notes/modules/` (81 .adoc).
