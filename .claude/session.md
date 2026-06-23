# Session context

## Сессия (2026-06-23, 15-я) — F-BR: section-title внутри list-continuation (`=== T`/setext после `+` → секция вместо параграфа)

Запрос «начни следующую задачу». master `c5f0d8b` (F-BQ смержен).

### Триаж (метка прошлой сессии устарела, см. [[feedback_frontier_triage]])
session.md 14-й сессии указывал на `_responses`/`_artifacts` в `http-api-design/src/main/asciidoc` — но showdiff показал их 0-diff
(уже починены F-BP/F-BQ). Свежий `frontier_parity.py /mnt/c/Work/docs` → **202 identical, 7 clean**. Реальные кандидаты d=8 —
**`.deploy`-версии** (`http-api-design.deploy/src/main/adoc/_responses.adoc` + `_artifacts.adoc`). Выбран как чистый single-root.

### Корень (verified asciidoctor 2.0.23, пробы A–G + S_plus/S_tilde)
Секция НИКОГДА не ребёнок list-item. При list-continuation `+` следующий блок парсится в контексте элемента; section-title там
**ДЕГРАДИРУЕТ в литеральный параграф** `<p>=== T</p>` внутри `<li>`:
- **ATX** (`* x` / `+` / `=== T`): `=== T` → параграф (кейсы A=cont+blank+heading, C=cont+adjacent, F=1 item) — БАЙТ-в-байт asciidoctor.
- **setext** (`* x` / `+` / `на` / `~~`): `на\n~~` → параграф (а на section-level `на` над `+`/`~~`/`^^` = setext sect4/sect2/sect3,
  ПОДТВЕРЖДЕНО S_plus1/S_tilde3 — `strip_setext_title` маппинг `=`1/`-`2/`~`3/`^`4/`+`5 КОРРЕКТЕН, не трогать).
- БЕЗ `+` (кейс B): пустая строка закрывает список, секция создаётся нормально (уже верно).

Тип триггера — наш флаг `self.in_continuation`.

### Фикс (1 файл, only adoc-parser/src/block.rs)
`scan_leaf_blocks`: guard `!self.in_continuation` на ОБА детекта секции:
- ATX `if !self.in_continuation && let Some((level,title)) = strip_any_section_marker(line)` (был безусловный).
- setext-ветка: добавлен `!self.in_continuation &&` в начало цепочки условий.
Строка проваливается в `scan_paragraph_fallback` (в continuation `is_directly_in_list_context && !in_continuation` ложно →
не закрывает список → читается как контент параграфа li).

**Тесты:** +1 parser event `test_list_continuation_demotes_section_title` (block.rs), +1 html
`test_section_title_demoted_in_list_continuation_html` (adoc-html, ATX + setext + негативы no-`<h3>`/`<h5>`/`sect`).

### Верификация
- clippy `--workspace` **0**. **test --workspace 0 упавших** (html 542→**543**, parser 649→**650**, compat **233/233**,
  html-compat, integration 29, render-core 25).
- **БАЙТ-НЕЙТРАЛЬНО:** база `/tmp/adoc_base` ПЕРЕСОБРАНА от master `c5f0d8b` (md5 base `1fed8ba`, new `447ced4`).
  - gate 344 (`gate_check.py`) — **0 diff**.
  - свип 860 файлов (`scratchpad/sweep_all.py` пересоздан в session-scratchpad: gate+frontier+adoc2docx+docs) — изменились
    **5**: 2 целевых `.deploy` (`_responses`,`_artifacts`) + 2 vue (`vue_ts_springboot` ×2) + родитель `http-api-design.adoc`
    (через include). Все улучшились.
- Семантически: **docs Identical 202→206** (+4: 2 `.deploy` + 2 vue к 0-diff vs asciidoctor); родитель `http-api-design.adoc`
  3926→3712 diff-lines (улучшение, остаток — несвязанный include-шум). frontier/adoc2docx/notes/gate стабильны.
- **vue флипнул через setext-демотацию, НЕ ATX** (`на` над `+`-маркером continuation): один upstream-рассинхрон раздувался
  позиционным differ'ом до 273 + ложная преамбула (документ «обзаводился секциями» через ложные sect4 `на` ×6). Классика
  [[feedback_frontier_triage]] — починка upstream обнуляет downstream.

### Состояние репо
- Ветка `fix/section-title-in-list-continuation` от master `c5f0d8b`, коммит **`b522081`** (`fix(lists): demote section title in
  list continuation to paragraph (F-BR)`). **Merge/push — ПО ЗАПРОСУ** (ещё не смержено).
- Изменено: `adoc-parser/src/block.rs` (2 guard'а + 1 unit-тест), `adoc-html/src/tests.rs` (+1 тест), TODO.md (+F-BR), session.md.
- `/tmp/adoc_base` = бинарь master `c5f0d8b` (md5 `1fed8ba`, актуальная база регресс-гарда).

### Остаток docs-корпуса (кандидаты след. сессий)
- `cheatsheet.adoc` (659) — крупный, требует showdiff-триажа на single/multi-root.
- `windows/wsl` (95) + `keycloak/index` (52) — те же 2 архитектурных автолинка (`macros` до `quotes`/specialchars, реордер;
  НЕ single-session, см. [[proj_sequential_quotes_rewrite]]).
- Если docs исчерпан — РАСШИРЯТЬ КОРПУС (см. [[compat_corpus_methodology]]): `frontier_parity.py <новый-root>`.

### Методология (без изменений, см. [[compat_corpus_methodology]] + [[feedback_html_byte_parity_scope]])
`frontier_parity.py <root>` / `showdiff.py <file>` (семантический DOM, скрипты в `/mnt/c/tmp/adoc-test/`). Регресс-гард:
`gate_check.py` (база `/tmp/adoc_base` пересобирать от текущего master) + `scratchpad/sweep_all.py` (raw-байт свип всех 4
корпусов; пересоздавать в session-scratchpad). Бинарь: `cargo build --release -p adoc-cli` (НЕ adoc-html — stale). ⚠ mtime на
/mnt/c ненадёжен → `cargo clean --release -p adoc-parser` перед build (см. [[feedback_wsl_build_staleness]]). НЕ доверять метке
прошлой сессии — showdiff каждый кандидат.
