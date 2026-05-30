# Session context

## Последняя сессия (2026-05-31) — Фаза 3: escaped character references (п.15)

Продолжение Фазы 3 (совместимость с Asciidoctor). Выбор кластера по `/tmp/nearmiss.py`:
самый крупный чистый flip-кластер из «1 diff away» — backslash перед entity (3 файла).

### Ветка `fix/escaped-char-reference` (от master; НЕ закоммичено)
- **Корень**: `\&#174;` / `\&#xA0;` / `\&copy;` — backslash экранирует character reference.
  Asciidoctor снимает `\` и выводит ref литералом (`&`→`&amp;`); мы сохраняли `\`.
  Обработчик `handle_inline_escape` (inline.rs:~620) матчил `\` перед `* _ ` # ^ ~ { [ < \ '`,
  но НЕ перед `&` → проваливалось в `_ => false`, backslash оставался текстом.
- **Фикс** (inline.rs):
  1. Новый arm в `handle_inline_escape` (перед общим escape-arm): при `\` + валидный char-ref
     — flush, skip `\`, эмит всего ref'а одним `Event::Text(Borrowed)` (важно: одним span'ом,
     иначе внутренний `#` в `\&#174;` запустит mark-синтаксис).
  2. Хелпер `char_ref_len_at(start)` — точная реплика Asciidoctor `CharRefRx`: named
     `[A-Za-z][A-Za-z]+\d{0,2}` (мин. 2 буквы), decimal `#` + 2–6 цифр, hex `#x` + ≥2 hex.
     Чисто лексически (без словаря сущностей: `\&bogusname;` тоже снимается).
  3. +2 unit-теста (`test_escaped_char_reference`, `test_backslash_before_invalid_char_reference_kept`).
- **Граничные случаи выверены против asciidoctor** (esc-test/esc2/esc3.adoc в /tmp): `\&#9;`
  (1 цифра) и `\&#1234567;` (7 цифр) и `\&a;` (1 буква) и `\&#xGG;` — backslash СОХРАНЯЕТСЯ;
  всё совпало с AD байт-в-байт.

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное (parser 429→431,
  html 302, integration 25, html_output 35, adoc_html_tests 6, author 6, html_compat 1,
  parsing_lab 1, doctests). +2 теста parser.
- Корпус `compare_full.py` (release): **Identical 142→145 (+3), Different 202→199, Errors 0**.
  Флипнули: multiple-authors, link-macro, ui-macros. Регрессий ноль (net ровно +3).
- TODO.md: baseline 142→145; п.15 отмечен `[x]` + записан остаток (preserve bare char-ref).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/escaped-char-reference` (только по запросу).
- Следующие чистые flip-кандидаты Фазы 3 (по near-miss):
  - **escaped-директива** `\ifdef`/`\endif` → снять `\`, вывести директиву литералом
    (admonitions, inter-document-xref). PREPROCESSOR-слой (не inline). ~2 flip'а.
  - **xref-id норм.** `#Substitutions`→`#_substitutions` (п.19/24): positional-and-named-attributes.
  - **alt двойная кавычка** (п.18): `<img alt=""…">` — author/revision-attribute-entries.
  - **`// end::para[]` утечка** тег-региона в выводе (verse.adoc) — tagged-region/comment.
  - **ОТДЕЛЬНО**: preserve bare char-ref (`&#174;` в обычном тексте → сохранять как сущность,
    не экранировать). Влияет на ряд файлов, но НЕ изолированный 1-diff — требует осторожности
    (внутри listing/literal оба экранируют — там не трогать).

### Предостережения
- НЕ `cargo fmt` (не fmt-clean). Коммит только по запросу. Верифицировать находки аудита.
- Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release `target/release/adoc`, 344 файла).
  near-miss: `/tmp/nearmiss.py`. Сравнение семантическое (DOM): литеральный `’` и `&#8217;`
  считаются Identical (нормализатор декодирует) — сырой `diff` может «врать». LSP, context7 MCP.
