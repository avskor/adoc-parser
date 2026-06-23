# Session context

## Сессия (2026-06-23, 2-я) — F-BE: атрибутированный span `[.role]#…#` в метке link-семейства (links.adoc корень A)

Запрос «начни следующую задачу». master `1a3f15c` (F-BD смержен). Взят документированный остаток F-BD — **корень A**
links.adoc (строки 17, 19): `[.overline]#overline#` внутри метки autolink. Сканер метки закрывал label на ПЕРВОМ `]`
(после `[.overline`) → `[.overline</a><mark>overline</mark>]`. asciidoctor гонит quotes ДО macros → `[.overline]#…#`
становится `<span>` ДО того, как inline-link regex ищет `]` (внутренние скобки съедены). Мы гоним macros ДО quotes
(зеркало legacy) → баг.

Ветка `fix/link-label-inner-span-macro` (от master `1a3f15c`, **НЕ закоммичена** — паттерн F-*: коммит ПО ЗАПРОСУ).

### Сделано — корень A (links.adoc → байт-в-байт с asciidoctor 2.0.23)
**Фикс (4 файла):**
- **subst/quotes.rs**: `pub(super) fn attributed_span_end(tags, src, bytes, lbrack)` — при `bytes[lbrack]=='['`
  детектит, открывается ли тут атрибутированный span (`[attrlist]` + `*`/`` ` ``/`_`/`#`, constrained ИЛИ unconstrained),
  возвращает индекс СРАЗУ ЗА закрывающим маркером. Переиспользует `attrlist_unconstrained`/`attrlist_constrained`
  (приватные в quotes.rs). Гейтинг как в pass_*: unconstrained — без open-boundary, constrained — требует
  `!is_word(bytes[lbrack-1])`. Superscript `~`/subscript `^` НЕ берут attrlist → не пробуются.
- **subst/macros.rs**: `pub(super) fn find_link_label_close(tags, s, open)` — как `find_macro_close_bracket`, но `]`,
  закрывающий attrlist атрибутированного span, НЕ терминатор метки: на внутреннем `[` зовёт `attributed_span_end` и
  ПРОПУСКАЕТ весь span. `\]` экранируется идентично. Проводка: try_link / try_mailto / try_autolink(URL[text]) →
  `find_link_label_close(&work.tags, …)`.
- **subst/mod.rs**: `pub(crate) fn link_label_close(s, open)` = обёртка `find_link_label_close(&[], …)` (легаси работает
  на сыром тексте без сентинелов; `TagToken` приватен → не светим в pub(crate)-сигнатуре).
- **inline.rs** (легаси, симметрия): 4 сайта (`try_link_macro` ×2 [++url++ и обычный], `try_mailto_macro`,
  `try_autolink`) → `crate::subst::link_label_close(…)` вместо `rest.find(']')`.

### Верификация
- clippy `--workspace` **0** (3 warning'а только под `--all-targets` — пред-существующие в тестах, есть на master).
- **test --workspace: 0 упавших, 1294 passed** (html 529→530 +test_attributed_span_in_link_label_html; parser 645
  [+7 кейсов в reproduces_legacy_on_link_inputs — pipeline==legacy для span-in-label]; compat 233).
- **Гейт 344/344 байт-в-байт** vs master `1a3f15c` (база `/tmp/adoc_base` = свежий master-бинарь; gate_check.py 0 diff —
  ни один гейт-файл не ставит span в метку ссылки).
- **Sweep frontier(250)+adoc2docx(52) new-vs-base: РОВНО 1 файл** — links.adoc (целевой). 0 регрессий.
- **links.adoc теперь БАЙТ-В-БАЙТ == asciidoctor 2.0.23** (newline-normalized; adoc2docx Identical 44→45). Корень B
  (F-BD) + корень A (эта сессия) = вся divergence снята.
- CLI-пробы == asciidoctor: целевой autolink (строки 17/19), link:/mailto со span, границы `[a [b] c]`→`a [b`,
  `[label]*next*`, leading span `[[.role]*bold*…]`.

### Состояние репо
- Ветка `fix/link-label-inner-span-macro` (от master `1a3f15c`, НЕ закоммичена). master чист == origin.
- Изменены: adoc-parser/src/subst/{quotes.rs, macros.rs, mod.rs}, adoc-parser/src/inline.rs,
  adoc-html/src/tests.rs (+1 тест), adoc-parser/src/subst/mod.rs (тест-кейсы).

### Остаток / следующая работа
- **Unconstrained `[.role]##span##` в метке URL-макроса** — РАСХОЖДЕНИЕ (синтетическое, НЕ в корпусе): asciidoctor
  при отсутствии open-boundary матчит unconstrained-attrlist `[^\]]+` на САМОЙ скобке макроса (после URL стоит word-char),
  поглощая `[pre [.role]##span##` целиком → URL становится bare (`class="bare"`), `<span class="pre [.role">`. Наш
  `find_link_label_close` детектит span только на ВНУТРЕННЕМ `[` (после макро-`[`), даёт «разумный» но не-идентичный
  результат. Constrained защищён boundary (char перед макро-`[` = URL word-char → constrained-attrlist там не открывается),
  поэтому корпусный случай совпадает. Чтобы покрыть unconstrained — надо детектить, что unconstrained-span,
  начинающийся НА/ДО макро-`[`, поглощает его → тогда отклонять link целиком (другой слой, высокий риск). Дефернуто.
- **Крупные adoc2docx** (НЕ триажены, вероятно мульти-root): test 1105, source 681, xml 291, callouts 195 — упираются
  в Rouge syntax-highlighter / sequential-quotes / нумерацию спец-секций.
- frontier single-diffs архитектурны (CHANGELOG replacements-before-macros, migration {asciidoctor-version} intrinsic).
- Методология: `frontier_parity.py /mnt/c/tmp/adoc2docx`, `showdiff.py <file>`, gate_check.py (база `/tmp/adoc_base`),
  base-vs-new sweep (inline bash: find frontier+adoc2docx, diff base vs new). Бинарь: `cargo build --release -p adoc-cli`
  (имя бинаря — `adoc`, НЕ adoc-cli). asciidoctor 2.0.23: `/usr/share/rubygems-integration/all/gems/asciidoctor-2.0.23/lib/asciidoctor/converter/html5.rb`.
