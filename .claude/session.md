# Session context

## Сессия (2026-06-23, 3-я) — F-BF: `[partintro]` вне book-части исключается (multi-special-ex.adoc)

Запрос «начни следующую задачу». master `afef333` (F-BE смержен). Триаж frontier (parity по обоим корпусам):
adoc2docx — 45 identical / 4 крупных мульти-root (test 1105 / source 681 / xml 291 / callouts 195 = Rouge/sequential-quotes);
frontier — 228 identical, ближайший ценный clean-div **`multi-special-ex.adoc` (87 token-diff)**. Остальные frontier-div
out-of-scope: manpage (146, др. бэкенд), `{docdate}`/`{asciidoctor-version}` intrinsics (1+1, env-зависимы),
CHANGELOG (1, replacements-before-macros).

Ветка `fix/partintro-exclude-outside-book` (от master `afef333`, **НЕ закоммичена** — паттерн F-*: коммит ПО ЗАПРОСУ).

### Корень (single-root, verified)
difflib на multi-special-ex: РОВНО 1 insert, 9 токенов = `<div class="openblock partintro">…</div>`, delta +9.
Harness гоняет `_includes`-фрагмент книги как **standalone article** (без `-d book`). asciidoctor `html5.rb convert_open`
ИСКЛЮЧАЕТ `[partintro]` блок (возвращает `''` + error в лог), если
`node.level > 0 || node.parent.context != :section || node.document.doctype != 'book'`. В article срабатывает
`doctype != book`. Мы рендерили `<div class="openblock partintro">` → +9 токенов сдвига.

### Сделано — фикс (4 файла, чистый рендер-слой adoc-html)
- **lib.rs**: поле `partintro_suppress: Vec<(usize, usize)>` = `(глубина delimited_block_stack ДО push Open, длина output ДО блока)`; init в `new()`.
- **blocks.rs** `start_delimited_block`, ветка `DelimitedBlockKind::Open`: `style==Some("partintro") && !self.doctype_book`
  → push `(stack.len(), output.len())` в `partintro_suppress`, **очистка `block_title_inner_html` = None** (title тоже
  исключается), НЕ эмитим контент. Иначе прежняя логика (abstract / open_block_with_title).
- **events.rs** `TagEnd::DelimitedBlock`: pop стек; если `partintro_suppress.last()` имеет глубину == stack.len() после
  pop → `partintro_suppress.pop()`, `output.truncate(pos)`, **`output.push('\n')`**, `return`. Один `\n` = лидирующий
  join-LF asciidoctor (`blocks.map(&:convert).join(LF)`: пустой partintro перед сиблингом → пустая строка на его месте).
- **tests.rs**: +1 тест `test_partintro_excluded_outside_book` (open-block+title excluded, масочный параграф excluded,
  book-режим рендерит — регресс-гард).

Оба вида partintro (explicit `[partintro]\n--…--` И масочный `[partintro]`-параграф, block.rs:2751) приходят как
`DelimitedBlockKind::Open` со стилем в meta → один гейт в Open-ветке покрывает оба. Гейт по `doctype_book` (залатан на
конце хедера, как asciidoctor) → book-режим не затронут (0 риска для существующих book-тестов).

### Верификация
- clippy `--workspace` **0**.
- **test --workspace: 0 упавших** (html 530→**531** +test_partintro_excluded_outside_book; parser 645; compat 233;
  render-core 25; html-compat/integration/author — все зелёные).
- **Гейт 344/344 байт-в-байт** vs master `afef333` (база `/tmp/adoc_base` пересобрана из чистого master через stash;
  gate_check.py 0 diff — ни один гейт-файл не ставит partintro вне book).
- **Sweep frontier(250)+adoc2docx(52)=302 new-vs-base: РОВНО 1 файл** (multi-special-ex), **0 регрессий**.
- **frontier Identical 228→229**; multi-special-ex ушёл из clean-div списка (token-identical 158==158).
- **multi-special-ex байт-в-байт == asciidoctor** КРОМЕ 1 строки = пустая концевая секция `=== Appendix sub` (отдельный
  предсуществующий корень, token-невидимо, НЕ partintro).
- 5/5 CLI-проб == asciidoctor 2.0.23: open-block+title excluded, параграф excluded, id+role excluded, book рендерит,
  mid-позиция partintro между параграфами (join-LF корректен).

### Состояние репо
- Ветка `fix/partintro-exclude-outside-book` (от master `afef333`, НЕ закоммичена). master чист == origin.
- Изменены: adoc-html/src/{lib.rs, blocks.rs, events.rs, tests.rs}.

### Остаток / следующая работа
- **multi-special-ex остаток (отдельный корень):** пустая концевая секция `=== Appendix sub` без тела → asciidoctor
  `<h3>…</h3>\n\n</div>` (шаблон `\n#{content}\n`, content пуст), у нас без пустой строки. Token-невидимо. Слайс
  «empty-section trailing blank line», затронет др. файлы — отдельная задача.
- **Вырожденный edge (НЕ в корпусе):** partintro как единственный блок части → наш безусловный `\n` даёт лишнюю пустую
  строку (asciidoctor `join([''])` без сепаратора). Условный `\n` = lookahead/deferred-separator, не стоит риска.
- **Крупные adoc2docx** (мульти-root): test 1105, source 681, xml 291, callouts 195 — Rouge syntax-highlighter /
  sequential-quotes / нумерация спец-секций.
- frontier single-diffs архитектурны: CHANGELOG replacements-before-macros, migration `{asciidoctor-version}` intrinsic,
  doctime-localtime `{docdate}` intrinsic, manpage (др. бэкенд).
- Методология: `frontier_parity.py /mnt/c/tmp/adoc2docx` (и `/adoc-frontier`), `showdiff.py <file>`, gate_check.py
  (база `/tmp/adoc_base` — пересобирать из текущего master через stash!), base-vs-new sweep (inline python: find
  frontier+adoc2docx, diff base vs new). Бинарь: `cargo build --release -p adoc-cli` (имя — `adoc`). asciidoctor 2.0.23:
  `/usr/share/rubygems-integration/all/gems/asciidoctor-2.0.23/lib/asciidoctor/converter/html5.rb`.
