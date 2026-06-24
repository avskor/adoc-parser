# Session context

## Сессия (2026-06-24, 18-я) — F-BU: литеральный (indented) параграф поглощает последующие строки до границы блока

Запрос «начни следующую задачу». master `de8d3fa` (F-BT смержен). Прошлая сессия рекомендовала F-BU как «PlantUML
@dot/@enddot», но методология [[feedback_frontier_triage]] требует свежего триажа → **метка оказалась ОШИБОЧНОЙ**.

### Триаж (свежий, не доверяя метке)
`frontier_parity.py` по 4 корпусам: **docs** 206 ident / 3 clean (cheatsheet 125 = F-BU; wsl 95 + keycloak 52 =
архитектурный автолинк-реордер, НЕ single-session); **frontier** 230 ident / 3 clean (manpage 146; doctime-localtime 1 =
недетерминированный `{localtime}` = НЕ баг; migration 1 = `{asciidoctor-version}` интринсик = спорно, мы не asciidoctor);
**adoc2docx** 45 ident / 4 крупных (1105/681/291/195, переплетённые). Выбран **cheatsheet (F-BU)** — узкий, single-root.

showdiff [1056] + рендер региона: `[uml]` open-блок (`--`) с `@startdot…@enddot`. Диаграммного расширения НЕТ — asciidoctor
рендерит контент как обычные параграфы. В source-стр. 355-364: после пустой строки idented `  node1 -> node2 -> node3`
(стр.362) стартует **литеральный параграф**, а НЕ-idented `}` (363) и `@enddot` (364) asciidoctor поглощает в ТОТ ЖЕ `<pre>`
(сохраняя ведущие 2 пробела первой строки). Мы рвали на `}` → `}\n@enddot` в отдельный параграф + стрип индентации opener'а.

### Правило (verified 12 проб A-L vs asciidoctor 2.0.23 = `StartOfBlockProc`)
Литеральный параграф (открыт indented-строкой) продолжается на ВСЕ смежные непустые строки независимо от отступа. Обрыв
ТОЛЬКО на: пустой строке / делимитере (`----`,`====`,`....`,`--`,`++++`,`____`,`****`,`////`) / md-fence ` ``` ` / table
`|===` / block-attr `[...]` / list-continuation `+`; (в list-контексте — list/dlist/callout-маркеры). НЕ обрывают
(поглощаются verbatim): не-idented текст, секция `== T`, admonition `NOTE:`, list-маркер ВНЕ списка, line-comment `//`.
Индентация: общий минимальный отступ стрипается (min=0 если есть flush-left строка → сохраняется всё). Уже было корректно.

### Фикс F-BU (1 файл, only adoc-parser/src/block.rs)
`scan_literal_paragraph`: цикл-сборщик (был `break` на любой не-idented-не-comment строке) → заменён на набор терминаторов
выше. Guard `!lines.is_empty()` гарантирует поглощение opener'а (защита от одиночного idented `+`: `is_list_continuation`
ловит его через `trim`). Спец-кейс line-comment удалён — комментарии теперь поглощаются естественно (probe J ✓).
scanner-функции (`is_delimiter`/`is_block_attribute`) отвергают ведущие пробелы → idented `  ----` остаётся литералом (probe L ✓).

**Тесты:** +1 parser `test_literal_paragraph_absorbs_following_lines` (поглощение + min-indent strip + обрыв на делимитере),
+1 html `test_literal_paragraph_absorbs_following_lines_html` (adoc-html/tests/html_output.rs).

### Верификация
- clippy `--workspace` **0**. **test --workspace 0 упавших** (parser **651**, html **544**, integration 29→**30**,
  compat 233/233, html-compat, render-core 25, author 7, cli 2).
- **БАЙТ-НЕЙТРАЛЬНО:** база `/tmp/adoc_base` пересобрана от master `de8d3fa` (md5 `0b70065`); gate 344 (`gate_check.py`)
  **0 diff**; свип 860 файлов (`scratchpad/sweep_all.py` пересоздан в session-scratchpad) — изменился **ТОЛЬКО 1** (целевой cheatsheet).
- Семантически (vs asciidoctor 2.0.23): cheatsheet **125→58** позиционно. Set-diff Counter: base 13→**7** уникальных, причём
  ВСЕ 7 были и в базе (`#mark#`/tree = F-BV) — фикс удалил ровно 6 элементов (весь dot-diagram split) и **0 новых**.
  dot-diagram блок теперь БАЙТ-в-байт == asciidoctor (`<pre>  node1 -&gt; node2 -&gt; node3\n}\n@enddot</pre>`).

### Состояние репо
- Ветка `fix/literal-paragraph-continuation` от master `de8d3fa`. Коммит `1b18515`. **Merge/push — ПО ЗАПРОСУ (ещё не смержено).**
- Изменено: `adoc-parser/src/block.rs` (терминаторы literal-параграфа), `adoc-parser/tests/integration.rs` (+1 тест),
  `adoc-html/tests/html_output.rs` (+1 тест), TODO.md (F-BU→[x] + F-BV follow-up), session.md.
- `/tmp/adoc_base` = бинарь master `de8d3fa` (md5 `0b70065`, актуальная база регресс-гарда).

### Кандидаты след. сессий
- **F-BV (остаток cheatsheet 58):** `#…#` mark/highlight внутри `[tree]` open-блоков — ОТДЕЛЬНЫЙ inline-слой
  (string-rewriting движок [[proj_sequential_quotes_rewrite]]), глубокий `#`/`##` corner через мультистрочный параграф +
  ASCII-дерево `root\n|-- …` разбито иначе. Низкоценно (tree-extension garbage-in, сам asciidoctor выдаёт артефакт).
- **migration.adoc (frontier, diff=1):** `{asciidoctor-version}` интринсик → `2.0.23`. Узко, но спорно (мы не asciidoctor;
  семантически «врать» о версии). Если делать — сеять как compat-таргет 2.0.23 в CLI/html-слое (как F-BT backend-intrinsics).
- **manpage.adoc (frontier, 146):** manpage backend — специализированный, крупный, отдельный триаж.
- **windows/wsl(95)+keycloak/index(52):** 2 архитектурных автолинка (`macros` до `quotes`/specialchars, реордер; НЕ
  single-session, см. [[proj_sequential_quotes_rewrite]]).
- **adoc2docx (4 крупных: 1105/681/291/195):** переплетённые дефекты, нужен showdiff-триаж изолировать доминирующий.
- **Отложенный doctype-intrinsics** (под F-BT): `ifdef::doctype-book/manpage/inline[]` — пересчёт при смене `:doctype:`. Малочастотно.

### Методология (без изменений, см. [[compat_corpus_methodology]] + [[feedback_html_byte_parity_scope]] + [[feedback_frontier_triage]])
`frontier_parity.py <root>` / `showdiff.py <file>` (семантический ПОЗИЦИОННЫЙ DOM-differ; скрипты `/mnt/c/tmp/adoc-test/`).
⚠ showdiff раздувает один upstream-рассинхрон в хвост — сверять SET элементов (Counter), не позиции (см. set-diff выше).
Корни: gate `/mnt/c/tmp/adoc-test`(344), frontier `/mnt/c/tmp/adoc-frontier`(250), adoc2docx `/mnt/c/tmp/adoc2docx`(52),
docs `/mnt/c/Work/docs`(214). Регресс-гард: `gate_check.py` (база `/tmp/adoc_base` пересобирать от ТЕКУЩЕГО master:
checkout master→`cargo clean --release -p adoc-cli`→build→cp→checkout branch) + `scratchpad/sweep_all.py` (raw-байт свип всех
4 корпусов). ⚠ mtime на /mnt/c ненадёжен → `cargo clean --release -p adoc-cli` перед каждым build (см. [[feedback_wsl_build_staleness]]).
НЕ доверять метке прошлой сессии — git log + showdiff + минимальные пробы каждый кандидат (эта сессия: метка «PlantUML» = ошибка).
