# Session context

## Сессия (2026-06-23, 13-я) — F-BP: блок-атрибут, примыкающий к list-item без пустой строки, отбрасывается

Запрос «начни следующую задачу». master `bccc15f` (F-BO смержен). session.md прошлой сессии устарел (писался до мержа F-BO).

### Триаж (notes-корпус, метрика семантического DOM)
- Скрипты в `/mnt/c/tmp/adoc-test/` (frontier_parity.py, showdiff.py, gate_check.py, refcache.py). Бинарь:
  `cargo build --release -p adoc-cli` → `target/release/adoc`.
- `frontier_parity.py /mnt/c/Work/docs/notes/modules` → 78 identical, 3 чистых расхождения.
- НЕ доверял меткам прошлой сессии — showdiff по всем 3, классифицировал каждый:
  - **windows/wsl (95)** + **keycloak/index (52)** — АРХИТЕКТУРНЫЕ автолинк-кейсы. Корень: `macros::extract` (автолинк)
    идёт ДО `quotes` и до экранирования спецсимволов (отложено в рендер); у asciidoctor `specialchars → quotes → macros`,
    поэтому к моменту автолинка backtick уже `<code>` (его `<` обрывает URL) и `<>` уже `&lt;`. Менять глобальный порядок —
    рискованно, НЕ чистый single-session фикс.
  - **ansible/tips (41)** — чистый БЛОЧНЫЙ корень. Выбран.

### Сделано — F-BP (ветка `fix/list-discards-abutting-block-attr` от master `bccc15f`)
**Баг:** `. Пример выравнивания слева` + сразу (без пустой строки) `[source, yaml]` + `----` — asciidoctor роняет
source-роль → plain `<pre>`, у нас оставался `<pre class="highlight"><code class="language-yaml">`.

**Корень (verified `parser.rb` 2.0.23 `read_lines_for_list_item`):**
- строки 1499-1501: block-метаданная строка (`[...]`/`.Title`/`:attr:`), примыкающая к list-item, читается в буфер строк
  элемента (`buffer << this_line`).
- строки 1453-1456: «a delimited block immediately breaks the list unless preceded by a list continuation» — делимитед-блок
  БЕЗ предшествующего `+` ломает список, метаданная остаётся без блока внутри элемента → ОТБРАСЫВАЕТСЯ; блок парсится заново
  на верхнем уровне БЕЗ неё.
- **Pending-атрибуты в открытом списке возможны ТОЛЬКО при примыкании** (без пустой строки) — иначе blank-ветка
  (`scan_header_constructs` block.rs 912/936) закрывает список в момент сканирования `[...]`/`.Title`, до установки pending.
  Значит при закрытии списка делимитед-блоком (не continuation) pending ВСЕГДА поглощён элементом → отбрасывается.

**Фикс (1 файл, only adoc-parser/src/block.rs):**
- новый хелпер `discard_list_absorbed_metadata` (обнуляет `pending_block_attrs` + `pending_block_title`).
- вызывается в путях делимитед-блока (`scan_block_containers` ~1247) и markdown-fence (~1261), в ветке
  `is_directly_in_list_context() && !self.in_continuation`, ПЕРЕД `close_list_contexts`.
- `+`-continuation «спасает» атрибут: `in_continuation` исключает ветку → атрибут доживает до вложенного блока (E7).

**Тесты:** +1 html-тест `test_list_discards_abutting_block_attr_html` (adoc-html/src/tests.rs): `[source]`/`[#id.role]`/
`[quote]` примыкание → отброшены; `.Title` примыкание вливается в параграф как текст (НЕ block-title div, отдельный феномен);
регрессии: `+`-continuation держит роль+вкладывает блок, пустая строка держит, top-level держит.

### Верификация
- clippy `--workspace` **0**. **test --workspace 0 упавших** (html **541** (+1), parser 648, compat 233, render-core 25,
  integration 29, html-compat).
- **БАЙТ-НЕЙТРАЛЬНО на старых корпусах:** база `/tmp/adoc_base` ПЕРЕСОБРАНА от текущего master `bccc15f`
  (stash→checkout master→`cargo clean --release -p adoc-parser`→build→cp→checkout branch→pop→clean+rebuild).
  - gate 344 — `gate_check.py` **0 diff**.
  - frontier(250)+adoc2docx(52)=302 — `/tmp/sweep_bvn.py` **0 diff**.
- **notes Identical 78→79** (ansible/tips → identical, выпал из расхождений).
- 10+ CLI-проб == asciidoctor 2.0.23 (`[source]`/`.Title`/`[#id.role]`/`[quote]` примыкание отброшены; `____` остаётся
  quoteblock нативно — поэтому PC лишь ВЫГЛЯДИТ как «атрибут выжил»; `+`-continuation E7/E9 и blank-line V2 держат роль;
  bonus — `[quote]`+`----` теперь plain listing, раньше у нас ошибочно quoteblock).

### Состояние репо
- Ветка `fix/list-discards-abutting-block-attr` от master `bccc15f`, НЕ закоммичена (ждёт запроса коммит/merge/push).
- Изменено: `adoc-parser/src/block.rs` (+хелпер `discard_list_absorbed_metadata`, 2 вызова), `adoc-html/src/tests.rs`
  (+1 тест), TODO.md (+F-BP), session.md.
- `/tmp/adoc_base` = бинарь master `bccc15f` (актуальная база регресс-гарда этой сессии).

### Пре-существующие дивергенции (НЕ в scope F-BP, отдельный феномен — поглощение параграфом)
- `. item`+`[.role]`+`параграф`: asciidoctor вливает «параграф» в текст элемента (`<p>item\nпараграф</p>`), у нас
  sibling-параграф с role.
- `. item`+`[.role]`+`NOTE:`: asciidoctor вливает `NOTE:` как литерал в параграф, у нас вложенный admonition.
- Это правило `read_paragraph_lines` (block-title и инлайн-текст не прерывают параграф list-item), родственно F-BO,
  НЕ затронуто фиксом delimited-пути. Причудливый `[source]`+`.Title`+`----` стэкнутый кейс — asciidoctor-квирк, вне scope.

### Остаток notes (2 архитектурных расхождения — кандидаты, требуют рерайта порядка субституций)
- **windows/wsl (95)** — автолинк URL внутри backtick inline-кода захватывает ЗАКРЫВАЮЩИЙ backtick в href.
- **keycloak/index (52)** — автолинк URL `http://<host>:<port>/…` (со спецсимволами `<>`) внутри inline-кода не срабатывает
  (`<` обрывает URL до экранирования).
- ОБА — следствие порядка `macros` ДО `quotes` в нашем движке (см. [[proj_sequential_quotes_rewrite]]). Чистого БЛОЧНОГО
  остатка в notes больше НЕТ.

### Методология (без изменений)
`frontier_parity.py <roots>` / `showdiff.py <file>` (семантический DOM, ПРАВИЛЬНАЯ метрика для не-verbatim — байт только
ВНУТРИ `<pre>`, см. [[feedback_html_byte_parity_scope]]). `gate_check.py` + `/tmp/sweep_bvn.py` — байт регресс-гард
(база `/tmp/adoc_base` пересобирать от текущего master). Бинарь: `cargo build --release -p adoc-cli`.
⚠ **mtime на /mnt/c НЕ обновляется надёжно** — надёжно только `cargo clean --release -p adoc-parser` перед build
(см. [[feedback_wsl_build_staleness]]). asciidoctor 2.0.23 для проб (наш `--no-standalone` ≈ asciidoctor `-s`).
НЕ доверять метке прошлой сессии — showdiff каждый кандидат (см. [[feedback_frontier_triage]]). Источник корпусов:
`/mnt/c/Work/docs/notes/modules/` (81 .adoc). asciidoctor list-item чтение: `parser.rb` `read_lines_for_list_item`
(1404+): делимитед-блок ломает список без `+` (1453-1456); block-метаданная в строке элемента — `buffer << this_line`
(1499-1501).
