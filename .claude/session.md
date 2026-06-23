# Session context

## Сессия (2026-06-23, 5-я) — F-BH: `(C)`/`(R)` курлятся рядом с буквой

Запрос «начни следующую задачу» → пользователь выбрал направление **«дефернутые остатки»** (AskUserQuestion,
т.к. корпуса исчерпаны). master `85a7a72` (F-BG смержен).

### Триаж: корпуса ИСЧЕРПАНЫ (verified фактом, не меткой)
- **Гейт** (`/mnt/c/tmp/adoc-test/`, 344): **344/344** byte-identical (регресс-гард чист).
- **Frontier** (250): **230 identical**, 3 CLEAN — все нерешаемы: `manpage.adoc` (146 diff, ДРУГОЙ бэкенд),
  `migration.adoc` (1 diff = `{asciidoctor-version}`→`2.0.23`, мы не asciidoctor — нерешаемо),
  `doctime-localtime.adoc` (1 diff = `{localtime}` — мы УЖЕ поддерживаем, diff = артефакт времени снятия refcache).
- **adoc2docx** (52): **45 identical**, 4 CLEAN — **ВСЕ Rouge-подсветка** (verified showdiff: первый diff в каждом
  файле = `<span class="kd/nc/na/nb/s1">` для Java/XML/Ruby/YAML; обёрточные классы `linenums`/`lineno`/`linenos gl`
  тоже часть Rouge-форматтера). test 1105 / source 681 / xml 291 / callouts 195. Реализация Rouge = многосессионная
  задача, byte-parity без неё недостижим.
- **Вывод:** лёгких single-root near-miss больше НЕТ. Развилка → AskUserQuestion → «дефернутые остатки».

### Сделано — F-BH (ветка `fix/copyright-registered-letter-adjacent` от master `85a7a72`, НЕ закоммичена)
Взят дефернутый остаток F-BG(1): `(C)`/`(R)` рядом с буквой не курлились.
- **Корень:** `apply_typographic_replacements` (`adoc-parser/src/inline.rs`) гейтил `(C)`/`(R)` условием
  `&& !matches!(bytes.get(i+3), Some(b'A'..=b'Z' | b'a'..=b'z'))` — курлил только если ЗА маркером НЕ буква.
  asciidoctor `REPLACEMENTS` `/\\?\(C\)/`/`/\\?\(R\)/` тип `:none` → замена ВЕЗДЕ, без контекстного гейта
  (как `(TM)`, у которого guard'а и не было — асимметрия). Проба asciidoctor 2.0.23: `a(C)b`→`a©b`, `x(R)y`→`x®y`,
  `prefix(C)suffix`→`prefix©suffix`, `(C) standalone`→`© standalone`.
- **Фикс (2 файла):**
  - `adoc-parser/src/inline.rs`: убран followed-by-letter guard из арм'ов `(C)` и `(R)` (+ комментарий о правиле
    asciidoctor). ЕДИНСТВЕННАЯ реализация — оба движка делегируют сюда (`subst/replacements.rs:64,95` + legacy
    `inline.rs:1095` + `subst/macros.rs:2091` link-target). Escaped `\(C)` запечатан escape-пассом в Literal leaf ДО
    replacements → соседство с буквой НЕ воскрешает курлинг (verified asciidoctor: `a\(C)b`→`a(C)b` литерал).
  - +2 теста: `test_typographic_copyright_registered_letter_adjacent` (parser, event-уровень + escaped-гард через
    конкатенацию — escaped splits в отдельные Text-leaves), `test_copyright_registered_letter_adjacent_html` (html).

### Верификация
- clippy `--workspace` **0**.
- **test --workspace 0 упавших** (parser 646→**647**, html 532→**533**, compat 233, render-core 25, html-compat зелёные).
- **Гейт 344/344 байт-в-байт** vs master `85a7a72` (база `/tmp/adoc_base` пересобрана из master через stash;
  `gate_check.py` 0 diff — паттерн `(C)`/`(R)`+буква НЕ встречается в гейте).
- **Sweep frontier(250)+adoc2docx(52)=302 new-vs-base: 0 расхождений** (`/tmp/sweep_bvn.py`). Ожидаемо для
  дефернутого остатка — паттерн не в корпусах, метрика parity не двигается, но реальная дивергенция устранена.
- CLI-пробы == asciidoctor 2.0.23 байт-в-байт (NCR-нейтрализация ©=&#169; ®=&#174; ™=&#8482;):
  `a(C)b/x(R)y/m(TM)n`, `prefix(C)suffix`, `(C) standalone`, escaped `a\(C)b`/`\(C)x` (литерал).

### Состояние репо
- Ветка `fix/copyright-registered-letter-adjacent` (от master `85a7a72`, **НЕ закоммичена** — паттерн F-*: коммит ПО ЗАПРОСУ).
- Изменены: `adoc-parser/src/inline.rs` (фикс + тест), `adoc-html/src/tests.rs` (+тест). TODO.md + session.md обновлены.

### Остаток / следующая работа (дефернутые остатки F-BG/F-BF, все «не в корпусе»)
- **mailto/email таргеты не курлятся**: `try_mailto` строит url из base+encoded, НЕ через reconstruct. asciidoctor курлит.
- **resolved-attr `{u}/path...[t]` Document-реинлайн**: unescaped `...` НЕ курлится (флаг подавляет ради защиты
  escaped, escape потерян в pass 1). Полный паритет = preserve backslash через реинлайн — отд. редизайн.
- **undefined-attr `\...` в trailing (MissingSkip path)**: курлит плейн-текстом.
- **`->`/arrow в bare URL**: ломает границу (URL-скан стопает на `>`). Отд. autolink-boundary баг.
- **Пустая концевая секция** (F-BF остаток): `=== sub` без тела → у asciidoctor `<h3>…</h3>\n\n</div>` (шаблон
  `\n#{content}\n`), у нас без пустой строки. Чистый рендер-слой, token-невидимо/байт-видимо.
- **Крупное:** Rouge-подсветка (4 adoc2docx-файла) — многосессионно; расширение корпуса новыми репо (паттерн 106-й).

### Методология (без изменений)
`frontier_parity.py <корпус>`, `showdiff.py <file>`, `gate_check.py` (база `/tmp/adoc_base` — пересобирать из
текущего master через stash!), `/tmp/sweep_bvn.py` (base-vs-new по frontier+adoc2docx). Бинарь:
`cargo build --release -p adoc-cli` (имя `adoc`, НЕ поддерживает `-s`/stdin — нужен файл, выводит полный документ).
asciidoctor 2.0.23 для проб (НЕ в ad-hoc для корпуса — через refcache). NCR-нейтрализация: наш raw UTF-8 == их `&#NNN;`.
