# Session context

## Сессия (2026-06-23, 10-я) — F-BM: sub/superscript `~…~`/`^…^` ложно матчили содержимое с пробелами

Запрос «начни следующую задачу». master `6a39972` (F-BL смержен).

### Триаж (notes-корпус, метрика семантического DOM)
- Скрипты потерялись из /tmp прошлой сессии → нашлись в `/mnt/c/tmp/adoc-test/` (frontier_parity.py, showdiff.py,
  gate_check.py, refcache.py, compare_full.py и др.). Бинарь пересобран: `cargo build --release -p adoc-cli` → `target/release/adoc`.
- `frontier_parity.py /mnt/c/Work/docs/notes/modules` → 75 identical, 6 чистых расхождений:
  plan(230)/qwen(194)/sbertech-index(134)/wsl(95)/keycloak(52)/tips(41).
- НЕ доверял метке прошлой сессии — прогнал showdiff по ВСЕМ 6, классифицировал каждый (см. остаток ниже).
  Взял `qwen` (194) — самое чистое ПРАВИЛО (не самый маленький diff): один корень дал весь каскад.

### Сделано — F-BM (ветка `fix/subscript-superscript-whitespace` от master `6a39972`)
**Баг:** `qwen.adoc` — `При INT4 (~33 ГБ VRAM) … обеспечивает ~40% стоимости.` у нас →
`(<sub>33 ГБ VRAM) обеспечивает </sub>40%`, у asciidoctor остаётся литералом (тильды с пробелами внутри).
**Корень (verified пробами asciidoctor 2.0.23):** asciidoctor `SubscriptRx`/`SuperscriptRx` = `~(\S+?)~` / `^(\S+?)^` —
содержимое между маркерами НЕ должно содержать whitespace. Активный путь — **subst/quotes.rs `simple_pair_open_close`**
(string-rewriting движок, дефолтный после рерайта — см. [[proj_sequential_quotes_rewrite]]), НЕ legacy `inline.rs`
`try_simple_pair`! Сначала по ошибке правил inline.rs (поведение бинаря не изменилось → понял что это мёртвый путь,
откатил).

**Фикс (1 файл, adoc-parser/src/subst/quotes.rs, ~6 строк):**
- в `simple_pair_open_close` добавлен bail `return None` на первом `bytes[j].is_ascii_whitespace()` перед закрывающим
  маркером. Проверка на СЫРЫХ байтах ДО прохода attributes (в asciidoctor quotes идут раньше attributes), поэтому:
  - `^a{sp}b^` → `<sup>a b</sup>` сохранён (literal `{sp}` без whitespace-байтов, пробел появляется ПОСЛЕ).
  - passthrough/charref-сентинел `^+a b+^` → `<sup>a b</sup>` сохранён (сентинел opaque, пропускается = non-whitespace).
- backtracking: при None pass_simple_pair продолжает скан с след. позиции (как asciidoctor non-greedy `\S+?`).

**Тесты:** +1 `subscript_superscript_require_non_whitespace_content` (subst::tests, хелпер `pipeline` = АКТИВНЫЙ движок;
inline.rs `parse()` = legacy, там тест бесполезен). Проверяет: `H~2~O and ~a b~`, `E^2^ and x^a b^`, `(~33 ГБ) ~40%`.

### Верификация
- clippy `--workspace` **0**. **test --workspace 0 упавших** (parser 647→**648**, html 538, compat 233, render-core 25,
  integration 29, html-compat 1).
- **БАЙТ-НЕЙТРАЛЬНО на старых корпусах:** база `/tmp/adoc_base` ПЕРЕСОБРАНА от текущего master `6a39972`
  (stash→checkout master→clean+build→cp→checkout branch→pop→rebuild).
  - gate 344 — `gate_check.py` **0 diff**.
  - frontier(250)+adoc2docx(52)=302 — `/tmp/sweep_bvn.py` **0 diff**.
- **notes Identical 75→76** (qwen → identical, выпал из расхождений).
- 4 CLI-пробы (`~33 ГБ~`/`H~2~O`/`^a{sp}b^`/`^+a b+^`) == asciidoctor 2.0.23 байт-в-байт.

### Состояние репо
- Ветка `fix/subscript-superscript-whitespace` от master `6a39972`, НЕ закоммичена (ждёт запроса коммит/merge/push).
- Изменено: `adoc-parser/src/subst/quotes.rs` (bail в `simple_pair_open_close`),
  `adoc-parser/src/subst/mod.rs` (+1 тест), TODO.md (+F-BM), session.md.
- `/tmp/adoc_base` = бинарь master `6a39972` (актуальная база регресс-гарда этой сессии).

### Остаток notes (5 чистых расхождений — классы верифицированы showdiff, кандидаты на след. сессии)
По убыванию diff. Классы РАЗНЫЕ (не один корень):
- **plan (230)** — `•` (U+2022) bullet-символы: asciidoctor распознаёт как `<ul>`, у нас остаются literal `•` в `<p>`.
- **sbertech/index (134)** — ordered-list `. Выполняем раздел …` десинк (item выпадает/неверная вложенность).
- **wsl (95)** — автолинк URL внутри backtick inline-кода захватывает ЗАКРЫВАЮЩИЙ backtick в href
  (`<a href="https://rubygems.org\`">`), граница code неверная.
- **keycloak/index (52)** — автолинк URL `http://<host>:<port>/…` (со спецсимволами `<>`) внутри inline-кода: asciidoctor
  делает `<a class="bare">` внутри `<code>`, мы НЕ автолинкуем (URL остаётся текстом).
- **tips (41)** — `[source,yaml]` примыкает к list-item БЕЗ пустой строки → asciidoctor ТЕРЯЕТ source-роль (plain `<pre>`),
  у нас остаётся `<pre class="highlight"><code class="language-yaml">`. Корень изолирован пробами pA/pB/pC/pD/pE:
  список+source без пустой строки = plain; параграф+source без пустой = highlighted; список+пустая+source = highlighted.

### Методология (без изменений)
`frontier_parity.py <roots>` / `showdiff.py <file>` (семантический DOM, ПРАВИЛЬНАЯ метрика для не-verbatim — байт только
ВНУТРИ `<pre>`, см. [[feedback_html_byte_parity_scope]]). `gate_check.py` + `/tmp/sweep_bvn.py` — байт регресс-гард
(база `/tmp/adoc_base` пересобирать от текущего master). Бинарь: `cargo build --release -p adoc-cli`.
⚠ **mtime на /mnt/c НЕ обновляется надёжно** — `touch` НЕ всегда форсит пересборку crate; надёжно только
`cargo clean --release -p adoc-parser` перед build (см. [[feedback_wsl_build_staleness]]). asciidoctor 2.0.23 для проб.
⚠ **inline разбор идёт через subst/ (string-rewriting), inline.rs InlineState — LEGACY/мёртвый путь** для дефолтного
режима (см. [[proj_sequential_quotes_rewrite]]); правки inline-конструктов делать в `subst/quotes.rs` и т.п., тесты —
через `pipeline()` в subst::tests, НЕ через `parse()` в inline::tests. НЕ доверять метке прошлой сессии — showdiff каждый
кандидат (см. [[feedback_frontier_triage]]). Источник реальных корпусов: `/mnt/c/Work/docs/notes/modules/` (81 .adoc).
