# Session context

## Сессия (2026-06-23, 17-я) — F-BT: CLI сеет html5 backend-intrinsics (ifdef::backend-html5 + inline {basebackend}/{filetype}/{outfilesuffix})

Запрос «начни следующую задачу». master `bdb5355` (F-BS смержен; session.md прошлой сессии устарел — писался ДО мержа,
git показал Merge уже сделан). Выбран кандидат **F-BT** из TODO (чистый, узкий, CLI-слой — рекомендация прошлой сессии).

### Триаж (verified asciidoctor 2.0.23, прямые пробы через CLI)
Asciidoctor при html5-рендере ставит intrinsics: `backend=html5`, `backend-html5`(set), `basebackend=html`,
`basebackend-html`(set), `filetype=html`, `filetype-html`(set), `outfilesuffix=.html`, `doctype=article`, `doctype-article`(set).
Наш `adoc-cli` сеял docname/docdate-семейство, но НЕ backend → проба подтвердила: `ifdef::backend-html5[]`→**NO**,
`{basebackend}`/`{filetype}`/`{outfilesuffix}` оставались **литералом**. **Корень:** `ifdef`/`ifndef` вычисляются в
ПРЕПРОЦЕССОРЕ (`preprocess_with_attrs`, до html-рендера) — CLI не клал backend-intrinsics в `initial_attrs`; html-слой
(`adoc-html/src/lib.rs:371-373`) сеет `backend`/`doctype` в `document_attrs` ТОЛЬКО для inline `{backend}`, не для препроцессора.

### Фикс F-BT (1 файл, only adoc-cli/src/main.rs)
После блока date/local-семейства (перед docname-блоком) через существующий `seed`-хелпер (кладёт в `initial_attrs` ДЛЯ
препроцессора + `html_attrs` ДЛЯ рендера) засеяны: `backend=html5`, `backend-html5`(пусто=set), `basebackend=html`,
`basebackend-html`(set), `filetype=html`, `filetype-html`(set), `outfilesuffix=.html`.
- **`doctype-<value>` СОЗНАТЕЛЬНО НЕ сеется:** проба `:doctype: book` → asciidoctor даёт `doctype-book`(set), НЕ
  `doctype-article`; суффикс трекает значение doctype (header `:doctype:` может сменить), а intrinsic мы не пересчитываем →
  жёсткий `doctype-article` остался бы ложно set для book/manpage. Вынесено в отложенный остаток (TODO под F-BT).
- seed НЕ локает → header `:outfilesuffix: .adoc`/`:doctype: book` по-прежнему ПОБЕЖДАЕТ (проба ✓, `doctype` обновляется
  препроцессором, `ifndef::backend-html5[]` корректно дропает PDF-only контент).

**Тесты:** новый файл `adoc-cli/tests/cli.rs` (+2 integration, гоняют бинарь через `env!("CARGO_BIN_EXE_adoc")` +
`CARGO_TARGET_TMPDIR`, без доп. зависимостей): `seeds_backend_intrinsics` (ifdef + inline) + `header_can_override_outfilesuffix`.

### Верификация
- clippy `--workspace` **0**. **test --workspace 0 упавших** (html **544**, parser **651**, cli **+2** новых, compat 233/233,
  html-compat 47, integration 29, render-core 25, author 7).
- **БАЙТ-НЕЙТРАЛЬНО (где ожидалось):** база `/tmp/adoc_base` ПЕРЕСОБРАНА от master `bdb5355` (md5 base `5b7323a`, new `0b70065`).
  - gate 344 (`gate_check.py`) — **0 diff** (gate-корпус не использует backend-html5).
  - свип 860 файлов (`scratchpad/sweep_all.py` пересоздан в session-scratchpad: gate+frontier+adoc2docx+docs) — изменился
    **ТОЛЬКО 1**: целевой `cheatsheet.adoc` (backend-html5 в корпусах редок → фикс предельно узкий).
- Семантически (vs asciidoctor 2.0.23): cheatsheet **659→125 БЕЗ ручного `-a backend-html5`** — элементы 0..1055 идентичны
  (passthrough/backend-html5 секции больше НЕ выпадают, все heading-id совпадают); первое расхождение [1056] = PlantUML
  `@dot/@enddot` (F-BU). Set-сравнение (ref 1192 / our 1198): по существу различаются **~6 элементов, ВСЕ = F-BU**
  (PlantUML/диаграммы + контекст: `#...#` highlight внутри passthrough — базовый `#mark#` сам == asciidoctor; literal-дерево).
  НЕ внесены F-BT.

### Состояние репо
- Ветка `fix/cli-seed-backend-intrinsics` от master `bdb5355`. Коммит `200867c`. **Merge/push — ПО ЗАПРОСУ** (ещё не смержено).
- Изменено: `adoc-cli/src/main.rs` (seed backend-intrinsics), `adoc-cli/tests/cli.rs` (НОВЫЙ, +2 теста), TODO.md (F-BT→[x] +
  отложенный doctype-intrinsics остаток), session.md.
- `/tmp/adoc_base` = бинарь master `bdb5355` (md5 `5b7323a`, актуальная база регресс-гарда).

### Кандидаты след. сессий
- **F-BU (PlantUML `@dot/@enddot`)** — остаток cheatsheet 125 (set-diff ~6 элементов); первое расхождение [1056], source-стр.
  ~303 «PlantUML Extension». Нужен showdiff-триаж: как asciidoctor рендерит diagram-блок (вероятно literal/listing fallback
  без расширения). Также внутри: `#...#` highlight в passthrough-контексте, literal-дерево разбито иначе.
- **Отложенный doctype-intrinsics** (под F-BT в TODO): `ifdef::doctype-book/manpage/inline[]` — пересчёт `doctype-<value>` при
  смене `:doctype:`. Малочастотный.
- Прочее (из прошлых session.md, ещё актуально): `windows/wsl`(95)+`keycloak/index`(52) — 2 архитектурных автолинка
  (`macros` до `quotes`/specialchars, реордер; НЕ single-session, см. [[proj_sequential_quotes_rewrite]]).
- Если docs исчерпан — РАСШИРЯТЬ КОРПУС (см. [[compat_corpus_methodology]]): `frontier_parity.py <новый-root>`.

### Методология (без изменений, см. [[compat_corpus_methodology]] + [[feedback_html_byte_parity_scope]] + [[feedback_frontier_triage]])
`frontier_parity.py <root>` / `showdiff.py <file>` (семантический ПОЗИЦИОННЫЙ DOM-differ; скрипты в `/mnt/c/tmp/adoc-test/`).
⚠ showdiff раздувает один upstream-рассинхрон в хвост — сверять SET элементов (Counter), не позиции. Корни корпусов:
gate `/mnt/c/tmp/adoc-test`(344), frontier `/mnt/c/tmp/adoc-frontier`(250), adoc2docx `/mnt/c/tmp/adoc2docx`(52),
docs `/mnt/c/Work/docs`(214). Регресс-гард: `gate_check.py` (база `/tmp/adoc_base` пересобирать от ТЕКУЩЕГО master через
checkout master→clean→build→cp→checkout branch) + `scratchpad/sweep_all.py` (raw-байт свип всех 4 корпусов; пересоздавать в
session-scratchpad). Бинарь: `cargo build --release -p adoc-cli`. ⚠ mtime на /mnt/c ненадёжен → `cargo clean --release -p
adoc-cli` перед build (см. [[feedback_wsl_build_staleness]]). НЕ доверять метке прошлой сессии — git log + showdiff каждый кандидат.
