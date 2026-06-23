# Session context

## Сессия (2026-06-23, 14-я) — F-BQ: closed-ATX заголовок секции (`== Title ==` → трейлинг-маркеры не отбрасывались)

Запрос «начни следующую задачу». master `8dc6539` (F-BP смержен — session.md 13-й сессии устарел, писался ДО коммита/мержа F-BP).

### Триаж — РАСШИРЕНИЕ КОРПУСА (ключевой результат сессии)
Старые корпуса исчерпаны на чистых блочных фиксах:
- **notes** (`/mnt/c/Work/docs/notes/modules`, 81 .adoc) — 79 identical, остаток = 2 архитектурных автолинка (windows/wsl 95,
  keycloak/index 52; оба `macros` до `quotes`, см. [[proj_sequential_quotes_rewrite]] — НЕ single-session).
- **frontier** (`/mnt/c/tmp/adoc-frontier`, 250) — 230 identical, остаток = manpage др. бэкенд + `{asciidoctor-version}`
  интринсик (migration.adoc) + localtime (non-bug).
- **adoc2docx** (`/mnt/c/tmp/adoc2docx`, 34) — 45 identical, остаток = rouge-подсветка ruby/yaml (source/xml/test/callouts;
  крупная фича, не блочный фикс).
- **НОВОЕ: `/mnt/c/Work/docs` целиком** — 96 .adoc ВНЕ notes (http-api-design, mgp, kubernetes-best-practices…) ранее не
  майнились. `frontier_parity.py /mnt/c/Work/docs` → **200 identical, 9 чистых расхождений**. Свежие мелкие кандидаты!

### Сделано — F-BQ (ветка `fix/closed-atx-section-title` от master `8dc6539`)
**Баг:** Release.adoc / BranchesInPTS.adoc в «closed-ATX» стиле (`= T =`, `== T ==`, `=== T ===`, `==== T ====`). asciidoctor
отбрасывает закрывающий run, равный ведущему; мы оставляли как текст (`Выпуск версии ==`) + портили auto-id.

**Корень (verified asciidoctor `SectionTitleRx` `/^((?:=|#){1,6})(?!\1[=#])\s+(\S.*?)(?:\s+\1)?$/`):**
опциональная группа `(?:\s+\1)?` — трейлинг run, ТОЧНО равный ведущему (тот же символ + тот же count), с whitespace перед
ним, отбрасывается ДО генерации id. Асимметричный (`== T =`) или беспробельный (`== T==`) трейлинг остаётся литералом.
Markdown ATX `## T ##` ведёт себя так же (проверено пробой) → фикс в обеих функциях.

**Фикс (1 файл, only adoc-parser/src/scanner.rs):**
- новый хелпер `strip_closed_atx_trailer(title, marker, level) -> &str` (zero-copy суб-слайс): считает трейлинг run байта
  `marker`, если `run == level` И перед run есть whitespace И остаток непуст → обрезает + trim_end; иначе title как есть.
- вызывается в `strip_section_marker` (marker `b'='`) и `strip_markdown_heading` (marker `b'#'`) после `rest[1..].trim()`.

**Тесты:** +1 scanner unit `test_strip_closed_atx_section_title` (симметрия/асимметрия/беспробел/`=`-в-title/markdown);
+1 html `test_closed_atx_section_title_html` (adoc-html: `Closed`/h4 `master`/`Asym =` kept/`NoSpace==` kept).

### Верификация
- clippy `--workspace` **0**. **test --workspace 0 упавших** (html **542** (+1), parser 649, compat 233, render-core 25,
  integration 29, html-compat).
- **БАЙТ-НЕЙТРАЛЬНО:** база `/tmp/adoc_base` ПЕРЕСОБРАНА от текущего master `8dc6539` (stash→checkout master→`cargo clean
  --release -p adoc-parser`→build→cp→checkout branch→pop→clean+rebuild; md5 base `ad31ee7`, new `1fed8ba`).
  - gate 344 — `gate_check.py` **0 diff**.
  - **свип ВСЕХ корпусов** (`scratchpad/sweep_all.py`, 860 файлов raw-байт base-vs-new: gate+frontier+adoc2docx+docs) —
    изменились **ТОЛЬКО 2 целевых файла** (Release.adoc, BranchesInPTS.adoc).
- Семантически без регрессий: frontier 230, adoc2docx 45, notes 79 СТАБИЛЬНЫ; **docs Identical 200→202** (оба флипнули).
- 5 CLI-проб байт-в-байт == asciidoctor 2.0.23: `== Title ==`→`Title`, `== Asym =`→`Asym =`, `==== master ====`→h4 `master`,
  `== NoSpace==`→`NoSpace==`, `## MD Two ##`→`MD Two`.

### Состояние репо
- Ветка `fix/closed-atx-section-title` от master `8dc6539`, **НЕ закоммичена** (ждёт запроса коммит/merge/push).
- Изменено: `adoc-parser/src/scanner.rs` (+хелпер `strip_closed_atx_trailer`, 2 вызова, +1 unit-тест),
  `adoc-html/src/tests.rs` (+1 тест), TODO.md (+F-BQ), session.md.
- `/tmp/adoc_base` = бинарь master `8dc6539` (актуальная база регресс-гарда этой сессии).

### Остаток docs-корпуса (кандидаты след. сессий, по возрастанию diff)
- `_responses.adoc` / `_artifacts.adoc` (8 каждый) — `=== heading` ВНУТРИ list-item: asciidoctor вливает как параграф в
  элемент, у нас → отдельная секция. Родственно `read_paragraph_lines`/F-BO (контекст списка). Чистый блочный кандидат.
- vue_ts_springboot ×2 (273/270), cheatsheet (659) — крупнее, требуют showdiff-триажа на single/multi-root.
- windows/wsl (95) + keycloak/index (52) — 2 архитектурных автолинка (`macros` до `quotes`/specialchars), реордер.

### Методология (без изменений, см. [[compat_corpus_methodology]] + [[feedback_html_byte_parity_scope]])
`frontier_parity.py <root>` / `showdiff.py <file>` (семантический DOM). **РАСШИРЯТЬ КОРПУС когда старые исчерпаны:
`/mnt/c/Work/docs` целиком даёт свежие кандидаты.** Регресс-гард: `gate_check.py` (база `/tmp/adoc_base` пересобирать от
текущего master) + `scratchpad/sweep_all.py` (raw-байт свип всех 4 корпусов). Бинарь: `cargo build --release -p adoc-cli`
(НЕ adoc-html — иначе stale). ⚠ mtime на /mnt/c ненадёжен → `cargo clean --release -p adoc-parser` перед build
(см. [[feedback_wsl_build_staleness]]). asciidoctor 2.0.23, наш `--no-standalone`/CLI body ≈ `asciidoctor -s`.
НЕ доверять метке прошлой сессии — showdiff каждый кандидат (см. [[feedback_frontier_triage]]).
