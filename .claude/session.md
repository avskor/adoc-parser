# Session context

## Сессия (2026-06-24, 19-я) — F-BW: автолинк рвался на `<`/`>` в теле URL (angle-bracket плейсхолдеры)

Запрос «начни следующую задачу». master `fe8742b` (F-BU смержен). Метку прошлой сессии «архитектурный реордер
macros/quotes» НЕ принимал на веру ([[feedback_frontier_triage]]) — и она оказалась **ОШИБОЧНОЙ**.

### Триаж (свежий, 4 корпуса)
`frontier_parity.py`: **docs** 206 ident / 3 clean (wsl 95, cheatsheet 58=F-BV, keycloak 52); **frontier** 230 ident / 3
(manpage 146 = спец-бэкенд; doctime 1 = недетерм.; migration 1 = `{asciidoctor-version}` интринсик); **adoc2docx** 45 ident /
4 крупных (1105/681/291/195). showdiff всех adoc2docx → единый корень = **rouge-подсветка** (asciidoctor токенизирует
Ruby/XML/Java в `<span class=…>`, мы выдаём сырой код) — полноценный хайлайтер, НЕ single-session → снят. wsl+keycloak:
прошлая метка «macros до quotes». **12 проб vs asciidoctor 2.0.23 ОПРОВЕРГЛИ метку:** backtick-URL без плейсхолдеров уже
линкуется идентично — реальный корень узкий: `<`/`>` в теле URL.

### Правило (verified 12 проб A-G vs asciidoctor 2.0.23)
asciidoctor escape'ит `<`→`&lt;`, `>`→`&gt;` ДО `macros`-пасса → литеральные `<`/`>` в теле URL = СОДЕРЖИМОЕ, не терминаторы.
Bare-URL тянется до пробела / `[` / `]` (и закрытия constrained-спана). Исключение — angle-форма `<scheme://…>`: ведущий `<`
парится с ПЕРВЫМ `>`, оба съедаются. Пробы: `http://h/<x>/y`→включает `<x>`; `http://h/<x`→`<x`; `http://h/x<y`→`x<y`;
`http://h/a>b>c`→все `>`; `<http://h/x>`→срез скобок; `` `http://h/<x>/y` ``→линк внутри `<code>`. (Вырожденный `http://h/x>` с
хвостовым `>` перед пробелом: asciidoctor выдаёт битый `&gt`, у нас корректный `&gt;` — расхождение в 1 редком кейсе, наш вывод
правильнее.)

### Фикс F-BW (2 движка)
`subst/macros.rs` `try_autolink` (ДЕФОЛТ) + `inline.rs` `try_autolink` (legacy-оракул, контракт «Mirror of legacy»):
1. убраны `<`/`>` из find-предиката скана URL (терминаторы теперь только whitespace/`[`/`]`, плюс span-`limit` в subst);
2. angle-блок (`preceded_by_angle`) ищет закрывающий `>` ВНУТРИ `rest[..url_end]` (первый `>`) вместо проверки `== Some(b'>')`
   на позиции `url_end` (которая после п.1 уже не указывает на `>`).

**Тесты:** +2 parser unit (`inline::tests::test_autolink_keeps_angle_bracket_placeholders` +
`…angle_bracketed_bare_form_strips_brackets`), +1 subst (`reproduces_legacy_on_angle_bracket_url_inputs` — паритет
subst↔legacy + проверка целевого URL), +1 html (`html_output.rs::test_autolink_keeps_angle_bracket_placeholders_in_url`).
Пре-существующий `subst::angle_bracket_url_matches_asciidoctor` ПРОШЁЛ (closed/unclosed/`<url[text]>`/`<email>`).

### Верификация
- clippy `--workspace` **0**. **test --workspace 0 упавших** (parser 651→**654**, html_output 48→**49**, html-lib 544,
  compat 233/233, integration 30, render-core 25, author 7, cli 2).
- **БАЙТ-НЕЙТРАЛЬНО:** база `/tmp/adoc_base` пересобрана от master `fe8742b` (md5 `faf8cb3e`); gate 344 (`gate_check.py`)
  **0 diff**; свип 860 файлов (`scratchpad/sweep_all.py` ПЕРЕСОЗДАН — был утерян из прошлого session-scratchpad) — изменился
  **ТОЛЬКО 1** (целевой keycloak/index.adoc).
- Семантически (vs asciidoctor 2.0.23): **keycloak 52→0** (frontier_parity: оба файла identical); docs Identical **206→207**.

### ⚠ Ловушка сборки (повтор [[feedback_wsl_build_staleness]])
`cargo clean --release -p adoc-cli` НЕДОСТАТОЧНО когда менялся adoc-parser — его rlib переиспользуется (грубый mtime /mnt/c).
Чистить ИМЕННО модифицированные крейты: `cargo clean --release -p adoc-parser -p adoc-html -p adoc-cli`. ТАКЖЕ: коммить фикс
в ветку ДО пересборки базы — иначе `git checkout master` переносит floating-изменения с собой и база собирается с твоим кодом
(симптом: base md5 == branch md5).

### Состояние репо
- Ветка `fix/autolink-angle-brackets-in-url` от master `fe8742b`. Коммит `f26dcd5`. **Merge/push — ПО ЗАПРОСУ (ещё не смержено).**
- Изменено: `adoc-parser/src/subst/macros.rs`, `adoc-parser/src/inline.rs` (оба `try_autolink`),
  `adoc-parser/src/subst/mod.rs` (+1 тест), `adoc-html/tests/html_output.rs` (+1 тест), TODO.md (F-BW→[x] + wsl follow-up), session.md.
- `/tmp/adoc_base` = бинарь master `fe8742b` (md5 `faf8cb3e`, актуальная база регресс-гарда).

### Кандидаты след. сессий
- **wsl (остаток 95):** URL в КОНЦЕ monospace-спана съедает закрывающий backtick + спан не оборачивается в `<code>`.
  Пре-существующий, отдельный дефект (constrained-span autolink cap: `autolink_url_limit` для space-boundary возвращает
  `bytes.len()` вместо капа на закрытие спана). Узкий, но субтильный — нужен аккуратный триаж span-detection.
- **F-BV (cheatsheet 58):** `#…#` mark/highlight внутри `[tree]` open-блоков — inline tree-extension garbage, низкоценно.
- **migration.adoc (frontier, diff=1):** `{asciidoctor-version}`→`2.0.23` интринсик. Узко, спорно (мы не asciidoctor).
- **manpage.adoc (frontier, 146):** manpage backend — спец, крупный, отдельный триаж.
- **adoc2docx (4 крупных = rouge):** синтаксический хайлайтер Ruby/XML/Java. Крупная фича, не single-session.
- **Отложенный doctype-intrinsics** (под F-BT): `ifdef::doctype-book/manpage/inline[]`. Малочастотно.

### Методология (без изменений; [[compat_corpus_methodology]] + [[feedback_html_byte_parity_scope]] + [[feedback_frontier_triage]])
`frontier_parity.py <root>` / `showdiff.py <file>` (ПОЗИЦИОННЫЙ DOM-differ — раздувает один upstream-рассинхрон в хвост;
сверять SET элементов Counter, не позиции). Скрипты `/mnt/c/tmp/adoc-test/`. Корни: gate `/mnt/c/tmp/adoc-test`(344),
frontier `/mnt/c/tmp/adoc-frontier`(250), adoc2docx `/mnt/c/tmp/adoc2docx`(52), docs `/mnt/c/Work/docs`(214). Регресс-гард:
`gate_check.py` (база `/tmp/adoc_base` от ТЕКУЩЕГО master) + `scratchpad/sweep_all.py` (raw-байт свип всех 4). НЕ доверять метке
прошлой сессии — git log + showdiff + минимальные пробы каждый кандидат (эта сессия: метка «реордер macros/quotes» = ошибка).
