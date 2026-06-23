# Session context

## Сессия (2026-06-23, 4-я) — F-BG: `replacements` в inline-link TARGET (`...`/`--` курлятся в href)

Запрос «начни следующую задачу». master `a8bdd1c` (F-BF смержен). Триаж frontier (parity по обоим корпусам):
frontier исчерпан — 229 identical, остаток CLEAN = manpage (146, др. бэкенд) + 2 env-intrinsic single-diff
(`{docdate}`/`{asciidoctor-version}`, нестабильны) + **`CHANGELOG.adoc` (1 token-diff, ЕДИНСТВЕННЫЙ стабильный)**.
adoc2docx — 45 identical / 4 крупных мульти-root (test 1105 / source 681 / xml 291 / callouts 195 = Rouge/sequential-quotes).

Ветка `fix/replacements-in-link-target` (от master `a8bdd1c`, **НЕ закоммичена** — паттерн F-*: коммит ПО ЗАПРОСУ).

### Корень (verified)
CHANGELOG строка 1173: `compare/v1.5.6.1...v1.5.6.2[full diff]`. asciidoctor курлит `...`→`&#8230;&#8203;` в href
(showdiff показывал raw `…​` т.к. python HTMLParser декодирует charref'ы в значениях атрибутов). asciidoctor гонит
`replacements` ДО `macros` → `...` в URL курлится до автолинковки. Наш движок (`subst/mod.rs run_pipeline`) гонит
`macros` ДО `replacements` (зеркало legacy) → URL извлекается в leaf-токен, `...` не виден replacements.
Метрика (`frontier_parity.py`/`showdiff.py` через `normalize_html`) NCR-нейтрализована (HTMLParser декодирует
`&#8230;`↔`…`), поэтому raw UTF-8 `…​` у нас == их `&#8230;&#8203;` → token-identical.

### Сделано — фикс (4 файла, движок + рендерер)
- **`adoc-parser/src/inline.rs`**: поле `InlineOptions.link_target_pre_substituted: bool` (default false = курлим);
  добавлено в `from_attr_lookup` literal (false, не attribute-derived — контекстный флаг).
- **`adoc-parser/src/subst/macros.rs`**: `reconstruct_link_target(work, span, options)` — прогоняет СЫРЫЕ сегменты
  таргета через `crate::inline::apply_typographic_replacements(seg, false, false)` (URL без пробелов → spaced em-dash
  невозможен; `false,false` = embedded mid-line). Sealed Literal/CharRef leaves сплайсятся VERBATIM → escaped `\...`
  (запечатан escape-пассом как `Literal("...")`) остаётся литералом, как asciidoctor `/\\?\.\.\./`. Новый helper
  `maybe_curl_link_target(seg, curl)`. `try_link` no-sentinel ветка теперь `reconstruct_link_target(...)?` (курлит,
  infallible). 4 caller'а (try_link + 3× try_autolink) прокидывают `options`. Курлинг ПОДАВЛЕН при флаге.
- **`adoc-html/src/lib.rs`**: `render_inline_value_with_subs_flag(..., link_target_pre_substituted)` (старый
  `_with_subs` делегирует false).
- **`adoc-html/src/events.rs`**: `AttributeReference` Document-ветка, combined `{attr}value[text]` реинлайн (строка
  ~278) ставит флаг **true** → НЕ курлит повторно. Корень: новый движок гонит escape ДО attributes, поэтому `\...` в
  захваченных trailing_brackets уже потерял backslash к моменту реинлайна → курлинг defeated бы escape. Гард — тест 605.
- **+2 теста**: `unescaped_ellipsis_in_url_target_curls` (parser, event-уровень: top-level курлит, флаг подавляет);
  `test_unescaped_ellipsis_in_link_target_curls_html` (html: link:/bare/URL[text] `...`→`…​`, `--`→`—​`).

### Верификация
- clippy `--workspace` **0**.
- **test --workspace 1297, 0 упавших** (html 531→**532**, parser 645→**646**, compat 233, render-core 25; интеграционные зелёные).
- **Гейт 344/344 байт-в-байт** vs master `a8bdd1c` (база `/tmp/adoc_base` пересобрана из чистого master через stash;
  gate_check.py 0 diff — ни один gate-URL не содержит `...`/`--`).
- **Sweep frontier(250)+adoc2docx(52)=302 new-vs-base: РОВНО 1 файл** (CHANGELOG), **0 регрессий** (inline python
  `/tmp/sweep_bvn.py`).
- **frontier Identical 229→230**; CHANGELOG.adoc token-identical (showdiff пуст), ушёл из CLEAN (4→3).
- CLI-пробы == asciidoctor 2.0.23: link:/bare/URL[text] `...`→`…​`, `--`→`—​` (token-equal через NCR-нейтрализацию).
- Edge-кейсы ПРОВЕРЕНЫ изолированно vs base: ВСЕ пред-существующие (BASE==NEW), НЕ регрессии (см. ниже).

### Состояние репо
- Ветка `fix/replacements-in-link-target` (от master `a8bdd1c`, НЕ закоммичена). master чист == origin.
- Изменены: `adoc-parser/src/{inline.rs, subst/macros.rs, subst/mod.rs[+test]}`, `adoc-html/src/{lib.rs, events.rs, tests.rs[+test]}`.

### Остаток / следующая работа (всё вне scope, пред-существующее, BASE==NEW — НЕ регрессии)
- **`(C)`-после-буквы**: `a(C)b`→`©` у asciidoctor, у нас литерал. Лимит `apply_typographic_replacements` (guard
  `!followed-by-letter`), есть и в плейн-тексте. Отд. задача.
- **mailto/email таргеты не курлятся**: `try_mailto` строит url из `base`+encoded subject/body, НЕ через reconstruct.
  asciidoctor курлит email. Деферно (mailto-url сложнее, не в корпусе).
- **resolved-attr `{u}/path...[t]` Document-реинлайн**: unescaped `...` НЕ курлится (флаг подавляет ради защиты
  escaped `\...`, чей backslash потерян в pass 1). Дивергенция от asciidoctor (курлит). Полный паритет = сохранить
  escape через реинлайн (preserve backslash в trailing_brackets нового движка) — отд. редизайн.
- **undefined-attr `\...` в trailing (MissingSkip path)**: курлит плейн-текстом (нет link). Деферно.
- **`->`/arrow в bare URL**: ломает границу (URL-скан стопает на `>`, `x->y`→`x-`+`&gt;y`). Отд. autolink-boundary баг.
- **Крупные adoc2docx** (мульти-root): test 1105, source 681, xml 291, callouts 195 — Rouge / sequential-quotes.

### Методология (без изменений)
`frontier_parity.py /mnt/c/tmp/adoc2docx` (и `/adoc-frontier`), `showdiff.py <file>`, `gate_check.py` (база
`/tmp/adoc_base` — пересобирать из текущего master через stash!), base-vs-new sweep (`/tmp/sweep_bvn.py` — find
frontier+adoc2docx, diff base vs new бинарей; asciidoctor НЕ звать вживую в ad-hoc — через refcache). Бинарь:
`cargo build --release -p adoc-cli` (имя — `adoc`). asciidoctor 2.0.23 не читает stdin через `-o - -s` — нужен файл.
