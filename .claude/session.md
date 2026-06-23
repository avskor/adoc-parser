# Session context

## Сессия (2026-06-23, 12-я) — F-BO: список НЕ прерывает параграф (маркер сразу за текстом параграфа поглощается)

Запрос «начни следующую задачу». master `c8bf4ed` (F-BN смержен). session.md прошлой сессии устарел (писался до мержа F-BN).

### Триаж (notes-корпус, метрика семантического DOM)
- Скрипты в `/mnt/c/tmp/adoc-test/` (frontier_parity.py, showdiff.py, gate_check.py, refcache.py и др.). Бинарь:
  `cargo build --release -p adoc-cli` → `target/release/adoc`.
- `frontier_parity.py /mnt/c/Work/docs/notes/modules` → 77 identical, 4 чистых расхождения.
- НЕ доверял меткам прошлой сессии — прогнал showdiff по ВСЕМ 4, классифицировал каждый. Взял `sbertech/index` (134) —
  самый высокоимпактный И самый чистый КОРЕНЬ (один десинк рушит весь хвост).

### Сделано — F-BO (ветка `fix/list-no-interrupt-paragraph` от master `c8bf4ed`)
**Баг:** `Устанавливаем … Ручная установка.` + сразу `. Выполняем раздел "Подготовка DTL".` (БЕЗ пустой строки) —
asciidoctor ПОГЛОЩАЕТ маркер в параграф (`<p>…\n. Выполняем …</p>`), у нас `. Выполняем` стартовал список → весь хвост
документа десинкался (134 token-diff).

**Корень (verified `parser.rb` 2.0.23, строки 36-40 + 754/764 + 962-968):**
`read_paragraph_lines reader, break_at_list = (skipped == 0 && options[:list_type])`. `StartOfListProc`/`StartOfBlockOrListProc`
ломают параграф на `AnyListRx` (любой маркер: `*`/`-`/`.`/`term::`) ТОЛЬКО когда `break_at_list` true, т.е. ВНУТРИ
списочного контекста. На верхнем уровне (`list_type` nil) маркер сразу за текстом параграфа = обычный текст, НЕ список.
Наши `scan_paragraph` (block.rs ~2611) и `scan_admonition` (~3020) ломали параграф на маркере БЕЗУСЛОВНО — known divergence
(старый комментарий это признавал).

**Фикс (1 файл, only adoc-parser/src/block.rs, оба цикла чтения параграфа):**
- break на `is_list_marker_unordered`/`is_list_marker_ordered`/`is_description_list_marker` обёрнут в
  `self.is_directly_in_list_context()` (callout уже был под `is_in_callout_list`).
- **`is_directly_in_list_context` (НЕ `is_in_list_context`!)** — точный аналог `options[:list_type]`: false, когда между
  нами и list-item на стеке стоит DelimitedBlock/PartIntro (asciidoctor парсит тело такого блока в свежем контексте без
  `list_type`) → внутри `--`/`====`/`****` вложенного в список маркер тоже поглощается. Проба `* outer`+`+`+`--`+`Para`+
  `* item`+`--` подтвердила: с `is_in_list_context` ломалось, с `is_directly_…` совпало.
- старый комментарий «known pre-existing divergence» удалён, новый объясняет правило `break_at_list` + почему `directly`.

**Тесты:** +1 html-тест `test_list_does_not_interrupt_paragraph_html` (adoc-html/src/tests.rs): ordered/unordered/dlist/
admonition поглощение + регрессии (список после пустой строки стартует, absorbed-then-blank-then-list, nested внутри
списка).

### Верификация
- clippy `--workspace` **0**. **test --workspace 0 упавших** (всего **1307 passed**; html 540, parser 648, compat 233,
  render-core 25, integration 29, html-compat).
- **БАЙТ-НЕЙТРАЛЬНО на старых корпусах:** база `/tmp/adoc_base` ПЕРЕСОБРАНА от текущего master `c8bf4ed`
  (stash→checkout master→`cargo clean --release -p adoc-parser`→build→cp→checkout branch→pop→clean+rebuild).
  - gate 344 — `gate_check.py` **0 diff**.
  - frontier(250)+adoc2docx(52)=302 — `/tmp/sweep_bvn.py` **0 diff**.
- **notes Identical 77→78** (sbertech → identical, выпал из расхождений).
- 12+ CLI-проб == asciidoctor 2.0.23 (поглощение `*`/`-`/`.`/`term::`/admonition; open-block-в-списке поглощает;
  регрессии list-first/после-blank/nested/callout/continuation-sibling все совпали).

### Состояние репо
- Ветка `fix/list-no-interrupt-paragraph` от master `c8bf4ed`, НЕ закоммичена (ждёт запроса коммит/merge/push).
- Изменено: `adoc-parser/src/block.rs` (gate на `is_directly_in_list_context` в 2 циклах + комментарии),
  `adoc-html/src/tests.rs` (+1 тест), TODO.md (+F-BO), session.md.
- `/tmp/adoc_base` = бинарь master `c8bf4ed` (актуальная база регресс-гарда этой сессии).

### Пре-существующая дивергенция (НЕ в scope F-BO, идентична master — проверено base-бинарём)
continuation-параграф через `+` с ГЛУБЖЕ-вложенным маркером (`. item`+`+`+`attached para`+`.. nested`): asciidoctor
ПОГЛОЩАЕТ `.. nested` в параграф, мы стартуем nested-список. SIBLING-маркер (`* sibling`/`. sibling`) корректно ломает
у обоих. Это отдельное правило list-continuation (как asciidoctor читает строки list-item), НЕ затронуто фиксом F-BO.

### Остаток notes (3 чистых расхождения — классы верифицированы showdiff, кандидаты на след. сессии)
По убыванию diff:
- **windows/wsl (95)** — автолинк URL внутри backtick inline-кода захватывает ЗАКРЫВАЮЩИЙ backtick в href
  (`<a href="https://rubygems.org\`">`), граница code неверная.
- **keycloak/index (52)** — автолинк URL `http://<host>:<port>/…` (со спецсимволами `<>`) внутри inline-кода: asciidoctor
  делает `<a class="bare">` внутри `<code>`, мы НЕ автолинкуем (URL остаётся текстом).
- **ansible/tips (41)** — `[source,yaml]` примыкает к list-item БЕЗ пустой строки → asciidoctor ТЕРЯЕТ source-роль (plain
  `<pre>`), у нас остаётся `<pre class="highlight"><code class="language-yaml">`.

### Методология (без изменений)
`frontier_parity.py <roots>` / `showdiff.py <file>` (семантический DOM, ПРАВИЛЬНАЯ метрика для не-verbatim — байт только
ВНУТРИ `<pre>`, см. [[feedback_html_byte_parity_scope]]). `gate_check.py` + `/tmp/sweep_bvn.py` — байт регресс-гард
(база `/tmp/adoc_base` пересобирать от текущего master). Бинарь: `cargo build --release -p adoc-cli`.
⚠ **mtime на /mnt/c НЕ обновляется надёжно** — надёжно только `cargo clean --release -p adoc-parser` перед build
(см. [[feedback_wsl_build_staleness]]). asciidoctor 2.0.23 для проб (наш `--no-standalone` ≈ asciidoctor `-s`).
⚠ inline разбор идёт через subst/ (string-rewriting), inline.rs InlineState — LEGACY (см. [[proj_sequential_quotes_rewrite]]).
НЕ доверять метке прошлой сессии — showdiff каждый кандидат (см. [[feedback_frontier_triage]]). Источник реальных корпусов:
`/mnt/c/Work/docs/notes/modules/` (81 .adoc). asciidoctor paragraph-чтение: `parser.rb` `read_paragraph_lines` +
`StartOfBlockProc`/`StartOfListProc`/`StartOfBlockOrListProc` (строки 36-40), `break_at_list = skipped==0 && list_type`.
