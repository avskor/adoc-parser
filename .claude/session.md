# Session context

## Сессия (2026-06-23, 16-я) — F-BS: auto-id секции из ФОРМАТИРОВАННОГО заголовка (по видимому тексту, не по сырому markup)

Запрос «начни следующую задачу». master `c24af34` (F-BR смержен).

### Триаж (кандидат session.md прошлой сессии — `cheatsheet.adoc`)
`/mnt/c/Work/docs/rest_api/cheatsheet3016790939091029093/cheatsheet.adoc` (474 стр., single-root, 0 include, 659 diff).
showdiff выявил **3 НЕЗАВИСИМЫХ дефекта**:
1. **section-id из форматированного заголовка** (доминирующий, 9 секций) → **F-BS, СДЕЛАНО**.
2. **`ifdef::backend-html5[]` не вычисляется** (backend-intrinsics не сеются CLI) → passthrough-секция выпадает →
   весь хвост рассинхронизируется позиционным differ'ом (классика [[feedback_frontier_triage]]) → **F-BT, OPEN**.
3. **PlantUML `@dot/@enddot` блок** (~125 diff остаток, source-стр. ~303) → **F-BU, OPEN**.

### Корень F-BS (verified asciidoctor 2.0.23, пробы [underline]#…#/_…_/*…*/`…`/^…^/[.role]#…#)
Asciidoctor строит id из *подставленного* (`apply_title_subs`) заголовка, у которого `InvalidSectionIdCharsRx` затем
срезает HTML-теги → инлайн-разметка вносит ВИДИМЫЙ ТЕКСТ, но не маркеры. Маппинг подтверждён:
- `[underline]#Basic formats#` → `_basic_formats` (было `_underlinebasic_formats`)
- `_Sidebar_ block` → `_sidebar_block` (было `__sidebar_block`)
- `*Bold*`→`_bold_…`, `` `code` ``→`_a_code_…`, `Super^script^`→`_superscript_…` (маркеры исчезают, текст склеивается без сепаратора)
- bare URL: `https://x.com` → текст СОХРАНЯЕТСЯ (`_…httpsexample_com…`) — наш `strip_urls_for_id` его УДАЛЯЕТ (расхождение,
  НО вне cheatsheet и НЕ регрессировано: strip_urls остаётся в пайплайне; см. ниже). icon/link → strip_urls обрабатывает.

### Фикс F-BS (1 файл, only adoc-parser/src/block.rs)
`generate_title_id`: вставлен шаг `Self::strip_inline_formatting(&resolved)` МЕЖДУ resolve_title_attr_refs и
apply_typographic_replacements. Новая ассоц-функция `strip_inline_formatting(title) -> Cow`:
- fast-path: нет маркера `* _ ` # ^ ~ [` → `Cow::Borrowed` (без парсинга);
- иначе `InlineParser::parse_str_with_subs(title, QUOTES)` (subs строится `NONE`+`add(QUOTES)` — поле приватно вне модуля),
  склейка Text/Code/InlinePassthrough, SoftBreak/HardBreak→пробел, остальное игнор.
- Только QUOTES → macros/URL/icon остаются ЛИТЕРАЛОМ → `generate_id`'s `strip_urls_for_id` работает как раньше (нет
  регресса по URL/icon), типографика следом.

**Тесты:** +1 parser `test_section_id_strips_inline_formatting` (6 кейсов, block.rs), +1 html
`test_section_id_strips_inline_formatting_html` (adoc-html: role/em/strong/code/sup + негатив no-`_underline`).

### Верификация
- clippy `--workspace` **0**. **test --workspace 0 упавших** (html 543→**544**, parser 650→**651**, compat **233/233**,
  html-compat 47, integration 29, render-core 25).
- **БАЙТ-НЕЙТРАЛЬНО:** база `/tmp/adoc_base` ПЕРЕСОБРАНА от master `c24af34` (md5 base `447ced4`, new `5b7323a`).
  - gate 344 (`gate_check.py`) — **0 diff** (в gate-корпусе нет форматированных заголовков).
  - свип 860 файлов (`scratchpad/sweep_all.py` пересоздан в session-scratchpad: gate+frontier+adoc2docx+docs) — изменился
    **ТОЛЬКО 1**: целевой `cheatsheet.adoc`.
- Семантически (vs asciidoctor): cheatsheet все heading-id ТЕПЕРЬ совпадают (diff `<h[1-6] id>` = 0). Полный diff
  659→125 при доп. `-a backend-html5` (остаток 125 = F-BU PlantUML; без backend-html5 хвост рассинхронен = F-BT).

### Состояние репо
- Ветка `fix/section-id-from-formatted-title` от master `c24af34`. **Merge/push — ПО ЗАПРОСУ** (ещё не смержено).
- Изменено: `adoc-parser/src/block.rs` (generate_title_id + strip_inline_formatting + 1 unit-тест),
  `adoc-html/src/tests.rs` (+1 тест), TODO.md (+F-BS/F-BT/F-BU), session.md.
- `/tmp/adoc_base` = бинарь master `c24af34` (md5 `447ced4`, актуальная база регресс-гарда).

### Кандидаты след. сессий
- **F-BT (backend-html5 intrinsics)** — чистый, узкий, CLI-слой (adoc-cli/main.rs seed). Закроет хвост cheatsheet (659→125).
- **F-BU (PlantUML `@dot/@enddot`)** — остаток cheatsheet 125, нужен отдельный triage.
- Прочее (из прошлой session.md, ещё актуально): `windows/wsl`(95)+`keycloak/index`(52) — 2 архитектурных автолинка
  (`macros` до `quotes`/specialchars, реордер; НЕ single-session, см. [[proj_sequential_quotes_rewrite]]).
- Если docs исчерпан — РАСШИРЯТЬ КОРПУС (см. [[compat_corpus_methodology]]): `frontier_parity.py <новый-root>`.

### Методология (без изменений, см. [[compat_corpus_methodology]] + [[feedback_html_byte_parity_scope]])
`frontier_parity.py <root>` / `showdiff.py <file>` (семантический DOM; скрипты в `/mnt/c/tmp/adoc-test/`). Корни корпусов:
gate `/mnt/c/tmp/adoc-test`(344), frontier `/mnt/c/tmp/adoc-frontier`(250), adoc2docx `/mnt/c/tmp/adoc2docx`(52),
docs `/mnt/c/Work/docs`(214). Регресс-гард: `gate_check.py` (база `/tmp/adoc_base` пересобирать от ТЕКУЩЕГО master через
stash→clean→build→cp→pop) + `scratchpad/sweep_all.py` (raw-байт свип всех 4 корпусов; пересоздавать в session-scratchpad).
Бинарь: `cargo build --release -p adoc-cli`. ⚠ mtime на /mnt/c ненадёжен → `cargo clean --release -p adoc-parser` перед build
(см. [[feedback_wsl_build_staleness]]). НЕ доверять метке прошлой сессии — showdiff каждый кандидат.
