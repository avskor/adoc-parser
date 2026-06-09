# Session context

## Последняя сессия (2026-05-31, поздняя) — Фаза 3: em-dash границы + ZWSP

Кандидат по near-miss на baseline 149. Кластер «типографика» (п.37) — самый безопасный/выгодный
из чистых flip (link через `{attr-ref}` оказался архитектурным — порядок подстановок, отложен).

### Ветка `fix/em-dash-boundaries` (от master; НЕ закоммичено)
- **Корень**: `inline.rs::apply_typographic_replacements`, bare-`--` арм (строка ~28). Старое
  правило: любой `--` (кроме space-space) → `—`. Это (а) слишком агрессивно (` --dir`→`—dir`,
  `S.S.T.--`→`—`; Asciidoctor оставляет `--`) и (б) без ZWSP (`cases--such`→`—` вместо `—​`).
- **Фикс**: bare `--` → `—`+ZWSP (`—​`) ТОЛЬКО для `\w--\w` (Asciidoctor `(\w)--(?=\w)`,
  `\w`=ASCII alnum+`_`). Иначе → **`None`** (не `Some("--",2)`!): первый `-` остаётся литералом,
  второй переразбирается → `-->` корректно даёт `-→` (asciidoctor: `A --> B`→`A -→ B`, проверено).
  Space-space правило (` -- `→thin-em-thin) не тронуто.
- Тесты: обновлены 2 дубля bare-em-dash (2668 и 3801) под ZWSP; `test_arrow_triple_not_replaced`
  (3763) `A --> B`: было `—>`, стало `-→`. +2 теста (`run --dir`, `For S.S.T.--` остаются).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 435→437).
- Корпус `compare_full.py` (release): **Identical 149→153 (+4), Different 191, Errors 0**.
  Flip: asg/README (`--dir`), dedication (`S.S.T.--`), continuation (ZWSP), callouts (бонус).
  0 регрессий (Different −4 ровно; по регэкспу Asciidoctor наш фикс строго консервативнее).
- Побочно резолвило em-dash-diff в revision-attribute-entries (2→1) и image-format (3→2) —
  не флипнули (остался alt-баг / link-баг соответственно).
- TODO.md: baseline 149→153; п.37 помечен `[~]` с под-пунктом em-dash `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/em-dash-boundaries` (только по запросу).
- Следующие чистые flip-кандидаты (по near-miss на 153):
  - **alt двойная кавычка** (п.18): `<img alt=""...">` — author-attribute-entries (1 diff),
    version-label (2 diff, оба alt), revision-attribute-entries (1 diff, теперь только alt).
    Корень — значение alt в image-макросе сохраняет кавычки. Флипнет ~3 файла. САМЫЙ ЧИСТЫЙ.
  - **xref-id норм.** `#Substitutions`→`#_substitutions` (positional-and-named-attributes, 1 diff).
  - **link `^`+rel/target** (литеральные `link:`/URL): description, xref-text-and-style (по 2 diff).
    NB: `{attr-ref}[text]` (icons-font/auto-ids/custom-ids/ROOT-index) — архитектурно (порядок subs).
  - **`// end::para[]` утечка** тег-региона (verse.adoc, 1 diff) + literal.adoc (`// end::indent[]`).
  - **апостроф в тексте макроса** (остаток п.37): xref/link display-текст не проходит REPLACEMENTS.

### Предостережения (без изменений)
- НЕ `cargo fmt`. Коммит только по запросу. Верифицировать находки.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release). near-miss: `/tmp/nearmiss.py`.
  Сравнение семантическое (DOM). LSP для навигации, context7 MCP для доков.

---

## Сессия (2026-05-31) — Фаза 3: escaped preprocessor-директива

Второй кандидат сессии. Выбор по `/tmp/nearmiss.py`: escaped-директива `\ifdef`/`\endif`
(admonitions, inter-document-xref — «1 diff away»). Preprocessor-слой (не inline).

### Ветка `fix/escaped-preprocessor-directive` (от master; НЕ закоммичено)
- **Корень**: `\ifdef::env-github[]` — backslash экранирует preprocessor-директиву. Asciidoctor
  снимает `\` и выводит `ifdef::...[]` литералом без вычисления; мы сохраняли `\`
  (`parse_conditional` возвращает None из-за `\`, строка падала в обычный output).
- **Фикс** (preprocessor.rs, `preprocess_with_attrs`): новый шаг «0» в начале цикла —
  `if let Some(rest) = line.strip_prefix('\\') && starts_with_conditional_directive(rest)`
  → при `!is_skipping` эмитим `rest` (строку без `\`), `continue`. Хелпер
  `starts_with_conditional_directive` проверяет префиксы `ifdef::`/`ifndef::`/`ifeval::`/`endif::`
  (`::` отсекает слова вроде `ifdefinitely`).
- **КРИТИЧНО — колонка 0**: проверяем СЫРОЙ `line`, НЕ `trimmed`. Asciidoctor распознаёт
  директивы только в начале строки. Первая версия на `trimmed` снимала `\` и при отступе →
  сломала conditionals.adoc (` \ifdef::just-an-example[]` в `[source,indent=0]` листинге, где
  отступ НАМЕРЕННО гасит директиву — это написано в комментарии самого файла). Column-0 чинит:
  indented `\ifdef` остаётся как есть.
- +4 unit-теста (block/inline strip, non-directive kept, indented kept).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 431→435).
- Корпус `compare_full.py` (release): **Identical 145→149 (+4), Different 195, Errors 0**.
  Blast radius — ровно 5 файлов с escaped-директивами (вне их вывод побайтово не менялся):
  admonitions, inter-document-xref, conditionals, ifdef-ifndef, ifeval — ВСЕ 5 теперь Identical
  (net +4, т.к. один был Identical уже на baseline 145). 0 регрессий.
- conditionals.adoc остаётся с сырым diff'ом (` \ifdef` vs `\ifdef` — лишний ведущий пробел от
  несрезанного `[source,indent=0]`), но нормализатор его прощает → Identical. Отдельная бага.
- TODO.md: baseline 145→149; пункт отмечен `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/escaped-preprocessor-directive` (только по запросу).
- Следующие чистые flip-кандидаты Фазы 3 (по near-miss):
  - **xref-id норм.** `#Substitutions`→`#_substitutions` (п.19/24): positional-and-named-attributes.
  - **alt двойная кавычка** (п.18): `<img alt=""…">` — author/revision-attribute-entries.
  - **`// end::para[]` утечка** тег-региона в выводе (verse.adoc) — tagged-region/comment.
  - **`[source,indent=0]`** не срезает общий отступ (conditionals.adoc) — блок-скан.
  - **ОТДЕЛЬНО**: preserve bare char-ref (`&#174;` в обычном тексте → сохранять как сущность,
    не экранировать). НЕ изолированный 1-diff; внутри listing/literal оба экранируют — не трогать.

### Предостережения
- НЕ `cargo fmt` (не fmt-clean). Коммит только по запросу. Верифицировать находки аудита.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release `target/release/adoc`, 344 файла).
  near-miss: `/tmp/nearmiss.py`. Сравнение семантическое (DOM): `’`/`&#8217;` и whitespace внутри
  `<code>` нормализуются → сырой `diff` может «врать». LSP для навигации, context7 MCP для доков.
