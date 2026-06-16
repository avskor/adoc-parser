# Session context

## Сессия (2026-06-16, 89-я) — РЕРАЙТ inline, Фаза 2 (20/N): escape `\((…))` index-term + `\\MM…MM` doubled-marker

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-next`** (off master `408bae9`, 343) —
**НЕ закоммичена, НЕ смержена, ОЖИДАЕТ авторизации** на commit + `git merge --no-ff` в master + `git push`
+ удаление ветки. passthrough-URL link (19/N) к началу сессии УЖЕ смержен (master HEAD `408bae9`).
base-бинарь `/tmp/adoc_base` ПЕРЕСОБРАН из чистого master `408bae9` (blast_toggle base 343, blast_force base 336).

### Выбор задачи
nearmiss под FORCE: ближайший — **subs.adoc (86 diff)**, len_delta=-4. ДВА корня escape: (1) строка 20
`\((DD AND CC) OR (DD AND EE))` — index-term-shorthand escape (1 diff @36); (2) строка 27 `\\__func__` —
doubled-backslash перед unconstrained-маркером (даёт каскад +4 → ~85 позиционных diff @46+). Оба = единственные
кейсы своих паттернов в корпусе (`\((` только subs:20; `\\**` ещё в outline:1487 но ВНУТРИ passthrough `+…+`
→ escape-пасс его не видит). Каскад чисто позиционный (хвост @119+ = сдвиг `<pre>`-блока на 4).

### Корень
- sequential escape-пасс (`subst/escape.rs`) НЕ обрабатывал обе формы (числились в Deferred). Под FORCE:
  `\((…))` → `\DD…EE` (movавил скобки, оставил `\`); `\\__func__` → `\\<em>func</em>` (оставил `\\`, курсивил).
- **legacy** (`inline.rs::handle_inline_escape`): index-арм (~876) — `\((` + `index_term_close` (первый `))`,
  жадно поглощает trailing `)`); non-concealed → `Text("((…))")`; `\(((…)))` concealed → `Text("("), IndexTerm,
  Text(")")`. doubled-арм (~917) — `\\MM` + `find_closing_unconstrained` → `Text("MM")`, inner reparse, `Text("MM")`
  (оба backslash дропаются, контент течёт). asciidoctor: те же выводы (пробы p1/p2/p5 байт-в-байт).

### Сделано (1 логический коммит — ОЖИДАЕТ, 2 файла)
- **subst/escape.rs**: импорт `sentinel_end`. `let quotes_on`. Index-арм в `Some(m)` (перед generic `\{`/`\[`):
  `index_escape(old,bytes,i,macros_on)` → `Macro`-leaf (своё событие, НЕ коалесцирующий Literal — как legacy
  отдельный Text); деклайн при sentinel в контенте. `Some(b'\\')`-арм: `doubled_marker_escape(old,bytes,i,
  quotes_on)` → open-`MM` `Macro`-leaf + RAW inner в `out` (течёт через char_refs/macros/attributes/quotes/
  replacements) + close-`MM` `Macro`-leaf; иначе старый fallback `\\` литерал. Хелперы: `index_escape`,
  `index_term_close` (порт), `doubled_marker_escape`, `find_closing_unconstrained` (порт; над escape-буфером —
  passthrough УЖЕ сентинели, скип через `sentinel_end`). Doc-модуль: обе формы перенесены Deferred→Handled.
- **subst/mod.rs**: doc run_pipeline (убран `\((`/`\\`-doubled из не-портированных); +тест
  `reproduces_legacy_on_index_and_doubled_marker_escape_inputs` (16 кейсов: non-concealed/concealed/no-close
  index, doubled `__`/`**`/`##`/<двойной бэктик>, inner-с-разметкой `\\__a*b*c__`/`*b*`, regression-guards
  неэскейпленных форм).

### КЛЮЧЕВОЕ — `Macro`-leaf даёт точное совпадение событий (гейт АДОПТИТ)
`Literal`-сентинель КОАЛЕСЦИРует с соседним текстом (`\{name}` → один Text); но legacy для `\((…))`/`\\MM`
пушит маркеры ОТДЕЛЬНЫМИ Text-событиями. Поэтому `macro_sentinel(vec![Text(..)])` (как macro-escape арм 18/N) —
flush pending + своё событие, НЕ seed. Для plain-inner (`func`) поток событий = legacy событие-в-событие → гейт
адоптит subs.adoc (не просто деклайн-фоллбэк). Inner-с-разметкой течёт через те же пассы (boundary у сентинеля
= boundary у legacy-reparse-края для constrained-кейсов; em-dash на краю inner — потенциальный дайвердж, не в
корпусе, гейт ловит).

### Верификация (airtight, чистый flip)
- clippy --workspace 0; cargo test --workspace зелёное (parser 550→551, html 433, compat 233/parsing-lab 1,
  render-core 15, integration 25). Пробы p_esc (p1 `\((…))`/p2 `\\__func__`/p5 `\((Two Words))`) — asciidoctor==FORCE;
  гейт==legacy байт-в-байт.
- **blast_toggle (гейт): 343→343, 0 ИЗМЕНЁННЫХ файлов** (airtight: вывод ≡ legacy на всех 344).
- **FORCE: Identical 336→337 (+1 FLIP subs.adoc 86→0, byte-identical 128=128), 0 REGR, 0 FARTHER, 0 паник.**

### Дальше (ОСТАЛОСЬ Фаза 2)
- nearmiss на 337 (FORCE): page-breaks(88), java/index(183), software-development-cookbook(183),
  java/monitoring(185), footnote(283), include(375); outline(5487 — cross-span финал).
- **escape:** `\\` (bare escaped backslash → legacy дропает первый, второй литерал = `\`; sequential оставляет
  `\\` → деклайн), `\\pass:`/`\\https` doubled (дом — passthrough/macros пассы). Все pre-existing-deferred.
- **macros (N+):** UI kbd|btn|menu (проброс `InlineOptions.experimental` — НЕ leaf), footnote (STATEFUL —
  реестр/нумерация/список = отд. сессия, донор 1954).
- **cross-span:** `*x*-- y` em-dash после close-span; A1 bare-autolink-in-mono; **ФИНАЛ:** снять gate → flip
  outline (cross-span @4545) при 343 неизменных.
- Скрипты `/mnt/c/tmp/adoc-test/`: `blast_toggle.py` (гейт), `blast_force.py` (FORCE), `diffone.py <file>`,
  `nearmiss.py` (под FORCE-env). CLI: `adoc [--no-standalone] file`. base пересобирать из master HEAD.

---

## Сессия (2026-06-16, 88-я) — РЕРАЙТ inline, Фаза 2 (19/N): passthrough-защищённый URL в `link:++url++[…]`

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-link-passthrough-url`** (off master `ba712cd`, 343) —
**НЕ закоммичена, НЕ смержена, ОЖИДАЕТ авторизации** на commit + `git merge --no-ff` в master + `git push`
+ удаление ветки. spec'd pass-макрос (18/N) к началу сессии УЖЕ смержен (master HEAD `ba712cd`). base-бинарь
`/tmp/adoc_base` ПЕРЕСОБРАН из чистого master `ba712cd` (blast_toggle base 343, blast_force base 335).

### Выбор задачи
nearmiss под FORCE: ближайший — **url.adoc (21 diff)**, len_delta=2. Все 21 — позиционный каскад от утечки
строки 80 `link:++https://example.org/?q=[a b]++[URL with special characters]`. Единственный «живой»
`link:++…++` в корпусе (остальные 3 — link-macro/ts-url-format/pass-macro — внутри `[source]`/`----`
verbatim, inline-subs не бегут, 0 diff). mailto/xref passthrough-формы в корпусе отсутствуют.

### Корень
- `try_link` отклонял `link:++url++[…]` намеренно (старый doc): к macros-времени `++url++` уже
  passthrough-сентинель (`try_double_plus` → `Passthrough(vec![PassPiece{raw:false}])`), `span_has_sentinel`
  → `None` → под gate откат на legacy (корректный), под FORCE отката нет → `link:…[…]` течёт литералом.
- **legacy** (`inline.rs::try_link_macro` ~2067): спец-кейс `rest.strip_prefix("++")` → URL = вербатим-текст
  между `++…++`, emitted `Cow::Borrowed(url)`; label reparse через `push_macro_label`. URL в href вербатим
  (`[a b]` НЕ percent-кодируется — рендерер не экранирует href ссылки).
- **asciidoctor**: общий механизм — `extract_passthroughs` извлекает `++url++` первым пассом → плейсхолдер,
  затем `link:PLACEHOLDER[…]` матчится, плейсхолдер восстанавливается в URL. Legacy спец-кейс = узкое
  приближение этого.

### Сделано (1 логический коммит — ОЖИДАЕТ, 2 файла, +~75/-17)
- **macros.rs**: `try_link` получил параметр `work: &Work`; старый whole-span `span_has_sentinel`-guard
  заменён на точечный — (1) sentinel в LABEL → decline (движок не репарсит label с sentinel-байтами;
  как и раньше), (2) URL-часть = ровно один passthrough-сентинель → `passthrough_url(work, url_part)`
  реконструирует вербатим-URL из пьес leaf'а → `Cow::Owned`, иначе `Cow::Borrowed(plain)`. Новая функция
  `passthrough_url` (lone-sentinel guard через `sentinel_end`, парс idx, матч `TagToken::Passthrough` → join
  `p.text`). Импорт `TagToken`. Doc `try_link` переписан (legacy спец-кейс + asciidoctor-механизм).
- **mod.rs**: +тест `reproduces_legacy_on_link_passthrough_url_inputs` (8 reproduction-кейсов: `[a b]`/`__`/
  space-protected, bare/label, plain-link regression-guard, в span; +отдельная gate-decline ассерта для
  passthrough-в-LABEL `link:http://x.com[++raw__text++]` — `try_parse` == None, движок не репарсит label).

### КЛЮЧЕВОЕ — gate-эквивалентность через РЕЗОЛВ сентинеля (не decline)
В отличие от прочих макросов (где sentinel в спане = decline), здесь URL-сентинель РЕЗОЛВИТСЯ: legacy
`++`-спец-кейс даёт ровно `Start(Link{url}), <label events>, End(Link)`, и subst после резолва пьес даёт
то же (`Cow::Owned==Cow::Borrowed` по PartialEq, label reparse тот же `run_pipeline` minus MACROS). Generalize
legacy узкого `++`-only на любую passthrough-форму (= asciidoctor); прочие plus-формы (triple `+++` legacy
declines; single `+` другой путь) под gate расходятся → fallback → 0 changed, под FORCE не в корпусе.

### Верификация (airtight, чистый flip)
- clippy --workspace 0; cargo test --workspace зелёное (parser 549→550, html 433, compat 233, render-core 15,
  parsing-lab 1). Пробы p_url1 (label+`[a b]`)/p_url2 (bare+`__`) — asciidoctor==FORCE байт-в-байт.
- **blast_toggle (гейт): 343→343, 0 ИЗМЕНЁННЫХ файлов** (airtight: вывод ≡ legacy на всех 344).
- **FORCE: Identical 335→336 (+1 FLIP url.adoc 21→0, diffone 216=216 байт-в-байт), 0 REGR, 0 FARTHER, 0 паник.**

### Дальше (ОСТАЛОСЬ Фаза 2)
- nearmiss на 336 (FORCE): **subs(86)**, page-breaks(88), java/index(183), software-development-cookbook(183),
  java/monitoring(185), footnote(283), include(375); outline(5487 — cross-span финал).
- **escape:** `\((…))` index-term shorthand (leaf, 1 кейс subs.adoc:20); `\\`/`\\MM` doubled-marker;
  `\\http`/`\\pass:` doubled (pre-existing legacy bug).
- **macros (6/N+):** UI kbd|btn|menu (проброс `InlineOptions.experimental` — НЕ leaf), footnote (STATEFUL —
  реестр/нумерация/список = отд. сессия, донор 1954).
- **cross-span:** `*x*-- y` em-dash после close-span; A1 bare-autolink-in-mono; **ФИНАЛ:** снять gate → flip
  outline (cross-span @4545) при 343 неизменных.
- Скрипты `/mnt/c/tmp/adoc-test/`: `blast_toggle.py` (гейт), `blast_force.py` (FORCE), `diffone.py <file>`,
  `nearmiss.py` (под FORCE-env). CLI: `adoc [--no-standalone] file`. base пересобирать из master HEAD.

---

## Сессия (2026-06-16, 87-я) — РЕРАЙТ inline, Фаза 2 (18/N): spec'd pass-макрос `pass:SPEC[…]`

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-pass-spec-macro`** (off master `a16596b`, 343) —
**НЕ закоммичена, НЕ смержена, ОЖИДАЕТ авторизации** на commit + `git merge --no-ff` в master + `git push`
+ удаление ветки. em-dash attr-ref-boundary (17/N) к началу сессии УЖЕ смержена (master HEAD `a16596b`).
base-бинарь `/tmp/adoc_base` ПЕРЕСОБРАН из чистого master `a16596b` (blast_toggle base 343, blast_force base 331).

### Выбор задачи
nearmiss под FORCE: ближайший — **format-column-content.adoc (8 diff)**, все diffs — утечка `pass:q[` вокруг
`[cols=…]`. Корень общий с align-by-column (20), pass.adoc (135), revision-line (220) — все используют
inline spec'd pass-макрос. Взял spec'd `pass:SPEC[…]` целиком (один корень, флипает 4 файла).

### Корень
- `passthrough.rs::try_pass_macro` обрабатывал ТОЛЬКО bare `pass:[…]` (`spec_len==0`); spec'd `pass:SPEC[…]`
  (`spec_len!=0`) был ОТЛОЖЕН (`return None`) → обёртка `pass:q[`/`]` течёт литералом, а `#e#` обрабатывался
  позже quotes-пассом → `pass:q[<mark>e</mark>]` вместо `<mark>e</mark>`.
- **asciidoctor**: `extract_passthroughs` извлекает `pass:SPEC[text]` ПЕРВЫМ пассом, применяет к контенту
  ИМЕННО spec'd субституции (resolve_pass_subs) и запечатывает результат (защита от остальных пассов).
- **legacy** (`inline.rs::push_pass_spec_content`): inner `InlineState::new(content, set, options).parse_inline`,
  затем `Text→InlinePassthrough` когда `!set.has(SPECIALCHARS)` (renderer экранирует Text безусловно).

### Сделано (1 логический коммит — ОЖИДАЕТ, 2 файла, +133/-10)
- **passthrough.rs**: новый dispatch-арм после bare `try_pass_macro` (тот же `b=='p'`); функция
  `try_pass_spec_macro` (parse `pass:` + spec_len!=0 + контент до первого `]` как `parse_bracket_macro`,
  `spec→pass_spec_to_subs`); функция `pass_spec_events` (inner `super::run_pipeline(content,set)` +
  `Text→InlinePassthrough` при отсутствии SPECIALCHARS; пустой контент → `Vec::new()` чтобы обойти
  empty-buffer guard run_pipeline). Результат запечатывается через `work.macro_sentinel(events)` (атомарный
  leaf — outer quotes/replacements не достают внутрь). Импорт `Event`. Doc модуля + `try_pass_macro` обновлены.
- **mod.rs**: +тест `reproduces_legacy_on_pass_spec_macro_inputs` (19 кейсов: q/q,a/c,a/quotes/macros/r/n,
  in-backtick cols-паттерн, пустой контент, mid-run flush, bare-форма regression-guard).

### КЛЮЧЕВОЕ — spec'd pass = flush-граница в ОБОИХ движках
В отличие от escaped `\pass:` (17/N донор — legacy flush НА бэкслеше, flat-движок не вставляет сентинель →
расхождение событий), spec'd pass **вставляет сентинель** там, где legacy делает `flush_text` → mid-run
(`the text pass:q[#x#] here`) и in-span совпадают событие-в-событие. Inner re-parse через `run_pipeline`
(а не `parse_legacy`) — тот же паттерн, что label-reparse в macros.rs; gate ловит любое расхождение.

### Верификация (airtight, +4 FLIP)
- clippy --workspace 0; cargo test --workspace зелёное (parser 548→549, html 433, compat 233, render-core 15,
  parsing-lab 1). 11 проб (q/q,a/c,q/c,a/quotes/macros/r/n/empty/bare/in-backtick) — asciidoctor==FORCE==legacy.
- **blast_toggle (гейт): 343→343, 0 ИЗМЕНЁННЫХ файлов** (airtight: вывод ≡ legacy на всех 344).
- **FORCE: Identical 331→335 (+4 FLIP), 0 REGR, 0 FARTHER, 0 паник.** Флипы: revision-line.adoc 220→0,
  pass.adoc 135→0, align-by-column.adoc 20→0, format-column-content.adoc 8→0 (диффы были позиционным
  каскадом от утёкшей обёртки `pass:q[`/`]` — устранение выровняло весь токен-стрим).

### Дальше (ОСТАЛОСЬ Фаза 2)
- nearmiss на 335 (FORCE): url(21), subs(86), page-breaks(88), pass→УЖЕ 0, java/index(183), revision-line→УЖЕ 0,
  footnote(283), include(375); outline(5487 — cross-span финал).
- **escape:** `\((…))` index-term shorthand (leaf, 1 кейс subs.adoc:20); `\\`/`\\MM` doubled-marker;
  `\\http` doubled (pre-existing legacy bug).
- **edge-case spec'd pass (НЕ в корпусе):** `pass:r[--]` (`--` на самом краю контента) — inner run_pipeline
  гонит replacements `(true,true)` → em-dash, legacy `edges=false` → литерал → gate declines (safe). Если
  встретится — нужен проброс edge-флага в run_pipeline.
- **macros (6/N+):** UI kbd|btn|menu (проброс `InlineOptions.experimental` — НЕ leaf), footnote (STATEFUL).
- **cross-span:** `*x*-- y` em-dash после close-span; A1 bare-autolink-in-mono; **ФИНАЛ:** снять gate → flip outline.
- Скрипты `/mnt/c/tmp/adoc-test/`: `blast_toggle.py` (гейт), `blast_force.py` (FORCE), `diffone.py <file>`,
  `nearmiss.py` (под FORCE-env). CLI: `adoc [--no-standalone] file`. base пересобирать из master HEAD.

---

## Сессия (2026-06-16, 86-я) — РЕРАЙТ inline, Фаза 2 (17/N): em-dash на границе attr-ref/attr-set сентинеля

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-emdash-attrref-boundary`** (off master `e25f45c`, 343) —
**НЕ закоммичена, НЕ смержена, ОЖИДАЕТ авторизации** на commit + `git merge --no-ff` в master + `git push`
+ удаление ветки. autolink-escape (16/N) к началу сессии УЖЕ смержена (master HEAD `e25f45c`). base-бинарь
`/tmp/adoc_base` ПЕРЕСОБРАН из чистого master `e25f45c` (blast_toggle base 343, blast_force base 330).

### Выбор задачи
nearmiss под FORCE: ближайший 1-diff = **subs-symbol-repl.adoc** (`@125 exp='—' got='--'`, строка 27
`|{empty}--{empty}`). Прочие 1-diff отсутствуют (следующий 8-diff format-column-content). Взял em-dash на
границе attr-ref — чистый flip, зеркалит legacy, НЕ трогает cross-span quote-проблему.

### Корень (диагностика пробами)
- asciidoctor резолвит `{empty}`→"" в пассе **attributes ДО replacements** → `--` оказывается на границах
  строки/между словами → spaced em-dash `&#8201;—&#8201;` (thin-em-thin). `a{empty}--{empty}b` →
  `a&#8212;&#8203;b` (word-вариант). Оба нормализуются в `—`.
- Наш движок НЕ резолвит attr-ref (эмитит `AttributeReference`, рендерер резолвит) → `{empty}` = **сентинель**.
  replacements гонялся по ВСЕМУ буферу `(true,true)`; внутренний `--` окружён сентинель-байтами (как `<>`) →
  не граница → НЕТ em-dash. Под FORCE `--`, под gate откат на legacy ` — ` (gate DECLINES).
- **Legacy механизм** (`inline.rs` flush_text ~1059): attr-ref = отдельное событие, **разбивает Text-ран**;
  край разрыва = граница (`left=start!=0`, `right=end<len`). Quote-контент — изолированный репарс
  `edges_are_line_boundaries=false` → НЕ граница. Так legacy различает `{empty}--{empty}` (em-dash) и
  `*--*` (литерал внутри strong) БЕЗ различения типов сентинелей — за счёт рекурсии. Плоский движок держит
  оба в одном буфере → различие только в ТИПЕ сентинеля (attr-ref vs quote).

### Архитектура фикса (subst/replacements.rs — split-by-attr-ref, REUSE legacy-функции)
- **Минимальный зеркалящий фикс:** разбить буфер на сегменты по **AttrRef/AttrSet** сентинелям (единственные,
  что в legacy разбивают Text-ран); применить `apply_typographic_replacements(seg, true, true)` посегментно;
  сентинели сохранить ВЕРБАТИМ между сегментами. Края сегментов у attr-ref-разрывов → реальные `^`/`$`
  (флаги функции) → em-dash формируется. Quote/passthrough/macro-сентинели остаются ВНУТРИ сегмента → их
  `<tag>`-не-граничная трактовка цела (`*--*`, `*--*{empty}` → `--` литерал).
- **Fast-path:** буфер БЕЗ attr-ref/attr-set = один сегмент = весь буфер `(true,true)` → байт-в-байт прежнее
  поведение (no regression by construction; нет лишней аллокации для частого случая plain-text).
- **НЕ копирует флаговую consume-логику в shared-функцию:** spaced em-dash при `i==0` сегмента `copy_end=0`
  (не съедает байт слева), сентинель ВНЕ сегмента → цел. Попытка крутить boundary-булевы в общей функции
  дропнула бы `bytes[i-1]` (= attr-ref TAG_TAIL) при `copy_end=i-1` → порча сентинеля. Поэтому split, а не флаг.
- **Cross-span quote-граница НЕ в скоупе:** `*x*-- y` (legacy формирует em-dash после close-span, у нас close-
  сентинель не-граница) — pre-existing divergence, БЫЛА и до меня (whole-buffer тоже не давал em-dash) →
  gate DECLINES, 0 REGR. Отдельная cross-span задача (дом — open vs close различение).

### Сделано (1 логический коммит — ОЖИДАЕТ, 2 файла)
- **subst/replacements.rs**: `run()` переписан (fast-path + split-loop); хелперы `apply_segment`,
  `has_boundary_sentinel`, `is_boundary_sentinel` (парсит idx сентинеля, матч `AttrRef|AttrSet`); модуль-doc
  «Attribute-reference sentinels are run boundaries». Импорт `{sentinel_end, TagToken, Work, TAG_LEAD}`.
- **subst/mod.rs**: +тест `reproduces_legacy_on_attr_ref_emdash_boundary_inputs` (19 кейсов: attr-ref/attr-set
  em-dash, real-spaces, `*--*{empty}` span-литерал, ellipsis/(C)/arrow рядом, apostrophe на краю, trailing-bracket
  `{url}[x]`, interleaved `{a}--{b}--{c}`).

### Верификация (airtight, чистый flip)
- clippy --workspace 0 (pre-existing `concat!` в adoc-html lib-тесте — не мой файл); cargo test --workspace
  зелёное (parser 547→548, html 433, compat 233, render-core 15, parsing-lab 1).
- **blast_toggle (гейт): 343→343, 0 ИЗМЕНЁННЫХ файлов** (airtight: вывод ≡ legacy на всех 344).
- **FORCE: Identical 330→331 (+1 FLIP subs-symbol-repl.adoc 1→0, diffone byte-identical 295=295), 0 REGR,
  0 FARTHER, 0 паник.** subs.adoc 86 без изменений (его diffs — callouts, не em-dash). 11 проб legacy==FORCE.

### Дальше (ОСТАЛОСЬ Фаза 2)
- nearmiss на 331 (FORCE): format-column-content (8), align-by-column (20), url (21), subs (86), page-breaks
  (88), pass (135), revision-line (220), footnote (283), include (375); outline (5487 — cross-span финал).
- **escape:** `\((…))` index-term shorthand (leaf, 1 кейс subs.adoc:20, не флипнет в одиночку);
  `\\`/`\\MM` doubled-marker (дом — quote-пассы, отложено с 8/N); `\\http` doubled (pre-existing legacy bug).
- **macros (6/N+):** UI kbd|btn|menu (проброс `InlineOptions.experimental` через pipeline — НЕ leaf),
  footnote (STATEFUL — реестр/нумерация/список = отд. сессия, донор 1954).
- **cross-span замены:** `*x*-- y` em-dash после close-span (open vs close различение сентинеля) — pre-existing.
- **A1 — bare autolink в монопространстве** (`` `http://x` ``): pre-existing gap (macros до quotes), реордер.
- **ФИНАЛ Фазы 2:** снять gate → flip outline (cross-span @4545) при 343 неизменных.
- Скрипты `/mnt/c/tmp/adoc-test/`: `blast_toggle.py` (гейт), `blast_force.py` (FORCE), `diffone.py <file>`,
  `nearmiss.py` (под FORCE-env). CLI: `adoc [--no-standalone] file` (НЕ `-s`). base пересобирать из master HEAD.

---

## Сессия (2026-06-16, 85-я) — РЕРАЙТ inline, Фаза 2 (16/N): escaped autolink `\http://…` (autolink escape)

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-autolink-escape`** (off master `05454b4`, 343) —
**ЗАКОММИЧЕНА (`468e7ad`), НЕ смержена, ОЖИДАЕТ авторизации** на `git merge --no-ff` в master + `git push`
+ удаление ветки. pass-escape (15/N) к началу сессии УЖЕ смержена (master HEAD `05454b4`). base-бинарь
`/tmp/adoc_base` ПЕРЕСОБРАН из чистого master `05454b4` (blast_toggle base 343, blast_force base 329).

### Выбор задачи
nearmiss под FORCE дал 15 Different. Ближайший 1-diff = **links.adoc** (`` `\https://…` `` in-backtick,
строка 17 — единственный diff; строка 123 `\https://` в `[source]`/`----` verbatim, не считается).
Второй 1-diff = subs-symbol-repl (em-dash `—` vs `--`, replacements, НЕ escape — отложен). Корпус-survey
`\http`: 4 места — autolinks.adoc:71 + links.adoc:123 (оба verbatim, ОК), links.adoc:17 (in-backtick →
flip), subs.adoc:23 (bare space, в paragraph → subs.adoc closer не flip). Взял escaped autolink целиком.

### Архитектура (escaped `\http://…` — дом в MACROS-пассе, НЕ escape.rs)
- **ПОЧЕМУ macros, а не escape.rs:** порядок пайплайна тут = passthrough → escape → char_refs →
  **macros (191) → attributes → quotes (202)** (macros ПЕРЕД quotes, вопреки asciidoctor). Автолинк живёт
  в macros. escape.rs blanket-арм оставляет `\http` литералом (advance только за `\`) → доживает до macros.
- **МЕХАНИЗМ (зеркало легаси):** при `\`+scheme дропнуть `\` (НЕ копировать в out), но ОСТАВИТЬ в src →
  следующая итерация на scheme `h`: `at_autolink_boundary(scheme_pos)` видит `\` в src → не автолинкует →
  URL течёт литералом. Точно как легаси `handle_inline_escape` (advance_by(1), input хранит `\`).
- **BOUNDARY-условие `escaped_autolink_boundary(work,bytes,i)`:** дропнуть когда (1) `at_autolink_boundary`
  (start/ws/`<>()[];`) ИЛИ (2) `bytes[i-1]` = constrained-маркер `` ` ``/`*`/`_`/`#` И
  `quotes::constrained_open_close(...)`=Some, ИЛИ (3) `^`/`~` И `quotes::simple_pair_open_close(...)`=Some.
  Случай (2)/(3) = pre-quotes стенд-ин для asciidoctor `>`-после-`<code>` (quotes ещё не пробежал, тег не
  материализован, но детектор спана говорит, СФОРМИРУЕТСЯ ли он). Проверка спан-формирования = НЕТ over-fire
  на `a*\http` (нет закрытия `*`) и `` a`\http `` (backtick после word) — совпадает с asciidoctor и легаси.
- **`\\http` (doubled) и mid-run после текста ИСКЛЮЧЕНЫ:** `\\http` — легаси дропает один `\`, asciidoctor
  держит оба (pre-existing legacy bug); мой движок держит оба (=asciidoctor) → gate declines vs legacy →
  fallback (нет корпус-кейса). mid-run `before \http://x` — легаси `flush_text` НА бэкслеше → 2 Text
  (URL в свежем ране), мой flat-движок мёржит в 1 Text → событийно расходится, HTML идентичен, gate declines.
- **A1 НЕ трогаю (pre-existing gap):** bare автолинк ВНУТРИ монопространства (`` `http://x` `` без `\`) не
  автолинкуется новым движком (macros до quotes, backtick не boundary) → `<code>http://x</code>` vs
  asciidoctor `<code><a>`. Это не escape, отдельный gap (нужен autolink-после-quotes / реордер). НЕ в скоупе.

### Сделано (1 логический коммит `468e7ad`, 4 файла, +122/-6)
- **quotes.rs**: `constrained_open_close`/`simple_pair_open_close` → `pub(super)` (+doc «reused by macros»).
- **macros.rs**: новый `\`-арм (перед autolink-армом) + хелпер `escaped_autolink_boundary` + модуль-doc.
- **escape.rs**: doc — `\https://…` перенесён из Deferred в «Handled by the macros pass».
- **mod.rs**: +тест `reproduces_legacy_on_autolink_escape_inputs` (15 кейсов).

### Верификация (airtight, чистый flip)
- clippy --workspace 0 (всё-таргетный `concat!`-warning в adoc-html lib-test — PRE-EXISTING, не мой файл);
  cargo test --workspace зелёное (parser 546→547, html 433, compat 233/233, render-core 15).
- **blast_toggle (гейт): 343→343, 0 ИЗМЕНЁННЫХ файлов** (airtight: вывод ≡ legacy на всех 344).
- **FORCE: Identical 329→330 (+1 FLIP links.adoc 1→0 байт-в-байт), subs.adoc closer 87→86, 0 REGR,
  0 FARTHER, 0 паник.**

### Дальше (ОСТАЛОСЬ Фаза 2)
- **escape (продолжение):** `\((…))` index-term shorthand (чистый leaf как 14/N, concealed-vs-flow; 1
  корпус-кейс subs.adoc:20, не флипнет в одиночку — субс.adoc 86 diffs), `\\`/`\\MM` doubled-marker
  (дом — quote-пассы, отложено с 8/N), `\\http` doubled (pre-existing legacy bug, asciidoctor держит оба).
- **A1 — bare autolink в монопространстве** (`` `http://x` ``): pre-existing gap (macros до quotes). Дом —
  autolink-детект ПОСЛЕ quotes (реордер или спец-обработка в quotes/macros). Архитектурно крупнее одного арма.
- **macros (6/N+)**: UI kbd|btn|menu (проброс `InlineOptions.experimental` через pipeline — НЕ leaf),
  footnote (STATEFUL — реестр/нумерация/список = отд. сессия, донор 1954).
- **ФИНАЛ Фазы 2:** снять gate → flip outline (cross-span @4545) при 343 неизменных.
- Скрипты `/mnt/c/tmp/adoc-test/`: `blast_toggle.py` (гейт), `blast_force.py` (FORCE), `diffone.py <file>`,
  `nearmiss.py` (под FORCE-env). base пересобирать из master HEAD (`cargo build --release -p adoc-cli`).

---

## Сессия (2026-06-16, 84-я) — РЕРАЙТ inline, Фаза 2 (15/N): escape `\pass:SPEC[…]` (порт pass_escape_prefix_len)

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-pass-escape`** (off master `b9f03ff`, 343) —
**НЕ закоммичена, НЕ смержена, ОЖИДАЕТ авторизации** на commit + `git merge --no-ff` в master + `git push`
+ удаление ветки. escape `\macro` (14/N) к началу сессии УЖЕ смержена (master HEAD `b9f03ff`). base-бинарь
`/tmp/adoc_base` ПЕРЕСОБРАН из чистого master `b9f03ff` (blast_toggle base 343, blast_force base 326).

### Выбор задачи
nearmiss под FORCE (`ADOC_QUOTES_SEQUENTIAL=1 ADOC_SUBST_FORCE=1 python3 nearmiss.py`) дал пять 1-diff
файлов. **ТРИ** флипает escaped pass-макрос: attribute-entry-substitutions.adoc, footnote.adoc,
literal-monospace.adoc — все `` `\pass:[]` ``/`` `\pass:c[]` `` (в монопространстве). Текущий баг под FORCE:
passthrough-пасс извлекал `pass:[]` как passthrough → оставался голый `\` → `<code>\</code>`. Прочие 1-diff:
links.adoc (`\https://` in-backtick — ОТЛОЖЕН: нужен seal URL-экстента + left-boundary, дом — macros-пасс
ПОСЛЕ quotes, где монопространство уже сентинелизировано), subs-symbol-repl.adoc (em-dash `—` vs `--` —
replacements, не escape). `\pass:` выбран как escape БЕЗ зависимости от левой границы → работает и in-backtick.

### Архитектура (escaped `\pass:SPEC[…]` — дом в PASSTHROUGH-пассе, НЕ escape.rs)
- **Почему passthrough, а не escape.rs:** escaped pass — НЕ плейн-литерал. Легаси (`handle_inline_escape`
  арм `pass_escape_prefix_len`) дропает `\`, оставляет `pass:SPEC[` литералом, а содержимое `[...]`+`]`
  ТЕЧЁТ через остальные subs (`\pass:c[*b*]` → `pass:c[<strong>b</strong>]`). Passthrough бежит ПЕРВЫМ и
  иначе извлёк бы `pass:[]` целиком — поэтому escape принадлежит туда (как уже сделанный `\+` escape).
- **passthrough.rs**: новый арм в `extract()` (после `\+`-арма, перед `+`-passthroughs): `b==\\` +
  guard `(i==0 || bytes[i-1] != \\)` + `pass_escape_prefix_len(src, i+1)` → `out.push_str(pass:SPEC[)`
  (дроп `\`), `i += 1 + prefix_len`, continue. Содержимое после `[` сканируется тем же циклом и
  последующими пассами как обычный текст. Helper `pass_escape_prefix_len` (порт легаси: `pass:` +
  опц. lowercase spec + `[`, длина = `5 + spec_len + 1`; reuse `scanner::pass_spec_len`).
- **`\\pass:` doubled ОТЛОЖЕН** (guard `bytes[i-1] != \\` гасит второй бэкслеш — как у `\+`). Поведение
  `\\pass:` НЕ изменилось моим коммитом (verified трассировкой: первый `\` копируется, второй не фаерит).
- **escape.rs**: только doc — `\pass:` перенесён из «Deferred» в «Handled by the quote/passthrough passes».

### КЛЮЧЕВОЕ — gate-эквивалентность на границе flush
tokenize коалесцирует сырой текст в `pending` до сентинеля; мой `\pass:` сентинель НЕ вставляет →
непрерывный Text. Легаси делает `flush_text` В ПОЗИЦИИ бэкслеша. ⇒ совпадает event-в-event ТОЛЬКО когда
escape стоит на границе flush (начало input ИЛИ край спана — пустой split). Bare `\pass:` mid-run после
текста того же рана (`before \pass:[x]`) даёт 1 Text vs легаси 2 Text — gate DECLINES, fallback (HTML
идентичен, adjacent-text мержится). **Все 3 корпусных кейса — in-backtick (escape в начале спана) → gate
ADOPTS.** Тест включает только boundary-кейсы; mid-run исключён (документирован).

### Сделано (1 логический коммит, 3 файла, +99/-3)
- **passthrough.rs** (код+doc+helper), **escape.rs** (doc), **mod.rs** (+тест
  `reproduces_legacy_on_pass_escape_inputs`, 14 кейсов: bare/spec пустые, content-flow quotes/specialchars,
  in-backtick корпус-паттерн, no-bracket→не-escape, non-`pass:` имя).

### Верификация (airtight, чистый flip)
- clippy --workspace 0; cargo test --workspace зелёное (parser 545→546, html 433, compat 233/233); subst 24 теста (+1).
- **blast_toggle (гейт): 343→343, 0 ИЗМЕНЁННЫХ файлов** (airtight: вывод ≡ legacy на всех 344).
- **FORCE: Identical 326→329 (+3 FLIP), 0 REGR, 0 FARTHER, 0 паник.** Флипы ровно предсказанные:
  attribute-entry-substitutions 1→0, footnote 1→0, literal-monospace 1→0. Все 4 пробы байт-в-байт с asciidoctor.

### Дальше (ОСТАЛОСЬ Фаза 2)
- **escape (продолжение):** `\https://…` autolink (in-backtick — нужен seal URL-экстента + left-boundary,
  дом — macros-пасс после quotes; bare-форма проще, но links.adoc расходится именно на in-backtick),
  `\((…))` index-term shorthand (чистый leaf как 14/N, concealed-vs-flow; 1 корпус-кейс subs.adoc, не флипнет
  в одиночку), `\\`/`\\MM` doubled-marker (дом — quote-пассы, отложено с 8/N).
- **macros (6/N+)**: UI kbd|btn|menu (проброс `InlineOptions.experimental` через pipeline — НЕ leaf),
  footnote (STATEFUL — реестр/нумерация/список = отд. сессия, донор 1954).
- **ФИНАЛ Фазы 2:** снять gate → flip outline (cross-span @4545) при 343 неизменных.
- Скрипты `/mnt/c/tmp/adoc-test/`: `blast_toggle.py` (гейт), `blast_force.py` (FORCE), `diffone.py <file>`,
  `nearmiss.py` (под FORCE-env = ранжир near-flip файлов нового движка).

---

## Сессия (2026-06-15, 83-я) — РЕРАЙТ inline, Фаза 2 (14/N): escape `\macro` (порт inline_macro_escape_len)

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-macro-escape`** (off master `9c6a219`, 343) —
**НЕ закоммичена, НЕ смержена, ОЖИДАЕТ авторизации** на commit + `git merge --no-ff` в master + `git push`
+ удаление ветки. anchor+index-term (13/N) к началу сессии УЖЕ смержена (master HEAD `9c6a219`). base-бинарь
`/tmp/adoc_base` ПЕРЕСОБРАН из чистого master `9c6a219` (blast_toggle base 343, blast_force base 325 —
base = master-движок под FORCE, т.к. blast_* пробрасывает env во ВСЕ subprocess'ы; дельта = чисто мой инкремент).

### Выбор задачи
По плану «Дальше» 82-й: escape `\macro` (порт `inline_macro_escape_len`) — помечен как ДЕШЁВЫЙ FORCE-win
(escaped = литерал, impl-движок не нужен). Взял именно его. Корпус-данные: `\indexterm:`/`\indexterm2:`
в `user-index.adoc` (handled), `\https://` ×4 (autolinks/links/subs) + `\((` ×1 (subs) — ОТЛОЖЕНЫ (отдельные
code-path'ы: autolink нужен left-boundary look-back, `\((` — concealed-vs-flow). Скоуп = только
`inline_macro_escape_len` (12 именованных макросов `name:target[…]`).

### Архитектура (escape `\name:target[…]` = некоалесцирующий leaf)
- **escape.rs**: `run(work)` → `run(work, subs)` (+ `macros_on = subs.has(MACROS)`, гейт как у легаси
  `inline_macro_escape_len`). Новый арм в `Some(m)` (после typographic, перед cref — триггеры
  s/l/a/x/m/i/f НЕ пересекаются с typographic/cref/smartquote/generic): `mlen>0` → drop `\`, запечатать
  `old[i+1..i+1+mlen]` как leaf. Функция `macro_escape_len(bytes,p)` — порт легаси (12 NAMES, reject `name::`
  блок-форма, target=non-ws до `[`, скан до `]` inclusive).
- **КЛЮЧЕВОЕ — некоалесцирующий leaf:** легаси-macro-escape пушит ОТДЕЛЬНЫЙ `Text` (НЕ мёрджит с хвостом:
  `\link:u[t] more` → `[Text("link:u[t]"), Text(" more")]` — эмпирически подтверждено probe-тестом). Поэтому
  `macro_sentinel(vec![Text(Owned(form))])` (атомарный, flush_pending → push), НЕ `literal_sentinel`
  (коалесцирует — разошёлся бы на хвостовом тексте). Переиспользую `Macro`-токен (opaque, verbatim, atomic).
- **sentinel-guard в `macro_escape_len`:** escape бежит ПОСЛЕ passthrough → target/content мог уже содержать
  сентинель (`\link:u[+pass+]`); встретив TAG_LEAD/TAG_TAIL в скане → return 0 (decline, gate fallback) —
  легаси видел вербатим-исходник. Зеркало `span_has_sentinel`-стражей в macros.rs.
- **mod.rs**: вызов `escape::run(&mut work, subs)`; doc run_pipeline + escape.rs модуль-doc (macro-escape в
  Handled, уточнён Deferred: `\pass:` не плейн-литерал, `\https://`/`\((` — отд. code-path'ы).

### Сделано (1 логический коммит, 2 файла, +170/-11)
- **escape.rs**: импорты (`Cow`/`Event`/`SubstitutionSet`/`TAG_LEAD`/`TAG_TAIL`), сигнатура `run`, арм
  `mlen>0`, функция `macro_escape_len`; модуль-doc.
- **mod.rs**: вызов + doc; +тест `reproduces_legacy_on_macro_escape_inputs` (24 кейса: 12 имён, хвостовой
  текст=отд. события, bare, attr-ref-like content, mid-word, в спане, `\image::` блок-форма, no-bracket).

### Верификация (airtight, чистый flip)
- clippy --workspace 0 (фикс explicit_auto_deref `*n`→`n`); cargo test --workspace зелёное (parser 544→545,
  html 433); subst 23 теста (+1).
- **blast_toggle (гейт): 343→343, 0 ИЗМЕНЁННЫХ файлов** (airtight: вывод ≡ legacy на всех 344).
- **FORCE: Identical 325→326 (+1 FLIP), 0 REGR, 0 FARTHER, 0 паник.** FLIP `user-index.adoc` 4→0 (diffone
  под FORCE = 0 diffs, 294=294). outline/subs/autolinks/links НЕ флипнули (прочие/отложенные расхождения).

### Дальше (ОСТАЛОСЬ Фаза 2)
- **escape (продолжение):** `\https://…` autolink (нужен seal URL-экстента + left-boundary; в sequential
  нельзя просто drop `\` — autolink перевыстрелит), `\((…))` index-term shorthand (concealed-vs-flow),
  `\pass:SPEC[…]` (drop `\`, но subs над контентом — НЕ плейн-литерал), `\\`-doubled, doubled-marker `\\MM`.
- **macros (6/N+)**: UI kbd|btn|menu (проброс `InlineOptions.experimental` через pipeline — НЕ leaf),
  footnote (STATEFUL — реестр/нумерация/список = отд. сессия, донор 1954).
- **ФИНАЛ Фазы 2:** снять gate → flip outline (cross-span @4545) при 343 неизменных.
- Скрипты `/mnt/c/tmp/adoc-test/`: `blast_toggle.py` (гейт), `blast_force.py` (FORCE), `diffone.py <file>`.

---

## Сессия (2026-06-15, 82-я) — РЕРАЙТ inline, Фаза 2 (13/N): macros (5/N) — anchor + index-term

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-macros-anchor-index`** (off master `f1226b6`, 343) —
**НЕ закоммичена, НЕ смержена, ОЖИДАЕТ авторизации** на commit + `git merge --no-ff` в master + `git push`
+ удаление ветки. leaf icon+STEM (12/N) к началу сессии УЖЕ смержена (master HEAD `f1226b6`). base-бинарь
`/tmp/adoc_base` ПЕРЕСОБРАН из чистого master (blast_toggle base 343, blast_force base 313).

### Выбор задачи
ОСТАЛОСЬ macros (5/N+): UI kbd|btn|menu (нужен проброс experimental — НЕ leaf), anchor, index-term,
footnote (stateful). Взял **anchor + index-term** — обе семьи чистые leaf (id/label/term verbatim, БЕЗ
re-parse, `subs` не нужен), структурно как icon/image. Объединил в один инкремент (прецедент 12/N: icon+STEM
вместе). footnote ОТЛОЖЕН (stateful), UI ОТЛОЖЕН (experimental-проброс).

### Архитектура (anchor + index-term = leaf)
- **anchor** (macros.rs): `try_anchor` (`[[id]]`/`[[id,label]]` — comma: id.trim_end / label.trim_start,
  пустой label дроп через `.then`), `try_bibliography_anchor` (`[[[id]]]` — оба компонента .trim(), пустой
  label ОСТАЁТСЯ `Some` — отличие от plain anchor, зеркалю донор), `try_anchor_macro` (`anchor:id[label]` —
  target `\S+`, whitespace/empty→decline). Диспетч `[`: фаерит ТОЛЬКО при `bytes[i+1]==[` (одиночный `[` =
  quotes attrlist `[.role]#x#`, отд. пасс ПОЗЖЕ — macros не трогает); `[[[` (bib) проверяется ПЕРЕД `[[`.
- **index-term** (macros.rs): `try_index_term` (`((…))`; `index_term_close` non-greedy `(.+?)\)\)(?!\))` —
  `))` со следующим `)` сползает на 1; форма по enclosing-скобкам контента: both→`ConcealedIndexTerm`,
  только-leading→`Text("(")`+flow `IndexTerm`, только-trailing→flow+`Text(")")`, neither→flow), `try_indexterm`
  (`indexterm:[p,s,t]`→Concealed), `try_indexterm2` (`indexterm2:[term]`→flow). Helper `concealed_index_term`
  (splitn(3,',') trim). Литеральный `(`/`)` = свой `Text`-event в Macro-leaf (токенайзер НЕ коалесцирует
  события macro-leaf — flush_pending + push раздельно → ≡ legacy flush_text+push).
- **span_has_sentinel guard** на ВСЕХ 6 (как у image/icon/stem): сентинель внутри (passthrough/escape/char-ref
  лифтнул из id/term) → decline, gate fallback (содержимое verbatim разошлось бы с legacy). Tag-поля
  `Cow::Owned` (==Borrowed по PartialEq → adopt). Failure-advance `+1` (как легаси для всех этих; anchor_macro
  легаси +7 но эквивалентно — внутри «anchor:» нет macro-старта).

### Сделано (1 логический коммит, 2 файла)
- **macros.rs**: армы `indexterm2:`/`indexterm:` (после icon), `anchor:` (перед asciimath), `[[[`/`[[` и `((`
  (после `<<`); функции try_anchor/try_bibliography_anchor/try_anchor_macro/index_term_close/try_index_term/
  try_indexterm/try_indexterm2 + helper concealed_index_term. Doc-комментарий модуля: (5/N) anchor+index-term.
- **mod.rs**: +2 теста `reproduces_legacy_on_anchor_inputs` (19 кейсов) / `reproduces_legacy_on_index_term_inputs`
  (18 кейсов); doc-комментарии `run_pipeline` обновлены.

### Верификация (airtight, чистый flip)
- clippy --workspace 0; cargo test --workspace зелёное (parser 544 = +2, html 433); subst 22 теста.
- **blast_toggle (гейт): 343→343, 0 ИЗМЕНЁННЫХ файлов** (airtight: вывод ≡ legacy на всех 344).
- **FORCE (base чистый master): Identical 313→325 (+12 FLIP), 0 REGR, 0 паник.** Флипы: document-attributes-ref
  5751→0, lexicon 498→0, span-cells 275→0, id 113→0, custom-attributes 82→0, bibliography 19→0, add-columns/
  add-cells-and-rows/release-and-progress/pass-macro/CONTRIBUTING/attribute-terms→0. **outline FARTHER 4797→5487**
  — ЭКСПЕКТЕД каскад (anchor/index-term теперь извлекаются, но прочие отложенные фичи расходятся; gate отклоняет).

### Дальше (ОСТАЛОСЬ Фаза 2)
- **macros (6/N+)**: UI kbd|btn|menu (нужен проброс `InlineOptions.experimental` через pipeline — НЕ leaf),
  footnote (STATEFUL — реестр/нумерация/список = отд. сессия, донор 1954). escape `\macro` (порт
  `inline_macro_escape_len` в escape.rs — дешёвый FORCE-win), escape маркеров doubled/`\\MM` (отложено с 8/N).
  specialchars = NO-OP. ФИНАЛ: снять gate → flip outline при 343 неизменных.

---

## Сессия (2026-06-15, 81-я) — РЕРАЙТ inline, Фаза 2 (12/N): macros (4/N) — leaf-макросы icon + STEM

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-macros-leaf`** (off master, 343) — **НЕ закоммичена,
НЕ смержена, ОЖИДАЕТ авторизации** на commit + `git merge --no-ff` в master + `git push` + удаление ветки.
image (11/N) к началу сессии УЖЕ смержена (master HEAD `a0c56a6`). base-бинарь `/tmp/adoc_base` ПЕРЕСОБРАН
из чистого master (legacy-эталон; blast_toggle подтвердил base 343, blast_force base 312).

### Выбор задачи
По плану следующий пункт — macros (4/N+): footnote/icon/UI(kbd|btn|menu)/stem/anchor/index-term. Взял
**leaf-макросы icon + STEM** — структурно идентичны inline image (leaf, БЕЗ label re-parse, БЕЗ options).
UI (kbd/btn/menu) ОТЛОЖЕН: нужен проброс `InlineOptions.experimental` через `run_pipeline`/`extract`
(рефактор сигнатуры + рекурсивные вызовы push_label/build_cross_reference) — отдельный инкремент.
footnote ОТЛОЖЕН (stateful). При experimental=off (дефолт) UI и так литерал → gate не страдает.

### Архитектура (icon/stem = leaf, как image)
- **icon** (`try_icon`, зеркало `try_icon_macro`+`parse_target_bracket_macro`): триггер `i`+`icon:`,
  `name`→`Tag::Icon`, attrlist (если непуст)→ОДИН raw `Text`. Empty-name → decline; `]` = первый после `[`.
- **STEM** (`try_stem`, зеркало `try_stem_macro`+`parse_bracket_macro_escaped`): три написания
  `stem:[`/`latexmath:[`/`asciimath:[` (триггеры `s`/`l`/`a`, `[` сразу после `:` → target пуст),
  variant→`Tag::Stem`, content→ОДИН raw `Text`. **`\]`-escape**: `]` за `\` не закрывает, все `\]`→`]`.
  Escape-пасс НЕ трогает `\]` (blanket-арм оставляет `\` литералом) → escaped-bracket доживает до macros.
- **span_has_sentinel guard** на обоих (как у image): если escape/passthrough/char-ref что-то лифтнул
  изнутри (`stem:[\{a}]`, `\--` внутри) → decline, gate fallback. Tag-поля `Cow::Owned` (==Borrowed по
  PartialEq → adopt). НЕТ left-boundary (как у легаси): `prefixicon:x[]` матчит icon в середине слова —
  ОБА движка одинаково (равенство держится).

### Сделано (1 логический коммит, 2 файла)
- **macros.rs**: ветки `icon:`/`stem:[`/`latexmath:[`/`asciimath:[` в `extract` (после image, перед `<<`);
  `try_icon(src,start)` и `try_stem(src,start,prefix_len,variant)`. Doc-комментарий модуля: leaf-макросы
  добавлены в (4/N), UI помечены как требующие проброса experimental.
- **mod.rs**: +тест `reproduces_legacy_on_leaf_macro_inputs` (22 кейса: icon bare/attrs/invalid, 3 STEM
  написания, `\]`-escape, mid-word match, span-wrap); doc-комментарии `run_pipeline` обновлены.

### Верификация (airtight, чистый flip)
- clippy --workspace 0; cargo test --workspace зелёное; subst 20 тестов (+1 leaf).
- **blast_toggle (гейт): 343→343, 0 ИЗМЕНЁННЫХ файлов** (airtight-инвариант: вывод ≡ legacy).
- **FORCE (blast_force, base чистый master): Identical 312→313.** **FLIP stem.adoc 5→0** (байт-в-байт),
  **0 REGR, 0 FARTHER.** 0 паник на 344. STEM-проба `stem:[x^2+y^2]`/`latexmath:[\sqrt{a}]` = байт-в-байт
  asciidoctor. icon-macro.adoc НЕ флипнул: пред-существующее РЕНДЕРЕР-расхождение (font `<i class="fa">`
  vs текстовый `[heart]` при отсутствии `:icons: font` у эталона) — есть и в base, к subst НЕ относится;
  события icon ≡ legacy (unit-тест + 0 REGR).

### Дальше (ОСТАЛОСЬ Фаза 2)
- **macros (5/N+)**: UI kbd|btn|menu (нужен проброс `InlineOptions.experimental` через pipeline — см.
  выше), anchor (`[[id]]`/`[[[bib]]]` 2629/2671), index-term (`((…))`/`indexterm:`/`indexterm2:` 2772),
  footnote (STATEFUL — реестр/нумерация/список = отд. сессия, донор 1954). escape `\macro` (порт
  `inline_macro_escape_len` в escape.rs — дешёвый FORCE-win), escape маркеров doubled/`\\MM` (отложено
  с 8/N). specialchars = NO-OP. ФИНАЛ: снять gate → flip outline при 343 неизменных.

---

## Сессия (2026-06-15, 80-я) — РЕРАЙТ inline, Фаза 2 (11/N): macros (3/N) — inline image

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-macros-image`** (off master `3739f30`, 343) — **НЕ
закоммичена, НЕ смержена, ОЖИДАЕТ авторизации** на commit + `git merge --no-ff` в master + `git push` +
удаление ветки. link (10/N) к началу сессии УЖЕ смержена (`3739f30`). base-бинарь `/tmp/adoc_base`
ПЕРЕСОБРАН из master HEAD `3739f30` (legacy-эталон; blast_toggle подтвердил base 343).

### Выбор задачи (data-driven, FORCE near-miss)
По плану 79-й следующий пункт — **macros (3/N+) image/footnote/icon/UI/stem/anchor/index-term**. FORCE
near-miss (base, 33 non-identical): кандидаты image.adoc (100 diff), footnote.adoc (283), id.adoc (115),
stem.adoc (5). Взял **inline image**: 100 diff = ЧИСТО литеральный макрос (diffone: `image:play.png[]`
оставался текстом), самодостаточный leaf, идеально ложится на `TagToken::Macro`. footnote ОТЛОЖЕН
(stateful — реестр/нумерация/ref-def/список внизу = отдельная сессия, выше риск).

### Архитектура (image = простейший leaf-макрос)
- **НЕТ label re-parse** (в отличие от xref/link): alt/width/height/align/float/link/role/title —
  СТРОКОВЫЕ поля `Tag::InlineImage`, не события. `Start(InlineImage)`+`End` строятся напрямую.
- **Триггер `i` + guard `!src[i..].starts_with("image::")`** — зеркало dispatch'а легаси (`image::` =
  блочный образ, инлайн-парсер оставляет литералом). `image:` и `irc://` (autolink) оба байт-`i`, но
  префиксы непересекающиеся → порядок ветвей нерелевантен.
- **target БЕЗ empty-guard** — донор `try_inline_image` его не имеет (`image:[alt]` матчится), зеркалю точно.
- span-guard declined при сентинеле (`image:x[+raw+]` — passthrough в attrs). Tag-поля = `Cow::Owned`
  (== Cow::Borrowed легаси по PartialEq → gate adopts).

### Сделано (1 логический коммит, 2 файла)
- **macros.rs**: импорт `parse_image_attrs`; ветка `image:` в `extract` (с `image::`-guard, перед `<<`);
  `try_image(src,start)` (зеркало `try_inline_image`); хелпер `owned(&str)->Cow<'static>` для опц. полей.
  Doc-комментарий модуля: image добавлен в (3/N), убран из «remaining»; `extract`-doc обобщён.
- **mod.rs**: +тест `reproduces_legacy_on_image_inputs` (19 кейсов).

### Верификация (airtight, чистый flip)
- clippy --workspace 0; cargo test --workspace зелёное (parser 540→541, html 433, render-core 15,
  parsing-lab 233/233); subst 19 тестов (+1 image).
- **blast_toggle (гейт): 343→343, 0 ИЗМЕНЁННЫХ файлов** (airtight).
- **FORCE (blast_force, base `3739f30`-legacy): Identical 311→312.** **FLIP image.adoc 100→0**
  (байт-в-байт с asciidoctor), closer id.adoc 115→113, **0 REGR, 0 FARTHER.** force_nearmiss 33→32.

### Дальше (ОСТАЛОСЬ Фаза 2)
1. **macros (4/N+)** — footnote (stateful! реестр+нумерация+список, донор try_footnote_macro 1954)/
   icon (1830)/UI kbd|btn|menu (1722/1745/1806, за `:experimental:`)/stem (1854)/anchor `[[id]]`/`[[[bib]]]`
   (try_anchor 2671, try_bibliography_anchor 2629)/index-term `((…))`/`indexterm:` (try_index_term 2772).
   Reuse `TagToken::Macro` + (для label-несущих) label-reparse.
2. **escape `\macro`** (`\xref:`/`\link:`/`\image:`/…) — порт `inline_macro_escape_len` (inline.rs 1174)
   в escape.rs: drop `\`, Literal(macro-text). Дешёвый FORCE-win (escaped = литерал, impl не нужен).
3. **escape маркеров+`\+` ВНУТРИ пассов** (отложено с 8/N — doubled-формы, `\\MM`).
4. **ФИНАЛ Фазы 2:** снять gate (или per-construct) → flip outline (cross-span @4545) при 343.
- Скрипты `/mnt/c/tmp/adoc-test/`: `blast_toggle.py` (гейт), `blast_force.py` (FORCE), `diffone.py <file>
  <limit>` (FORCE-дифф под `ADOC_QUOTES_SEQUENTIAL=1 ADOC_SUBST_FORCE=1`), `/tmp/force_nearmiss.py`.

---

## Сессия (2026-06-15, 79-я) — РЕРАЙТ inline, Фаза 2 (10/N): macros (2/N) — link-семейство

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-macros-links`** (off master `4a69fc7`, 343) — **НЕ
закоммичена, НЕ смержена, ОЖИДАЕТ авторизации** на commit + `git merge --no-ff` в master + `git push` +
удаление ветки. cross-ref (9/N) к началу сессии УЖЕ смержена (`4a69fc7`). base-бинарь `/tmp/adoc_base` =
legacy-эталон (legacy неизменен → валиден без пересборки; blast_toggle подтвердил base 343).

### Выбор задачи (по плану 78-й)
Следующий пункт плана — **macros (2/N) link/url/mailto/autolink/email**. Это связный «link»-срез: строит
поверх инфры cross-ref (`TagToken::Macro` + label-reparse), флипает nav/URL-кластеры по всему корпусу.

### Сделано (1 логический коммит, 3 точки + тест)
- **inline.rs**: `url_encode_into` → `pub(crate)` (reuse для mailto query-encode).
- **macros.rs**: в `extract` добавлены триггеры `l`(link:)/`m`(mailto:)/`h`/`f`/`i`(scheme)/`@`(email);
  failed-макрос advance 1 байт. Новые функции (зеркала legacy-доноров):
  - `try_link` (зеркало `try_link_macro` 2059, plain-форма; `++url++` отложена через span-guard),
  - `try_mailto` (зеркало `try_mailto_macro` 2154; `?subject=&body=` через `url_encode_into`),
  - `scheme_at`/`at_autolink_boundary` (по ПРЕДЫДУЩЕМУ байту: whitespace/`<>()[];`/start; extracted-конструкт
    оставляет TAG_TAIL = non-boundary, совпадает с legacy-видом позиции),
  - `try_autolink` (зеркало `try_autolink` 2480; trailing-punct strip только для bare, `[label]` форма),
  - `try_email` (зеркало `try_email_autolink` 2556; backward-scan local part в `src`, стоп на non-local
    байте — TAG_LEAD/TAG_TAIL естественно ограничивают; возвращает `local_start` → caller делает
    `out.truncate(out.len()-(i-local_start))` перед splice, т.к. local part уже скопирована в `out`),
  - `build_link`/`push_label` (общие; `push_label` пуст для "" — зеркало `push_macro_label("")`).
  - `parse_link_attrs` (reuse из attributes.rs): `^`-blank-window/role/window/nofollow/subject/body.
- **mod.rs**: doc run_pipeline + macros-комментарий обновлены; +тест `reproduces_legacy_on_link_inputs`.

### Ключевые инварианты (для будущих macro-сессий)
- **email truncate безопасен:** local-part байты `[A-Za-z0-9._+-]` НЕ содержат `:`/`/`/`<` → НИ ОДИН макрос
  не сработал внутри local-run в этом проходе → `out`-хвост байт-в-байт == `src[local_start..i]`. Сентинели
  прошлых пассов (0x01/0x02) НЕ local-part → backward-scan стопает до них (== legacy `text_start`).
- **Tag::Link поля = Cow::Owned** (== Cow::Borrowed легаси по PartialEq → gate ОК), как у cross-ref.
- **`++url++` форма link declined:** к macros-time passthrough уже сентинель → span_has_sentinel → None →
  gate fb на legacy (который её рендерит). Дешёвый отложенный кейс, не баг.

### Верификация (airtight, огромный FORCE-прирост)
- clippy --workspace 0; cargo test --workspace зелёное (parser 539→540, html 433, render-core 15,
  parsing-lab 233/233); subst 18 тестов (+1 link, 40 кейсов).
- **blast_toggle (гейт): 343→343, 0 ИЗМЕНЁННЫХ файлов** (airtight).
- **FORCE (blast_force, base legacy): Identical 111→311 (+200!).** 200 FLIP, 21 closer, **4 FARTHER, 0 REGR**.
  Link даёт +57 поверх cross-ref baseline (254→311). FARTHER: 3 файла (page-breaks/attribute-terms/
  span-cells) — **0 моих триггеров** (каскад cross-ref `` `<<<` ``/отложенного, мой код не трогал) +
  pass-macro.adoc (отложенный `link:++url++[]`). force_nearmiss 90→33.

### Дальше (ОСТАЛОСЬ Фаза 2)
1. **macros (3/N+)** — image/footnote/icon/UI(kbd/btn/menu)/stem/anchor(`[[id]]`/`[[[bib]]]`)/index-term(`((…))`).
   Доноры: try_inline_image 2276, try_footnote_macro 1954, try_icon_macro 1830, try_stem_macro 1854,
   try_kbd/btn/menu 1722/1745/1806, try_anchor 2671, try_bibliography_anchor 2629, try_index_term 2772.
2. **link `++url++` форма** (passthrough-in-URL) — спец-обработка, если понадобится флип pass-macro.adoc.
3. **escape `\macro`** (`\xref:`/`\link:`/…) — порт `inline_macro_escape_len` (inline.rs 1174) в escape.rs.
4. **escape маркеров+`\+` ВНУТРИ пассов** (отложено с 8/N — doubled-формы, `\\MM`).
5. **ФИНАЛ Фазы 2:** снять gate (или per-construct) → flip outline (cross-span @4545) при 343.
- Скрипты `/mnt/c/tmp/adoc-test/`: `blast_toggle.py` (гейт), `blast_force.py` (FORCE), `diffone.py <file>
  <limit>` (FORCE-дифф под `ADOC_QUOTES_SEQUENTIAL=1 ADOC_SUBST_FORCE=1`), `/tmp/force_nearmiss.py`.

---

## Сессия (2026-06-15, 78-я) — РЕРАЙТ inline, Фаза 2 (9/N): macros (1/N) — cross-reference (`xref:` + `<<>>`)

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-macros`** (off master `713d62b`, 343) — **НЕ
закоммичена, НЕ смержена, ОЖИДАЕТ авторизации** на commit + `git merge --no-ff` в master + `git push` +
удаление ветки. base-бинарь `/tmp/adoc_base` ПЕРЕСОБРАН из master HEAD `713d62b` (включает 8/N
marker-escape).

### Выбор задачи (data-driven, FORCE-карта)
По плану 77-й следующий крупный пункт — **macros** (САМОЕ большое, multi-session). FORCE near-miss (base):
233 non-identical, доминирующий **9-diff кластер ≈13 nav-файлов = чистый `xref:target[]`/`[label]`**
(движок оставлял макрос литералом). Взял **cross-reference (xref + `<<>>`)** как первый срез macros:
строит ВСЮ инфраструктуру (leaf-токен + extract-пасс + recursive label-reparse + ordering), флипает
кластер, самодостаточен. link/url/image/footnote/icon/UI/stem/anchor/autolink/email/index-term — отдельными
сессиями (reuse инфры).

### Архитектурная модель (КЛЮЧ для будущих macro-сессий)
- **Порядок пайплайна:** passthrough → escape → char_refs → **macros** → attributes → quotes →
  replacements → post_replacements. **macros ПЕРЕД attributes** (вопреки asciidoctor macros-после-attrs):
  легаси потребляет макрос ЦЕЛИКОМ, поэтому `{anchor}` в target макроса (`xref:{anchor}[]`) остаётся
  литералом в target, НЕ становится отдельным AttrRef. attributes-первым извлёк бы `{anchor}` в сентинель
  → span макроса понёс бы его → declined. macros ПОСЛЕ passthrough/escape (чтобы `+xref+`/`\xref:`
  были уже нейтрализованы/защищены).
- **Leaf-токен `TagToken::Macro(Vec<Event<'static>>)`** (tokenize.rs): держит Start + label-события + End
  как ОДНУ owned-последовательность, в tokenize разворачивается (flush_pending + push клонов). Опрозрачен
  всем поздним пассам — макрос АТОМАРЕН, НЕ участвует в cross-span overlap (в отличие от Open/Close span).
  `macro_sentinel(events)` регистратор. (`Event<'static>`→`Event<'a>` ковариантно на push.)
- **Label re-parse = зеркало `push_macro_label`:** explicit label переразбирается через
  `super::run_pipeline(l, subs.without(MACROS))` (MACROS off → вложенный макрос литерал, рекурсия конечна).
  Пустой explicit label (`<<a,>>`) → НЕТ событий (как `push_macro_label("")=[]`, guard `!l.is_empty()`);
  no-label форма (`xref:x[]`/`<<x>>`) → `Text(target)` (легаси None-ветка, ровно тот Text, что нужен
  рендереру для auto-swap unlabeled-xref placeholder).
- **Sentinel-free span guard:** если span макроса несёт сентинель (passthrough/escape/char-ref уже извлекли
  оттуда RAW-текст — `xref:x[+raw+]`) → declined (gate fallback). Обычный случай (plain target, text/quote
  label) сентинелей не несёт.
- **Tag::CrossReference{target,label}:** рендерер юзает label ТОЛЬКО как `is_none()` (был ли explicit), текст
  берёт из событий. target/label полей = Cow::Owned (== Cow::Borrowed легаси по PartialEq, gate ОК).

### Сделано (1 логический коммит, 3 точки + тест)
- **tokenize.rs**: `TagToken::Macro(Vec<Event<'static>>)` + арм в tokenize + `macro_sentinel`.
- **macros.rs** (НОВЫЙ ~210 строк): `extract(work, subs)` скан L→R skip-сентинели; `try_xref` (зеркало
  `try_xref_macro`: find `[`/`]`, `end<=start` reject, target non-empty), `try_cross_ref` (зеркало
  `try_cross_reference`: find `>>`, comma split+trim, `#`-strip, content non-empty), `build_cross_reference`,
  `span_has_sentinel`. Failed-макрос → advance past 'x'/'<' (легаси `pos+=1`).
- **mod.rs**: `mod macros;`, вызов после char_refs ДО attributes гейт MACROS; doc run_pipeline обновлён;
  +тест `reproduces_legacy_on_cross_reference_inputs` (28 кейсов: empty/explicit label, antora target,
  `#frag`, attr в label/target, span вокруг макроса, `<<>>` bare/labelled/`#`-strip/trim, invalid формы,
  mid-word `prefixref:`).

### Верификация (airtight по гейту, огромный FORCE-прирост)
- clippy --workspace 0; cargo test --workspace зелёное (parser 538→539, html 433, render-core 15);
  parsing-lab 233/233. subst 17 тестов (+1 cross-reference).
- **blast toggle-on (гейт): 343→343, 0 ИЗМЕНЁННЫХ файлов** (airtight — gate адаптирует совпадающий xref;
  nav-кластер УЖЕ был identical под base, легаси xref корректен).
- **FORCE (blast_force, base `713d62b`): Identical 111→254 (+143!).** 143 FLIP, 37 closer, **10 FARTHER,
  0 REGR** (airtight-инвариант держится). xref/`<<>>` пронизывают ВЕСЬ корпус → +143, не только 13 nav.
  FARTHER (+1…+15) — каскад отложенной фичи: спот-чек faq.adoc — диффы от URL-макроса
  (`https://…[label]`, НЕ поддержан), сам xref верен. force_nearmiss 233→90 non-identical.

### Дальше (ОСТАЛОСЬ Фаза 2)
1. **macros (2/N) — link/url/mailto:** `link:url[attrs]` (+`++url++` форма), URL-автолинк `https://…[label]`
   (держит 9-diff quote.adoc + FARTHER-кластер faq/index/_requests), bare-автолинк http/https/ftp/irc,
   email-автолинк, mailto query-encode. Донор `try_link_macro` 2059, `try_mailto_macro` 2154, `try_autolink`
   2480, `try_email_autolink` 2556, `parse_link_attrs`. Reuse `TagToken::Macro` + label-reparse.
2. **macros (3/N+) — image/footnote/icon/UI(kbd/btn/menu)/stem/anchor(`[[id]]`/`[[[bib]]]`)/index-term(`((…))`).**
   Доноры: try_inline_image 2276, try_footnote_macro 1954, try_icon_macro 1830, try_stem_macro 1854,
   try_kbd/btn/menu 1722/1745/1806, try_anchor 2671, try_bibliography_anchor 2629, try_index_term 2772.
3. **escape `\macro`** (`\xref:`/`\link:`/…) — порт `inline_macro_escape_len` (inline.rs 1174) в escape.rs:
   drop `\`, Literal(macro-text); guard span без сентинеля. Дешёвый FORCE-win (escaped форма = литерал,
   impl макроса не нужен). СЕЙЧАС `\xref:` диверг → gate fallback (безвреден).
4. **escape маркеров+`\+` ВНУТРИ пассов** (отложено с 8/N — doubled-формы, `\\MM`).
5. **ФИНАЛ Фазы 2:** снять gate (или per-construct) → flip outline (cross-span @4545) при 343.
- Скрипты `/mnt/c/tmp/adoc-test/`: `blast_toggle.py` (гейт), `blast_force.py` (FORCE base-vs-new обе под
  env), `diffone.py <file> <limit>` (FORCE-дифф), `/tmp/force_nearmiss.py`. base пересобирать из master HEAD.

---

## Сессия (2026-06-15, 77-я) — РЕРАЙТ inline, Фаза 2 (8/N): escape маркеров + `\+` span-aware (ВНУТРИ пассов)

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-marker-escape-v2`** (off master `18aaacf`, 343)
— **СМЕРЖЕНА `--no-ff` в master + ЗАПУШЕНА (master `8db6fcc`), ветка удалена** (по авторизации «merge
and push»). Коммиты `f143140` (код+docs) + merge `8db6fcc`. base-бинарь `/tmp/adoc_base` ПЕРЕСОБРАН из
master HEAD `18aaacf` (включает 7/N char-refs).

### Выбор задачи (по плану 76-й, НЕ data-driven пивот)
FORCE near-miss: 1-diff subs-symbol-repl = `{empty}--{empty}` (не escape, отложен); большой кластер
**9-diff = macros** (~13 nav: xref/link). Marker-escape **без near-miss** (глубокий корень в мульти-root
файлах). Macros — отдельная многосессионная работа (RAW-подстроки label, leaf `Vec<Event>`). Взял
**marker-escape** как пункт №1 плана: самодостаточный, ФУНДАМЕНТАЛЬНЫЙ для финала (без него снятие гейта
даст регрессии на `\*`/`\_`/…), верифицируется как airtight через gate + FORCE.

### Семантика asciidoctor (пробы /tmp/esc_probe*.adoc, ГЛАВНОЕ — цель рерайта = asciidoctor, НЕ legacy)
Единое правило для ВСЕХ маркеров (`* _ ` # ^ ~`) и `\+`: `\`+marker роняет backslash **ТОЛЬКО если
образуется валидный спан/passthrough на этой позиции** (drop → литеральные маркеры, контент проходит
ОСТАЛЬНЫЕ пассы: `\*_em_*`→`*<em>em</em>*`, `\+*b*+`→`+<strong>b</strong>+`), иначе **сохраняет**
`\marker`. `open_boundary` удовлетворяется самим `\` (работает `word\*bold*`→`word*bold*`). **Legacy
ОШИБОЧНО всегда роняет `#`/`^`/`~`/`\+`** (`\# no mark`→`# no mark`) — новый движок матчит asciidoctor
(`\# no mark`). Для `\+`: drop зависит от валидности single-plus (close-rule): `x\+y+ z`→`x+y+ z` (drop,
close перед пробелом), `a\+b+c`→`a\+b+c` (keep, close перед word).

### Сделано (1 логический коммит, 3 точки + docs)
- **quotes.rs**: хелперы `constrained_open_close`/`simple_pair_open_close` (детект открытия БЕЗ
  сентинелей); escape-ветки в `pass_constrained` (`* _ ` #`) и `pass_simple_pair` (`^ ~`) — при `\`+marker
  (single, не doubled, `bytes[i-1]!='\\'`): валиден спан → emit marker+raw-content+marker, i=close+1;
  иначе → emit `\`+marker, i+=2. Bare-ветки отрефакторены на те же хелперы (events байт-в-байт).
- **passthrough.rs**: `\+…+` (валидный single-plus) → drop `\`, emit `+` литералом, i+=2 (контент/
  закрывающий `+` идут через нормальные пассы). Guard `bytes[i+2]!='+'`, `bytes[i-1]!='\\'`.
- **escape.rs/mod.rs/quotes.rs**: docs (маркеры/`\+` теперь обрабатываются в своих span-aware пассах,
  больше не «deferred здесь»; escape.rs else-ветка по-прежнему держит `\marker` ДО quotes-пасса).

### Почему ВНУТРИ пассов, а не escape-first (ключ)
`\` внутри уже-открытого спана (`` `\` `` — контент `\`) — это контент, НЕ escape. escape-first спрятал
бы закрывающий маркер → рвал спан (`` (`\`) and (`]`) ``, keyboard-macro +18 в 75-й). Span-aware: пасс
СНАЧАЛА открывает спан (i прыгает за close), `\` внутри content так и не рассматривается как escape.
escape.rs (flat, ПЕРЕД quotes) оставляет `\marker` нетронутым (else-ветка) → quote-пасс решает.

### КОАЛЕСЦИРУЮЩЕЕ различие (важно для понимания гейта)
`\`+marker даёт RAW-байты в буфере → КОАЛЕСЦИРУЮТ в ОДИН Text при tokenize. Legacy флашит текст вокруг
escape порознь (`word\*bold*`: legacy `[Text("word"),Text("*bold*")]`, движок `[Text("word*bold*")]`).
**HTML идентичен**, события ≠ → gate (сравнивает события) отклоняет → fallback на legacy (безвреден, тот
же HTML). Поэтому gate adoption растёт, но blast_toggle=343→343 (HTML не меняется). Событийно-РАВНЫ
legacy только escape-в-начале-рана простые кейсы (`\*bold*`, `\*_em_*`, `` `\*bold*` ``, `\+x+`) — они в
`reproduces_legacy_on_marker_escape_inputs`; остальные (keeps, word-prefix) — в `marker_escape_matches_asciidoctor`.

### Верификация (airtight по гейту)
- clippy --workspace 0; cargo test --workspace зелёное (parser 536→538, html 433, render-core 15);
  parsing-lab 233/233. subst: 16→18 тестов (+2 marker-escape; renamed `escape_marker_left_untouched`→
  `marker_escape_does_not_tear_spans`).
- **blast toggle-on (гейт): 343→343, 0 ИЗМЕНЁННЫХ файлов** (airtight, нулевая регрессия корпуса — НЕ
  может регрессировать: valid-span `\`+marker сейчас даёт `\<strong>`≠legacy → не adopted; правлю только
  эти → output тот же, что fallback давал).
- **FORCE (blast_force, prev-new=base+env vs cur-new=мой+env): Identical 111→111, 0 flips** (ожидаемо,
  без near-miss). **subs.adoc 122→87 closer −35** (`\*Stars*`→`*Stars*` реальный фикс). **span-cells
  271→274 (+3) — АРТЕФАКТ позиционного ndiffs**: единственная контент-правка (строка 18 `` (`\+`) ``)
  ТЕПЕРЬ даёт `<code>+</code>` == asciidoctor (изолированно байт-в-байт; base-FORCE был сломан
  `<code>\`)...`), но +4 токена в файле, рассинхронизированном неподдержанным `[[id]]`-anchor (269→273
  токенов), сдвигают позиционную метрику. НЕ контент-регрессия (diff base-FORCE↔cur-FORCE = ровно 1 строка, в плюс).

### Дальше (ОСТАЛОСЬ Фаза 2)
1. **macros** — САМОЕ большое (link/xref/image/footnote/icon/kbd/btn/menu/stem/anchor/autolink/email +
   `[[id]]` + `((…))`; leaf-токен с произвольным `Vec<Event>`; держит 9-diff кластер ~13 nav-файлов +
   outline cross-span). Донор `handle_inline_macro` inline.rs ~416-680. Анализ в 74-й (RAW-подстроки
   label, recursive sub-pipeline label-subs с MACROS off).
2. **ФИНАЛ Фазы 2:** снять gate (или per-construct) → flip outline (cross-span @4545) при 343.
- **ОТЛОЖЕНО (после macros или с ним):** doubled-формы (`\**`/`\##`/`\++`/`\+++`), `\\MM` double-backslash,
  пре-существующий `a\*b*c` (asciidoctor роняет, движок сохраняет — close-assertion subtlety).
- Скрипты в `/mnt/c/tmp/adoc-test/`: `blast_toggle.py` (гейт), `blast_force.py` (FORCE — env наследуется
  обоими бинарями subprocess'ом, => сравнивает prev-new vs cur-new, идеальная изоляция правки),
  `/tmp/force_nearmiss.py`. base пересобирать из master HEAD в начале сессии (`cp target/release/adoc /tmp/adoc_base`).

---

## Сессия (2026-06-15, 76-я) — РЕРАЙТ inline, Фаза 2 (7/N): char-refs survival + escape `\&#…;` за гейтом

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-marker-escape`** (off master `3fdb828`, 343)
— имя ветки историческое (завёл под marker-escape, пивотнул на char-refs). **НЕ закоммичена, НЕ
смержена, ОЖИДАЕТ авторизации** на commit + `git merge --no-ff` в master + `git push` + удаление
ветки. base-бинарь `/tmp/adoc_base` ПЕРЕСОБРАН из master HEAD `3fdb828` (включает 6/N non-marker
escape; прежний был от `5421e0e`).

### Data-driven пивот: marker-escape → char-refs
План 75-й сессии ставил следующим **escape маркеров** (`\*`/`\_`/`` \` ``/`\#`/`\^`/`\~` + `\+`
span-aware). Но FORCE-карта (blast_force, base 108) показала: ВСЕ near-miss 1-3 diff — это **char-refs**,
НЕ marker-escape (у marker-escape нет near-miss — это глубокий корень в мульти-root файлах). diffone:
ui.adoc (1: `&#8942;`), toc-ref.adoc (1: `&#8211;`), title-links.adoc (2: `&#167;`) — все в составе
343 (base diff=0, legacy их обрабатывает, мой движок — нет); subs-symbol-repl (3: `\&#8201;` escape +
`{empty}--{empty}`). Пивотнул на char-refs (зеркало пивота 74-й сессии macros→curved-quotes): контейнерный,
3 чистых флипа, низкий риск. Marker-escape остаётся следующим (см. «Дальше»).

### Ключевое открытие (направление char-ref)
`apply_typographic_replacements` (донор replacements-пасса) выдаёт **литеральные** Unicode-символы
(`\u{2019}`/`\u{2014}`/…), НЕ entity. Значит char-refs ИЗ ИСХОДНИКА (`&#167;`) — отдельная проблема:
legacy эмитит их как `InlinePassthrough` (рендерер НЕ экранирует `&`), мой движок оставлял в Text
(рендерер → `&amp;#167;`). Два случая, ОБА — отдельные события (legacy флашит, НЕ коалесцирует как
`Literal`): **survival** (`&#167;`→InlinePassthrough, raw) и **escape** (`\&#167;`→Text, drop `\`,
рендерер экранирует — зеркало legacy arm ~975).

### Сделано (1 логический коммит)
- **tokenize.rs**: `TagToken::CharRef { text, raw }` (raw=true→InlinePassthrough, raw=false→Text);
  флашит pending (отдельное событие, не коалесцирует). `Work::char_ref_sentinel(text, raw)`.
- **subst/char_refs.rs** (НОВЫЙ): `run(work)` — скан буфера, скип сентинелей, валидный `&…;`
  (`char_ref_len`, порт `char_ref_len_at` из inline.rs: named/decimal/hex) → `CharRef{raw=true}`.
- **escape.rs**: арм `\&#…;` (m==`&` && char_ref_len(i+1)>0) → drop `\`, `CharRef{raw=false}` —
  отдельный Text-event (рендерер экранирует `&`), И запечатывает `&` от survival-пасса. Импорт
  `char_ref_len` из `char_refs`.
- **mod.rs**: `mod char_refs;`, вызов `char_refs::run` ПОСЛЕ escape, ДО attributes, гейт
  `SPECIALCHARS && REPLACEMENTS` (= legacy `preserve_char_refs`). +тест
  `reproduces_legacy_on_char_ref_inputs` (survival named/dec/hex, invalid `&`, в `*`/`` ` ``/`_`-спанах,
  escape, beside replacements).

### Архитектурное решение: char-refs ДО quotes
Извлекать char-refs ПЕРЕД quote-пассами обязательно: `#` внутри `&#167;` (десятичный/hex) иначе был
бы взят mark-пассом за маркер. Legacy потребляет ref АТОМАРНО до рассмотрения маркера на той позиции.
**Известное расхождение (НЕ в тестах, гейт ловит fallback'ом):** патологический `#&#167;#` — legacy
mark хватает ВНУТРЕННИЙ `#` (`<mark>&</mark>167;#`), мой движок (extract-first) даёт `<mark>&#167;</mark>`.
Редко; `&` ДО quotes лучше, чем после (после — расходился бы на частом `&#167; #x#`). `*`/`` ` ``/`_`/`"`-спаны
безопасны (их маркеры не совпадают с `#` в ref).

### Верификация (всё зелёное, airtight)
- clippy --workspace 0; cargo test --workspace зелёное (parser 535→536, html 433, render-core 15);
  parsing-lab 233/233 (+1 subst-тест, 16 subst).
- **blast toggle-on (гейт): 343→343**, 0 регрессий, 0 flips (airtight по построению).
- **FORCE (blast_force): 108→111** raw-идентичных. 3 FLIP (title-links 2→0, ui 1→0, toc-ref 1→0),
  2 closer (subs-symbol-repl 3→1, document-attributes-ref 6434→6433), **0 FARTHER, 0 REGR** (airtight).
  Спот-чек: `*§ \&#167;*`/`` `&#x2026;` `` — new==legacy==asciidoctor байт-в-байт.
- Остаток subs-symbol-repl @125 = `{empty}--{empty}`→`—` (deferred attribute-resolution + replacements,
  НЕ char-ref, pre-existing).

### Дальше (ОСТАЛОСЬ Фаза 2)
1. **escape маркеров+`\+` ВНУТРИ пассов** (`\\?` в quote/passthrough-пассах, span-aware — модель
   asciidoctor; даст `\*bold*`→`*bold*`, `\+x+`→`+x+` БЕЗ промахов `` (`\`) ``). Донор
   `handle_inline_escape` inline.rs arms 1010 (KEEP `\` для `*_` `` ` `` без закрытия) + 1025 (DROP).
   Без near-miss флипов (глубокий корень), но нужен для финала. Семантика single vs double backslash
   (`\\*` defer), span-awareness — детали в анализе ниже по файлу (75-я).
2. **macros** — САМОЕ большое (link/xref/image/footnote/icon/kbd/btn/menu/stem/anchor/autolink/email +
   `[[id]]` + `((…))`; leaf с произвольным `Vec<Event>`; держит outline cross-span + 9-diff кластер
   ~13 nav-файлов). Донор `handle_inline_macro` inline.rs 416-680. Анализ в 74-й.
3. **ФИНАЛ Фазы 2:** снять gate (или per-construct) → flip outline (cross-span @4545) при 343.
- Скрипты в `/mnt/c/tmp/adoc-test/`: `blast.py` (toggle-off), `blast_toggle.py` (гейт), `blast_force.py`
  (FORCE), `diffone.py <file> <limit>` (FORCE-дифф с `ADOC_QUOTES_SEQUENTIAL=1 ADOC_SUBST_FORCE=1`),
  `/tmp/force_nearmiss.py` (список non-identical FORCE по возрастанию diff). base пересобирать из
  master HEAD в начале сессии. **9-diff кластер = macros** (xref/link не обрабатываются).

---

## Сессия (2026-06-15, 75-я) — РЕРАЙТ inline, Фаза 2 (6/N): escape `\` пасс (не-маркерный) за гейтом

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-escape`** (off master `5421e0e`, 343) —
**НЕ закоммичена, НЕ смержена, ОЖИДАЕТ авторизации** на commit + `git merge --no-ff` в master +
`git push` + удаление ветки. base-бинарь `/tmp/adoc_base` собран из master HEAD `5421e0e` (343).

### Контекст
Фаза 0 (toggle) + Фаза 1 (quotes) + Фаза 2 (1-5/N: replacements/post_replacements/passthrough/
attributes/curved-quotes) — в master. Цель Фазы 2: перенести остальные пассы, в финале снять gate →
flip outline. См. [[proj_sequential_quotes_rewrite]]. Следующий по плану 74-й сессии — escape
(разблокирует 1-diff unresolved-references + escaped smart-quote).

### Сделано (1 логический коммит, escape — НЕ-МАРКЕРНЫЙ)
- **tokenize.rs**: `TagToken::Literal(String)` + `Work::literal_sentinel`. Токенизатор переделан на
  коалесцирующий `pending`-буфер: `Literal` ФЛАШИТ предыдущий ран и СИДИТ pending своим текстом → escaped
  char МЕРЖИТСЯ со следующим раном в ОДИН Text (зеркалит legacy: дроп backslash, char в next flush).
  `flush_pending` хелпер; все НЕ-Literal токены флашат pending перед эмитом (поведение прежних токенов
  сохранено байт-в-байт). SmartQuote ОСТАЁТСЯ отдельным Text (флашит, не коалесцирует) — 3 события на
  `"`…`"` сохранены.
- **subst/escape.rs** (НОВЫЙ): `run(work)` — дроп backslash + `Literal`-сентинел для НЕ-маркерных escape:
  типографика (`\--`/`\->`/`\=>`/`\<-`/`\<=`/`\...`/`\(C)`/`\(R)`/`\(TM)`, порт `typographic_escape_len`),
  smart-quote openers (`\"`` ``/`\'`` ``), `\{`/`\[`/`\<`/`\'`. `\\` (двойной) и trailing `\` — оставлены.
- **mod.rs**: `escape::run` ПОСЛЕ passthrough, ДО attributes. +2 теста (`reproduces_legacy_on_escape_inputs`
  15 кейсов, `escape_marker_left_untouched` 2 кейса).

### ДВА контекстных бага escape-first, найденных blast'ом, и итоговое решение
escape-first (до passthrough) дал ДВА бага (FARTHER в FORCE):
1. **Маркеры** (`\*`/`\_`/`` \` ``/`\#`/`\^`/`\~`): `\` ВНУТРИ открытого span — это контент, не escape.
   `` (`\`) `` — escape-first прятал ЗАКРЫВАЮЩИЙ backtick monospace в Literal → span рвался
   (`<code>` `` ` ``)...`). keyboard-macro 16→34, outline +18, replacements +3. **Решение: НЕ обрабатывать
   маркеры в escape** — их `\\?` принадлежит ВНУТРЬ quote-пассов (span-aware, модель asciidoctor),
   отдельная сессия. Оставлены untouched → gate отклоняет, FORCE-faithful (legacy тоже держит `\`).
2. **escape ВНУТРИ passthrough**: `` `+\{name}+` `` — `\{` внутри single-plus passthrough (verbatim!),
   но escape-first калечил его ДО passthrough-extract → порча сентинела (утечка цифры `0name...}`).
   reference-attributes 338→339, span-cells +3. **Решение: passthrough ПЕРВЫМ, escape ВТОРЫМ** (как
   asciidoctor: passthrough защищает контент до всех субституций). Тогда `\` в буфере всегда top-level.
   Попутно `\+` ОТЛОЖЕН (требует escape-aware passthrough `\\?`, иначе passthrough-first съест `+x+`).

### Ключевая семантика (трассировка legacy, все воспроизведены)
- `\{name}`→Text("{name}") ОДНО событие (коалесценция). `\{author} and {author}`→Text+AttrRef.
  `\"`` ``…`` `" ``→Text(`` "`…`" ``). `it\'s`→[Text("it"),Text("'s")] (apostrophe прямой, escape бьёт
  replacements). `+\{name}+`/`` `+\{name}+` ``/`pass:[\{x}]`→`\{` verbatim (passthrough защищает).
  `` `\` ``→`<code>\</code>` (маркер untouched, span формируется, `\`=контент).

### Верификация (всё зелёное)
- clippy --workspace 0; cargo test --workspace 18 ok-групп (parser 533→535, html 433, render-core 15);
  parsing-lab 233/233 (+2 subst-теста, 15 subst всего).
- **blast toggle-off 343→343** (legacy не тронут), **toggle-on 343→343** (gate airtight, 0 regr, 0 flips).
- **FORCE (blast_force.py): 107→108** raw-идентичных, **unresolved-references FLIP 1→0**, 3 closer
  (bibliography 12→11, subs 123→122, subs-symbol-repl 4→3), **0 FARTHER, 0 REGR** (airtight).
  Спот-чек: `` (`\`) and (`]`) `` и `` `+\{name-of-attribute}+` `` теперь байт-в-байт с base.

### Дальше (ОСТАЛОСЬ Фаза 2)
1. **escape маркеров+`\+` ВНУТРИ пассов** (`\\?` в quote/passthrough-пассах, span-aware — правильная
   модель asciidoctor; даст `\*bold*`→`*bold*`, `\+x+`→`+x+` БЕЗ промахов `` (`\`) ``).
2. **macros** — САМОЕ большое (link/xref/image/footnote/icon/kbd/btn/menu/stem/anchor/autolink/email +
   `[[id]]` + `((…))`; leaf-токен с произвольным `Vec<Event>`; держит outline cross-span). Донор
   `handle_inline_macro` inline.rs 416-680. Анализ в 74-й сессии (RAW-подстроки label).
3. **char-refs** (`&#167;` survival, донор `char_ref_len_at` 1122) + **char-ref escape** `\&#…;`.
4. **ФИНАЛ Фазы 2:** снять gate (или per-construct) → flip outline (cross-span @4545) при 343.
- Скрипты в каталоге корпуса `/mnt/c/tmp/adoc-test/`: `blast.py` (toggle-off), `blast_toggle.py`
  (toggle-on gate), `blast_force.py` (FORCE), `diffone.py <file> <limit>` (FORCE-дифф:
  `ADOC_QUOTES_SEQUENTIAL=1 ADOC_SUBST_FORCE=1`). base-бинарь пересобирать из master HEAD в начале сессии.

---

## Сессия (2026-06-15, 74-я) — РЕРАЙТ inline, Фаза 2 (5/N): curved smart quotes `:double`/`:single` пасс за гейтом

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-curved-quotes`** (off master `e9ce613`, 343) —
**СМЕРЖЕНА `--no-ff` в master + ЗАПУШЕНА (master `7995142`), ветка удалена** (по авторизации
«merge and push»). Коммиты `7d13f7c` (код) + `01391c2` (docs). base-бинарь `/tmp/adoc_base` ПЕРЕСОБРАН
из master HEAD `e9ce613` (343). **ВАЖНО:** Phase 2 (4/N) attributes уже СМЕРЖЕНА в master ранее (`e9ce613`).

### Решение по объёму (data-driven пивот с macros)
Изначально завёл ветку `-macros` (план: самый большой пасс), но FORCE-карта показала ДВА файла в
1-2 diff от флипа: image-position (242→2) и unresolved-references (214→2). diffone выявил корни: оба
блокируются **curved smart quotes** (`"`​`pushy`​`"`→`“pushy”`), unresolved-references ещё и escape
`\{name}`. Curved-quotes — наивысший ROI (разблокирует ОБА + массу прозы), намного меньше/безопаснее
macros. Пивотнул, ветку переименовал в `-curved-quotes`. Macros отложен (глубокие сложности с RAW-
подстроками — см. ниже), ему — отдельная сессия.

### Анализ macros (для будущей сессии — почему отложен)
Donor `handle_inline_macro` (inline.rs 416-680). Ключевая трудность: span-макросы (link/xref) можно
сделать open/close-сентинелями (label остаётся в буфере между ними — уже прошёл quotes/replacements,
что зеркалит legacy re-parse label с MACROS-disabled). НО `Tag::CrossReference{label}`/footnote-текст/
image-alt несут **RAW-подстроку** (исходный label ДО подстановок), которая к moment'у macros уже стёрта
ранними пассами (сентинели в буфере). Faithful-подход: extract макросов как РАННИЙ пасс (как passthrough/
attributes) с захватом RAW target/label + recursive-вычисление label-событий через под-пайплайн с
label-subs (MACROS off) → leaf `TagToken::Macro(Vec<Event>)`. Ordering vs passthrough/attributes extract
требует аккуратности (`link:{url}[...]` — attributes уже извлёк `{url}`). Большой, отдельная сессия.

### Сделано (1 коммит)
- **tokenize.rs**: `TagToken::SmartQuote { text: &'static str, opening: bool }` + `Work::smart_quote_sentinel`
  + arm в tokenize → `Event::Text(Cow::Borrowed(text))` (литерал-char `“`/`”`/`‘`/`’`, КАК legacy, не
  `&#8220;`-entity). Раздельный leaf-сентинель на КАЖДУЮ curly = три Text-события (open/inner/close),
  зеркалит legacy `try_smart_quotes`.
- **quotes.rs**: `pass_smart_quotes(work, quote, open_curly, close_curly)` в `run_all` ПОСЛЕ strong
  (unc+con), ДО monospace — слот asciidoctor QUOTE_SUBS. `find_smart_quote_close` (skip сентинелей,
  первый `` ` ``+quote после непустого контента). **Leading-edge подавление**: `pass_constrained`
  отказывает bare `` ` ``/`_`/`#`-open, если непосредственно перед позицией — SmartQuote-OPEN сентинель
  (`smart_quote_leading_edge(&work.tags, bytes, marker, i)` + `sentinel_index_before` — backward-скан
  TAG_TAIL→digits→TAG_LEAD). Флаг legacy `smart_quote_leading_edge` воспроизведён ПОРЯДКОМ пассов
  (strong до :double → exempt; mono/em/mark после → подавлены), не полем парсера. Нет open-boundary/
  attrlist (паритет с legacy, не широкий asciidoctor `(^|[^\w;:}])`-regexp). sup/sub (`^`/`~`) НЕ
  подавляются (legacy try_simple_pair без assertion).
- **mod.rs/quotes.rs doc**: curved quotes помечены реализованными; +тест `reproduces_legacy_on_smart_quote_inputs`.

### Ключевая семантика (трассировка legacy-тестов, все воспроизведены)
- `"`​`text`​`"` → Text(“),Text(text),Text(”). `"`​``end points``​`"` → inner literal `` `end points` ``
  (constrained mono подавлен на leading edge; :double ДО mono unc → `` `` `` не матчится первым).
  `"`​`_em_ x`​`"`/`"`​`#mk# x`​`"` → подавлены. `"`​`a `c` b`​`"` → mono ОТКРЫВАЕТСЯ (после пробела,
  не leading edge — sentinel_before=None). nested `'`​`outer "`​`inner`​`" end`​`'` (:double inner, :single
  outer). unclosed/empty → литерал. Escaped `\"`​`…`​`"` → diverge (escape отложен) → gate fallback.

### Верификация
- clippy 0, test --workspace зелёное (parser 532→533, html 433, render-core 15), parsing-lab 233/233
  (+1 subst-тест, 12 subst всего).
- **blast toggle-off 343→343** (legacy байт-в-байт не тронут), **toggle-on 343→343** (гейт, 0 регрессий,
  0 flips, airtight). base пересобран из master `e9ce613`.
- **FORCE (blast_force.py): 97 → 107** raw-идентичных, **0 REGR, 0 FARTHER**, 10 FLIP, 11 closer.
  image-position 2→0 FLIP, unresolved-references 2→1 (остаток = `\{name}` escape, отложен). Прошлые
  FARTHER (footnote/replacements) не ухудшились.

### Дальше (ОСТАЛОСЬ Фаза 2)
1. **escape** `\*`/`\_`/`` \` ``/`\{`/`\pass:`/`\"` — самый дешёвый следующий (1-diff unresolved-references
   + escaped smart-quote). Донор `handle_inline_escape` inline.rs ~821.
2. **char-refs** (`&#167;` survival — донор `char_ref_len_at` 1122, гейт specialchars&&replacements).
3. **macros** — САМОЕ большое (см. анализ выше), отдельная сессия.
4. **ФИНАЛ:** снять gate (или per-construct) → flip outline (cross-span @4545) при 343.
- Скрипты: `blast.py` (toggle-off), `blast_toggle.py` (toggle-on gate), `blast_force.py` (FORCE),
  `diffone.py <file> <limit>` (с `ADOC_QUOTES_SEQUENTIAL=1 ADOC_SUBST_FORCE=1` для FORCE-диффа).

---

## Сессия (2026-06-15, 73-я) — РЕРАЙТ inline, Фаза 2 (4/N): attributes `{name}`/`{set:}` extract пасс за гейтом

Запрос «продолжи фазу 2». Ветка **`feat/subst-phase2-attributes`** (off master `967dcd4`, 343) —
**1 коммит `c60aa27`, НЕ смержена, НЕ запушена, ОЖИДАЕТ авторизации** на `git merge --no-ff` в
master + `git push` + удаление ветки. base-бинарь `/tmp/adoc_base` собран из master HEAD `967dcd4`
(343). **ВАЖНО:** Phase 2 (3/N) passthrough уже СМЕРЖЕНА в master между сессиями (`967dcd4`).

### Контекст
Фаза 0 (toggle) + Фаза 1 (quotes) + Фаза 2 (1-3/N passthrough+replacements+post_replacements) — в master.
Фаза 2 = перенести ОСТАЛЬНЫЕ пассы (`subs=normal`: passthrough → specialchars → quotes →
**attributes** → replacements → macros → post_replacements → restore), довести FORCE до байт-идентичности,
в финале СНЯТЬ gate → flip outline. См. [[proj_sequential_quotes_rewrite]], план greedy-yawning-pumpkin.

### Анализ перед реализацией (КЛЮЧЕВОЙ вопрос 72-й сессии разрешён)
**Legacy НЕ резолвит `{name}` инлайн** — эмитит `Event::AttributeReference{name,fallback,trailing_brackets}`,
рендерер резолвит. `fallback` ВСЕГДА `None` (нет синтаксиса `{name:fallback}` — все 5 `fallback:`
в inline.rs либо `None`, либо в `mod tests`). `{set:...}` → `Event::Attribute`. Доноры: `try_attribute_reference`
(inline.rs 2320) + `try_inline_set` (2396) + `is_valid_attr_name` (2442). В диспетче `{` — в
`handle_inline_macro` (арм 555, гейт `has_attributes`), НЕ конкурирует с quotes/passthrough (разные
первые байты), всегда доходит.

### Сделано (1 коммит)
- **`subst/attributes.rs`** (НОВЫЙ): `extract(work)` сканит буфер L→R, при `{` зовёт `try_attr`
  (порт `try_attribute_reference`): валидация имени `\w[\w-]*`, захват trailing `[brackets]`/`/path[brackets]`
  (skip `[[`, skip без закрывающего `]`), `{set:...}` → `try_set` (порт `try_inline_set`: `name!`→unset
  `!name`, `name:value`, `name`). Возврат `Extracted::{Ref{name,trailing},Set{name,value}}` + end-индекс,
  caller регистрирует сентинел.
- **tokenize.rs**: `TagToken::AttrRef{name,trailing_brackets}` → `Event::AttributeReference{fallback:None}`,
  `TagToken::AttrSet{name,value}` → `Event::Attribute`; хелперы `attr_ref_sentinel`/`attr_set_sentinel`.
  (Cow::Owned vs legacy Cow::Borrowed — равны по PartialEq, как passthrough-pieces.)
- **mod.rs**: `attributes::extract` ПОСЛЕ passthrough, ДО quotes, гейт `subs.has(ATTRIBUTES)`.

### Архитектурное решение: attributes ДО quotes (вопреки порядку asciidoctor)
Asciidoctor: quotes→attributes. Но legacy захватывает trailing-bracket НА attribute-ref; если бы quotes
шёл первым, он съел бы `[.role]*x*` как attributed-strong (`{a}[.role]*x*`). Extract ДО quotes защищает
brackets (→ AttrRef(trailing=`[.role]`) + ГОЛЫЙ strong, = legacy). Граничные байты для quotes идентичны
(`{`/`}`/сентинел все non-word). Резолва нет → единственное, что важно для паритета, — воспроизвести
events legacy. (Это legacy-специфичное расхождение с asciidoctor, не для корпуса — для flip outline.)

### Баг, пойманный FORCE: UTF-8 порча (mojibake)
Первый прогон FORCE дал 4 REGR (`_foundations`/`monitoring`/`index`/`error-handling` 0→N) — diffone
показал кириллицу как Latin-1 (`Базовые`→`Ð\x91Ð°Ð·Ð¾Ð²Ñ\x8bÐµ`). Корень: fall-through copy
`out.push(bytes[i] as char)` — `byte as char` для continuation-байта ≥0x80 даёт U+0080..00FF и
перекодирует в 2 байта. Фикс: `utf8_char_len(bytes[i])` + `push_str(&src[i..i+len])` (как passthrough).
Gate это ловил (toggle-on держался 343 — порченый текст ≠ legacy → fallback), но FORCE обнажил. После
фикса 0 REGR. **Урок: любой fall-through copy в пассах ОБЯЗАН быть char-aware (utf8_char_len), не байтовым.**

### Верификация
- clippy 0, test --workspace зелёное (parser 531→532, html 433), parsing-lab 233/233 (+1 subst-тест
  `reproduces_legacy_on_attribute_inputs`, 11 subst всего; убран кейс `{a}[[anchor]]` — это macros).
- **blast toggle-off 343, toggle-on 343** (гейт держит, 0 регрессий, 0 flips, airtight).
- **FORCE (blast_force.py): 92 → 97** raw-идентичных, **0 REGR**, 5 FLIP, 12 closer, 2 FARTHER.
  FARTHER (footnote 281→283, replacements 450→494) — каскады отложенных macros/char-refs (diffone:
  footnote @13 `footnote:[]` литерал; replacements @12 `<a href=#char-ref-sidebar>` link/char-ref),
  НЕ баги attributes. Стали FARTHER т.к. `{attr}` теперь резолвится и сдвигает позиции относительно
  ещё-литерального macro — экспектед «фикс обнажает следующий слой».

### Дальше (ОСТАЛОСЬ Фаза 2)
1. **macros** — САМОЕ большое: link/xref/image/footnote/icon/kbd/btn/menu/stem/anchor/autolink/email +
   inline-anchor `[[id]]` + concealed index-term `((…))`. Нужен overhaul токенизатора: leaf-токены с
   ПРОИЗВОЛЬНЫМ `Vec<Event>` (напр. `TagToken::Macro(Vec<Event<'static>>)`), т.к. macro-события
   разнотипны. Донор `handle_inline_macro` (inline.rs 416-680). Именно macros держит 2 FORCE-FARTHER
   и outline cross-span (footnote `<<xref>>` каскад).
2. **char-refs** (`&#167;` survival — legacy эмитит InlinePassthrough при specialchars+replacements,
   inline.rs 398), **escape** `\*`/`\{`/`\pass:` (донор `handle_inline_escape` 821), **curved smart-quotes**
   `"…"`/`'…'`, **spec'd `pass:SPEC[]`**. specialchars — NO-OP (Event::Text сырой).
3. **ФИНАЛ Фазы 2:** снять gate (или per-construct) → flip outline (cross-span @4545 overlap) при 343.
- Скрипты: `blast.py` (toggle-off), `blast_toggle.py` (toggle-on gate), `blast_force.py` (FORCE),
  `diffone.py <file> <limit>` (с `ADOC_QUOTES_SEQUENTIAL=1 ADOC_SUBST_FORCE=1` для FORCE-диффа).

---

## Сессия (2026-06-15, 72-я) — РЕРАЙТ inline, Фаза 2: passthrough extract/restore пасс за гейтом

Запрос «продолжи». Ветка **`feat/subst-phase2-passthrough`** (off master `296834b`, 343) —
**1 коммит `691a208`, НЕ смержена, НЕ запушена, ОЖИДАЕТ авторизации** на `git merge --no-ff` в
master + `git push` + удаление ветки. base-бинарь `/tmp/adoc_base` собран из master HEAD `296834b`
(343). **ВАЖНО:** Phase 2 (1-2/N replacements+post_replacements) уже СМЕРЖЕНА в master между
сессиями (это и есть `296834b`).

### Контекст
Фаза 0 (toggle) + Фаза 1 (quotes-пайплайн) + Фаза 2 (1-2/N replacements+post_replacements) — в master.
Фаза 2 = перенести ОСТАЛЬНЫЕ пассы asciidoctor (`subs=normal`: **passthrough-extract** → specialchars →
quotes → attributes → replacements → macros → post_replacements → restore), довести FORCE-движок до
байт-идентичности, в финале СНЯТЬ gate → flip outline. См. [[proj_sequential_quotes_rewrite]], план
`~/.claude/plans/greedy-yawning-pumpkin.md`.

### Сделано (1 коммит, passthrough — FIRST в пайплайне, фундамент)
- **`subst/passthrough.rs`** (НОВЫЙ): `extract(work, subs)` сканит буфер L→R (первый пасс, сентинелов
  ещё нет), извлекает `+++/++/+/bare pass:[]` в `TagToken::Passthrough(Vec<PassPiece{text,raw}>)` →
  сентинел. Контент опакен для quotes/replacements/post (сентинел-байты non-word). Зеркалит legacy:
  - **try_triple_plus** → raw piece (InlinePassthrough); **try_double_plus** → !raw piece (Text,
    специалчарс-escape рендерером), `++++`→пусто (но сентинел-слот остаётся → сохраняет split текста);
  - **try_single_plus** → `single_plus_pieces` (порт push_single_plus_content: literal Text + embedded
    `pass:[]`→raw, spec'd-с-specialchars→Text); открытие constrained (prev не word, content не space,
    closing skip pass-региона);
  - **try_pass_macro** ТОЛЬКО bare (spec_len==0)→raw; spec'd ОТЛОЖЕН (re-runs subs → non-leaf events,
    не лезет в PassPiece) → None → текст остаётся → gate отклоняет.
- **tokenize.rs**: `PassPiece{text:String, raw:bool}`, `TagToken::Passthrough(Vec<PassPiece>)`,
  `Work::passthrough_sentinel`, arm в tokenize (raw→InlinePassthrough Owned, !raw→Text Owned).
- **mod.rs**: `passthrough::extract(&mut work, subs)` ПЕРВЫМ (безусловно — legacy гонит `+`/`pass:`
  независимо от флагов, движок и так только при QUOTES). run_pipeline теперь зеркалит empty-guard
  parse_legacy: пустые события → `[Text(input)]` (нужно для `++++`).
- **РЕФАКТОР inline.rs**: `pass_spec_to_subs` вынесена из `impl InlineState` в `pub(crate) fn`
  модульного уровня (нужна single_plus_pieces для SPECIALCHARS-членства; DRY против дрейфа).

### Ключевой баг, найденный blast'ом и исправленный: hard-break ` +\n`
image-ref FORCE 129→520 (FARTHER) обнажил: в legacy ` +\n` перехватывается как hard-break НА ПРОБЕЛЕ
(handle_inline_formatting/check_hard_break, гейт has_post_replacements) ДО того, как passthrough
увидит `+`. В моём ПОСЛЕДОВАТЕЛЬНОМ пайплайне passthrough идёт ПЕРВЫМ, post_replacements — ПОСЛЕДНИМ
→ single-plus жадно хватал hard-break `+` (`` `id` +\n(or `+[[x]]+`... `` → склейка). Фикс: в
try_single_plus guard `prev==space && next=='\n'` → None (оставить ` +\n` для post_replacements),
ГЕЙТНУТ на POST_REPLACEMENTS (без него legacy открывает single-plus с content=`\nfoo`). После фикса
image-ref ушёл из FARTHER, верность 91→92.

### Верификация
- clippy 0, test --workspace зелёное (parser 530→531, html 433), parsing-lab 233/233 (+1 subst-тест
  `reproduces_legacy_on_passthrough_inputs`, 10 subst-тестов всего).
- **blast toggle-off 343, toggle-on 343** (гейт держит, 0 регрессий, 0 flips, airtight).
- **FORCE (blast_force.py): 85 → 92** raw-идентичных, **0 REGR**, 7 FLIP, 31 closer, 6 FARTHER.
  FARTHER (add-columns +1, duplicate-cells, id 545→574, troubleshoot-unconstrained 490→553,
  footnote 87→176, outline 7384→7481) — ЭКСПЕКТЕД каскады отложенных macros/attr. diffone footnote:
  @125 `<<ex-footnote>>` литерал (xref отложен), НЕ баг passthrough. 0 REGR = ни один ранее-идеальный
  файл не сломан.

### Дальше (ОСТАЛОСЬ Фаза 2, по убыванию фундаментальности)
1. **attributes** `{name}` — СНАЧАЛА проверить: legacy эмитит `Event::AttributeReference` (резолв в
   рендерере) или резолвит инлайн? От этого зависит, что эмитить токенизатору. Донор: handle_inline_macro
   `{`-арм, char_ref. Скорее всего отдельный `TagToken::AttrRef(name)` → Event::AttributeReference.
2. **macros** — САМОЕ большое: link/xref/image/footnote/icon/kbd/btn/menu/stem/anchor/autolink/email,
   много типов событий → overhaul токенизатора (нужны leaf-токены с произвольными Event). Донор:
   `handle_inline_macro` (inline.rs ~390). Возможно `TagToken::Macro(Vec<Event<'static>>)`.
3. **char-refs** (`&#167;` survival — legacy InlinePassthrough при specialchars+replacements),
   **escape `\*`/`\pass:`**, **curved smart-quotes** `"…"`/`'…'`, **spec'd `pass:SPEC[]`**.
   specialchars — NO-OP (Event::Text сырой).
4. **ФИНАЛ Фазы 2:** снять gate (или per-construct) → flip outline (cross-span @4545 overlap) при 343.
- Скрипты корпуса: `blast.py` (toggle-off), `blast_toggle.py` (toggle-on gate), `blast_force.py`
  (FORCE), `diffone.py <file> <limit>` (с ADOC_QUOTES_SEQUENTIAL=1 ADOC_SUBST_FORCE=1 для FORCE-диффа).

---

## Сессия (2026-06-14, 71-я) — РЕРАЙТ inline, Фаза 2 НАЧАТА: replacements + post_replacements пассы за гейтом

Запрос «начни фазу 2». Ветка **`feat/subst-phase2-passes`** (off master `9cc1c2c`, 343) —
**2 коммита, НЕ смержена, НЕ запушена, ОЖИДАЕТ авторизации** на `git merge --no-ff` в master +
`git push` + удаление ветки. base-бинарь /tmp/adoc_base ПЕРЕСОБРАН из master HEAD `9cc1c2c` (343).

### Контекст
Фаза 0 (toggle) + Фаза 1 (quotes-пайплайн + токенизатор за differential-equality gate) — в master.
Фаза 2 = перенести ОСТАЛЬНЫЕ пассы пайплайна asciidoctor (`subs=normal`: passthrough-extract →
specialchars → quotes → attributes → replacements → macros → post_replacements → restore), довести
FORCE-движок до байт-идентичности, в финале СНЯТЬ gate → flip outline при 343 неизменных.
См. [[proj_sequential_quotes_rewrite]], план `~/.claude/plans/greedy-yawning-pumpkin.md`.

### AIRTIGHT-гарантия (подтверждена кодом, inline.rs ~224)
`parse_str_with_subs_options` при toggle-on зовёт `try_parse`, иначе `parse_legacy`. Гейт ВНУТРИ
try_parse сравнивает candidate с ТЕМ ЖЕ `parse_legacy`. ⇒ toggle-on вывод ≡ `parse_legacy` ВСЕГДА
(adopt: new==legacy; decline: legacy). Нет отдельного продакшн-пути для ячеек/спанов с edges=false —
edges-семантика гейта самосогласована, edges-флаг для нового движка НЕ релевантен. 0 регрессий по построению.

### Сделано (2 коммита)
- `run_pipeline(text, subs)` теперь прокидывает `subs`; каждый пасс гейтится на своём флаге SubstitutionSet.
- **`6574062` (1/N) replacements** (`subst/replacements.rs`): `crate::inline::apply_typographic_replacements`
  стал `pub(crate)`, применяется к ВСЕМУ буферу разом с (true,true). Анализ: сентинел-байты 0x01/0x02 =
  `<>`-границы тегов asciidoctor (non-space/non-word) ⇒ `*--*`→литерал `--`, top-level `--`→em-dash,
  span-internal `-- x`/`x --`→литерал, `*don't*`→curly — совпадает с legacy И asciidoctor. char-ref
  restore НЕ здесь (с passthrough). +1 тест (18 кейсов).
- **`55f0a2f` (2/N) post_replacements** (`subst/post_replacements.rs`): hard-break ` +`. Новый
  `TagToken::HardBreak` + `break_sentinel` в tokenize.rs → Event::HardBreak. ` +\n` всюду; ` +` на конце
  буфера. КЛЮЧ: edges-флаг НЕ нужен — ` +` внутри спана идёт перед close-сентинелом (TAG_LEAD), т.е.
  не конец и не \n ⇒ литерал автоматически (`*x +*`→`<strong>x +</strong>`, `*x* +`→`...<br>`). +1 тест (12 кейсов).

### Верификация
- clippy 0, test --workspace зелёное (parser 530, html 433), parsing-lab 233/233 (+2 subst-теста, 8 всего).
- blast toggle-off **343**, toggle-on **343** (гейт держит, 0 регрессий, 0 FARTHER в toggle-режимах).
- **FORCE (blast_force.py, метрика прогресса): 46 → 85** raw-идентичных файлов, **0 паник**.
  5 FORCE-FARTHER (subs/quotes, image-size, subs/attributes, subs/post-replacements,
  document-attributes-ref) — ЭКСПЕКТЕД каскад: ` +` рядом с нерезолвленным `{attr}`/macro (напр.
  ячейка `{y} +` в post-replacements.adoc); верный `<br>` сдвигает позиции пока `{attr}` литерал.
  diffone подтвердил: @13 `{plus}`, @31 `<<table-post>>` — отложенные attributes/macros, НЕ баг hard-break.
  Gate эти тексты отклоняет (new≠legacy из-за `{attr}`) → сойдутся, когда лягут attributes/macros.

### Дальше (ОСТАЛОСЬ Фаза 2, по убыванию приоритета/фундаментальности)
1. **passthrough extract/restore** — FIRST в пайплайне, фундамент (иначе quotes лезет внутрь `+...+`).
   Нужны Code/InlinePassthrough события в токенизаторе; донор: `scanner::passthrough_span_len`/
   `single_plus_span_len`/`pass_macro_span_len`, `try_*_passthrough` в inline.rs (759-).
2. **attributes** `{name}` — СНАЧАЛА проверить, эмитит ли legacy `Event::AttributeReference` (резолв в
   рендерере) или резолвит инлайн; от этого зависит, что эмитить токенизатору.
3. **macros** — САМОЕ большое: link/xref/image/footnote/icon/kbd/btn/menu/stem/anchor/autolink/email,
   много типов событий → overhaul токенизатора. Донор: `handle_inline_macro` (inline.rs 390-).
4. **char-refs** (`&#167;` survival — legacy эмитит passthrough, рендерер не экранирует), **escape `\*`**,
   **curved smart-quotes** `"…"`/`'…'`. **specialchars — NO-OP** (Event::Text сырой, рендерер экранирует).
5. **ФИНАЛ Фазы 2:** снять gate (или per-construct) → flip outline (cross-span @4545 даёт overlap) при 343.
- Скрипты корпуса: `blast.py` (toggle-off), `blast_toggle.py` (toggle-on gate), `blast_force.py` (FORCE),
  `diffone.py <file> <limit>` (с ADOC_QUOTES_SEQUENTIAL=1 ADOC_SUBST_FORCE=1 для FORCE-диффа одного файла).

---

## Сессия (2026-06-14, семидесятая) — РЕРАЙТ inline, Фаза 1: quotes-пайплайн за differential-equality gate (БЕЗ флипа, 0-регрессий по построению)

Запрос «Продолжи фазу 1». Ветка **`feat/subst-phase1-quotes`** (off master c566b10, 343) —
ЗАКОММИЧЕНА. **НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на
`git merge --no-ff` в master + `git push origin master` + удаление ветки.**
base-бинарь /tmp/adoc_base собран из master c566b10 (343).

### Что это за фаза (контекст рерайта)
Фаза 0 (инертный scaffolding + toggle `ADOC_QUOTES_SEQUENTIAL=1`) уже в master. Фаза 1 —
реализовать `quotes`-субституцию как string-rewriting (gsub-последовательность плоских пассов
по всей строке) + токенизатор, чтобы воспроизвести cross-span OVERLAP, который рекурсивная
legacy-модель не может (см. [[proj_sequential_quotes_rewrite]], план greedy-yawning-pumpkin).
Самая рискованная фаза. Мандат «0 регрессий на 343» НЕ-обсуждаем.

### Ключевое архитектурное решение: differential-equality gate (scaffold Фазы 1)
`try_parse` гоняет НОВЫЙ пайплайн И legacy, возвращает `Some(new)` ТОЛЬКО при побайтовом
равенстве `Vec<Event>`, иначе `None` → fallback на legacy. Это делает корпус **0-регрессий
ПО ПОСТРОЕНИЮ** (вывод == legacy всегда) и даёт честную метрику покрытия. Gate — ВРЕМЕННЫЙ:
Фаза 2 его снимает, чтобы расходящиеся (overlapping) кейсы флипнули outline. Диагностика
`ADOC_SUBST_FORCE=1` минует gate (сырой вывод движка) для замера верности.

### Анализ перед реализацией (важно для будущих сессий)
- **Парсер выдаёт Events, не HTML.** Экранирование `<>&` — задача рендерера (Event::Text сырой).
  Поэтому specialchars-перезапись в Фазе 1 НЕ нужна (и вредна) — работаем на СЫРОМ тексте.
- **Overlap = порядок пассов.** strong-пасс по всей строке ДО mono-пасса → strong-сентинелы
  в строке до того, как mono их оборачивает. Edge-флаги воспроизводятся ЕСТЕСТВЕННО: mono-пасс
  видит литеральный `_` (word-char) перед backtick → не открывается (`_`code`_`→`<em>`code`</em>`).
- Порядок QUOTE_SUBS (без curved, отложены): strong(unc,con), mono(unc,con), em(unc,con),
  mark(unc,con), sup, sub.

### Что сделано
- **inline.rs**: `parse_legacy(text,subs,options)` вынесена из `parse_str_with_subs_options`
  (тело после toggle-чека) — чтобы subst вызывал legacy для сравнения без рекурсии.
- **`subst/tokenize.rs`** (НОВЫЙ): сентинел `\x01<dec-idx>\x02` в рабочей `String`; side-table
  `Vec<TagToken::{Open{kind,id,roles},Close(kind)}>`; `Work{buf,tags}`; `tokenize`→`Vec<Event>`
  БЕЗ балансировки (overlap сохраняется); `SpanKind`→Tag/TagEnd; `utf8_char_len`, `sentinel_end`.
- **`subst/quotes.rs`** (НОВЫЙ): `run_all` (10 пассов в порядке QUOTE_SUBS); generic
  `pass_unconstrained`/`pass_constrained`/`pass_simple_pair`; `[attrlist]` (`parse_attrs`/
  `parse_shorthand` зеркало legacy; constrained требует open-boundary, unconstrained нет;
  mark+attrlist→InlineSpan, bare→Highlight); `find_closing_constrained`/`_unconstrained`
  (скип сентинел-регионов; mono extra-close `(?![\w"'`])` только в bare-ветке — зеркало legacy).
- **`subst/mod.rs`**: `try_parse` (gate QUOTES + sentinel-byte reject + equality gate / FORCE);
  `enabled()`/`force()`/`env_true`; `run_pipeline`; +6 unit-тестов (differential vs legacy на
  32 quotes-only пробах; overlap-кейс; gate adopts/declines; sentinel/no-quotes reject).

### Статус (верифицировано)
- clippy --workspace 0; test --workspace зелёное (parser 522→528, html 433); parsing-lab 233/233.
- **blast toggle-OFF 0 diff vs base** (инертность); **toggle-ON+gate 0 diff vs base** (Фаза 1
  гейт — корпус не изменён, 343 неизменны). Замер прямым `gate_check.py` (base-vs-new, без asciidoctor).
- **FORCE-диагностика**: 46/344 файлов идентичны base на уровне файла (сильно занижено — 1
  inline-текст с отложенной фичей флипает весь файл). **0 паник на 344 файлах** (сырой пайплайн
  робастен). Спот-чек расхождений: ВСЕ — отложенные фичи (don't→don’t replacements, ` +`→`<br>`
  post-repl, `<<>>`/`xref:` macros, `` `+*word*+` `` passthrough), НИ ОДНОГО бага quotes.
- Скрипты в корпусе: `gate_check.py` (KEY=VAL→env нового бинаря), `blast_force.py`.

### Что дальше — Фаза 2
Перенести остальные пассы (passthrough-extract ПЕРВЫМ, затем specialchars, attributes,
replacements, macros, post-replacements, restore) + escape `\` + curved smart-quotes. По мере
роста верности FORCE-расхождения тают. КОГДА движок воспроизводит 343 байт-в-байт под FORCE —
СНЯТЬ equality gate (или сделать его per-construct) чтобы cross-span overlap флипнул outline
(4813→0). Гейт Фазы 2: Identical→344, 0 регрессий, 0 FARTHER. Затем Фаза 3 — swap дефолта,
удаление legacy quotes + edge-флагов (`emphasis_leading_edge`/`smart_quote_leading_edge`/
`edges_are_line_boundaries`) + toggle.

---

## Сессия (2026-06-14, шестьдесят девятая) — РЕРАЙТ inline на string-rewriting (Фаза 0: инфраструктура+toggle, инертно)

Запрос «продолжи» → дошли до архитектурного решения. Фаза 0 (инертная инфраструктура)
СДЕЛАНА на ветке `feat/sequential-quotes-engine` и **СМЕРЖЕНА в master по авторизации
пользователя** (merge --no-ff + push, ветка удалена). **Фаза 1 — новая ветка off master.**
Старт: 68-я (escape `\*`) УЖЕ смержена И запушена пользователем (master=7a8fecc, 343).
base-бинарь /tmp/adoc_base пересобран из master 7a8fecc.

### Как сюда пришли (важный контекст для будущих сессий)
- 68-я закрыла escape `\*` (один из 2 корней outline), БЕЗ флипа. Остался 1 корень outline:
  **cross-span strong @4545**.
- Разобрал его пробами (probe727): asciidoctor применяет QUOTES как последовательные плоские
  gsub-пассы по всей строке (strong ДО monospace); `[0-9]*…*` = strong с ролью "0-9",
  пересекает границы code-спанов → НЕВАЛИДНЫЙ overlapping HTML (`<code>…<strong>…</code>…
  <strong>…</code>`). Рендерер вложенность НЕ валидирует — выдал бы overlap как есть. Блокер
  ТОЛЬКО в парсере (рекурсивная модель даёт лишь вложенные теги).
- 2 независимых анализа (Explore+Plan-агенты): ограниченного подхода НЕТ (детектор = полный
  рерайт за гейтом). Рекомендация обоих — WONTFIX (343/344). **Пользователь дважды, с полным
  знанием рисков, выбрал РЕРАЙТ.**

### План (утверждён) — `~/.claude/plans/greedy-yawning-pumpkin.md`
Реплицировать substitutor asciidoctor: extract_passthroughs → specialchars → quotes
(gsub-последовательность с сентинел-тегами) → attributes → replacements → macros →
post_replacements → restore; финал — токенизатор строки-с-сентинелами в Vec<Event> БЕЗ
балансировки (overlap сохраняется). Dual-engine за toggle (legacy = дефолт, не трогать до
Фазы 3), blast-гейт 0-регрессий. Фазы: 0 инфра / 1 passthrough+specialchars+quotes (самая
рискованная) / 2 остальные пассы → flip outline / 3 swap дефолта + удаление legacy quotes.

### Что сделано (Фаза 0)
- **`adoc-parser/src/subst/mod.rs`** (НОВЫЙ): `enabled()` (OnceLock из env
  `ADOC_QUOTES_SEQUENTIAL`, `1`/`true`), `try_parse(text,subs,options)->Option<Vec<Event>>`
  (Phase 0: всегда None = fallback на legacy). Doc-коммент объясняет string-rewriting и
  transitional-статус. **ОТСТУПЛЕНИЕ ОТ ПЛАНА**: toggle через env+OnceLock, НЕ через
  `InlineOptions.sequential_quotes` — меньше инвазивности для scaffolding (удаляется в Фазе 3).
- **lib.rs**: `mod subst;`.
- **inline.rs** `parse_str_with_subs_options`: ветвление ПЕРЕД построением InlineState —
  `if subst::enabled() && let Some(ev)=subst::try_parse(...) { return ev }`. Только top-level
  (inner-reparse — legacy исключительно).
- **`/mnt/c/tmp/adoc-test/blast_toggle.py`** (НОВЫЙ): ставит env=1, `import blast` (base-бинарь
  master игнорирует переменную, новый читает). 

### Статус (верифицировано)
- clippy --workspace 0; test --workspace зелёное (parser 522, html 433); parsing-lab 233/233.
- toggle sanity: вывод on==off (md5 совпал).
- **blast toggle-OFF vs base: 343→343, 0 изменённых файлов (инертно).**
- **blast toggle-ON vs base: 343→343, 0 изменённых файлов (инертно)** — движок Фазы 0
  отклоняет всё, fallback на legacy. Плумбинг работает end-to-end.

### Что дальше — Фаза 1 (следующая сессия)
Реализовать в `subst/`: extract_passthroughs (переиспользовать сканеры scanner.rs:
passthrough_span_len/single_plus_span_len/pass_macro_span_len), specialchars-пасс, quotes как
gsub-последовательность с сентинелами + токенизатор. Перенести attrlist-логику
(parse_inline_shorthand:2870, try_inline_attr_span:2896) и escape-правила (inline.rs:772).
Гейт: blast toggle-ON приближается к 344, 0 регрессий на 343. Самая рискованная фаза
(воспроизвести leading-edge-семантику порядком пассов вместо edge-флагов).

---

## Сессия (2026-06-14, шестьдесят восьмая) — Фаза 3: escape `\*`/`\_`/`` \` `` сохраняет `\` без пары (один из 2 корней outline; БЕЗ флипа)

Запрос «продолжи». Ветка **`fix/escape-backslash-keep-when-no-span`** —
ЗАКОММИЧЕНА. **НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на
`git merge --no-ff` в master + `git push origin master` + удаление ветки.**
Старт: housekeeping 67-й закрыт сам (мерж 67-й УЖЕ выполнен И запушен —
origin/master == master == 4463225 (343), дерево чисто, веток нет). base-бинарь
/tmp/adoc_base пересобран из master HEAD (343).

### Выбор задачи
nearmiss на 343: остался ТОЛЬКО `outline` (4814, Δ4) — 2 корня (diffone 12): @2041
escape `\*` (изолированный content-diff, НЕ влияет на len_delta) + каскад с @4545
(cross-span strong, это и есть +4 токена). Выбран escape `\*` (contained, безопасный
путь документирован 66-й). cross-span strong НЕ берётся — глубоко архитектурный (проба
ниже).

### Реальная семантика (пробы asciidoctor 2.0.23)
- **escape (consume-on-match)**: quote-regexp'ы несут опц. `\\?` и снимают ведущий `\`
  ТОЛЬКО при реальном матче. Таблица (lone/no-pair): `\*`→`\*`, `\_`→`\_`, `` \` ``→`` \` ``
  (keep — мы ошибочно дропали); `\*bold*`→`*bold*` (валидный → drop, literal — уже верно);
  `\#lone`→`#lone` (asciidoctor ДРОПАЕТ # всегда — мы совпадаем, НЕ трогать); `\^`/`\~`
  asciidoctor keep, у нас вообще сломано (`\^lone`→`\</sup>` — отдельный механизм sup/sub,
  НЕ трогал); `\{`/`\[`/`\<`/`\'`/`\\` — каждый со своей нюансной логикой, отложены.
- **cross-span strong @4545** (проба probe727): исходник outline:727
  `` `[1-9][0-9]*.` (ordered), …, `<([1-9][0-9]*|\.)>` ``. asciidoctor →
  `<code>[1-9]<strong class="0-9">.</code> … <code>&lt;([1-9][0-9]</strong>|\.)&gt;</code>`.
  Механизм: QUOTE_SUBS строго `strong` ДО `monospace`; constrained strong `[0-9]*…*` матчит
  поверх ВСЕЙ строки (роль "0-9" из `[...]` перед `*`), открывается в 3-м code-спане, закрыт
  в 5-м — strong ПЕРЕСЕКАЕТ границы code-спанов (невалидно-вложенный HTML). Наша рекурсивная/
  иерархическая модель спанов (найти span → рекурс внутрь) этого не воспроизводит без
  переписывания inline-модели на последовательные line-level regexp-пассы. ГЛУБОКО
  АРХИТЕКТУРНЫЙ, высокий риск на весь корпус, 1 файл. ОТЛОЖЕН.

### Что сделано
- **ПАРСЕР** inline.rs `handle_inline_escape`: новый escape-арм ПЕРЕД blanket'ом (стр.952).
  Гейт: `has_quotes && peek_at(1)∈{*,_,`} && find_closing_constrained(marker, pos+2).is_none()`
  → flush, emit `\`+маркер как литеральный Text, advance_by(2). Регрессионно-безопасно ПО
  ПОСТРОЕНИЮ (None ⇒ нет пары ⇒ asciidoctor тоже keep). Some-кейс не тронут (blanket дропает).
- +1 parser (`test_escaped_marker_no_span_keeps_backslash`: `\*`/`\_`/`` \` `` без пары →
  `\` сохранён; контраст test_escaped_bold/italic/monospace — с парой дропается),
  +1 html (`test_escaped_marker_no_span_keeps_backslash_html`: corpus `` `\* literal` ``→
  `<code>\* literal</code>`, `` `\*bold*` ``→`<code>*bold*</code>` без `<strong>`, prose `\_lone`).

### Статус (верифицировано)
- clippy --workspace 0; test --workspace зелёное (parser 521→522, html 432→433);
  parsing-lab 233/233.
- diffone outline: @2041 escape `\*` УШЁЛ; первый diff теперь @4545 (cross-span strong),
  total 4814→4813.
- **Корпус: Identical 343 (БЕЗ флипа)**. blast (base 343): **outline 4814→4813 closer,
  0 регрессий, 0 других файлов**. Корректный фикс + закрытие одного из 2 корней outline.

### Что дальше
- nearmiss на 343: останется ТОЛЬКО outline (4813) — ЕДИНСТВЕННЫЙ корень @4545 cross-span
  strong, глубоко архитектурный (рерайт inline на line-level QUOTES-пассы). Других Different
  нет. **Корпус-driven compat достиг предела**: рост Identical требует ЛИБО этого рерайта
  (высокий риск, 1 файл), ЛИБО расширения корпуса. Обсудить направление с пользователем.

---

## Сессия (2026-06-14, шестьдесят седьмая) — Фаза 3: a-ячейка = embedded-документ + Markdown thematic breaks → ФЛИП syntax-quick-reference

Запрос «продолжи». Ветка **`fix/acell-nested-doc-header-content-div`** —
ЗАКОММИЧЕНА (`519149f`). **НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на
`git merge --no-ff` в master + `git push origin master` + удаление ветки.**
Старт: housekeeping 66-й закрыт сам (мерж 66-й УЖЕ выполнен И запушен —
origin/master == master == fde83c0 (342), дерево чисто, веток нет). base-бинарь
/tmp/adoc_base пересобран из master HEAD (342) через временный worktree.

### Выбор задачи
nearmiss на 342: остались ТОЛЬКО 2 Different, ОБА мульти-root spec/каталог-файлы:
syntax-quick-reference (2788, Δ−31, НЕ разведан), outline (4814, Δ4, 2 архитектурных
корня отложены 66-й). Выбран syntax-quick-reference.

### Разведка (diffone)
Первый diff @6: asciidoctor `<div id="content">`, мы — `<div id="preamble">` сразу,
без content-обёртки. count: у нас content×1 (но встроен в текст `:summary: AsciiDoc is
<div id="content">` на стр.1810!) + preamble×2; у asciidoctor по 1. Корень контекстный
(не в минимальной пробе — преамбула/IMPORTANT/CRLF дают content-div корректно).
Локализовано: исходник 1101-1124 — `[%collapsible.result]` example с таблицей
`[.unstyled]\n|===\na|\n:url-home:...`. Ячейка `a|` парсится вложенным `Parser::new`
через ТОТ ЖЕ `self` (общий footnote/xref-стейт). Её ведущие атрибут-записи дают
ложный `TagEnd::Header` → перезаписывает `content_start`/`preamble_start` родителя
(content-div вставляется ПОСЛЕ ячейки) + эмитит лишний `<div id="header">` в ячейке.

### Реальная семантика (пробы asciidoctor 2.0.23)
- **a-ячейка = embedded-документ**: asciidoctor для неё header div НЕ выдаёт, ведущие
  атрибут-записи резолвятся (`{url-home}`→ссылка) но НЕ образуют `#header`.
- **Markdown thematic breaks**: asciidoctor распознаёт `---`/`***`/`___` и spaced-формы
  (`- - -`/`* * *`/`-  -  -`) как `<hr>` по regexp `^ {0,3}([-*_])( *)\1\2\1$` — РОВНО
  3 маркера (4=`----` listing, 2=`--` open), одинаковые промежутки, 0-3 ведущих пробела,
  rstrip. Все 15 граничных кейсов пройдены пробами. mid-paragraph (без пустой строки до)
  — ВСЕ формы (включая `'''`) поглощаются как текст; наш парсер рвёт параграф на любом
  thematic — известная предсуществующая дивергенция (block.rs:2379, коммент 2390-2391),
  в корпусе не регрессирует.

### Что сделано
- **РЕНДЕРЕР** lib.rs: поле `cell_render_depth: usize` (init 0). events.rs:
  инкремент/декремент вокруг вложенного `Parser::new(&raw)` цикла ячейки (TagEnd::TableCell,
  CellStyle::AsciiDoc); гейт `Tag::Header` start (`depth>0` → `return`, no-op) и
  `TagEnd::Header` (новый guard-арм `if depth>0` → пусто). Атрибут-записи идут отдельным
  Event::Attribute — резолв не ломается.
- **ПАРСЕР** scanner.rs: `is_thematic_break` = `trim()=="'''" || is_markdown_thematic_break`.
  Новый приватный `is_markdown_thematic_break` (точный разбор regexp asciidoctor по байтам).
  Диспетчер (block.rs:1248 scan_leaf_blocks) проверяет thematic ДО delimited(1260)/list(1264) —
  3-символьные формы не коллидируют с 4-симв. делимитерами и 2-симв. `--`.
- +3 теста: scanner `test_is_thematic_break` (8 valid + 7 invalid границ), html
  `test_markdown_thematic_breaks` (7 форм + 4 negative), html
  `test_asciidoc_cell_leading_attribute_entries_no_header_html` (standalone: header→content→
  preamble порядок, ячейка без header div, ровно 1 content+1 preamble, `{url-home}` резолвится).

### Статус (верифицировано)
- clippy --workspace 0; test --workspace зелёное (parser 520→521, html 430→432);
  parsing-lab 233/233.
- syntax-quick-reference diffone: **2788 → 33 (после cell-фикса) → 0 (после thematic, байт-в-байт)**.
- **Корпус: Identical 342→343 (+1 ФЛИП)**. blast (base 342): **РОВНО 1 флип
  (syntax-quick-reference 2788→0), 0 регрессий, 0 FARTHER**.

### Что дальше
- nearmiss на 343: останется ТОЛЬКО outline (4814) — 2 архитектурных корня (@2041
  escape `\*`, @4545 cross-span strong), ОБА отложены 66-й как глубоко архитектурные/
  низкий ROI. Других Different-файлов нет. Дальнейший рост Identical требует одного из
  этих архитектурных корней (см. заметки 66-й) ИЛИ расширения корпуса.

---

## Сессия (2026-06-14, шестьдесят шестая) — Фаза 3: ` +` hard-break только на реальном крае строки (не в reparsed-спанах)

Запрос «продолжи». Ветка **`fix/outline-escape-and-monospace-hardbreak`** —
ЗАКОММИЧЕНА. **НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на
`git merge --no-ff` в master + `git push origin master` + удаление ветки.**
Старт: housekeeping 65-й закрыт сам (мерж 65-й УЖЕ выполнен И запушен —
origin/master == master == a9bfe05 (342), дерево чисто, веток нет). base-бинарь
/tmp/adoc_base пересобран из master HEAD (342).

### Выбор задачи
nearmiss на 342: остались ТОЛЬКО 2 Different, ОБА мульти-root spec/каталог-файлы:
**outline (6647, Δ−3)**, syntax-quick-reference (2788, Δ−31). Выбран outline (ближе
по дельте, конкретные корни из session-заметок 65-й). Кластеризация diff'ов (gap>8):
3 корня — @2041 (escape `\*`, изолирован, 1 diff), @2707 (` +` hard-break в monospace,
каскад ~4600), @4545 (cross-span strong, каскад). Выбран hard-break: каскад + чистый,
contained, давний «отложенный баг» (`` `x +` ``→`<br>`, упомянут в TODO/session многих сессий).

### Реальная семантика (пробы asciidoctor 2.0.23)
- **Hard-break** = ` +` на РЕАЛЬНОМ крае строки. asciidoctor применяет line-break
  replacement ПОСЛЕ рендера спанов, поэтому трейлинг ` +` внутри спана ограничен
  закрывающим тегом (`</code>`), а НЕ `$`: `` `x +` ``→`<code>x +</code>`,
  `` `` + +`` ``→`<code> + +</code>`, `` `z +` ``→`<code>z +</code>` (все литералы, БЕЗ `<br>`).
  Top-level одиночная строка `foo +` (end-of-string) → `<br>` (проба `<p>...plus<br></p>`).
  `+\n` mid-string (реальный newline) → `<br>` всегда, даже внутри monospace.
- Корень в нашем коде: `check_hard_break` матчил ` +` на end-of-string БЕЗУСЛОВНО —
  но reparsed-контент спана попадает в end-of-string на искусственном крае. Та же
  проблема, что у spaced em-dash (`edges_are_line_boundaries`).

### Что сделано
- **ПАРСЕР** inline.rs `check_hard_break`: end-of-string случай (` +` без следующего
  байта) теперь даёт hard-break ТОЛЬКО при `self.edges_are_line_boundaries` (true
  лишь top-level, false во всех inner-reparse). Случай ` +\n` (mid-string newline) —
  без изменений (безусловно). Зеркало em-dash-границ из `fix/monospace-replacements-subs`.
- +1 parser (`test_monospace_edge_trailing_space_plus_stays_literal`: `` `x +` ``/
  `` ` + +` ``/`` `+ +` `` без HardBreak; `line one +` И `` `a +\nb` `` — с HardBreak),
  +1 html (`test_monospace_trailing_space_plus_not_hard_break_html`).

### Статус (верифицировано)
- clippy --workspace 0; test --workspace зелёное (parser 519→520, html 429→430);
  parsing-lab 233/233.
- **Корпус: Identical 342 (БЕЗ флипа)**. blast (base 342): **outline 6647→4814 closer,
  0 регрессий, 0 других файлов изменено** (фикс широкий — все reparsed-спаны с
  трейлинг ` +`, но в корпусе затронут только outline).
- Это корректный фикс + схлопывание крупнейшего каскада outline. Флипа нет: остаются
  2 АРХИТЕКТУРНЫХ корня (см. ниже).

### Что дальше — outline остаток (2 корня, ОБА архитектурные, флип заблокирован)
- **@2041 escape `\*`** (изолирован, 1 diff): `` (`\* is an asterisk`) `` — asciidoctor
  СОХРАНЯЕТ `\` когда маркер НЕ образует валидную разметку (`\*` без закрывающей `*` →
  литерал `\*`; `\*bold*` валидный → `\` съеден). Наш blanket-escape (inline.rs ~952)
  съедает `\` перед `*`/`` ` ``/`_`/`#`/... БЕЗУСЛОВНО (баг и в обычном тексте: `\* x`→`* x`).
  Фикс: съедать `\` лишь при валидном спане (consume-on-match). РИСК (validity-чек должен
  совпасть с regexp asciidoctor) + НИЗКИЙ ROI: в корпусе НЕТ других файлов с этим багом
  (всего 2 Different — оба хвост), дал бы ≤1 diff, флип заблокирован @4545. ОТЛОЖЕН.
  Безопасный путь если делать: KEEP `\` только когда `find_closing_*`=None (нет спана
  заведомо → asciidoctor тоже сохраняет → 0 регрессий vs current).
- **@4545 cross-span strong** (каскад 4813 diff, стр.727): `` `[1-9][0-9]*.` `` —
  asciidoctor парсит `[0-9]*` как constrained strong с РОЛЬЮ "0-9" (`[...]` перед `*` =
  attrlist), причём `<strong class="0-9">` тянется ЧЕРЕЗ границы code-спанов (открыт в
  3-м `<code>`, закрыт в последнем — невалидно-вложенный HTML). Артефакт line-level
  QUOTES-пасса asciidoctor (strong матчится по всей строке поверх monospace). Наша
  рекурсивная/изолированная модель спанов это НЕ воспроизведёт без слома модели. ГЛУБОКО
  АРХИТЕКТУРНЫЙ, блокирует флип outline. ОТЛОЖЕН.
- **syntax-quick-reference** (2788, Δ−31): не разведан, мульти-root.

---

## Сессия (2026-06-14, шестьдесят пятая) — Фаза 3: emphasis leading-edge подавляет strong/mono + docyear/localyear

Запрос «продолжи». Ветка **`fix/emphasis-leading-edge-suppresses-strong-mono`** —
ЗАКОММИЧЕНА (`57870bf`). **НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на
`git merge --no-ff` в master + `git push origin master` + удаление ветки.**
Старт: housekeeping 64-й закрыт сам (мерж 64-й УЖЕ выполнен И запушен —
origin/master == master == fea2329 (341), дерево чисто, веток нет). base-бинарь
/tmp/adoc_base пересобран из master HEAD (341) через временный worktree.

### Выбор задачи
nearmiss на 341: остались ТОЛЬКО 3 Different, ВСЕ мульти-root spec/каталог-файлы
(других near-miss для побочных флипов больше нет): **document-attributes-ref (953,
Δ−3)**, syntax-quick-reference (2788), outline (6647). Выбран document-attributes-ref.
Анализ gap'ов diff-позиций (diffone | awk): ровно 3 корня — @726 (`{docyear}`,
изолирован), @1043 (`{localyear}`, изолирован), @6257→@7232 contiguous = ОДИН
структурный десинк, каскадящий ~950 ложных diff'ов до конца файла. len_delta=-3 =
ровно 3 лишних токена. Фикс всех трёх → флип.

### Реальная семантика (пробы asciidoctor 2.0.23)
- **Десинк @6257**: исходник стр.1216 `_`inline` not yet supported._` — внутри
  emphasis `_..._` стоит `` `inline` ``. asciidoctor НЕ делает `<code>` (backtick
  литерален), мы — делаем. Порядок QUOTE_SUBS (выведен пробами): **strong → monospace
  → emphasis → mark**. На ведущем крае emphasis-спана constrained strong (`*`) и
  monospace (`` ` ``) ещё видят ЛИТЕРАЛЬНЫЙ внешний `_` (word-char) — их open-ассерт
  `(^|[^\w…])` его отвергает → литерал. Mark (`#`) идёт ПОСЛЕ emphasis (видит `>` от
  `<em>`) → открывается; unconstrained/`~`/`^` open-ассерта не имеют → открываются.
  После внешних `*`/`#` (не word-char) внутренние markers открываются. Пробы покрыли
  все 10 комбинаций (см. ниже), все совпали.
- **docyear/localyear**: asciidoctor — `docyear` из mtime ФАЙЛА-источника (целевой
  файл 2026-03-15 → 2026; проба mtime=2019 → dy=2019), `localyear` из NOW (2026).
  Оба = 2026 сейчас → совпадают. CLI УЖЕ сидит весь date-family через chrono
  (docdate/doctime/.../localdate/...) — не хватало только года.

### Что сделано
- **ПАРСЕР** inline.rs: поле `emphasis_leading_edge: bool` (зеркало
  `smart_quote_leading_edge`), init false; гейт в `try_constrained`
  (`flag && start_pos==0 && marker∈{*,`` ` ``}` → return false); установка
  `inner_parser.emphasis_leading_edge = marker == b'_'` в ОБОИХ репарс-сайтах
  (try_constrained @inner + try_unconstrained @inner — покрывает `_` и `__`).
- **CLI** main.rs: `seed("docyear", input_mtime.format("%Y"))` (из mtime, как doc*),
  `seed("localyear", now.format("%Y"))` (из now, как local*). Та же chrono-машинерия.
  Комментарий блока обновлён.

### Статус (верифицировано)
- clippy --workspace 0; test --workspace зелёное (parser 516→519, html 428→429);
  parsing-lab 233/233.
- document-attributes-ref diffone: **953→2 (после inline-фикса, каскад схлопнулся)
  →0 (после date-фикса, байт-в-байт)**, len ref==our==7230.
- **Корпус: Identical 341→342 (+1 ФЛИП)**. blast (base 341): **РОВНО 1 флип, 0
  регрессий, 0 FARTHER** (inline-фикс широкий — затрагивает всё содержимое `_..._`
  с ведущим `*`/`` ` `` — но ни один другой файл не изменился).
- Проба docyear робастности: mtime=2019 → dy=2019/ly=2026 байт-в-байт с asciidoctor
  (doc* следует за mtime, не «now» → устойчиво к ре-checkout'у).
- Тесты: +2 parser (`test_emphasis_leading_edge_suppresses_strong_and_mono`:
  `_`inline`_`/`_*b*_`/`__`inline`__`; `test_emphasis_leading_edge_does_not_suppress
  _mark_or_unconstrained`: `_#m#_`→mark, `_``c``_`→code; `..._suppression_is_leading_only`:
  `_x `c` y_`/`*`c`*`), +1 html (`test_emphasis_leading_edge_keeps_strong_mono_literal_html`).
  Date-семейство (как и docdate/localdate ранее) без юнит-теста — clock/mtime-зависимо,
  охраняется корпусом; робастность подтверждена пробой.

### Что дальше
- nearmiss на 342 (2 Different, ОБА мульти-root spec/каталог): **syntax-quick-reference
  (2788)**, **outline (6647, Δ3 — `\*` экранирование + `+` hard-break)**. Применять ту
  же методику: diffone | awk на gap'ы → выделить отдельные корни → проба asciidoctor →
  фикс корня (даже если файл не флипнет одним фиксом, схлопывание каскадов приближает).
- **Остаток кластера col-spec** (из 64-й): голый `cols="^~m,..."` БЕЗ `%autowidth` —
  per-column `~` = autowidth колонки, мы эмитим `width:…%`. НЕ в корпусе — отложено.
- **Pre-existing шире**: `*`/`.` list-маркер после строки параграфа БЕЗ blank —
  asciidoctor поглощает, мы прерываем.

---

## Сессия (2026-06-14, шестьдесят четвёртая) — Фаза 3: `~` autowidth-маркер в col-spec не должен съедать стиль колонки

Запрос «продолжи». Ветка **`fix/session64-nearmiss`** — ЗАКОММИЧЕНА.
**НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на `git merge --no-ff` в
master + `git push origin master` + удаление ветки.** Старт: housekeeping 63-й
закрыт сам (мерж 63-й УЖЕ выполнен И запушен — origin/master == master == ac6aaf6
(340), дерево чисто, веток нет). base-бинарь /tmp/adoc_base пересобран из master HEAD (340).

### Выбор задачи
nearmiss на 340 (4 Different, весь «трудный хвост»): **character-replacement-ref
(625, Δ113)**, document-attributes-ref (953), syntax-quick-reference (2788),
outline (6647). Выбран character-replacement-ref — таблица `[%autowidth,cols="^~m,^~l,^~"]`,
diff'ы стартуют с #86 (`<code>` vs голый `<p>`) = кластер «стили колонок m/e/s/l не наследуются».

### Корень (одна точка, пробы asciidoctor 2.0.23)
- Col-spec `^~m`: `~` — это **токен ширины autowidth** (регэксп asciidoctor `(\d+%?|~)`).
  Наш `parse_col_spec` (attributes.rs:127) парсил ТОЛЬКО цифры как ширину → `~` не
  потреблялся → rest=`~m` (len 2) → проверка стиля `rest.len()==1` ПРОВАЛИВАЛАСЬ →
  колонка получала Default вместо Monospace/Literal. (Стиль колонки УЖЕ наследуется в
  block.rs `resolve_style`@1916 и рендерится в blocks.rs@230-264 — не хватало лишь
  разбора `~`.) Пробы: `^~m`→`<code>`, `^~l`→`<div class="literal"><pre>`, `^~`→plain.
- colgroup для целевого файла уже совпадал (`%autowidth` → голый `<col>`, рендерер
  гейтит на `has_autowidth`). Так что НЕ кластер colgroup — чисто стиль ячеек.

### Что сделано (ПАРСЕР, одна точка)
- **attributes.rs::parse_col_spec**: после цифр-ширины потребляется опциональный `%`;
  при отсутствии цифр потребляется `~` (autowidth-маркер). spec.width при `~` = 0.

### Статус (верифицировано)
- clippy --workspace 0; test --workspace зелёное (parser 515→516, html 427→428);
  parsing-lab 233/233.
- character-replacement-ref diffone: **625→0 (ФЛИП, байт-в-байт)**, len ref==our==756.
- **Корпус: Identical 340→341 (+1 ФЛИП)**. blast (base 340): **РОВНО 1 флип, 0 регрессий,
  0 FARTHER**.
- Тесты: +1 parser (`test_parse_col_spec_autowidth_marker_keeps_style`: `^~m`/`^~l`/`^~`/`50%s`),
  +1 html (`test_table_col_autowidth_marker_inherits_style_html`).

### Что дальше
- nearmiss на 341 (3 Different, ВСЕ архитектурные/мульти-root): document-attributes-ref
  (953, Δ−3 — docyear/localyear date-интринсики [риск] + inline-в-link),
  syntax-quick-reference (2788, мульти-root), outline (6647, Δ3 — `\*` экранирование +
  `+` hard-break, мульти-root).
- **Остаток кластера col-spec**: голый `cols="^~m,..."` БЕЗ `%autowidth` — asciidoctor даёт
  голый `<col>` (per-column `~` = autowidth колонки), мы эмитим `width: …%` (рендерер
  гейтит colgroup только на table-level `%autowidth`, не на per-column `~`). НЕ в корпусе
  как Different — отложено.
- **Pre-existing шире (НЕ трогал)**: unordered/ordered list-маркер (`*`/`.`) после строки
  параграфа БЕЗ blank — asciidoctor поглощает в параграф, мы прерываем.

---

## Сессия (2026-06-14, шестьдесят третья) — Фаза 3: вложенная таблица `!===` (разделитель `!`)

Запрос «продолжи». Ветка **`fix/nested-table-bang-delimiter`** — ЗАКОММИЧЕНА
(`05c0c8d`). **НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на
`git merge --no-ff` в master + `git push origin master` + удаление ветки.**
Старт: housekeeping 62-й закрыт сам (мерж 62-й УЖЕ выполнен И запушен —
origin/master == master == 5cc8e7a (339), дерево чисто, веток нет).
base-бинарь /tmp/adoc_base пересобран из master HEAD (339).

### Выбор задачи
nearmiss на 339 (5 Different): **table (37, Δ15)** — ближайший к флипу; остаток =
ровно Root 2 из 62-й (вложенная `!===`-таблица). Прочее архитектурное/мульти-root:
character-replacement-ref (625), document-attributes-ref (953),
syntax-quick-reference (2788), outline (6647). Выбран table.

### Реальная семантика (пробы asciidoctor 2.0.23)
- **Nested в a-ячейке**: `[cols="2,1"]` `!===` → полноценная таблица, `!` — разделитель
  ячеек, cols 66.6666%/33.3334%, implicit header (blank после 1-й строки) Col1/Col2,
  body C11/C12. a-ячейка УЖЕ ре-парсится рекурсивно (`Parser::new(&raw)`), рендер
  вложенной таблицы (colgroup-ширины через `parse_col_widths`/`format_col_width`) УЖЕ
  готов — не хватало лишь распознавания `!===` сканером.
- **Top-level `!===`** (edge, НЕ в корпусе): asciidoctor «missing leading separator»
  (ждёт `|`, не `!`) — наш парсер распознаёт `!`-разделитель безусловно; регрессий нет,
  т.к. top-level `!===` в корпусе отсутствует.
- Все 4 вхождения `!===` в корпусе: table.adoc (цель), delimited.adoc (содержимое
  обычной `|`-ячейки — НЕ блочно-сканируется, безопасно), nested.adoc/outline.adoc
  (внутри `` `!===` `` inline-литерала, безопасно).

### Что сделано (ПАРСЕР)
- **scanner.rs**: `is_table_delimiter` принимает префикс `!` (4-й к `|`/`,`/`:`); сплиттер/
  escape параметризованы байтом разделителя — `find_unescaped_sep`/`split_unescaped_sep`/
  `unescape_cell_sep` + `parse_table_cells_with_sep(line, sep)`. `|`-обёртка
  `parse_table_cells` оставлена ТЕСТ-ОНЛИ (`#[cfg(test)]` — иначе dead-code в lib-сборке,
  ловит `clippy --workspace`); `unescape_cell_pipes` удалена (0 вызовов).
- **block.rs** `scan_table`: разделитель из первого байта `opening_delim` (`!`→`b'!'`,
  иначе `b'|'`), формат для `!` остаётся Native (это PSV, не CSV/DSV); `sep` протащен в
  цикл PSV (`parse_table_cells_with_sep` + `unescape_cell_sep`).

### Статус (верифицировано)
- clippy --workspace 0; test --workspace зелёное (parser 513→515, html 426→427);
  parsing-lab 233/233.
- table.adoc diffone: **37→0 (ФЛИП, байт-в-байт)**, len ref==our==690.
- **Корпус: Identical 339→340 (+1 ФЛИП)**. blast (base 339): **РОВНО 1 флип, 0 регрессий,
  0 FARTHER**; delimited.adoc (риск, `!===` как содержимое `|`-ячейки) остался 0 diffs.
- Тесты: +1 scanner (`test_parse_table_cells_bang_separator` + расширен
  `test_is_table_delimiter`: `!===`/`!====`/негативы `!==`/`!`), +1 parser
  (`test_bang_delimiter_nested_table_splits_on_bang`), +1 html
  (`test_nested_bang_table_inside_asciidoc_cell_html`).

### Что дальше
- nearmiss на 340 (4 Different, ВСЕ архитектурные/мульти-root): character-replacement-ref
  (625, Δ113 — m-колонка `<code>`-наследование, кластер), document-attributes-ref (953,
  Δ−3 — docyear/localyear date-интринсики [риск] + inline-в-link), syntax-quick-reference
  (2788, мульти-root), outline (6647, Δ3 — `\*` экранирование + `+` hard-break, мульти-root).
- **Pre-existing шире (НЕ трогал)**: unordered/ordered list-маркер (`*`/`.`) после строки
  параграфа БЕЗ blank — asciidoctor поглощает в параграф, мы прерываем (см. 62-ю).

---

## Сессия (2026-06-14, шестьдесят вторая) — Фаза 3: callout-маркер не прерывает top-level параграф

Запрос «продолжи». Ветка **`fix/callout-marker-no-paragraph-interrupt`** — ЗАКОММИЧЕНА
(`82b8824`). **НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на
`git merge --no-ff` в master + `git push origin master` + удаление ветки.**
Старт: housekeeping 61-й закрыт сам (мерж 61-й УЖЕ выполнен И запушен —
origin/master == master == 62b0407 (339), дерево чисто, веток нет).
base-бинарь /tmp/adoc_base пересобран из master HEAD (339) через временный worktree.

### Выбор задачи
nearmiss на 339 (5 Different, ВСЕ архитектурные/мульти-root): **table (597, Δ1)**,
character-replacement-ref (625, Δ113 — m-колонка `<code>`-наследование, кластер),
document-attributes-ref (953, Δ−3), syntax-quick-reference (2788, мульти-root),
outline (6647, Δ3). Выбран table — структурно ближайший (Δ1).

### Реальная семантика (пробы asciidoctor 2.0.23)
table.adoc — документация с примерами AsciiDoc-исходника, где `<1>`,`<2>` — текстовые
аннотации. ДВА корня:
- **Root 1 (СДЕЛАНО)**: `|=== <1>` (суффикс ` <1>` → НЕ валидный делимитер) открывает
  параграф; следующие `<2>`/`<4>` должны ПРОДОЛЖАТЬ его, а не открывать colist.
  Реальное правило: callout-маркер `<N>` распознаётся как НОВЫЙ callout-список ТОЛЬКО на
  границе блока (после blank), НЕ как продолжение открытого параграфа. Пробы: `Some intro.\n<1>…`
  → ОДИН параграф (поглощает); `Some intro.\n\n<1>…` → colist (с warning «no callout found»).
  NB: asciidoctor так же поглощает `* item`/`. item` после строки параграфа (НЕ прерывают) —
  это pre-existing ШИРЕ расхождение нашего парсера (мы прерываем), НЕ трогал (риск).
- **Root 2 (НЕ сделано — отдельная крупная фича)**: вложенная таблица `!===` (разделитель
  ячеек `!`) внутри `a`-style ячейки (`[cols="1,2a"]`, ячейка `Cell 2.2` содержит
  `[cols="2,1"] !=== … !===`). Наш парсер `!===` НЕ распознаёт (CSV/DSV-сессия 61
  намеренно исключила `!`). `a`-ячейка УЖЕ ре-парсится рекурсивно
  (`adoc-html/events.rs:1086 Parser::new(&raw)`), нужно лишь научить BlockScanner
  делимитеру `!===` с разделителем `!`.

### Что сделано (ПАРСЕР, узкий фикс Root 1)
- **block.rs**: в ДВУХ местах прерывания открытого параграфа (`scan_paragraph` @~2379 +
  admonition-continuation @~2770) условие `is_callout_list_item` ГЕЙТНУТО на
  `self.is_in_callout_list()`. Top-level (НЕ в colist) → callout-маркер поглощается;
  внутри colist → завершает continuation текущего item'а и открывает следующий sibling.
- Гейт критичен: первая наивная версия (безусловно убрать callout из break) дала
  **3 регрессии** (localization 0→60, cookbook 0→1999, java/index 0→2007) — там
  `<1>…\n+\ncont-para\n<2>…` (continuation-параграф в colist-item, затем sibling `<2>`):
  без гейта `<2>` поглощался в текст. blast поймал → добавлен `is_in_callout_list()`.

### Статус (верифицировано)
- clippy --workspace 0; test --workspace зелёное (parser 512→513, html 424→426);
  parsing-lab 233/233.
- table.adoc diffone: **597→37 diffs** (НЕ флип — остаток = вложенная `!===`-таблица).
- **Корпус: Identical 339 (БЕЗ флипа — корректное улучшение, прецедент: partnums)**.
  blast (base 339): table.adoc closer 597→37, **0 регрессий** (339→339).
- Тесты: +1 parser (`test_callout_marker_does_not_interrupt_top_level_paragraph`),
  +2 html (`test_callout_marker_top_level_paragraph_not_colist_html`,
  `test_callout_marker_inside_list_splits_items_html`).

### Что дальше
- **table.adoc флип = реализовать вложенную `!===`-таблицу** (Root 2, см. выше). Объём:
  протащить разделитель ячеек (`|`→`!`) через `scanner::parse_table_cells`/`unescape_cell_pipes`/
  colspan-парсинг; добавить `!` в `is_table_delimiter` (3+ `=`); `scan_table` детект `!`→
  separator `!` (формат Native/PSV, не Csv/Dsv). РИСК: `delimited.adoc` (сейчас Identical)
  содержит `!===` как СОДЕРЖИМОЕ ячейки `|===`-таблицы (строки 109-112: примеры делимитеров) —
  не сломать. Рендер вложенной таблицы УЖЕ работает (a-cell рекурсивный парс), нужны
  width% из `[cols="2,1"]` (ref: 66.6666%/33.3334%).
- Прочие nearmiss на 339: character-replacement-ref (m-колонка `<code>`-наследование),
  document-attributes-ref, syntax-quick-reference, outline — все архитектурные/мульти-root.
- **Pre-existing шире (НЕ трогал)**: unordered/ordered list-маркер (`*`/`.`) после строки
  параграфа БЕЗ blank — asciidoctor поглощает в параграф, мы прерываем → отдельный список.
  Широкое изменение (`*`/`.` очень частые), своя оценка регрессий нужна.

---

## Сессия (2026-06-14, шестьдесят первая) — Фаза 3: shorthand `,===`/`:===` + colgroup для format-таблиц

Запрос «продолжи». Ветка **`fix/csv-dsv-shorthand-and-colgroup`** — ЗАКОММИЧЕНА.
**НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на `git merge --no-ff` в
master + `git push origin master` + удаление ветки.** Старт: housekeeping 60-й закрыт
сам (мерж 60-й УЖЕ выполнен И запушен — origin/master == master == 82c9519 (338),
дерево чисто). base-бинарь /tmp/adoc_base пересобран из master HEAD (338).

### Выбор задачи
nearmiss на 338 (6 Different, весь «трудный хвост», ВСЕ архитектурные/мульти-root):
**data (181, Δ77)**, table (597, Δ1 — `|=== <1>` callout-суффикс → невалид-делимитер,
ДВА корня), character-replacement-ref (625, Δ113 — m-колонка `<code>`-наследование),
document-attributes-ref (953, Δ−3 — docyear/localyear интринсики [риск] + inline-в-link),
syntax-quick-reference (2788, мульти-root), outline (6647, Δ3, мульти-root spec).
Выбран **data** — корни связные (все про CSV/DSV-таблицы), ограниченная область.

### Реальная семантика (пробы asciidoctor)
data.adoc = 5 CSV/DSV-таблиц. ТРИ корня, все вокруг format-таблиц:
- **Root 1 (colgroup)**: asciidoctor эмитит `<colgroup>` с `<col>` на каждую колонку для
  ВСЕХ таблиц. Наш рендерер (blocks.rs:190) эмитит colgroup ТОЛЬКО при `cols` в meta.named.
  `scan_table` (native) синтезирует `cols` (block.rs:1828), `scan_delimited_format_table`
  (CSV/DSV/TSV) — НЕТ → format-таблицы без colgroup. (нормализатор compare срезает
  `style`, так что важно само наличие `<colgroup><col>…`).
- **Root 2 (shorthand)**: `,===` (CSV) / `:===` (DSV) НЕ распознавались → проза. Пробы:
  `,===`/`:===` рвут открытый параграф как `|===` (= полноценные делимитеры блока).
- **Root 3 (escaped include)**: `\include::customers.csv[]` в `,===` → УЖЕ работал
  (препроцессор снимает backslash → литерал `include::…[]` = одна CSV-ячейка).

### Что сделано (ПАРСЕР)
- **scanner.rs** `is_table_delimiter`: `|`-only → префиксы `|`/`,`/`:` (+ 3+ `=`, остаток
  всё `=`). `!===` НЕ парсится. Все 3 call-site (диспетч block.rs:1052 + 2 para-break
  2368/2759) получают единообразное поведение — корректно (shorthand = делимитер блока).
- **block.rs** `scan_table`: формат из первого байта `opening_delim` (`,`→Csv, `:`→Dsv,
  иначе `block_attrs.table_format()` — `|===` уважает `format=`). Закрытие по точному
  совпадению строки делимитера работает как было.
- **block.rs** `scan_delimited_format_table`: `block_attrs` → `mut`; после `num_cols`
  синтез `cols` (зеркало 1828) при отсутствии явного `cols=`.

### Статус (верифицировано)
- clippy --workspace 0; test --workspace зелёное (parser 510→512, html 423→424);
  parsing-lab 233/233.
- data diffone: **0 diffs** (был 181), len ref==our==210.
- **Корпус: Identical 338→339 (+1 ФЛИП, data 181→0)**. Blast (base 338): **РОВНО 1
  флип, 0 регрессий, 0 FARTHER**.
- Тесты: +2 parser (`test_csv_shorthand_delimiter_routes_to_format_and_synthesizes_cols`
  [`,===` → CSV + cols="2"], `test_dsv_shorthand_delimiter_routes_to_format` [`:===` → DSV]),
  +1 scanner (расширен `test_is_table_delimiter`: `,===`/`:====`/негативы `,==`/`:`/
  `:name: value`/`!===`), +1 html (`test_csv_dsv_shorthand_delimiter_and_colgroup_html`:
  `,===` 3-col colgroup+thead, `:===` DSV, single-field 100%-col без header).

### Что дальше
- nearmiss на 339 (5 Different, ВСЕ архитектурные/мульти-root): table (597, Δ1 —
  `|=== <1>` callout-суффикс делает строку невалид-делимитером → проза/литерал, ДВА корня),
  character-replacement-ref (625, Δ113 — m-колонка `<code>`-наследование, кластер),
  document-attributes-ref (953, Δ−3 — docyear/localyear date-интринсики [риск] +
  inline-в-link-тексте), syntax-quick-reference (2788, мульти-root), outline (6647, Δ3 —
  `\*` экранирование + `+` hard-break, мульти-root spec).
- Pre-existing — см. сессии 36/38/40/42/.../60.
- Известный clippy-warning (НЕ мой, pre-existing, только `--all-targets`): `concat!` в
  adoc-html/src/tests.rs. Гейт проекта `cargo clippy --workspace` чист.

---

## Сессия (2026-06-14, шестидесятая) — Фаза 3: счётчики литеральны в verbatim styled-параграфах и passthrough

Запрос «продолжи». Ветка **`fix/counter-verbatim-and-passthrough`** — ЗАКОММИЧЕНА
(`1907213`). **НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на
`git merge --no-ff` в master + `git push origin master` + удаление ветки.**
Старт: housekeeping 59-й закрыт сам (мерж 59-й УЖЕ выполнен И запушен —
origin/master == master == 0fd22fa (337), дерево чисто, ветки fix/* удалены).
base-бинарь /tmp/adoc_base пересобран из master HEAD (337) — изначально был 336
(align-by-cell `pages/` показывал ложный флип 371→0 — это фикс 59-й, уже в master;
NB: ДВА файла align-by-cell — `examples/` [base 0] и `pages/` [фикс 59-й]).

### Выбор задачи
nearmiss на 337 (7 Different, весь «трудный хвост»): **counters (136, Δ9)**, data
(181, Δ77 — CSV/DSV `,===` + colgroup, мульти-root), table (597, Δ1 — `|=== <1>`
callout-суффикс делает строку невалидным делимитером → текст параграфа; ДВА корня),
character-replacement-ref (625, Δ113 — m-колонка `<code>`-наследование, архитектурный
кластер), document-attributes-ref (953, Δ−3 — docyear/localyear date-интринсики
[рискованно] + inline-в-link-тексте), syntax-quick-reference (2788, мульти-root),
outline (6647, Δ3 — `\*` экранирование + `+` hard-break, мульти-root spec).
diffone counters: @38/@68/@77 изолированы, @142→@282 — ОДИН сплошной каскад
(промежуток @78-@141 идентичен → `====` example-блоки и `----` listing уже корректны).
Выбран counters — наименьший и сводится к ОДНОМУ концептуальному корню.

### Реальная семантика (проба counters.adoc + asciidoctor)
Счётчики `{counter:N}`/`{counter2:N}` резолвятся asciidoctor в `attributes`-субституции,
которой НЕТ в verbatim-контекстах и passthrough'ах. Препроцессор же раскрывал их
ВЕЗДЕ. ДВА под-корня (оба независимы — счётчики «name»/«seq1»/«pnum» раздельны):
- **Root A**: `[source]`/`[listing]`/`[literal]` styled-параграф (одиночный, БЕЗ `----`).
  Препроцессор УЖЕ скипал delimited verbatim-fences (`----`/`....`/`++++`/`////`), но НЕ
  styled-параграф. `{counter2:seq1}` в `[source]`-параграфе резолвился в ПУСТО → пустой
  блок дропался → каскад @142→@282 (поэтому 130+ из 136 diff'ов — ОДИН корень).
- **Root B**: inline-passthrough `+...+`/`++...++`/`+++...+++`/`pass:[]` (строки 12/21:
  `` `+{counter:name}+` ``). asciidoctor извлекает passthrough ДО attributes-субституции.

### Что сделано (ПАРСЕР)
- **Рефактор**: 4 passthrough-сканера (`pass_spec_len`, `pass_macro_span_len`,
  `passthrough_span_len`, `single_plus_span_len`) вынесены из `impl InlineState` в
  `scanner.rs` как stateless `pub fn` (они и были stateless — `(s,i)`, без self/'a;
  место по CLAUDE.md). inline.rs: 11 call-sites `Self::`→`crate::scanner::`. Байт-в-байт.
- **Root B** (preprocessor.rs `expand_counters`): скан по байтовому индексу `i` (вместо
  moving `rest`-слайса — чтобы `single_plus_span_len` видел РЕАЛЬНЫЙ предыдущий символ:
  `C+a+` НЕ passthrough); при `+`/`p` пробуем passthrough-span → копируем verbatim, НЕ
  раскрываем/НЕ инкрементим счётчик.
- **Root A** (preprocessor.rs `preprocess_with_attrs`): поля `verbatim_para_pending`/
  `in_verbatim_para`; секция 4a (после skip, перед fence-4b): pending взводится в секции 6
  после эмита verbatim-style attr-строки (`is_verbatim_style_attr_line` — first positional
  ∈ source/listing/literal, шортхенд `%`/`.`/`#`/space стрипается); следующая строка: если
  delimiter → fence (4b) handles, если blank → orphan, иначе → in_verbatim_para (строки
  untouched до blank). Зеркало delimited-fence-логики.

### Статус (верифицировано)
- clippy --workspace 0; test --workspace зелёное (parser 508→510, html 422→424, всего
  1026); parsing-lab 233/233.
- counters diffone: **0 diffs** (был 136), len ref==our==283.
- **Корпус: Identical 337→338 (+1 ФЛИП, counters 136→0)**. Blast (base 337,
  пересобран из master): **РОВНО 1 флип, 0 регрессий, 0 FARTHER**.
- Тесты: +2 parser (`test_counter_literal_inside_passthrough` [single/double/triple/pass +
  word-preceded `+` не span], `test_counter_literal_in_styled_verbatim_paragraph`
  [source/listing-параграф, multi-line, `[source]\n----` → fence, `[example]` НЕ verbatim]),
  +1 html (`test_counter_literal_in_styled_paragraph_and_passthrough_html`: source-параграф
  verbatim + counter2 не дропается + `+...+` литерал).

### Что дальше
- nearmiss на 338 (6 Different): data (181, Δ77 — CSV/DSV colgroup + строки, мульти-root),
  table (597, Δ1 — `|=== <1>` callout невалид-делимитер + ДВА корня), character-replacement-ref
  (625, Δ113 — m-колонка `<code>`-наследование), document-attributes-ref (953, Δ−3 —
  docyear/localyear date-интринсики [риск] + inline-в-link), syntax-quick-reference (2788,
  мульти-root), outline (6647, Δ3 — мульти-root spec). ВСЕ архитектурные/мульти-root.
- Pre-existing — см. сессии 36/38/40/42/43/44/45/46/47/48/49/50/51/52/53/54/55/56/57/58/59.
- Известный clippy-warning (НЕ мой, pre-existing, виден только `--all-targets`): `concat!`
  в adoc-html/src/tests.rs:2025. Гейт проекта `cargo clippy --workspace` чист.

---

## Сессия (2026-06-14, пятьдесят девятая) — Фаза 3: single-plus passthrough `+…+` охватывает backtick'и

Запрос «продолжи». Ветка **`fix/single-plus-passthrough-spans-backtick`** —
ЗАКОММИЧЕНА. **НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на
`git merge --no-ff` в master + `git push origin master` + удаление ветки.**
Старт: housekeeping 58-й закрыт сам (мерж 58-й УЖЕ выполнен И запушен —
origin/master == master == 4175727 (336), дерево чисто, ветки fix/* удалены;
session.md 58-й устарел — «ОЖИДАЕТ авторизации», но фактически смержено).
base-бинарь /tmp/adoc_base обновлён до 336 (master HEAD) ПЕРЕД фиксом.

### Выбор задачи
nearmiss на 336 (8 Different, все «трудный хвост»): counters (136 — мульти-root:
verbatim `{counter:}` + listing↔admonition + include'ы), data (181 — CSV/DSV),
**align-by-cell (371, Δ−16)**, table (597, ДВА корня), character-replacement-ref
(625), document-attributes-ref (953 — docyear/localyear интринсики + inline),
syntax-quick-reference (2788), outline (6647). diffone align-by-cell: ВСЕ 371 diff
идут ПОДРЯД с @153 = ОДИН каскадный рассинхрон, повторяется в строках 37/52/99 →
**single-root** (хоть и архитектурный). Прочие — мульти-root. Since Different=8 и все
здесь, флип требует полного закрытия файла → выбран single-root align-by-cell.

### Реальная семантика (проба /tmp/abc.adoc строки 37 vs asciidoctor)
- `` (`<n>+`) or duplication (`+<n>*+`), place the `+^+` `` → asciidoctor сворачивает
  ВСЁ в ОДИН `<code>&lt;n&gt;`) or duplication (`&lt;n&gt;*`), place the `^+</code>`.
- **Корень — порядок субституций asciidoctor**: single-plus passthrough `+…+`
  извлекается ГЛОБАЛЬНО ДО quotes/monospace, НЕжадно слева-направо, и контент МОЖЕТ
  включать backtick'и. Здесь `+…+` пары съедают внутренние backtick'и ₂₃₄₅ (как
  литералы), поэтому внешний `` ` `` матчится от ₁ до ₆. `<n>`→specialchars, `^+`
  литерал. (Модель проверена: реконструкция байт-в-байт == вывод asciidoctor.)
- Наш парсер посимвольный: `` `<n>+` `` сворачивался в отдельный `<code>` (backtick₂
  закрывал). Inner-reparse monospace (try_constrained:1224) УЖЕ корректно обрабатывает
  `+…+` как passthrough — нужно лишь научить СКАН закрывающего маркера пропускать
  single-plus регионы (как он уже пропускает `++`/`+++`/`pass:[]`).

### Что сделано (1 корень, ПАРСЕР inline.rs)
- Хелпер `single_plus_span_len(s, i)` — зеркало `try_single_plus_passthrough`: не
  `++`/`+++`; open `+` не после word-char НИ backslash (`` `\+` `` экранирован —
  span-cells регрессия!); контент-первый ≠ space; close `+` по constrained-правилу
  (не после `+`/space, не перед `+`/word); `pass:[]` внутри пропускается.
- `find_closing_constrained` И `find_closing_unconstrained`: ветка `b==b'+'` теперь
  пробует `passthrough_span_len` (++/+++), затем `single_plus_span_len` — пропуск
  региона. Симметрично существующему пропуску ++/+++/pass:.
- Тесты: +2 parser (`_spans_backtick` [a +`b`+ c→один code], `_escaped_plus_does_not_
  span_backtick` [`\+` + `n+`→два code]), +1 html (align-by-cell-строка + escaped guard).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 506→508, html 421→422);
  compat parsing-lab 233/233.
- align-by-cell diffone: **0 diffs** (был 371), len ref==our==550.
- span-cells: регрессия 0→2 (escaped `\+`) НАЙДЕНА blast'ом и ИСПРАВЛЕНА (backslash
  guard) → снова 0.
- **Корпус: Identical 336→337 (+1 ФЛИП, align-by-cell 371→0)**. Blast (base 336):
  РОВНО 1 флип; **0 регрессий, 0 FARTHER**.

### Что дальше
- nearmiss на 337 (7 Different): counters (136 — мульти-root: verbatim `{counter:}` НЕ
  резолвить [архитектурно, препроцессор резолвит в document-order до классификации
  verbatim] + `[source]`-параграф counter-ref + listing↔admonition + include'ы), data
  (181 — CSV/DSV `,===`, мульти-root), table (597, Δ1 — ДВА корня + огромный сдвиг),
  character-replacement-ref (625, Δ113), document-attributes-ref (953 — docyear/
  localyear интринсики [НЕдетерминированы от даты — рискованно] + inline @6257),
  syntax-quick-reference (2788, мульти-root), outline (6647, МУЛЬТИ-root spec).
- Pre-existing — см. сессии 36/38/40/42/43/44/45/46/47/48/49/50/51/52/53/54/55/56/57/58.

---

## Сессия (2026-06-14, пятьдесят восьмая) — Фаза 3: ведущий край smart-quote подавляет constrained mono/em/mark

Запрос «продолжи». Ветка **`fix/curved-quote-double-backtick-literal`** —
ЗАКОММИЧЕНА (`1c5e8b3`). **НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на
`git merge --no-ff` в master + `git push origin master` + удаление ветки.**
Старт: housekeeping 57-й закрыт сам (мерж 57-й УЖЕ выполнен И запушен —
origin/master == master == 78f2273 (335), дерево чисто, ветки fix/* удалены;
session.md 57-й устарел — там «ОЖИДАЕТ авторизации», но фактически смержено).
base-бинарь /tmp/adoc_base обновлён до 335 (master HEAD) ПЕРЕД фиксом.

### Выбор задачи
nearmiss на 335 (9 Different, все «трудный хвост»): counters (136, Δ9 — мульти-root:
verbatim `{counter:}` НЕ должен резолвиться + listing мис-классифицирован как
admonition-warning), data (181, Δ77), **troubleshoot (212, Δ−4)**, align-by-cell
(371, Δ−16), table (597, Δ1 — ДВА корня), character-replacement-ref (625, Δ113),
document-attributes-ref (953), syntax-quick-reference (2788), outline (6647).
diffone troubleshoot: первый diff @366 из 588 (62% идентично). Хвост @366→ —
ЧИСТЫЙ позиционный сдвиг +4 (наш `<code>…</code>` = 5 токенов вместо 1 текстового;
len our 588 = ref 584 + 4, ровно одна вставка). **Single-root** — line 83.

### Реальная семантика (пробы /tmp/mono_open,edge2,tb_probe vs asciidoctor)
- Конструкция `"``end points``"` (двойной backtick внутри curved-quote маркеров):
  asciidoctor → `“`end points`”` (внутренние одинарные backtick ЛИТЕРАЛЬНЫ, НЕ
  monospace). Мы → `“<code>end points</code>”` («на пару backtick впереди»).
  `"`x`"` (одинарный) → `“x”`; `"```x```"` (тройной) → `“<code>x</code>”`.
- **Корень — порядок QUOTE_SUBS asciidoctor**: `:double`/`:single` (curved-quote
  `"`…`"`/`'`…`'`) идут ПОСЛЕ `:strong constrained` (`*`) но ПЕРЕД
  `:monospaced/:emphasis/:mark constrained` (`` ` ``/`_`/`#`). На ведущем крае
  span'а monospace/em/mark видят `;` от выведенного `&#8220;`/`&#8216;` → их
  open-ассерт `(^|[^\w;:…])` падает → литерал. Strong уже сматчился ПРОТИВ
  исходного backtick (open-класс strong `[^\w;:}]` `` ` `` разрешает) → открывается.
  Unconstrained (`**`/`` `` ``/`__`/`##`) и super/sub (`^`/`~`) open-ассерта НЕ
  имеют → открываются всегда (тройной → inner `` ``…`` `` unconstrained → `<code>`).
- Пробы подтвердили: `"`*bold*`"`→strong, `"`_em_`"`→литерал, `"`#mk#`"`→литерал,
  `"`^x^`"`→`<sup>`, `"`~x~`"`→`<sub>`, `"`**b**`"`→strong, mid `"`a `c` b`"`→`<code>`.

### Что сделано (1 точка в ПАРСЕРЕ inline.rs)
- Поле `InlineState.smart_quote_leading_edge` (default false; true только для
  inner-рерана в `try_smart_quotes`).
- Гейт в `try_constrained` (после `is_word_char_before`): `flag && start_pos == 0
  && matches!(marker, b'`'|b'_'|b'#')` → return false. `*` (strong) НЕ блокируется
  (идёт до `:double`), unconstrained/super-sub не проходят через try_constrained.
- Доком-коммент на поле и на гейт (порядок субституций — почему именно так).
- Тесты: +3 parser (double-backtick inner literal, em/mark edge literal,
  suppression-leading-only [mid-content `<code>` сохраняется]), +1 html
  (double vs triple, em-edge vs strong-edge).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 503→506, html 420→421);
  compat parsing-lab 233/233.
- troubleshoot diffone: **0 diffs** (был 212), len ref==our==584.
- **Корпус: Identical 335→336 (+1 ФЛИП, troubleshoot 212→0)**. Blast (base 335):
  РОВНО 1 флип; **0 регрессий, 0 FARTHER**. Попутно фиксит латентные em/mark
  edge-кейсы (не в корпусе, но к asciidoctor-паритету).

### Что дальше
- nearmiss на 336 (8 Different): counters (136, Δ9 — мульти-root), data (181, Δ77 —
  CSV/DSV `,===`, мульти-root), align-by-cell (371, Δ−16 — inline `<n>`/`^+` в
  backtick), table (597, Δ1 — ДВА корня + огромный сдвиг), character-replacement-ref
  (625, Δ113), document-attributes-ref (953, Δ−3), syntax-quick-reference (2788,
  Δ−31 — мульти-root), outline (6647, Δ3 — МУЛЬТИ-root spec).
- Pre-existing идея (НЕ в корпусе): smart-quote `"`…`"` open-диспетч не проверяет
  word-границу перед `"` (`a"`code`"b` → должен быть литерал — constrained).
- Pre-existing — см. сессии 36/38/40/42/43/44/45/46/47/48/49/50/51/52/53/54/55/56/57.

---

## Сессия (2026-06-14, пятьдесят седьмая) — Фаза 3: monospace close-граница `` `' ``, sup/sub субституции, bare-word role-span

Запрос «продолжи». Ветка **`fix/monospace-close-boundary-quote-tick`** —
ЗАКОММИЧЕНА (`c1d183a`). **НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на
`git merge --no-ff` в master + `git push origin master` + удаление ветки.**
Старт: housekeeping 56-й закрыт сам (мерж 56-й УЖЕ выполнен И запушен —
origin/master == master == 088c30d (334), дерево чисто, ветки fix/* удалены).
base-бинарь /tmp/adoc_base обновлён до 334 (master HEAD) ПЕРЕД фиксом.

### Выбор задачи
nearmiss на 334 (10 Different). Кандидаты с малым |Δ|: table (Δ1, ДВА корня +
огромный сдвиг), troubleshoot (Δ−4, вложенные backtick→литерал, архитектурно),
**text (249, Δ−5)**. diffone text: первый diff @459 (apos-блок) — ВСЁ до него
идентично. Оказался ТРИ корня в одном файле, ВСЕ нужны для флипа.

### Реальная семантика (исходник asciidoctor.rb QUOTE_SUBS/REPLACEMENTS + пробы /tmp/apostest)
- **Корень A — monospace close-граница**: constrained monospace `` `…` `` имеет
  более строгое закрытие чем прочие quotes — `(?![\w"'`])`: закрывающий backtick
  НЕ может сопровождаться `"`, `'` ИЛИ `` ` ``. Без этого `` `' `` (backtick+апостроф
  = типографский правый апостроф `’`, REPLACEMENTS строка 504 `[/\\?`'/, '&#8217;']`)
  ошибочно матчится как закрытие monospace: `` the `'00s … werewolves`' `` сворачивало
  4 строки в `<code>`. Пробы: `` `'00s and werewolves`' ``→два `’`; `` `code`' ``→`` `code’ ``
  (первый backtick литерал); `` `bar`" ``→литерал. Одиночный `` `' `` у нас УЖЕ
  работал (apply_typographic_replacements строка 122).
- **Корень B — sup/sub субституции**: superscript/subscript `^…^`/`~…~` (unconstrained,
  `\S+?`) получают ПОЛНУЮ normal-группу (attributes/quotes/replacements/macros):
  `^a{sp}b^`→`<sup>a b</sup>`, `^*z*^`→`<sup><strong>`, `^a--b^`→em-dash, `^url[t]^`→link,
  `^(C)^`→©. Наш `try_simple_pair` эмитил сырой `Event::Text` — `{sp}` не резолвился.
- **Корень C — bare-word role-span**: `parse_quoted_text_attributes` — attrlist без
  `.`/`#` шортхенда берётся ВЕРБАТИМ как одна роль (`{role => str}`): `[big]##O##`→
  `<span class="big">`, `[a.b]##x##`→role "a.b" (точки НЕ делятся, в отличие от
  shorthand `[.a.b]`→"a b"). Только первый позиционный (split по `,`: `[r1,r2]`→r1).
  **Constrained** `[role]#x#` требует opening word-границу (`word[role]#x#`→литерал;
  `[big]#O#nce`→литерал — close перед word-символом); **unconstrained** `##…##` может
  mid-word (`hel[x]##lo##rld`→span). Наш диспетч триггерил attr-span только на `[.`/`[#`.

### Что сделано (3 точки в ПАРСЕРЕ inline.rs)
- `try_constrained`: monospace-специфичный close-чек (`marker == b'`'` && after_close
  ∈ `"'``) → return false.
- `try_simple_pair` (sup/sub): `Event::Text(inner)` → рекурсивный реран
  `InlineState::new(inner, self.subs, self.options).parse_inline`.
- Диспетч attr-span (@551): гейт расширен на bare-word (peek(1) alnum/`_`, не только
  `.`/`#`); `try_inline_attr_span` — первый позиционный (split `,` + trim), bare-word →
  одна роль вербатим (без split по `.`); `is_word_char_before` перенесён в
  CONSTRAINED-ветку (unconstrained mid-word сохраняется).
- Тесты: +3 parser (обновлён `test_non_shorthand_bracket_not_span`→`_is_role_span`
  [кодировал баг], +`_not_split_on_dot`, +`_rejected_after_word_char`), +3 html
  (bareword-role, backtick-apostrophe, superscript-full-subs).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 501→503, html 417→420);
  compat parsing-lab 233/233.
- text.adoc diffone: **0 diffs** (был 249). Все пробы apostest IDENTICAL.
- **Корпус: Identical 334→335 (+1 ФЛИП, text.adoc 249→0)**. Blast (base 334): РОВНО
  1 флип; бонус document-attributes-ref 6363→953 closer (фиксы B/C применимы шире);
  **0 регрессий, 0 FARTHER**.

### Что дальше
- nearmiss на 335 (9 Different): counters (136, Δ9 — мульти-root: verbatim `{counter:}`
  АРХИТЕКТУРНО + listing мис-классифицирован как admonition-warning @142), data
  (181, Δ77 — CSV/DSV `,===`, мульти-root), troubleshoot-unconstrained-formatting
  (212, Δ−4 — вложенные backtick→литерал, архитектурно), align-by-cell (371, Δ−16 —
  inline `<n>`/`^+` в backtick), table (597, Δ1 — ДВА корня: `|=== <1>` не точный
  делимитер + callout-list-item рвёт параграф + огромный сдвиг),
  character-replacement-ref (625, Δ113), syntax-quick-reference (2788, Δ−31 —
  мульти-root), document-attributes-ref (953, Δ — было 6363, ОСТАЛИСЬ корни),
  outline (6647, Δ — МУЛЬТИ-root).
- Pre-existing — см. сессии 36/38/40/42/43/44/45/46/47/48/49/50/51/52/53/54/55/56 (без изменений).

---

## Сессия (2026-06-14, пятьдесят шестая) — Фаза 3: double-plus passthrough `++…++` применяет specialchars (экранирует `<>&`), не raw

Запрос «продолжи». Ветка **`fix/double-plus-passthrough-specialchars`** —
ЗАКОММИЧЕНА (`1d2d6e8`). **НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на
`git merge --no-ff` в master + `git push origin master` + удаление ветки.**
Старт: housekeeping 55-й закрыт сам (мерж 55-й УЖЕ выполнен И запушен —
origin/master == master == 9d51b71 (333), дерево чисто, ветки fix/* удалены).
base-бинарь /tmp/adoc_base обновлён до 333 (master HEAD) ПЕРЕД фиксом.

### Выбор задачи
nearmiss на 333 (11 Different): кандидаты с малым |len_delta| — block-name-table
(431, Δ−2), table (597, Δ1 — ДВА корня). diffone block-name-table @95: эталон
`[<LABEL>]` как ОДИН текст-токен внутри `<code>`, наш `[`, `<label>` (РЕАЛЬНЫЙ
HTML-тег!), `]` — мы выводили `<LABEL>` НЕ экранированным. Single-root.

### Реальная семантика (пробы /tmp/pass_probe,pass_probe2,pp3 vs asciidoctor)
- **`++…++` (double-plus, unconstrained) применяет ТОЛЬКО `specialcharacters`** —
  экранирует `<`/`>`/`&`, как `+…+` (single). `+++…+++` (triple) и `pass:[]` (без
  spec) — raw, без субституций. Пробы: `++[<LABEL>]++`→`[&lt;LABEL&gt;]`,
  `++a & b++`→`a &amp; b`, `+++[<LABEL>]+++`→`[<LABEL>]` (сырой).
- **НЕ применяются** quotes/replacements/attributes/inline-репарсинг: `++*x*++`→`*x*`,
  `++a -- b++`→`a -- b`, `++{foo}++`→`{foo}`. Работает mid-word (`a++bc++d`→`abcd`),
  пустой `++++`→ничего. Все эти случаи у нас УЖЕ совпадали.

### Что сделано (1 точка в парсере)
- **ПАРСЕР** inline.rs `try_double_plus_passthrough`: `Event::InlinePassthrough`
  (raw) → `Event::Text` (рендерер html-экранирует). Триггерит ровно specialchars,
  без реран субституций (Text — уже-распарсенный leaf, рендерер только экранирует).
  Triple-plus остался `InlinePassthrough` (raw). Док-коммент.
- Тесты: +1 parser (`test_double_plus_passthrough_applies_specialchars`), +1 html
  (`test_double_plus_passthrough_escapes_specialchars_html`); 2 parser-теста
  обновлены (`test_passthrough_inside_monospace`, `test_pass_macro_inside_single_plus`
  — кодировали старый `InlinePassthrough` для double-plus → теперь `Text`; backtick/
  `pass:[y]` не содержат `<>&`, остаются литералом).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 500→501, html 416→417);
  compat parsing-lab 233/233.
- block-name-table diffone: **0 diffs** (был 431). Все пробы IDENTICAL.
- **Корпус: Identical 333→334 (+1 ФЛИП)**. Blast (base 333): РОВНО 1 флип —
  block-name-table 431→0; **0 регрессий**. outline.adoc «FARTHER» 6586→6647 (+61) —
  **АРТЕФАКТ нормализатора, НЕ регрессия**: единственная изменённая строка
  (page-break `` `++<<<++` ``) теперь `<code>&lt;&lt;&lt;</code>` БАЙТ-В-БАЙТ с
  asciidoctor (было `<code><<<</code>` — невалидный HTML). Доказано: нормализатор
  токенизирует сырой `<<<` как `'<','<','<'` (≠ эталон `'<<<'`), а `&lt;&lt;&lt;` →
  `'<<<'` (== эталон, `new==ref: True`). 2-токенный сдвиг переразложил позиционное
  выравнивание гигантского мульти-root spec → счётчик ВЫРОС, хотя строка стала верна.

### Что дальше
- nearmiss на 334 (10 Different): counters (136, Δ9 — АРХИТЕКТУРНЫЙ verbatim
  `{counter:}`), data (181, Δ77 — CSV/DSV `,===`, мульти-root),
  troubleshoot-unconstrained-formatting (212, Δ−4 — nested-backtick, архитектурно),
  text (249, Δ−5 — `+ +` hard-break в monospace + apostrophe NCR), align-by-cell
  (371, Δ−16 — inline `<n>`/`^+` в backtick), table (597, Δ1 — ДВА корня:
  `|=== <1>` не точный делимитер + callout-list-item рвёт параграф),
  character-replacement-ref (625, Δ113), syntax-quick-reference (2788, Δ−31 —
  мульти-root), document-attributes-ref (6363, Δ73 — мульти-root), outline (6647,
  Δ — МУЛЬТИ-root).
- Pre-existing — см. сессии 36/38/40/42/43/44/45/46/47/48/49/50/51/52/53/54/55 (без изменений).

---

## Сессия (2026-06-14, пятьдесят пятая) — Фаза 3: list-item принципиальный `<p>` — literal-параграф закрывает его, пустой принципал держит `<p></p>`

Запрос «продолжи». Ветка **`fix/list-item-principal-p-empty-and-literal`** —
ЗАКОММИЧЕНА (`36d9642`). **НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на
`git merge --no-ff` в master + `git push origin master` + удаление ветки.**
Старт: housekeeping 54-й закрыт сам (мерж 54-й УЖЕ выполнен И запушен —
origin/master == master == 47fe571, дерево чисто, ветки fix/* удалены). base-бинарь
/tmp/adoc_base обновлён до 332 (master HEAD) ПЕРЕД фиксом — корректная база для blast.

### Выбор задачи
nearmiss на 332 (12 Different): replacements закрыт 54-й. Ближайший — **complex
(120, Δ4)** — заметки 54-й: ДВА корня, оба нужны для флипа. Подтверждено diffone +
эмпирикой на asciidoctor (пробы pA1/pA2/pB1..pB4 в /tmp).

### Реальная семантика (пробы /tmp/pA1,pA2,pB1,pB2,pB3,pB4 vs asciidoctor)
- **Корень A** (b-complex, ` $ cmd` literal-параграф в list-item БЕЗ `+`): отступный
  literal-параграф = ОТДЕЛЬНЫЙ блок; asciidoctor закрывает принципиальный `</p>`
  ПЕРЕД `<div class="literalblock">`. Наш guard закрытия `<p>` при старте суб-блока
  НЕ включал `Tag::LiteralParagraph` → literalblock вкладывался в открытый `<p>`,
  `</p>` закрывался ПОСЛЕ. (Путь через `+`-continuation для `----`/listing закрывал
  верно — pB3 OK; баг только для literal-параграфа через отступ.)
- **Корень B** (complex-only, `. {empty}` + `+` + listing): обычный list-item
  (olist/ulist/colist) с ПУСТЫМ принципалом + присоединённый блок — asciidoctor
  ВСЕГДА оборачивает принципал (`<p></p>`), даже пустой. Это ПРОТИВОПОЛОЖНО dd:
  `convert_dlist` эмитит `<p>` только при `dd.text?` (откатывает пустой). Наш
  откат пустого `<p>` (введён для empty-dd, сессия 2026-06-13) срабатывал для ВСЕХ
  list-контекстов → `. {empty}`+блок терял `<p></p>`. (pB2 `. {empty}` БЕЗ блока —
  у нас уже верно `<p></p>`; баг только при наличии присоединённого блока.)

### Что сделано (оба корня — в guard'е events.rs start_tag @366)
- **РЕНДЕРЕР** lib.rs: новый enum **`LiPara { OpenItem, OpenDd, Closed }`** (+ метод
  `is_open()`) заменил `li_p_open: Vec<bool>`. Дискриминатор «item vs dd» нужен
  только в guard'е, но обновлены все push/pop-сайты: dd-push (events.rs ×3 стиля) →
  `OpenDd`; open_li_paragraph (blocks.rs, regular item + callout) → `OpenItem`;
  все pop-сравнения `== Some(true)` → `.is_some_and(LiPara::is_open)`.
- **РЕНДЕРЕР** events.rs guard: (A) добавлен `Tag::LiteralParagraph` в match-список
  тегов суб-блока; (B) откат пустого `<p>` (`truncate`) теперь ТОЛЬКО при
  `is_dd && ends_with("<p>")`; иначе (item пустой ИЛИ непустой любой) → `</p>\n`
  (даёт `<p></p>` для пустого item). `last_mut = LiPara::Closed`.
- Тесты: +2 html (`test_list_item_literal_paragraph_closes_principal_p_html`,
  `test_list_item_empty_principal_keeps_p_with_block_html`). Существующий
  `test_dd_empty_principal_with_attached_block_no_paragraph_html` (негатив корня B —
  dd-откат сохранён) проходит.

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 500, html 414→416);
  compat parsing-lab 233/233.
- complex diffone: **0 diffs** (был 120). Пробы pA1/pA2/pB1/pB2/pB3/pB4 — все IDENTICAL.
- **Корпус: Identical 332→333 (+1 ФЛИП)**. Blast (base 332): РОВНО 1 флип —
  complex 120→0; outline closer 6587→6586 (мульти-root spec, тот же паттерн где-то);
  **0 регрессий, 0 FARTHER**.

### Что дальше
- nearmiss на 333 (11 Different): counters (136, Δ9 — АРХИТЕКТУРНЫЙ verbatim
  `{counter:}`), data (181, Δ77 — CSV/DSV `,===`, мульти-root),
  troubleshoot-unconstrained-formatting (212, Δ−4 — nested-backtick, архитектурно),
  text (249, Δ−5 — `+ +` hard-break в monospace + apostrophe NCR), align-by-cell
  (371, Δ−16 — inline `<n>`/`^+` в backtick), block-name-table (431, Δ−2 — `++…++`
  escape), table (597, Δ1 — ДВА корня), character-replacement-ref (625, Δ113),
  syntax-quick-reference (2788, Δ−31 — мульти-root), document-attributes-ref (6363,
  Δ73 — мульти-root), outline (6586, Δ1 — МУЛЬТИ-root).
- Pre-existing — см. сессии 36/38/40/42/43/44/45/46/47/48/49/50/51/52/53/54 (без изменений).

---

## Сессия (2026-06-14, пятьдесят четвёртая) — Фаза 3: monospace `` `text` `` получает полную normal-группу subs (replacements + char-ref restore)

Запрос «продолжи». Ветка **`fix/monospace-replacements-subs`** — ЗАКОММИЧЕНА
(`bcb9ed5`). **НЕ смержена, НЕ запушена — ОЖИДАЕТ явной авторизации на
`git merge --no-ff` в master + `git push origin master` + удаление ветки.**
Старт: housekeeping 53-й закрыт сам (origin/master == master == 9a8b30e, дерево
чисто, ветки fix/* удалены — merge+push 53-й прошли). base-бинарь /tmp/adoc_base
на 331 (master HEAD); обновить до 332 ПОСЛЕ авторизации мержа.

### Выбор задачи
nearmiss на 331 (13 Different): **replacements (4, Δ0)** — заметки 53-й помечали
«NCR, скип» по инерции, но это НЕ типографический фон (`'`/`"`). Здесь asciidoctor
выдаёт литеральный `§`/`#`/`@`, мы — NCR `&#167;`/`&#35;`/`&#64;`. diffone-нормализатор
декодирует entity на обеих сторонах: значит asciidoctor выдаёт ВАЛИДНУЮ entity
`&#167;` (→`§`), а мы экранируем `&`→`&amp;`, ломая reference. 4 diff'а, все char-ref
в monospace `` `&#167;` `` → чистый single-root.

### Реальная семантика (исходник substitutors.rb + REPLACEMENTS-таблица + пробы)
- **Constrained/unconstrained monospace `` `text` `` получает ПОЛНУЮ normal-группу
  subs** (specialchars, quotes, attributes, **replacements**, macros, post_repl) —
  как проза. Asciidoctor применяет `(C)`→©, `--`→em-dash, `...`→ellipsis И ВОССТАНАВЛИВАЕТ
  char-refs ВНУТРИ `<code>`. Наш код хардкодил «monospace literal — no replacements»
  (`self.subs.without(REPLACEMENTS)`) — заблуждение. Char-ref restore — последнее
  правило REPLACEMENTS-таблицы: specialchars экранирует `&#167;`→`&amp;#167;`, потом
  `replacements` через `:bounding` восстанавливает `&` (тело: named `[A-Za-z][A-Za-z]+\d{0,2}`,
  decimal `#`+2-6 цифр, hex `#x`+2-5). Литеральный passthrough `` `+...+` ``/`pass:[]`
  перехватывается раньше → остаётся verbatim независимо от subs.
- **Спейс-em-dash `(^|\n| |\\)--( |\n|$)` анкорится на КРАЯХ СТРОКИ.** Asciidoctor
  гоняет replacements ПОСЛЕ обёртки в `<code>`, поэтому `--` на крае спана ограничен
  символами тега `>`/`<`, НЕ `^`/`$` → остаётся литералом (`` `--` `` → `<code>--</code>`).
  `a -- b`/`x--y` (внутренние границы) → em-dash как обычно.

### Что сделано
- **ПАРСЕР** inline.rs `try_constrained`/`try_unconstrained`: убран
  `.without(REPLACEMENTS)` для backtick (оба сайта) — monospace репарсится с полной
  `self.subs`.
- **ПАРСЕР** inline.rs: поле `InlineState.edges_are_line_boundaries` (true ТОЛЬКО для
  top-level текста @221, default false для inner-репарсинга спанов). `flush_text`
  вычисляет `left/right_is_boundary` (`start != 0 || edges...` / `end < len || edges...`)
  и передаёт в `apply_typographic_replacements` (новые параметры). Спейс-em-dash
  правило (единственное край-зависимое) трактует край-flush как границу КРОМЕ истинного
  края input не-строки (= край спана). Mid-input края = legacy «граница» → `{empty}--{empty}`
  (пустой attr-ref) даёт em-dash на крае строки ячейки.
- Тесты: +2 parser (`test_monospace_applies_replacements`,
  `test_monospace_edge_em_dash_stays_literal`), +1 html
  (`test_monospace_replacements_and_char_refs_html`).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 500 +2, html 414 +1);
  compat parsing-lab 233/233.
- replacements diffone: **0 diffs** (был 4). Пробы char-ref/replacements/passthrough/
  все --- случаи (standalone/spaced/word/lead/trail) совпали с asciidoctor.
- **Корпус: Identical 331→332 (+1 ФЛИП)**. Blast (base 331): РОВНО 1 файл —
  replacements 4→0, **0 регрессий**. (Промежуточно ловились 2 регрессии:
  hard-line-breaks/sdr-001 `` `--` `` → em-dash [исправлено флагом границ]; затем
  subs-symbol-repl `{empty}--{empty}` → литерал [исправлено: mid-input края = граница].)

### Что дальше
- nearmiss на 332 (12 Different): **complex (120, Δ4 — ДВА корня в lists/examples:
  (A) literal-параграф ` $ cmd` в list-item БЕЗ `+` — asciidoctor закрывает `</p>`
  ДО literalblock, мы держим открытым; (B) пустой `<p></p>` перед listingblock —
  asciidoctor эмитит, мы опускаем. Оба нужны для флипа)**, counters (136, Δ9 —
  АРХИТЕКТУРНЫЙ verbatim `{counter:}`), data (181, Δ77 — CSV/DSV `,===`, мульти-root),
  troubleshoot-unconstrained-formatting (212, Δ−4 — nested-backtick, архитектурно),
  text (249, Δ−5 — `+ +` hard-break в monospace + apostrophe NCR), align-by-cell
  (371, Δ−16 — inline `<n>`/`^+` в backtick), block-name-table (431, Δ−2 — `++…++`
  escape), table (597, Δ1 — ДВА корня), character-replacement-ref (625, Δ113),
  syntax-quick-reference (2788, Δ−31 — мульти-root), document-attributes-ref (6363,
  Δ73 — мульти-root), outline (6587, Δ1 — МУЛЬТИ-root).
- **Латентный (НЕ регрессия, pre-existing, обнажён анализом)**: top-level
  intermediate-flush после ФОРМАТНОГО маркера трактуется как граница (`foo*b*-- c`
  → em-dash, asciidoctor литерал, т.к. `>` перед `--`). flush_text не знает тип
  конструкции, ограничивающей run; `}` (attr-ref) должен быть прозрачным, `>` (тег) —
  нет. Оставлен legacy (нет корпусного кейса).
- Pre-existing — см. сессии 36/38/40/42/43/44/45/46/47/48/49/50/51/52/53 (без изменений).

---

## Сессия (2026-06-14, пятьдесят третья) — Фаза 3: table-делимитер 3+ `=` + директивы на колонке 0 + verbatim `indent`

Запрос «продолжи». Ветка **`fix/table-delim-length-verbatim-indent`** —
ЗАКОММИЧЕНА (`a791b55`). **MERGE в master ОТКЛОНЁН авто-классификатором** (коммит
на master без явной авторизации) — master ЛОКАЛЬНО на 2607545 (нетронут), ветка
СОХРАНЕНА, base-бинарь /tmp/adoc_base обновлён до 331. **ОЖИДАЕТ: явная
авторизация на `git merge --no-ff` в master + `git push origin master` + удаление
ветки.** Старт: housekeeping 52-й закрыт сам (origin/master == master == 2607545,
дерево чисто, ветки fix/* удалены — пуш 52-й прошёл).

### Выбор задачи
nearmiss на 330 (14 Different): replacements (4 — NCR, скип). Выбран **image-size
(177, Δ92)** — заметки 52-й ОШИБОЧНО называли «контекстный корень выше строки 99,
изолированно OK». Перепроверка: таблица `|====` (строки 99-125) НЕ парсится даже в
ПОЛНОЙ изоляции (и без title). Заметка была неверна.

### Реальная семантика (исходник parser.rb is_delimited_block?/adjust_indentation! + пробы)
ТРИ независимых корня, все в verbatim/table-парсинге:
- **Table-делимитер = `|` + 3+ `=`** (не ровно `|===`). `is_delimited_block?`:
  tip=первые 4 символа, `uniform?` проверяет что весь хвост после `|` = `=`.
  Минимум `|===` (3 `=`), `|==` (2) НЕ делимитер. Закрытие — по ТОЧНОЙ строке
  открытия (не любой делимитер): `|====` внутри `|===`-таблицы = ячейка `====`
  (delimited.adoc — иначе таблица рвётся преждевременно; это и была мгновенная
  регрессия при наивном расширении). `open 4 close 3` у asciidoctor «работает»
  лишь дочитыванием до EOF.
- **Условные директивы (`ifdef`/`ifndef`/`ifeval`/`endif`) — только колонка 0.**
  Отступленная ` ifdef::...` = литерал (так авторы держат директивы verbatim
  в listing). Колонка-0 директива ВНУТРИ listing ВСЁ ЕЩЁ обрабатывается
  (reader-level, проба `:x: 1` выживает). [Пре-существующее, НЕ чинил: наш парсер
  не определяет `backend-html5` как атрибут по умолчанию → col-0 `ifdef::backend-html5[]`
  ложно-false; image-size не затронут, т.к. там все директивы с пробелом.]
- **`indent` атрибут verbatim-блоков** (`adjust_indentation!`): `indent=0` срезает
  общий ведущий отступ (min по НЕпустым строкам; отменяется если хоть одна непустая
  строка flush-left), `indent=N` заменяет на N пробелов, отсутствие/негатив —
  preserve. `indent=0` — zero-copy суффиксный срез; только N>0 аллоцирует.

### Что сделано
- **ПАРСЕР** scanner.rs `is_table_delimiter`: `== "|==="` → `strip_prefix('|')` +
  `len>=3 && all(=='=')`. Тест обновлён (`|====`/`|==========` valid, `|==`/`|`
  /`|=== x` нет).
- **ПАРСЕР** block.rs `scan_table`: захват `opening_delim`, закрытие по
  `line.trim() == opening_delim` (не `is_table_delimiter`).
- **ПАРСЕР** preprocessor.rs: гейт `at_column_0 = !line.starts_with([' ','\t'])`
  на endif-чек и `parse_conditional`. +2 теста.
- **ПАРСЕР** attributes.rs: `verbatim_indent() -> Option<i32>` (Ruby-to_i: знак +
  ведущие цифры).
- **ПАРСЕР** block.rs: свободные fn `reindent_verbatim_lines` (алгоритм
  adjust_indentation) + `resolve_callouts_in_lines` (общий callout-strip+resolve,
  принимает/возвращает `Cow`); `push_callout_events_resolved` теперь берёт
  `Cow<'a,str>`. Обе verbatim-функции (scan_source_block + ветка is_verbatim)
  переведены на reindent+helper (дедупликация callout-логики).
- Тесты: +4 html (table 4+ `=`, exact-match терминатор, verbatim indent
  0/3/preserve/flush-left, listing+indented-ifdef-литерал).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 496, html 413 +4);
  compat parsing-lab 233/233.
- image-size diffone: **0 diffs** (был 177). Все indent/делимитер/директива-пробы
  совпали с asciidoctor.
- **Корпус: Identical 330→331 (+1 ФЛИП)**. Blast (base 330): РОВНО 1 файл —
  image-size 177→0, **0 регрессий, 0 затронутых других** (промежуточная регрессия
  delimited.adoc 0→296 от наивного расширения делимитера устранена exact-match
  терминатором).

### Что дальше
- nearmiss на 331 (13 Different): replacements (4 — NCR, скип), **complex (120,
  Δ4 — ДВА корня в lists/examples: (A) literal-параграф ` $ cmd` присоединён к
  list-item БЕЗ `+` — `</p>` держим открытым через literalblock, asciidoctor
  закрывает ДО; (B) `. {empty}` ordered-элемент с пустым принципалом → asciidoctor
  эмитит `<p></p>`, мы опускаем (каскад −2 токена через хвост). Оба нишевые, нужны
  ОБА для флипа)**, counters (136 — АРХИТЕКТУРНЫЙ verbatim `{counter:}`), data
  (181, Δ77 — CSV/DSV `,===`/`:===`/`!===` НЕ парсятся, мульти-root),
  troubleshoot-unconstrained-formatting (212, Δ−4 — nested-backtick, архитектурно),
  text (249, Δ−5 — `+ +` hard-break в monospace + apostrophe NCR), align-by-cell
  (371, Δ−16 — inline `<n>`/`^+` в backtick), block-name-table (431, Δ−2 —
  `++…++` escape), table (597, Δ1 — ДВА корня), character-replacement-ref (625,
  Δ113), syntax-quick-reference (2788, Δ−31 — мульти-root), document-attributes-ref
  (6363, Δ73 — мульти-root), outline (6587, Δ1 — МУЛЬТИ-root).
- **Пре-существующее (всплыло, НЕ чинил)**: `backend-html5`/`backend-pdf`/etc. не
  заданы как doc-атрибуты по умолчанию → col-0 `ifdef::backend-html5[]` оценивается
  ложно-false (asciidoctor задаёт их по backend). Кандидат если найдётся корпусный
  кейс.
- Pre-existing — см. сессии 36/38/40/42/43/44/45/46/47/48/49/50/51/52 (без изменений).

---

## Сессия (2026-06-13, пятьдесят вторая) — Фаза 3: table frame/grid классы + interactive SVG → `<object>`

Запрос «продолжи». Ветка **`fix/image-svg-frame-grid-and-interactive-svg`**
(переименована из `fix/table-frame-grid-classes` после расширения скоупа) —
ЗАКОММИЧЕНА (`5b9da4f`), смержена в master (`533d12e`, --no-ff). base-бинарь
/tmp/adoc_base обновлён до 330. **ОЖИДАЕТ: явная авторизация на `git push origin
master` + удаление ветки** (пуш — outward-facing). Старт: housekeeping 51-й
закрыт сам (origin/master == master == 88599d1, дерево чисто, ветки fix/* удалены
— пуш 51-й прошёл).

### Выбор задачи
nearmiss на 329 (15 Different): replacements (4 — NCR, скип). Прогнал diffone по
кандидатам: complex (120, Δ4 — МУЛЬТИ-root: literal-параграф в list-item `</p>`-
перестановка + `{empty}`-принципал + `+`-continuation к предку, 3 корня, не флип),
text/troubleshoot/align-by-cell — архитектурные inline. **image-svg (259, Δ8)** —
ДВА корня, оба про этот файл, len_delta=-8 = ровно 2×4 пропущенных токена → флипнет
закрытием ОБОИХ.

### Реальная семантика (исходник html5.rb + пробы /tmp/p_fg,p_fg2,p_isvg)
- **Table frame/grid** (convert_table:859-860): `frame = 'ends' if (frame = attr
  'frame','all','table-frame') == 'topbot'; classes = ['tableblock',
  "frame-#{frame}", "grid-#{attr 'grid','all','table-grid'}"]`. Значение verbatim,
  без валидации; default «all»; `topbot`→`ends`; fallback на doc-attr
  table-frame/table-grid. Наш рендерер ХАРДКОДИЛ `frame-all grid-all`.
- **Interactive SVG** (convert_image): для SVG (format=svg ИЛИ target содержит
  `.svg`) при safe<SECURE и `opts=interactive` → `<object type="image/svg+xml"
  data="{image_uri}"{width}{height}>{fallback}</object>`, fallback = `<img
  src="{image_uri(fallback)}" alt{width}{height}>` при `fallback=` attr, иначе
  `<span class="alt">{alt}</span>`. Object И fallback-img оба несут width/height.
  Raster+interactive → `<img>` (object только для SVG). `opts=inline` (встроить
  SVG-исходник) НЕ поддержан — нужно читать файл, падаем в `<img>`. (Нормализатор
  diffone сортирует атрибуты — `data` перед `type` в эталоне.)

### Что сделано
- **РЕНДЕРЕР** blocks.rs `start_table`: захардкоженный `frame-all grid-all` заменён
  чтением `frame`/`grid` из meta.named с fallback на document_attrs
  table-frame/table-grid, мапа `topbot`→`ends`, default «all».
- **ПАРСЕР** attributes.rs `ImageAttrs`: +поля `format`/`fallback`/`interactive`
  (парсинг `format=`, `fallback=`, `opts`/`options` split-comma на `interactive`).
- **ПАРСЕР** event.rs `Tag::BlockImage`: +поля `interactive: bool`/`fallback:
  Option<CowStr>` (+ into_static). block.rs scan_block_macros: `is_svg = format==svg
  || target.contains(".svg")`, `interactive = is_svg && img_attrs.interactive`.
  (Путь через meta НЕ годился — emit_block_metadata @315 фильтрует "format".)
- **РЕНДЕРЕР** media.rs `start_block_image`: +параметры interactive/fallback;
  выделена ветка построения внутреннего элемента (object vs img), link-обёртка и
  title сохранены.
- Тесты: +2 html (`test_table_frame_grid_classes_html`,
  `test_block_image_interactive_svg_html`); 1 обновлён (integration
  test_block_image — деструктуризация Tag::BlockImage с новыми полями).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 496, html 409 +2);
  compat parsing-lab 233/233.
- image-svg diffone: **0 diffs** (был 259). Пробы p_fg/p_fg2 (frame/grid: ends/none,
  topbot→ends, sides/cols, rows, default, doc-attr fallback+override) и p_isvg
  (interactive, fallback-img, raster→img, format=svg) совпали с asciidoctor.
- **Корпус: Identical 329→330 (+1 ФЛИП)**. Blast (base 329): РОВНО 1 файл —
  image-svg 259→0, **0 регрессий, 0 затронутых других файлов** (frame/grid-фикс в
  одиночку давал image-svg 259→258 closer — оба корня нужны для флипа).

### Что дальше
- nearmiss на 330 (14 Different): replacements (4 — NCR, скип), **complex (120,
  Δ4 — МУЛЬТИ-root, 3 корня: (1) literal-параграф ` $ cmd` в list-item без `+` —
  `</p>` должен закрываться ДО literalblock, мы держим `<p>` открытым; (2) `.
  {empty}` ordered list-item с пустым принципалом → asciidoctor эмитит `<p></p>`,
  мы опускаем; (3) `+`-continuation после blank к ПРЕДКУ-list-item)**, counters
  (136 — АРХИТЕКТУРНЫЙ verbatim `{counter:}`), image-size (177, Δ92 — КОНТЕКСТНЫЙ
  корень выше строки 99), data (181, Δ77 — CSV/DSV таблицы, мульти-root),
  troubleshoot-unconstrained-formatting (212, Δ−4 — архитектурно nested-backtick),
  text (249, Δ−5 — `+ +` hard-break в monospace + apostrophe NCR), align-by-cell
  (371, Δ−16 — inline `<n>`/`^+` в backtick, архитектурно), block-name-table (431,
  Δ−2 — `++…++` escape, архитектурно), table (597, Δ1 — ДВА корня),
  character-replacement-ref (625, Δ113), syntax-quick-reference (2788, Δ−31 —
  мульти-root), document-attributes-ref (6363, Δ73 — мульти-root), outline (6587,
  Δ1 — МУЛЬТИ-root).
- **complex кластер 1** (literal-параграф `</p>`-перестановка) — самостоятельный,
  хорошо определённый; стоит проверить blast (может флипать другой файл, где это
  единственное расхождение). Но complex сам не флипнет (3 корня).
- Pre-existing — см. сессии 36/38/40/42/43/44/45/46/47/48/49/50/51 (без изменений).

---

## Сессия (2026-06-13, пятьдесят первая) — Фаза 3: blank-строка в open-блоке dlist-continuation обрывала вывод

Запрос «продолжи». Ветка **`fix/dlist-continuation-openblock-truncation`** —
ЗАКОММИЧЕНА, смержена в master (--no-ff). base-бинарь /tmp/adoc_base обновлён до
329. **ОЖИДАЕТ: явная авторизация на `git push origin master` + удаление ветки**
(пуш — outward-facing). Старт: housekeeping 50-й уже закрыт сам (origin/master ==
master == 0d49cac, дерево чисто, ветки fix/* удалены — пуш 50-й прошёл).

### Выбор задачи
nearmiss на 328 (16 Different): replacements (4 — NCR, скип). Ведущий не-NCR —
**ts-url-format (110, Δ108)** — вывод обрывался на 35 токенах из 143. diffone @33:
эталон `<div class="exampleblock">`, наш `</div></body>` (конец документа). Файл =
dlist-item (`term::`) + `+`-continuation + open-блок `--`, внутри параграф + два
titled example-блока (`====`) с source.

### Корень (пробы /tmp/p_ddex,p_dd_list,p_dd_2para,p_dd_exfirst,p_dd_noplus,p_ob_ex,p_dd_ex_direct)
Сужено бинарным поиском: баг ⟺ **`+`-continuation + open-блок `--` + ЛЮБОЙ второй
блок после внутренней blank-строки** (не про example специально — `----` listing и
даже два параграфа тоже рвут; open-блок+example БЕЗ dlist — OK; `term::` сразу `--`
БЕЗ `+` — OK; example прямо в dd без open — OK). Механика: `+` открывает open-блок
(возвращает Start, `in_continuation`→false), стек = `[DescriptionList,
DescriptionListEntry, DelimitedBlock(open)]`. Первый параграф ОК, blank →
`had_blank_line=true`. На втором блоке (в ts-url первым ловится title-guard
`.Solution A`) срабатывает guard `is_in_list_context() && !in_continuation &&
had_blank_line` → `close_list_contexts()` находит на ВЕРШИНЕ стека DelimitedBlock
(не список) → сразу `break`, возвращает ПУСТО → затем `event_buffer.pop()` = None
→ парсер думает «поток кончился» и обрывает всё (вкл. незакрытые dd/openblock/dl).

### Что сделано
- **ПАРСЕР** block.rs: новый хелпер `is_directly_in_list_context()` — сканирует
  стек сверху, возвращает true только если innermost-контейнер = list-item;
  DelimitedBlock/PartIntro — БАРЬЕР (return false, блок владеет своими blank-
  строками, закрывается только своим делимитером через `check_close_delimited_block`).
  Все 8 blank-line-driven guard-сайтов (block-attr, title, admonition, table,
  delimiter, md-fence, comment, paragraph-fallback) переведены с `is_in_list_context`
  на `is_directly_in_list_context` (replace_all по уникальному префиксу
  `is_in_list_context() && !self.in_continuation`). НЕ тронуты `+`-continuation
  сайты @1058/1070 (там broad-семантика верна). Док-коммент объясняет
  truncation-механику.
- Тест: +1 html `test_dlist_continuation_openblock_multiple_children_html`
  (все 3 ребёнка open-блока выживают + закрытие врапперов; негатив — blank всё
  ещё закрывает top-level список).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (996 total: parser 496,
  html 407); compat parsing-lab 233/233.
- ts-url-format diffone: **0 diffs** (был 110). Все 7 проб + 3 негатива IDENTICAL.
- **Корпус: Identical 328→329 (+1 ФЛИП)**. Blast (base 328): ts-url-format 110→0
  (флип), complex.adoc 152→120 (closer, тот же корень — continuation open-блоки),
  **0 регрессий, 0 FARTHER**.

### Что дальше
- nearmiss на 329 (15 Different): replacements (4 — NCR, скип), counters (136 —
  АРХИТЕКТУРНЫЙ verbatim `{counter:}`), **complex (120, Δ — было 152→120, ОСТАЛИСЬ
  другие корни в том же lists-examples файле; смотреть diffone)**, **image-size
  (177, Δ92 — таблица `[%autowidth]`/`|====` НЕ распознаётся в ПОЛНОМ документе, но
  ИЗОЛИРОВАННО (строки 99-125) OK → КОНТЕКСТНЫЙ корень выше строки 99)**, data (181,
  Δ77 — CSV/DSV таблицы, мульти-root), troubleshoot-unconstrained-formatting (212,
  Δ−4 — архитектурно), text (249, Δ−5 — то же), image-svg (259, Δ8 — ДВА корня:
  table `frame-ends grid-none` И `opts=interactive`→`<object>`), align-by-cell (371,
  Δ−16 — inline `<n>`/`^+` в backtick, архитектурно), block-name-table (431, Δ−2 —
  `++…++` escape, архитектурно), table (597, Δ1 — ДВА корня), character-replacement-ref
  (625, Δ113), syntax-quick-reference (2788, Δ−31 — мульти-root), document-attributes-ref
  (6363, Δ73 — мульти-root), outline (6587, Δ1 — МУЛЬТИ-root).
- Pre-existing — см. сессии 36/38/40/42/43/44/45/46/47/48/49/50 (без изменений).

---

## Сессия (2026-06-13, пятидесятая) — Фаза 3: нумерация частей книги + `[float]`-заголовки + `sectnumlevels`

Запрос «продолжи». ДВЕ ветки, обе смержены в master (--no-ff), base-бинарь
/tmp/adoc_base обновлён до 328. **ОЖИДАЕТ: явная авторизация на `git push origin
master` + удаление двух веток** (пуш — outward-facing). Старт: push 49-й уже
прошёл (origin/master == master == 9010e50, дерево чисто, ветки fix/* удалены).

### Ветка 1: `fix/book-part-numbering` (`dd3c2f0`, merge `ea2e0c2`) — КОРРЕКТНО, БЕЗ ФЛИПА
Выбор: outline (6597, Δ1) выглядел single-root — первые 6 diff'ов «Part I:
Fundamentals» vs «Fundamentals». Документ `:doctype: book` + `:partnums:` +
`:part-signifier: Part`. **Семантика** (пробы /tmp/p_part1..7, html5.rb
convert_section): части (level-0 секции книги) под `:partnums:` получают префикс
`{signifier+" " если задан}{roman}: ` (signifier="Part"→«Part I: », unset→«I: »);
римские заглавные сквозные глобальные; нумерация частей зависит ТОЛЬКО от
partnums, глав — от sectnums (независимы, P5); главы сквозные через части
(глобальный chapter-number, P4); префикс попадает и в TOC (P6).
**РЕНДЕР-CORE** SectionNumberer: `part_counter` + `part_prefix(signifier)` +
`to_roman`. **РЕНДЕРЕР** blocks.rs start_section_div: book-part ветка ставит
`pending_section_caption` (тот же канал что appendix — в заголовок И TOC),
signifier экранируется. **Бонус-багфикс (pre-existing)**: TOC внешний `<ul>`
класс = реальный asciidoctor-уровень секции (`level-1`, было `(level-1).max(1)`)
→ body sect0 (book part ИЛИ article level-0) теперь `sectlevel0`, не `sectlevel1`
(проба p_art0). +2 html, +2 core теста. **Корпус: Identical 327 (БЕЗ флипа —
нет файла где нумерация частей единственное расхождение)**; blast: outline
closer 6597→6587, **0 регрессий**. Закоммичено как корректное улучшение
(part-numbering — реальная книжная фича + pre-existing sectlevel0 багфикс).
outline НЕ single-root (мульти-root: escaped `\*`, pre-existing `+ +`→hard-break
в monospace [TODO.md строка 256], и др. — spec-файл со всеми конструкциями).

### Ветка 2: `fix/float-discrete-headings-sectnumlevels` (`3a7e203`, merge на master) — +1 ФЛИП
Выбор после ветки 1: **section (347, Δ−40)** — diffone @39 `<h1 class="float">`
vs наш `class="sect0"`. ТРИ корня section.adoc, все про секции/нумерацию:
- **(1) `[float]` = синоним `[discrete]`** (standalone-заголовок, не секция).
  Парсер УЖЕ имел scan_discrete_heading + Tag::Heading, но триггер только на
  `[discrete]`. **ПАРСЕР** block.rs: хелпер `is_discrete_style(s)` =
  `matches!(s, "discrete"|"float")`, три проверки section-маркера переведены
  (scan_section триггер @1426, header-detect skip @625, scan_discrete_heading
  id-gen @1534). Класс = буквальное имя стиля (`[float]`→`class="float"`,
  `[discrete]`→`class="discrete"`; роль `[float.r]`→`class="float r"`). Не
  секция, не в TOC, не нумеруется (пробы /tmp/p_disc, p_disc2). Строка 1450
  scan_section `!= "float"` filter осталась мёртвой защитой (float перехватывается
  раньше).
- **(2) `sectnumlevels` ограничивает глубину нумерации** (default 3). **РЕНДЕРЕР**:
  поле `sectnumlevels` (lib.rs), гейт в start_section_title: нумеровать только
  при `display_level <= sectnumlevels+1` (asciidoctor level = display−1).
  Фиксит pre-existing баг — мы всегда нумеровали asciidoctor-level-4 (display 5)
  секции (проба p_snl: default обрезает на level 3).
- **(3) `sectnumlevels` значение парсится Ruby-`to_i`** (ведущие цифры): строка
  178 `:sectnumlevels: 2 <.>` (callout-суффикс в документации) → 2, а наш
  `parse::<u8>` падал → оставался default 3 (проба p_snlc подтвердил
  asciidoctor берёт 2).
+2 html, +1 parser теста. **Корпус: Identical 327→328 (+1 ФЛИП, section.adoc
347→0)**; blast (base 327): РОВНО 1 файл, **0 регрессий, 0 затронутых других**.

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 496, html 406,
  core +2); compat parsing-lab 233/233.
- section.adoc diffone: **0 diffs** (был 347). Все пробы совпали с asciidoctor.
- **Корпус итог: Identical 327→328** (ветка 1 без флипа, ветка 2 +1 флип).

### Что дальше
- nearmiss на 328 (16 Different): replacements (4 — NCR, скип), ts-url-format
  (110, Δ108 — обрезка open-блока в dd-continuation), counters (136 —
  АРХИТЕКТУРНЫЙ verbatim `{counter:}`), complex (152, Δ143), **image-size (177,
  Δ92 — таблица `[%autowidth]`/`|====` НЕ распознаётся в ПОЛНОМ документе, но
  ИЗОЛИРОВАННО (строки 99-125) распознаётся (`fit-content` есть!) → КОНТЕКСТНЫЙ
  корень выше по файлу, НЕ в таблице; искать что ломает state до строки 99)**,
  **data (181, Δ77 — CSV/DSV таблицы `[%header,format=csv/dsv]` + shorthand
  `,===`; БОЛЬШАЯ многокорневая фича, не парсим colgroup для autocols)**,
  troubleshoot-unconstrained-formatting (212, Δ−4 — архитектурно), text (249,
  Δ−5 — то же), image-svg (259, Δ8 — ДВА корня: table `frame-ends grid-none` И
  `opts=interactive` SVG → `<object>`), align-by-cell (371, Δ−16 — inline-формат
  `<n>`/`^+` в backtick-тексте, архитектурно), block-name-table (431, Δ−2 —
  `++…++` double-plus escape, архитектурно), table (597, Δ1 — `|=== <1>` не
  точный делимитер + `<2>` callout-list-item рвёт параграф, ДВА корня),
  character-replacement-ref (625, Δ113), syntax-quick-reference (2788, Δ−31),
  document-attributes-ref (6363, Δ73), **outline (6587, Δ1 — МУЛЬТИ-root, НЕ
  флипнет одним фиксом)**.
- Pre-existing — см. сессии 36/38/40/42/43/44/45/46/47/48/49 (без изменений).

---

## Сессия (2026-06-13, сорок девятая) — Фаза 3: горизонтальный dlist colgroup-ширины + qanda `<p>`-обёртка ответа и группировка термов

Запрос «продолжи». Ветка **`fix/horizontal-dlist-colgroup-widths`** —
ЗАКОММИЧЕНА (`3e39dbc`), смержена в master (`10b2174`, --no-ff). base-бинарь
/tmp/adoc_base обновлён до 327. **ОЖИДАЕТ: явная авторизация пользователя на
`git push origin master` + удаление ветки** (пуш — outward-facing). Старт:
push 48-й сессии УЖЕ прошёл (origin/master == master == 6eeb22f, дерево чисто,
ветки fix/* удалены — housekeeping 48-й закрыт сам).

### Выбор задачи
nearmiss на 326 (18 Different): replacements (4 — NCR, скип). Кандидаты с малой
|len_delta| (один структурный корень + позиционный каскад): table (597, Δ1 — два
корня, рискованно), **description (299, Δ7)**, image-svg (259, Δ8 — два корня).
diffone description @92: эталон `<colgroup><col><col></colgroup>`, мы `<tr>` сразу
— ДВА корня в одном файле, оба про description-list.

### Реальная семантика (исходник html5.rb convert_dlist + пробы /tmp/p_hl1,p_hl2,p_hlx,p_qa,p_qa2,p_qa3,p_dl2)
- **Горизонтальный dlist + labelwidth/itemwidth → `<colgroup>`** (html5.rb:550-557):
  colgroup эмитится ⟺ есть labelwidth ИЛИ itemwidth; первый `<col>` несёт
  `style="width: {labelwidth без хвостового %}%;"` при наличии labelwidth, иначе
  голый `<col>`; второй — то же для itemwidth. `.chomp '%'` (значение `25` и `25%`
  дают `25%`). Плоский `[horizontal]` (без ширин) — БЕЗ colgroup (совпадал).
- **qanda dlist** (html5.rb:533-546): каждый ответ оборачивается `<p>{dd.text}</p>`
  (если dd.text есть; пустой ответ — без `<p>`); смежные термы (несколько `term::`
  подряд, один ответ) группируются в ОДИН `<li>` с `<p><em>…</em></p>` на каждый
  терм. Наш парсер термы группирует верно (нормальный dlist p_dl2 — два `<dt>`,
  один `<dd>`); баг был ТОЛЬКО в qanda-рендерере: каждый терм открывал новый `<li>`,
  ответ шёл голым текстом без `<p>`.

### Что сделано
- **РЕНДЕРЕР** blocks.rs `start_description_list`, ветка Horizontal: после
  `<table>\n` эмитит `<colgroup>` из meta.named labelwidth/itemwidth (strip_suffix
  '%').
- **РЕНДЕРЕР** events.rs qanda: `DescriptionTerm` — первый терм группы открывает
  `<li>\n<p><em>`, последующие только `<p><em>` (через общий флаг
  `hdlist_in_term_group`; qanda и horizontal не сосуществуют в одном списке);
  `DescriptionDescription` start — `<p>` + push li_p_open + dd_output_start (для
  отката пустого); end — откат голого `<p>` (пустой ответ) либо `</p>`, затем
  `</li>`. Присоединённый блок в ответе закрывает принципиальный `<p>` через
  существующий style-agnostic guard (`li_p_open.last()`).
- Тесты: +2 html (`test_qanda_adjacent_terms_grouped_html`,
  `test_horizontal_dlist_colgroup_widths_html`), 1 обновлён
  (`test_qanda_description_list_html` кодировал баг — ответ без `<p>`).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 495, html 402);
  compat parsing-lab 233/233.
- description diffone: **0 diffs** (был 299). Пробы qanda (inline/empty/grouped)
  и colgroup (both/label-only/item-only/percent) совпали с asciidoctor.
- **Корпус: Identical 326→327 (+1 ФЛИП)**. Blast (base 326): РОВНО 1 файл —
  description.adoc 299→0, **0 регрессий, 0 closer/FARTHER**. Оба корня
  (colgroup + qanda) встречаются вместе только в description.adoc; colgroup-корень
  есть ещё в horizontal/paragraph/CHANGELOG, но там labelwidth внутри listing-блоков
  (документация синтаксиса) → не рендерится как dlist, файлы уже Identical.

### Что дальше
- nearmiss на 327 (было 18 Different, минус description → 17): replacements (4 —
  NCR, скип), ts-url-format (110, Δ108 — обрезка open-блока в dd-continuation),
  counters (136 — АРХИТЕКТУРНЫЙ verbatim `{counter:}`), complex (152, Δ143),
  image-size (177, Δ92), data (181, Δ77), troubleshoot-unconstrained-formatting
  (212, Δ−4 — nested/double-backtick → литерал, архитектурно), text (249, Δ−5 — то
  же), image-svg (259, Δ8 — ДВА корня: table `frame-ends grid-none` И
  `opts=interactive` SVG → `<object>`), section (347, Δ−40), align-by-cell (371,
  Δ−16), block-name-table (431, Δ−2 — `++…++` double-plus escape, архитектурно),
  table (597, Δ1 — `|=== <1>` не точный делимитер + `<2>` callout-list-item рвёт
  параграф; тот же over-eager break-список из 48-й сессии, отдельный корень),
  character-replacement-ref (625, Δ113), syntax-quick-reference (2788),
  document-attributes-ref (6363), outline (6597, Δ1).
- Pre-existing — см. сессии 36/38/40/42/43/44/45/46/47/48 (без изменений).

---

## Сессия (2026-06-13, сорок восьмая) — Фаза 3: section-маркер НЕ прерывает открытый параграф

Запрос «продолжи». Ветка **`fix/section-marker-no-interrupt-paragraph`** —
ЗАКОММИЧЕНА (`99ff0f6`), смержена в master (`a827d7a`, --no-ff). base-бинарь
/tmp/adoc_base обновлён до 326. **ОЖИДАЕТ: явная авторизация пользователя на
`git push origin master` + удаление ветки** (пуш — outward-facing). Старт:
push 47-й сессии УЖЕ прошёл (origin/master == master == 0eca83a, дерево чисто,
ветки fix/* удалены — housekeeping 47-й закрыт сам).

### Выбор задачи
nearmiss на 325 (19 Different): replacements (4 — NCR, скип). Сильнейшие
single-token кандидаты: **admonition (197, Δ−10)**, table (597, Δ1), image-svg
(259, Δ8). diffone admonition @74: эталон держит `[IMPORTANT] <.>\n.Feeding\n====
<.>\n…` как ОДИН параграф, мы рвём на `==== <.>` в секцию `<div class="sect3">
<h4>`. Выбран admonition — чистое single-root правило.

### Реальная семантика (пробы /tmp/p_sec1..4, pb_{list,olist,thematic,image,admon,mdfence,delim,battr,pagebreak,dlist})
- **Section-заголовок НЕ прерывает открытый параграф**: `para\n== Heading\nmore`
  → ОДИН параграф (p_sec1). admonition `bl-c`: `[IMPORTANT] <.>` не оканчивается
  на `]` → не attr-строка → параграф; `.Feeding` (точка-title) и `==== <.>`
  (section-маркер с хвостом) — строки-продолжения, литеральный текст (p_sec2).
- **На границе блока (после blank)** `==== <.>` ВАЛИДНАЯ секция level-3 (p_sec3) —
  дело именно в мид-параграфном контексте. Голый `====` → example block (p_sec4).
- **Полное правило asciidoctor** (`read_paragraph_lines`/`StartOfBlockProc`,
  block_terminates_paragraph=true): открытый параграф рвётся ТОЛЬКО на делимитере
  блока (`----`, markdown-fence) и block-attr-строке `[...]`. НЕ рвут (пробы pb_*):
  section-заголовок, `*`/`.` list-маркеры, thematic break `'''`, `image::`,
  `NOTE:`-admonition, page break `<<<`, dlist `term::`. **Наш break-список
  СЛИШКОМ агрессивен** — но это НЕСКОЛЬКО отдельных корней.

### Что сделано
- **ПАРСЕР** block.rs: убран `scanner::strip_any_section_marker(line).is_some()`
  из break-условий в ДВУХ местах — `scan_paragraph` (@2194) и `scan_admonition`
  (@2583, принципиальный параграф admonition). Док-комментарии. Section на границе
  блока по-прежнему ловит диспетчер scan_leaf_blocks (@774) после blank.
- Тест: +1 html `test_section_marker_does_not_interrupt_paragraph` (мид-параграф
  `==`/`====` не рвут; негатив — секция после blank работает).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 495, html 400);
  compat parsing-lab 233/233 (1 тест зелёный).
- admonition diffone: **0 diffs** (был 197). Пробы p_sec1..4 совпали с asciidoctor.
- **Корпус: Identical 325→326 (+1 ФЛИП)**. Blast (base 325): РОВНО 1 файл —
  admonition.adoc 197→0, **0 регрессий, 0 closer/FARTHER** (затронут лишь 1 файл —
  редкая конструкция «параграф+section без blank» в корпусе только тут).

### Что дальше
- nearmiss на 326 (было 19 Different, минус admonition → 18): replacements (4 —
  NCR, скип), ts-url-format (110, Δ108 — обрезка open-блока в dd-continuation),
  counters (136 — АРХИТЕКТУРНЫЙ verbatim `{counter:}`), complex (152, Δ143),
  image-size (177, Δ92), data (181, Δ77), troubleshoot-unconstrained-formatting
  (212, Δ−4 — nested/double-backtick → литерал, архитектурно), text (249, Δ−5 —
  то же), image-svg (259, Δ8 — ДВА корня: table `frame-ends grid-none` И
  `opts=interactive` SVG → `<object>`), description (299, Δ7), section (347,
  Δ−40), align-by-cell (371, Δ−16), block-name-table (431, Δ−2 — `++…++`
  double-plus escape, архитектурно), table (597, Δ1 — `|=== <1>` не точный
  делимитер + `<2>` callout-list-item рвёт параграф; ТОТ ЖЕ over-eager
  break-список, отдельный корень).
- **Кандидат-родственник этой сессии**: table.adoc — убрать callout-list-item/
  прочие из break-списка + сделать table-делимитер точным. НО общий over-fix
  (list/image/thematic/admonition/dlist не должны рвать параграф) РИСКОВАН — много
  файлов в корпусе кладут список/образ сразу после строки параграфа БЕЗ blank,
  рассчитывая на текущее (наше) поведение → проверять blast пошагово, по одному
  break-условию.
- Pre-existing — см. сессии 36/38/40/42/43/44/45/46/47 (без изменений).

---

## Сессия (2026-06-13, сорок седьмая) — Фаза 3: пустая стилевая (m/e/s) ячейка таблицы → голый `<td></td>`

Запрос «продолжи». Ветка **`fix/empty-styled-table-cell`** — ЗАКОММИЧЕНА
(`2fdf54a`), смержена в master (merge-commit, --no-ff). base-бинарь /tmp/adoc_base
обновлён до 325. **ОЖИДАЕТ: явная авторизация пользователя на `git push origin
master` + удаление ветки** (пуш — outward-facing). Состояние на старте: git чист,
origin/master == master == b743936 (пуш 46-й сессии прошёл).

### Выбор задачи
nearmiss на 324 (20 Different): replacements (4 — NCR, скип). Топ single-root по
малому |len_delta|: **table-ref (135, Δ−8)** — рекомендация 46-й сессии, корень
@848 известен. diffone подтвердил: эталон `</td>` (пустая ячейка), наш
`<p class="tableblock"><code></code></p>`. Таблица `[cols="1m,2,1m,2,2"]`, col2 (m)
пустая в нескольких строках.

### Реальная семантика (пробы /tmp/p_emptym, p_empty2, p_empty3, p_nonempty)
- **Пустая ячейка → `[]`** (table.rb Cell#content: empty text → нет параграфов):
  - default empty → `<td></td>` (УЖЕ корректно, через `p_start`-откат)
  - **m/e/s empty → `<td></td>`** (НАШ БАГ: эмитили `<p class="tableblock"><code></code></p>`)
  - header empty → `<th></th>` (УЖЕ корректно)
  - **literal empty → `<div class="literal"><pre></pre></div>`** (СОВПАДАЕТ, обёртка
    сохраняется даже пустой)
  - **AsciiDoc empty → `<div class="content"></div>`** (СОВПАДАЕТ, обёртка сохраняется)
- Непустые/мультипараграфные m/e/s — без изменений (проба p_nonempty IDENTICAL).

### Что сделано
- **РЕНДЕРЕР** blocks.rs `start_table_cell`: arm'ы Emphasis/Strong/Monospace теперь
  тоже записывают `p_start = Some(output.len())` после обёртки (раньше — только
  default `_`-arm). Literal/AsciiDoc маркер НЕ ставят (их обёртка сохраняется пустой).
- **РЕНДЕРЕР** events.rs `TagEnd::TableCell`: единый `let is_empty = p_start ==
  Some(output.len())`; arm'ы e/s/m откатывают ПОЛНУЮ обёртку (`<p class="tableblock"><em>`
  и т.п.) при is_empty, иначе закрывают как раньше; default `_`-arm переведён на
  `is_empty`. Мультипараграфные ячейки не триггерят (p_start указывает после ПЕРВОЙ
  обёртки, далеко ниже финальной длины; каждый para непуст).
- Тест: +1 html (`test_table_cell_empty_styled_no_wrapper_html`: m/e/s empty →
  `<td></td>` без пустого inline-враппера, default/header empty без регрессии,
  непустая m сохраняет обёртку).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 495, html 399);
  compat parsing-lab 233/233 (1 тест зелёный).
- table-ref diffone: **0 diffs** (был 135).
- **Корпус: Identical 324→325 (+1 ФЛИП)**. Blast (base 324): РОВНО 1 файл —
  table-ref.adoc 135→0, **0 регрессий**, 0 затронутых других файлов.

### Что дальше
- nearmiss на 325 (пересчитать; было 20 Different, минус table-ref → 19):
  replacements (4 — NCR, скип), ts-url-format (110, Δ108 — обрезка open-блока в
  dd-continuation), counters (136 — АРХИТЕКТУРНЫЙ verbatim `{counter:}`), complex
  (152, Δ143), image-size (177, Δ92), data (181, Δ77), admonition (197, Δ−10),
  troubleshoot-unconstrained-formatting (212, Δ−4 — nested/double-backtick →
  литерал, архитектурно), text (249, Δ−5 — то же), image-svg (259, Δ8),
  section (347, Δ−40), align-by-cell (371, Δ−16), block-name-table (431, Δ−2 —
  `++…++` double-plus escape, архитектурно/рискованно), table (597, Δ1).
- Pre-existing — см. сессии 36/38/40/42/43/44/45/46 (без изменений).

---

## Сессия (2026-06-13, сорок шестая) — Фаза 3: cols-спек таблицы бьётся по `;` так же, как по `,`

Запрос «продолжи». Ветка **`fix/table-cols-semicolon-separator`** — ЗАКОММИЧЕНА
(`c516f33`), смержена в master (`1745038`, --no-ff). base-бинарь /tmp/adoc_base
обновлён до 324. **ОЖИДАЕТ: явная авторизация пользователя на `git push origin
master` + удаление ветки** (пуш — outward-facing). Хвост 45-й сессии разрешён:
push фактически прошёл (origin/master был на 9040a04, дерево чистое, ветка
удалена).

### Выбор задачи
nearmiss на 322 (22 Different): replacements (4 — NCR, скип). Топ single-root
по малому |len_delta| при многих diff'ах: **add-title (252, Δ−6)**. diffone @303:
эталон `<col><col></colgroup>` (3 колонки), наш пустой `<colgroup>` (1 `<col>`)
+ `<tbody>` вместо `<thead>`. Таблица `[cols=1;m;m]`.

### Реальная семантика (пробы /tmp/p_semi, p_sep смешанные разделители)
- **Разделитель cols = `,` ИЛИ `;`, ВЗАИМОИСКЛЮЧАЮЩЕ**: есть запятая → split по
  `,`; иначе → по `;`. `1;m;m`→3, `2*;m`→3, `1; m; m`→3 (trim); смешанные
  `1,m;m`→1, `1;m,m`→1 (split по `,`, не-сплитнутый `;`-кусок = невалидный спек,
  отбрасывается/ленивый default). `;` используют БЕЗ кавычек: attrlist-сплиттер
  сам режет запятые, поэтому `[cols=1,m,m]` требует кавычек, а `[cols=1;m;m]`
  выживает голым.
- При 1 колонке (вместо 3) три ячейки первой строки `|A | B | C` становятся 3
  СТРОКАМИ → ломается и colgroup (1 `<col width:100%>`), и header-детекция
  (`cells_before_blank_col_width == num_cols`: 3 ≠ 1 → нет thead).

### Что сделано
- **ПАРСЕР** attributes.rs `table_col_specs`: `let sep = if trimmed.contains(',')
  { ',' } else { ';' };` вместо `split(',')`. Док-коммент про attrlist-сплиттер.
- **РЕНДЕРЕР** blocks.rs `parse_col_widths`: то же правило разделителя (рендерер
  ДУБЛИРУЕТ парсинг cols для colgroup-ширин — зеркалю правило, ссылка на парсер
  в комментарии).
- Тесты: +1 parser (`test_table_col_specs_semicolon_separator`: `1;m;m`→3 +
  стили m/m, `2*;m`→3, смешанный `1,m;m`→2), +1 html
  (`test_table_cols_semicolon_separator_html`: 3×`<col>` 33.3333/33.3334, thead,
  `<code>` в m-ячейках).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 495, html 398);
  compat parsing-lab 233/233.
- add-title diffone: **0 diffs** (был 252).
- **Корпус: Identical 322→324 (+2 ФЛИПА)**. Blast (base 322): РОВНО 2 файла —
  add-title 252→0 И image-ref 748→0 (бонус — `[cols=2;2;3;3]`, тот самый
  pre-existing colgroup/thead-корень из сессий 41/42!), **0 регрессий**, 0
  затронутых других файлов.

### Что дальше
- nearmiss на 324 (пересчитать; было 22 Different, минус add-title/image-ref →
  20): replacements (4 — NCR, скип), ts-url-format (110, Δ108 — обрезка
  open-блока в dd-continuation), table-ref (135, Δ−8 — лишний пустой
  `<p class="tableblock"><code></code>` в пустой m-ячейке @848), counters (136 —
  АРХИТЕКТУРНЫЙ verbatim `{counter:}`), complex (152, Δ143), image-size (177,
  Δ92), data (181, Δ77), admonition (197, Δ−10), troubleshoot-unconstrained-
  formatting (212, Δ−4 — nested/double-backtick → литерал, архитектурно),
  text (249, Δ−5 — то же), image-svg (259, Δ8).
- Pre-existing — см. сессии 36/38/40/42/43/44/45 (без изменений).

---

## Сессия (2026-06-13, сорок пятая) — Фаза 3: `-`-маркер вкладывается под `*` + класс стиля маркера на `<ul>`

Запрос «продолжи». Ветка **`fix/unordered-dash-marker-nesting`** — ЗАКОММИЧЕНА
(`db3b773`), смержена в master (`65e2113`). **ПУШ ОТКЛОНЁН авто-классификатором**
(прямой push в master без явной авторизации) — master ЛОКАЛЬНО ahead 2, ветка
СОХРАНЕНА как страховка, base-бинарь /tmp/adoc_base обновлён до 322.
**ОЖИДАЕТ: явная авторизация пользователя на `git push origin master` +
удаление ветки.**

### Выбор задачи
nearmiss на 321 (23 Different): рекомендация 44-й сессии — **unordered (145,
Δ4)**. diffone @271: эталон `<div class="ulist"><ul><li>` (вложенный), наш
`</li><li>` (плоско). Тег `nest-alt` (`* L1` / `- L2` / `* L1`).

### Реальная семантика (пробы /tmp/p_un1..5, p_sq/p_cl/p_ov/p_foo/p_role/p_sqr/p_id/p_nest_*)
- **Матчинг по СТРОКЕ маркера, не по числу**: `-` ≠ `*`; число звёзд = ИДЕНТИЧНОСТЬ,
  не уровень. p5 (`- a`/`** b`/`* c`): `*` вкладывается ГЛУБЖЕ `**` (не возврат).
  Маркер матчит открытый предок → sibling; не матчит → вложение в внутренний item.
- **Стиль маркера** (`[square]`/`[circle]`/`[disc]`/`[none]`/`[no-bullet]`/любой
  keyword) → класс на `<div class="ulist {style} {roles}">` И `<ul class="{style}">`.
  Роль — ТОЛЬКО на div (p_role: `ulist myrole` + plain `<ul>`). Комбо `[square.myrole]`:
  div `ulist square myrole`, ul `square`. Стиль НЕ распространяется на вложенные
  (p_cl), но вложенный со СВОИМ `[circle]` класс несёт (p_ov marker-override).
  asciidoctor эмитит id/roles/style и на вложенных списках (p_nest_sq).

### Что сделано
- **ПАРСЕР** scanner.rs `is_list_marker_unordered`: `-` → identity `0` (было `1`,
  коллизия с `*`); `*`-счёт остаётся 1..N. depth = идентичность маркера для
  матчинга (`==`, без арифметики; рендерер unordered depth игнорирует — `ListItem
  { checked, .. }`, `UnorderedList` без depth-поля). Док-коммент.
- **РЕНДЕРЕР** blocks.rs `start_unordered_list`: две ветки (top/nested)
  УНИФИЦИРОВАНЫ через `write_meta_attrs` (nested теперь тоже несёт id/roles/style
  на div — было pre-existing-роняние, верно по p_nest_*); класс стиля добавлен на
  `<ul>` (`ul_class` = checklist/bibliography/style). Bibliography осталась
  top-level-only (`!is_inside_list_item()`).
- Тесты: +2 html (`test_unordered_dash_marker_nests_under_star` — p1/p2/p5;
  `test_unordered_list_marker_style_class` — square/role/combo/nested-override/
  unstyled-nested); scanner-тест +2 ассерта (`- item`→(0,...), `-no-space`→None).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 494, html 397);
  compat parsing-lab 233/233.
- unordered.adoc diffone: **0 diffs** (был 145).
- **Корпус: Identical 321→322 (+1 ФЛИП)**. Blast (base ebc2e35/321): РОВНО 1 файл
  — unordered.adoc 145→0, **0 регрессий**, 0 затронутых других файлов.

### Что дальше
- nearmiss на 322 (пересчитать; было 23 Different, минус unordered → 22):
  replacements (4 — NCR, скип), ts-url-format (110, len_delta=108 — обрезка
  open-блока в dd-continuation), table-ref (135, len_delta=−8), counters (136 —
  АРХИТЕКТУРНЫЙ verbatim `{counter:}`), complex (152, len_delta=143),
  image-size (177, len_delta=92), data (181, len_delta=77), admonition (197,
  len_delta=−10), troubleshoot-unconstrained-formatting (212, len_delta=−4),
  text (249), add-title (252, len_delta=−6), image-svg (259, len_delta=8).
- Pre-existing — см. сессии 36/38/40/42/43/44 (без изменений).

---

## Сессия (2026-06-13, сорок четвёртая) — Фаза 3: include `leveloffset` сдвигает level-0 заголовки

Запрос «продолжи». Ветка **`fix/include-leveloffset-level0`** — ЗАКОММИЧЕНА
(`7f1b7da`), смержена в master (`e5ff3b1`), запушена, ветка удалена.
Baseline: Identical 318, master `91d4e24`; base-бинарь /tmp/adoc_base был на
318, после мержа обновлён до 321.

### Выбор задачи
nearmiss на 318 (26 Different): replacements (4 — NCR, скип). Сильнейший
single-root сигнал = малая |len_delta|. **architecture/index (189,
len_delta=4)** — diffone @21 показал ПЕРВЫЙ же diff структурным: эталон
`<div class="sect1"><h2 id="_мониторинг">`, наш `<h1 class="sect0">`. Каскад
189 diff'ов — от одного структурного расхождения.

### Реальная семантика (исходник + пробы /tmp/p_lo/p1..p5)
- Файл: `= Архитектура` + plantuml + `include::monitoring.adoc[leveloffset=+1]`;
  monitoring.adoc = `= Мониторинг` (L0) + три `== ...` (L1).
- **leveloffset сдвигает И level-0**: `= Section Zero` под `leveloffset=+1` →
  `<div class="sect1"><h2>` (L1), `== Sub` → `<div class="sect2"><h3>` (L2)
  (p1). Заголовок с N `=` = секция уровня N−1; offset сдвигает уровень.
- **Отрицательный offset демоутит** до level-0: `== Level One` под `-1` →
  `<h1 class="sect0">` (p2/p3/p5). Минимум — один `=` (level 0): `= Zero` под
  `-1` остаётся `<h1 class="sect0">` (p5). Клампинг `1..=6` `=`.
- Латентный предел (нет корпуса): `======` (L5) под `+1` у asciidoctor вообще
  НЕ рендерит секцию (p4 — заголовок исчезает); мы клампим в 6 `=` (остаётся L5).

### Что сделано
- **ПАРСЕР** preprocessor.rs `apply_level_offset`: guard `eq_count >= 2` →
  `(1..=6).contains(&eq_count)` (пускает level-0 `= Title`); `clamp(2, 6)` →
  `clamp(1, 6)` (минимум — один `=`, level 0). Док-коммент.
- Тесты: +2 (`test_level_offset_level0_promoted` `= X`+1→`== X`,
  `test_level_offset_level0_clamped_at_zero` `= X`−1→`= X`); 1 обновлён
  (`test_level_offset_clamp_min`: `== Title` −5 → `= Title`, старый ассерт
  `== Title` кодировал баговый минимум 2).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 494, html 395);
  compat parsing-lab 233/233.
- architecture/index diffone: **0 diffs** (был 189).
- **Корпус: Identical 318→321 (+3 ФЛИПА)**. Blast (base 91d4e24): РОВНО 3 файла —
  architecture/index 189→0, software-development-cookbook 2481→0, java/index
  2313→0 (все включают суб-доки с `leveloffset=+1` — один корень ломал всю
  секционную вложенность), **0 регрессий**.

### Что дальше
- nearmiss на 321 (пересчитать; было 23 Different после флипов): replacements
  (4 — NCR, скип), ts-url-format (110, len_delta=108 — обрезка open-блока в
  dd-continuation, теряем `====` example-блоки после первого параграфа;
  отдельный корень), table-ref (135, len_delta=−8 — table-cell `<code>`-ячейка,
  смотрел @848: лишний пустой `<p class="tableblock"><code></code>`), counters
  (136 — АРХИТЕКТУРНЫЙ verbatim `{counter:}`), unordered (145, len_delta=4 —
  Level-2 list item не создаёт вложенный `<div class="ulist"><ul>`, держим
  плоско @271), complex (152, len_delta=143), image-size (177, len_delta=92),
  data (181, len_delta=77), admonition (197, len_delta=−10),
  troubleshoot-unconstrained-formatting (212, len_delta=−4), text (249),
  add-title (252, len_delta=−6).
- **unordered (145, Δ4)** — хороший single-root кандидат: вложенный список
  не открывается (разведан @271). Смотреть исходник unordered.adoc.
- Pre-existing — см. сессии 36/38/40/42/43 (без изменений) + continuation-отступ
  в table-ячейке тримится; emit_row_cells col_idx наивный (латентный).

---

## Сессия (2026-06-13, сорок третья) — Фаза 3: явный оператор выравнивания ячейки побеждает дефолт колонки

Запрос «продолжи». Ветка **`fix/table-cell-explicit-alignment`** — ЗАКОММИЧЕНА
(`a3f2667`), смержена в master (`b1f52f2`), запушена, ветка удалена.
Baseline: Identical 317, master `5b5d958`; base-бинарь /tmp/adoc_base уже был
на 317, после мержа обновлён до 318.

### Выбор задачи
Заметки 42-й сессии рекомендовали **cell.adoc 965→1** (ОДИН diff @574:
`halign-left` эталон vs `halign-right` наш на rowspan=3 ячейке) и предполагали
корень в col_idx (rowspan-сдвиг). **Гипотеза ОПРОВЕРГНУТА разбором грида** —
для этой ячейки col_idx уже верный.

### Реальная семантика (разбор таблицы `[cols="e,m,^,>s"]` строки 120-126)
- Ячейка `7` (`.3+<.>m|7`): rowspan 3, `<`=halign **Left**, `.>`=valign Bottom,
  m-стиль. Грид: Row1 `5`(col0), `6`(2.2+,col1-2), `7`(col **3**). col_idx=3 —
  и наивный счёт emit_row_cells, и occupancy-aware дают **одно и то же** (rowspan
  стартует в этой строке, ведущих занятых колонок нет). col_idx НЕ виноват.
- Колонка col3 = `>` (Right). Ячейка ставит явный `<` (Left), НО старый
  resolve_align не мог отличить явный Left от дефолтного Left
  (`halign==Left && cell.halign==Left` — условия идентичны) → всегда накрывал
  дефолтом колонки. Asciidoctor уважает явный оператор → halign-left.
- valign совпадал (Bottom — недефолт, явный `.>` отличался от дефолта Top, не
  накрывался). Только halign ломался.

### Что сделано
- **ПАРСЕР** scanner.rs: `CellSpec` + `ExactCellSpec` получили поля
  `halign_explicit`/`valign_explicit` (по образцу `style_explicit`).
  `parse_cell_align_prefix`/`parse_cell_align_suffix` возвращают флаги
  (`(&str, HAlign, VAlign, bool, bool)`) — true когда оператор реально присутствует
  (suffix: бывший единый `found` разбит на halign/valign). Протянуто через
  parse_cell_spec_exact, оба литерала pending/default_spec, суффиксный путь,
  push CellSpec; конструктор в block.rs append_cell_continuation → false/false.
- **ПАРСЕР** block.rs resolve_align: эвристика `value==default` заменена на
  `if !cell.halign_explicit { halign = col_default }` (и valign). Строго более
  корректно: меняет поведение ТОЛЬКО для явного `<`/`.<` поверх недефолтной
  колонки (раньше — баг, теперь — Left/Top как у asciidoctor); все остальные
  комбинации байт-в-байт прежние.
- Тесты: +1 html `test_table_cell_explicit_left_overrides_cols_align_html`
  (явный `<` поверх `>`-колонки, явный `.<` поверх `.>`-колонки, негатив-
  наследование); scanner-тесты parse_cell_align_prefix (10 assertions с флагами),
  parse_cell_spec_exact (+ `.3+<.>m` явные флаги). aligned_cell helper выводит
  флаги из value!=default (все его кейсы — недефолтные операторы).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 492, html 396);
  compat parsing-lab 233/233.
- cell.adoc diffone: **0 diffs** (был 1).
- **Корпус: Identical 317→318 (+1 ФЛИП)**. Blast (base 5b5d958): РОВНО 1 файл —
  cell.adoc 1→0, **0 регрессий** (ни одного позиционного сдвига в других файлах).

### Что дальше
- nearmiss на 318 (26 Different): replacements (4 — NCR, скип), ts-url-format
  (110, len_delta=108 — обрезка open-блока в dd-continuation), table-ref (135,
  len_delta=-8), counters (136, len_delta=9 — АРХИТЕКТУРНЫЙ verbatim `{counter:}`),
  unordered (145, len_delta=4 — вложенность списка), complex (152, len_delta=143),
  image-size (177, len_delta=92), data (181, len_delta=77), architecture/index
  (189, len_delta=4), admonition (197, len_delta=-10).
- **Латентный (НЕ исправлен, нет корпусного кейса)**: emit_row_cells col_idx
  всё ещё наивный (не occupancy-aware) — баг проявился бы при ячейке ПОСЛЕ
  ведущей rowspan-занятой колонки с отличающимся col-spec. В корпусе не
  манифестируется (см. align-by-cell 371, table.adoc 597 — другие корни).
- Pre-existing — см. сессии 36/38/40/42 (без изменений) + continuation-отступ
  в table-ячейке тримится.

---

## Сессия (2026-06-13, сорок вторая) — Фаза 3: blank-строка в DEFAULT/стилевой table-ячейке → несколько `<p class="tableblock">`

Запрос «продолжи». Ветка **`fix/table-cell-multi-paragraph`** — ЗАКОММИЧЕНА
(`931b4d5`), смержена в master (`4b477a9`), запушена, ветка удалена.
Baseline: Identical 314, master `92ca10a`; base-бинарь /tmp/adoc_base
пересобран с него (чистый release ДО ветки), теперь обновлён до 317.

### Выбор задачи
nearmiss на 314: replacements (4 — NCR, скип). Сильнейший single-root сигнал
= малая `|len_delta|` при многих diff'ах. **highlight-lines (185,
len_delta=2)** — diffone @166 показал РОВНО +2 токена (`<p class="tableblock">`
+ `</p>`): DEFAULT-ячейка с blank-строкой даёт ДВА параграфа, мы схлопывали
в один. Корень общий с subs-symbol-repl (165) и cell.adoc (965).

### Семантика asciidoctor (исходник table.rb + пробы /tmp/p_cellp/p1..p6 IDENTICAL)
- **Cell#content (table.rb:371-385)**: если RAW `@text` содержит `\n\n`
  (DOUBLE_LF) → `text.split(/\n{2,}/)`, КАЖДЫЙ параграф оборачивается
  стилевым inline-враппером (m→`<code>`, e→`<em>`, s→`<strong>`), для
  default/header — как есть. html5.rb оборачивает каждый в
  `<p class="tableblock">…</p>`. Пустая ячейка → `[]` (нет `<p>`,
  `<td></td>`). Несколько blank подряд = один split (`\n{2,}`).
- Внутри параграфа одиночный `\n` СОХРАНЯЕТСЯ (p6: `line one\nline two`).
- Literal/AsciiDoc ячейки НЕ бьются (handled separately). Header-СТРОКИ
  (thead) используют cell.text — не бьются.
- **Известный предел (pre-existing, НЕ трогал)**: continuation-отступ внутри
  параграфа asciidoctor СОХРАНЯЕТ (`one\n  two`), наш `cell_text` тримит
  (`one\ntwo`). Старый код тоже тримил; нормализатор корпуса НЕ схлопывает
  внутренние пробелы, но флипнувшие файлы отступов в мульти-пара ячейках не
  имели → корпусной выгоды от фикса отступа сейчас нет, риск > выгоды.

### Что сделано
- **ПАРСЕР** event.rs: новый `Event::TableCellParagraphBreak` (unit-маркер,
  + into_static). parser.rs пропускает его через `other` (subs-стек не
  трогается); per-para Text идут Owned-путём (inline-парсинг, NORMAL subs).
- **ПАРСЕР** block.rs: `cell_paragraphs(cell, style)` — split на blank-строки
  (trim+filter внутри параграфа, join `\n`); AsciiDoc/Literal → один элемент.
  В emit body-ячейки: `paras.len()<=1` → старый `cell_text` (байт-в-байт,
  zero-copy); иначе Text(para) с `TableCellParagraphBreak` между (эмиссия в
  reverse: Text(pN),Break,…,Text(p1) → pop-порядок Start,p1,Break,p2,…,End).
  Header-строки не затронуты (single Text).
- **РЕНДЕРЕР** events.rs: арм `TableCellParagraphBreak` — закрывает текущий
  `<p class="tableblock">` (+ стилевой враппер) и открывает следующий; стиль
  с верха `cell_style_stack`. m/e/s → `</code></p><p class="tableblock"><code>`
  и т.п.; default/header → `</p><p class="tableblock">`.
- **adoc-compat-tests** builder.rs: no-op арм (ASG держит cell-текст плоско).
- Тесты: +1 html `test_table_cell_multi_paragraph_html` (default 2-para,
  3-para, m-колонка, e-ячейка, multiple-blank→1 split, single-para
  не затронут); обновлён `test_table_cell_literal_preserves_blank_and_indent`
  (plain-ячейка теперь split, не collapse — старый ассерт кодировал баг).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (html 394, всего +1);
  compat parsing-lab 233/233.
- Пробы p1..p6 IDENTICAL.
- **Корпус: Identical 314→317 (+3 ФЛИПА)**. Blast (base 92ca10a):
  align-cell 211→0, highlight-lines 185→0, subs-symbol-repl 165→0 (флипы);
  cell.adoc 965→1, image-svg 282→259 (ближе); image-ref 746→748 (+2 —
  позиционный шум поверх pre-existing colgroup/thead корня @13: ячейка
  `image::sunset…` + blank + `image::chart…` РЕАЛЬНО бьётся, наш split @661
  идентичен эталону; первое расхождение файла @13 — нет 4×`<col>` и `<thead>`,
  не мой домен). **0 семантических регрессий**.

### Что дальше
- **cell.adoc 965→1** — ОДИН diff @574: `halign-left` (эталон) vs `halign-right`
  (наш) на rowspan=3 ячейке. ОТДЕЛЬНЫЙ корень: emit_row_cells col_idx
  (выравнивание/стиль) не учитывает rowspan-сдвиг занятых колонок
  (TODO.md:228, докинфо-сессия). Чистый single-token флип если починить —
  нужна occupancy-aware col_idx в пассе выравнивания (как в build_table_rows).
  ХОРОШИЙ кандидат на следующую задачу (+1 флип, тот же домен).
- nearmiss на 317 (пересчитать): ts-url-format (110, len_delta=108 — обрезка
  open-блока в dd-continuation), table-ref (135), counters (136 —
  АРХИТЕКТУРНЫЙ verbatim `{counter:}`), unordered (145, len_delta=4 —
  вложенность списка), complex (152, len_delta=143), image-size (177,
  len_delta=92), data (181, len_delta=77), architecture/index (189,
  len_delta=4), admonition (197, len_delta=-10).
- Вскрытый pre-existing: image-ref/много-колоночные таблицы — нет
  `<colgroup>` с N×`<col>` и `<thead>` (главный корень image-ref @13).
- Pre-existing — см. сессии 36/38/40 (без изменений) + continuation-отступ
  в table-ячейке тримится (asciidoctor сохраняет).

---

## Сессия (2026-06-13, сорок первая) — Фаза 3: пустой `<p></p>` в dd без principal-текста

Запрос «продолжи». Ветка **`fix/empty-dd-principal-paragraph`** — ЗАКОММИЧЕНА
(`c75f7ff`), смержена в master (`23b4420`), запушена, ветка удалена.
Baseline: Identical 304, master `49f95b2`; base-бинарь /tmp/adoc_base
пересобран с него (скопирован чистый release-бинарь master ДО ветки).

### Выбор задачи
nearmiss на 304: replacements (4 — NCR, скип). Разведка diffone выявила
ОДИН общий корень у группы файлов: **sdr-007 (130, len_delta=−2)** оказался
ЧИСТЫМ single-root флипом — единственное различие = лишний пустой `<p></p>`
в `<td class="hdlist2">` (our=153 vs ref=151, ровно +2 токена, 130
позиционных diff'ов — каскад от вставки 2 токенов).

### Семантика asciidoctor (пробы /tmp/p_dd/p1..p7, все IDENTICAL)
- **dd с ПУСТЫМ principal-текстом + присоединённый блок** (list / open-block
  через `+` / nested dlist через смежность) → asciidoctor НЕ эмитит
  принципиальный `<p>` вовсе (convert_dlist: `<p>` только при `dd.text?`).
  Формы: p1 horizontal+ulist (`<td class="hdlist2">` сразу ulist), p2
  normal+openblock (`<dd>` сразу openblock), p3 normal+paragraph-via-`+`
  (`<dd>` сразу `<div class="paragraph">`), p7 normal+nested-dlist.
- p4 (principal-текст ЕСТЬ + блок) → `<p>principal text</p>` сохраняется.
- p5/p6 (полностью пустой `term::` + blank + параграф) — lazy principal:
  следующий параграф становится principal-текстом dd (`<p>Next para</p>`);
  у нас УЖЕ работало (IDENTICAL).

### Что сделано
- **РЕНДЕРЕР** events.rs start_tag, guard закрытия `<p>` при старте
  суб-блока (Tag::Paragraph/UnorderedList/OrderedList/DescriptionList/
  DelimitedBlock/SourceBlock/BlockImage/Table/Admonition при
  `li_p_open.last()==Some(&true)`): если `output.ends_with("<p>")` (principal
  пуст — ничего не дописано после открывающего `<p>`) → откатить `<p>`
  (`truncate(len-3)`) вместо эмиссии `</p>`. Проверка `ends_with("<p>")`
  робастна: текст/чекбоксы/маркеры (`<input…> `, `&#10003; `) дают иное
  окончание → ложного отката нет. Работает для normal/styled (`<dd>\n<p>`)
  и horizontal (`<td class="hdlist2">\n<p>`); существующий
  `dd_output_start`-rollback полностью-пустого dd не затронут.
- +1 html-тест `test_dd_empty_principal_with_attached_block_no_paragraph_html`
  (horizontal+ulist, normal+openblock, normal+nested-dlist, позитив
  principal+block).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (976).
- Пробы p1..p7 IDENTICAL.
- **Корпус: Identical 304→314 (+10 ФЛИПОВ!)**. Blast (base 49f95b2):
  CHANGELOG 1994→0, sdr-002 831→0, release-and-progress-reviews 406→0,
  sdr-005 372→0, sdr-003 318→0, sdr-004 314→0, sdr-006 205→0, sdr-008
  199→0, sdr-001 153→0, sdr-007 130→0; closer: cookbook 2582→2481,
  ts-url-format 125→110; **0 регрессий** (description.adoc 298→299 — diff
  base-наш vs new-наш = ровно удалённые пустые `<p></p>` перед
  ulist/olist/dlist, все совпадают с asciidoctor; +1 — позиционный шум
  поверх ДРУГОГО pre-existing корня: отсутствующий `<colgroup><col><col>`
  гориз. dlist, первый расходящийся токен @92 идентичен в base и new).

### Что дальше
- nearmiss на 314 (30 Different): replacements (4 — NCR, скип),
  ts-url-format (110, len_delta=108 — ОСТАТОК: обрезка контента open-блока
  внутри dd-continuation, теряем example-блоки `====` после первого
  параграфа; отдельный корень), table-ref (135), counters (136 —
  АРХИТЕКТУРНЫЙ: `{counter:}` в verbatim-блоках, block-context awareness в
  препроцессоре), unordered (145, len_delta=4 — вложенность списка),
  complex (152, len_delta=143), subs-symbol-repl (165 — blank в DEFAULT
  table-cell → второй `<p class="tableblock">`, pre-existing, тот же корень
  у cell.adoc 965), image-size (177, len_delta=92), data (181, len_delta=77).
- Вскрытый pre-existing: горизонтальный dlist НЕ эмитит `<colgroup><col><col>`
  (description.adoc главный корень; >2 колонок?).
- Pre-existing — см. сессии 36/38/40 (без изменений).

---

## Сессия (2026-06-13, сороковая) — Фаза 3: block-media trailing-content + image link/role/title/float-align/imagesdir

Запрос «продолжи». Ветка **`fix/block-media-macro-trailing-content`** —
ЗАКОММИЧЕНА (`ed651fe`), смержена в master (`54317ee`), запушена, ветка
удалена. Baseline: Identical 303, master `32ac8cc`; base-бинарь /tmp/adoc_base
пересобран с него (worktree).

### Выбор задачи
nearmiss на 303: replacements (4 — NCR, скип); **image.adoc (125,
len_delta=−1)** — оказалось ПЯТЬ корней (закрыты все → флип).

### Семантика asciidoctor (пробы /tmp/p_img/p1..p4,t1..t7,lnk,role + исходник gem'а)
- **BlockMediaMacroRx** (`^(image|video|audio)::(\S|\S.*?\S)\[(.+)?\]$`,
  rx.rb:421): строка обязана ЗАКАНЧИВАТЬСЯ `]` (после rstrip — t6/t7 → блок);
  trailing-контент (`image::x[] <.>`, даже `image::x[]trailing`) → ПАРАГРАФ
  (p1-p4). Target непустой, без whitespace по КРАЯМ (`\S…\S`), внутренний
  пробел OK (`a b`→`a%20b`, t2; ` x`/`x ` → параграф, t1/t3). Вложенный `]`
  при концовке на `]` — rfind корректен (t4).
- **block image link= из БЛОК-АТРИБУТНОЙ строки** (`[#id,link=…]`): мёржится
  в макрос, оборачивает `<img>` в `<a class="image" href>` (html5.rb:641).
  Макрос-attrs приоритетнее блок-строки.
- **convert_inline_image** (html5.rb:1185-1233): span class = `image` + float
  + role (align НЕ эмитится для inline!); title → атрибут `<img>` (после
  width/height). Нормализатор сортирует атрибуты — порядок img-атрибутов не
  важен.
- **convert_image** (block): classes = imageblock, float, `text-{align}`, role
  (фикс. порядок). Наш баг — итерация `named`-Vec по ПОРЯДКУ ВСТАВКИ
  (block.rs мёржил align ПЕРЕД float → `text-center right`).
- **image_uri/normalize_web_path/web_path** (abstract_node.rb, path_resolver.rb):
  unsecure без data-uri → uriish target (UriSniffRx, схема ≥2) или web-root
  `/…` → verbatim (spaces→%20); иначе непустой imagesdir префиксится
  (`imagesdir`+`/`+target, scheme `//` сохраняется через uri_prefix). imagesdir
  читается ЖИВО (mid-document `:imagesdir:` действует на последующие).

### Что сделано
- **ПАРСЕР** scanner.rs: `match_block_media(line, prefix)` (общий для
  image/video/audio) — strip_suffix(']') + find('[') + target whitespace-guard;
  3 функции стали врапперами. +9 кейсов в test_is_block_image.
- **ПАРСЕР** block.rs scan_block_macros: link = `img_attrs.link` или
  `block_attrs.named["link"]` (макрос приоритетнее).
- **ПАРСЕР** event.rs/inline.rs: `Tag::InlineImage` +поля `role`,`title`.
- **РЕНДЕРЕР** media.rs: start_inline_image — class `image+float+role` (align
  убран), title-атрибут; стал `&self`-методом (для image_uri). image_base_class
  — фикс. порядок float→align (lookup по ключу в Vec). НОВОЕ: `image_uri(&self)`
  + `is_uriish` (зеркало preprocessor); start_block_image/inline зовут его.
  +5 html-тестов (link-из-attr, trailing→параграф, float/align-порядок,
  imagesdir, role/title; test_inline_image_align переписан под align-ignored).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (975).
- Пробы все IDENTICAL; image.adoc 125→0.
- **Корпус: Identical 303→304 (+1 флип)**. Blast (base 32ac8cc): РОВНО 1 файл
  (image.adoc 125→0), **0 регрессий** (ни одного позиционного сдвига в других).

### Что дальше
- nearmiss на 304: replacements (4 — NCR, скип), ts-url-format (125,
  len_delta=106), sdr-007 (130), table-ref (135), counters (136),
  unordered (145), complex (152, len_delta=143), sdr-001 (153),
  subs-symbol-repl (165), **image-size (177, len_delta=92)** и
  **image-ref/image-svg** — возможно частично закрыты image-фиксами этой
  сессии (проверить diffone перед выбором), data (181, len_delta=77).
- Известный предел imagesdir: `..`/`.`/`//` внутри joined-пути НЕ
  нормализуются (web_path partition_path не реализован; нет корпусного кейса).
- Pre-existing — см. сессии 36/38 (без изменений).

---

## Сессия (2026-06-13, тридцать девятая) — Фаза 3: uriish include-таргет → link

Запрос «продолжи». Ветка **`fix/uriish-include-link`** — ЗАКОММИЧЕНА
(`a261d72`), смержена в master (`594d16a`), запушена, ветка удалена.
Baseline: Identical 302, master `ca6a35e`; base-бинарь /tmp/adoc_base
пересобран с него (worktree) — лежавший был от 06e6b03 (устарел).

### Выбор задачи
nearmiss на 302: replacements (4 — NCR, скип);
**apply-subs-to-text.adoc (115, len_delta=6)** — ОДИН корень:
`include::pass:example$pass.adoc[tag=in-macro]` (Antora resource-id)
рендерился «Unresolved directive…», эталон — bare-ссылка.

### Семантика asciidoctor (пробы /tmp/p_inc/p1..p3 + reader.rb)
- **resolve_include_path (reader.rb:1240-1248)**: таргет uriish
  (`UriSniffRx = \A\p{Alpha}[\p{Alnum}.+-]+:/{0,2}` — схема ≥2 символов
  до `:`; однобуквенная `a:` — файловый путь, Windows-диски) и нет
  `allow-uri-read` → строка заменяется на `link:<target>[role=include]`;
  attrlist и opts=optional ОТБРАСЫВАЮТСЯ (optional работает только на
  file-not-found ветке). Рендер: `<a href="…" class="bare include">`.
- Таргет с пробелом asciidoctor оборачивает `pass:c[…]` — только чтобы
  пройти СВОЙ link-regex; наш link-макрос принимает пробелы как есть и
  даёт тот же HTML → эмитим БЕЗ обёртки (форму `link:pass:c[x][role=…]`
  наш inline-парсер как раз НЕ понимает — проверено пробой l1).

### Что сделано
- **ПАРСЕР** preprocessor.rs resolve_includes_rec: is_uriish(path)
  (зеркало UriSniffRx на char::is_alphabetic/is_alphanumeric) → эмиссия
  `link:{target}[role=include]` до файловых операций/guard'ов.
- +1 тест (5 кейсов: scheme-таргет, URL, optional-у-URI, пробел,
  однобуквенная схема → unresolved).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (970).
- Пробы p1..p3 IDENTICAL (наш рендер link-строки = asciidoctor байт-в-байт).
- **Корпус: Identical 302→303 (+1 флип)**. Blast (base ca6a35e): ровно
  2 файла — apply-subs-to-text 115→0 (флип),
  syntax-quick-reference 2828→2788 (ближе), **0 регрессий**.

### Что дальше
- nearmiss на 303: replacements (4 — NCR, скип), image (125),
  ts-url-format (125, len_delta=106), sdr-007 (130), table-ref (135),
  counters (136), unordered (145), complex (152, len_delta=143),
  sdr-001 (153), subs-symbol-repl (165), image-size (177, len_delta=92),
  data (181, len_delta=77).
- Кандидаты-корни: `++…++` double-plus НЕ экранирует спецсимволы
  (block-name-table); syntax-quick-reference — file-level корень
  (нет `<div id="content">`).
- Pre-existing — см. сессии 36/38 (без изменений).

---

## Сессия (2026-06-13, тридцать восьмая) — Фаза 3: revision line после attr-entries + точная модель RevisionInfoLineRx

Запрос «продолжи». Ветка **`fix/metadata-revision-line`** — ЗАКОММИЧЕНА
(`e1f4275`), смержена в master (`d5d3f24`), запушена, ветка удалена.
Baseline: Identical 301, master `06e6b03`; base-бинарь /tmp/adoc_base
пересобран с него (worktree).

### Выбор задачи
nearmiss на 301: replacements (4 — NCR, скип); **metadata.adoc (111,
len_delta=3)** — ОДИН корень: вся дельта — отсутствующий
`<span id="revdate">` (позиционный сдвиг).

### Семантика asciidoctor (пробы /tmp/p_meta/p1..p16 + parse_header_metadata
parser.rb:1815-1866, RevisionInfoLineRx rx.rb:42)
- **Структура header**: author line = первая непустая не-attr строка
  (БЕЗ исключения section-маркеров: `= T`+`== Sec` без blank → author
  «== Sec», p14; `v2.0, ...` первой строкой → тоже author, p11);
  attr-entries/комментарии прозрачны и ДО author, и МЕЖДУ author и rev
  (process_attribute_entries трижды). Rev line = следующая непустая
  не-attr строка после author.
- **RevisionInfoLineRx** (`^(?:[^\d{]*(.*?),)? *(?!:)(.*?)(?: *,?: *(.*))?$`)
  матчит почти всё: freeform-строка → revdate (корпусный кейс: строка
  `hazards...\` после `:description:` с callout `<.>`); запятая без цифр
  до неё → revnumber SET-EMPTY (рендер `version ,`, p5/p16); хвостовое
  голое `:` → revremark set-empty (пустой span); v-компонента — slice(1)
  буквально (`version 5` → `ersion 5`!), только строчная v (V2.0 → date);
  capture с запятой стартует с первой цифры/`{`; `:`-старт компоненты →
  unshift (строка уходит в body, p9).
- **КОНФЛИКТ эталонов**: author line ПОСЛЕ attr-entry (p2/p10) — asciidoctor
  делает author, но parsing-lab (block/header/adjacent-to-body: `= T` +
  `:toc:` + `first paragraph`) требует ПАРАГРАФ. Оставлен спек
  (parsing-lab) — известная дивергенция, в pre-existing-списке.

### Что сделано
- **ПАРСЕР** scanner.rs parse_revision_line: переписан под регэксп;
  `RevisionInfo { version: Option, date, remark: Option }` (Some("") =
  set-empty ≠ None); тесты переписаны + freeform-кейсы.
- **ПАРСЕР** block.rs: хелпер consume_header_attr_entries (комменты +
  attr-entries с multiline, стоп на прочем) — зовётся между author и rev
  и вместо хвостового цикла; author/rev-проверки БЕЗ
  strip_any_section_marker; rev-арм эмитит Event::Attribute и для
  set-empty version/remark (рендерер уже attribute-driven, set-empty
  поддерживал).
- Тесты: +1 html (7 кейсов), scanner-тесты на Option + 2 новых.

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (969).
- Пробы p1..p16 и v-формы IDENTICAL (кроме p2/p10 — спек-дивергенция
  by design).
- **Корпус: Identical 301→302 (+1 флип)**. Blast (base 06e6b03): ровно
  1 файл — metadata.adoc 111→0, **0 регрессий**.

### Что дальше
- nearmiss на 302: replacements (4 — NCR, скип), apply-subs-to-text (115),
  image (125), ts-url-format (125, len_delta=106), sdr-007 (130),
  table-ref (135), counters (136), unordered (145), complex (152,
  len_delta=143), sdr-001 (153), subs-symbol-repl (165).
- Кандидаты-корни: `++…++` double-plus НЕ экранирует спецсимволы
  (block-name-table); syntax-quick-reference — file-level корень
  (нет `<div id="content">`).
- Pre-existing — см. сессию 36 + НОВОЕ: author line после attr-entry
  (спек vs asciidoctor, осознанная дивергенция).

---

## Сессия (2026-06-13, тридцать седьмая) — Фаза 3: нумерация appendix (буквенные цепочки + appendix-caption + per-parent ordinals)

Запрос «продолжи». Ветка **`fix/appendix-numbering`** — ЗАКОММИЧЕНА
(`9c6ebe0`), смержена в master (`be3044a`), запушена, ветка удалена.
Baseline: Identical 300, master `18dab28`; base-бинарь /tmp/adoc_base
пересобран с него (worktree).

### Выбор задачи
nearmiss на 300: replacements (4 — NCR, скип); **appendix.adoc (24)** —
три корня: кастомный `:appendix-caption:`, нумерация подсекций `A.1.`,
appendix бампил арабский счётчик.

### Семантика asciidoctor (пробы /tmp/p_appx/p1..p9 + ИСХОДНИК gem'а)
- **assign_numeral (abstract_block.rb:408-423)**: appendix → numeral =
  документ-глобальный counter 'appendix-number' (буква A,B,…; через части
  и уровни); caption = `"{appendix-caption} {numeral}: "` если атрибут ЕСТЬ
  (даже пустой → " A: ", p8), иначе `"{numeral}. "` (p2/p5 — unset).
  Appendix НЕ потребляет ordinal родителя (After Appendix = 2, p1).
  Chapter — глобальный counter 'chapter-number' (сквозь части, p3);
  прочие — per-parent ordinal.
- **sectnum (section.rb:119-122)**: конкатенация numeral'ов предков
  (parent level>1 asciidoctor) → подсекции appendix `A.1.`, `A.1.1.`;
  вложенный appendix (p7): свой заголовок — ТОЛЬКО caption («Appendix A:»,
  без префикса родителя), но потомки несут полную цепочку `1.A.1.`.
- **numbered**: appendix — ВСЕГДА true (parser.rb:1619, независимо от
  sectnums — caption виден и без него, p4); подсекции нумеруются только
  при `:sectnums:`.
- **per-parent ordinal**: article body-sect0 рестартит детей с 1 (p9);
  book-части НЕ рестартят (chapter-number глобальный). Doctype ЗАПИРАЕТСЯ
  в header: `:doctype: book` mid-body не меняет структурную семантику
  (корпусный appendix.adoc).

### Что сделано
- **RENDER-CORE** SectionNumberer: appendix_letters[6] (буква занимает
  уровень в цепочках потомков, counters уровня не трогаются);
  appendix_prefix(level, caption: Option<&str>) вместо appendix_caption();
  number_prefix строит цепочку из букв/цифр; reset_descendant_ordinals().
- **РЕНДЕРЕР**: дефолт document_attrs «appendix-caption»→«Appendix»
  (unset `!` удаляет ключ → форма «A. »; значение html_escape'ится);
  blocks.rs appendix-арм читает атрибут; events.rs TagEnd::Header —
  фиксация doctype_book; start_section_div: is_sect0 && !doctype_book →
  reset_descendant_ordinals.
- Тесты: +3 html (нумерация/caption-формы/sect0-reset article vs book),
  расширен core section_numbering.

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (967).
- Пробы p1..p9 IDENTICAL (через normalize_html(get_body_content(..))).
- **Корпус: Identical 300→301 (+1 флип)**. Blast (base 18dab28): ровно
  1 файл — appendix.adoc 24→0, **0 регрессий**.

### Что дальше
- nearmiss на 301: replacements (4 — NCR, скип), metadata (111),
  apply-subs-to-text (115), image (125), ts-url-format (125,
  len_delta=106), sdr-007 (130), table-ref (135), counters (136),
  unordered (145), complex (152, len_delta=143).
- Кандидаты-корни: `++…++` double-plus НЕ экранирует спецсимволы
  (block-name-table); syntax-quick-reference — file-level корень
  (нет `<div id="content">`).
- Pre-existing-список — см. сессию 36 ниже (без изменений).

---

## Сессия (2026-06-13, тридцать шестая) — Фаза 3: level-0 спец-секции + partintro + части в TOC

Запрос «продолжи». Ветка **`fix/part-special-sections`** — ЗАКОММИЧЕНА
(`42fcbde`), смержена в master (`fd99bb7`), запушена, ветка удалена.
Baseline: Identical 298, master `2c4a292`; base-бинарь /tmp/adoc_base
пересобран с него (worktree).

### Выбор задачи
nearmiss на 298: replacements (4 — NCR, скип);
**part-with-special-sections (103)** + **multipart-book (109)** — общие корни.

### Семантика asciidoctor (пробы /tmp/p_part/p1..p13, m1 + ИСХОДНИК gem'а)
- **initialize_section (parser.rb:1593-1626)**: стиль на секции (slot 1) =
  спец-секция; `sect_level = 1 if sect_level == 0` (и в article — p8);
  book+`[abstract]` → chapter, level=1 с ЛЮБОЙ глубины; `sect\d$`-стили —
  не спец. КОЭРСИЯ DISPLAY-ONLY: вложенность/закрытие решается по СЫРОМУ
  уровню ДО initialize_section (p12: `[appendix] = X` после части закрывает
  часть, сиблинг; «Appendix A:» нумерация работает).
- **partintro (next_section:400-440)**: первый не-секционный блок части —
  если open-блок без стиля → рестайл partintro (intro НЕ открыт: следующие
  блоки СНАРУЖИ, error «illegal block content...», p9); если `[partintro]`
  параграф → конверсия в open-блок (одноблочный, p7/p10); иначе НОВЫЙ
  open-блок partintro, intro открыт → все блоки до первой секции ВНУТРИ
  (p2/p11); intro есть и у части без глав (p5, error в лог).
- **TOC**: convert_outline — части видимы (level 0 → класс sectlevel1);
  вложенность по ДЕРЕВУ, класс ul — по level ПЕРВОГО ребёнка (дети
  appendix level 2 → sectlevel2, главы части level 1 → sectlevel1);
  коэрснутый colophon (level 1) — СИБЛИНГ части в одном ul.
- **Header order**: h1, `<div class="details">`, toc div (наш авто-TOC
  вставлялся ДО details — Event::Toc ставил позицию в момент `:toc:`).
- **dlist**: любой стиль кроме horizontal/qanda → `<div class="dlist X">` +
  `<dt>` БЕЗ hdlist1 (p13, включая unknown `[foo]`).

### Что сделано
- **ПАРСЕР** block.rs: scan_section — sect_style/book/display_level
  (закрытие по effective_level, эмиссия/контекст по display_level);
  PartIntro закрывается на любом заголовке секции; part_awaiting_intro
  взводится на голой level-0 секции в book. handle_part_intro (новый, в
  диспетчере между scan_leaf_blocks и scan_block_macros): рестайл/обёртка;
  BlockContext::PartIntro; армы в close_all_open_contexts и
  check_close_delimited_block.
- **RENDER-CORE**: TocEntry.depth (pub); toc_steps — стек depth, EnterLevel
  несёт display-level ВХОДЯЩЕГО entry, ul только для встреченных уровней.
- **РЕНДЕРЕР**: start_section_title — entry для всех body-секций
  (depth = sect0_stack.len()); finish.rs sectlevel = max(level-1,1);
  events.rs TagEnd::Header — перестановка toc_insert_position после
  render_author_details; DlistStyle::Styled.
- Тесты: +4 html, расширен core toc_structure_steps (+ book-сценарий).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (964).
- Пробы p1..p13, m1 IDENTICAL (через normalize_html(get_body_content(..))!).
- **Корпус: Identical 298→300 (+2 ФЛИПА)**. Blast (base 2c4a292): ровно
  4 файла — 2 флипа, appendix.adoc 158→24 и outline.adoc 8681→6597 ближе,
  **0 регрессий**.

### ВАЖНО для методики
- При ручном сравнении проб звать `normalize_html(get_body_content(html))` —
  БЕЗ get_body_content токены ПУСТЫЕ (void-теги `<meta>` в head ломают
  skip_depth) и сравнение тривиально «identical». Скрипты корпуса делают
  это правильно; ошибка только при прямом вызове normalize_html.

### Что дальше
- nearmiss пересчитать на 300; кандидаты по прошлому списку: replacements
  (4 — NCR, скип), **appendix.adoc (24!)** — почти флип после этой сессии,
  metadata (111), apply-subs-to-text (115), image (125), ts-url-format (125),
  sdr-007 (130), table-ref (135), counters (136), unordered (145).
- Кандидаты-корни: `++…++` double-plus НЕ экранирует спецсимволы
  (block-name-table 431); syntax-quick-reference — file-level корень
  (нет `<div id="content">`).
- Pre-existing из прошлых сессий: пустой `<p></p>` в dd с вложенным
  `:::`-dlist (description 298); `'''`/`<<<` после списка не закрывают
  контексты; blank в DEFAULT-ячейке → второй `<p>`; footnotes-div внутри
  a-ячейки; `[square]`-класс; компактный colist-`<li><p>`; `== heading` не
  прерывает параграф; `[abstract]`-параграф → quoteblock; `:icons:`-colist;
  unknown-style в class на quote/sidebar; list-merge через
  continuation-attrlist; author-line после attr-entry в header; label
  block-anchor `[[id,label]]` над блоком не побеждает `.Title`;
  `\\https://…` двойной backslash; CSV drop incomplete row;
  eager-`\\`-escape ест первый backslash.

---

## Инструменты корпуса (2026-06-13): кэш эталонов asciidoctor

`refcache.py` в `/mnt/c/tmp/adoc-test/` оборачивает `compare_full.run_cmd`:
HTML asciidoctor кэшируется в `~/.cache/adoc-ref-cache/` (ключ: версия gem +
аргументы + sha256 файла; наш бинарь не кэшируется, таймауты не кэшируются).
Полный прогон корпуса 1м28с→22с, blast 37с. Все скрипты подхватывают кэш
автоматически через импорт compare_full. Скрипты теперь ПЕРСИСТЕНТНО в
каталоге корпуса (не в /tmp): `nearmiss.py`, `blast.py`,
`diffone.py <rel-path> [limit]`. `ADOC_REF_REFRESH=1` — пересчитать кэш
(после обновления gem'а или правки include-зависимостей — их нет в ключе).
Верифицировано: hit/miss/инвалидация по содержимому; счётчики идентичны
некэшированным (Identical 298).

---

## Сессия (2026-06-12, тридцать пятая, часть 3) — Фаза 3: quoted paragraph + markdown blockquote + одиночные кавычки в attrlist

Та же сессия, третья задача. Ветка **`fix/quoted-paragraph-and-md-blockquote`**
— ЗАКОММИЧЕНА (`48ace39`), смержена в master (`6426a5f`), запушена, ветка
удалена. Baseline: Identical 297. **Итог сессии: 295→298 (+3 за три задачи).**

### Выбор задачи
nearmiss: **quote.adoc (109 diff)** — ожидался один корень (`-- Author`
attribution), оказалось ТРИ.

### Семантика asciidoctor (пробы /tmp/p_subs/p11, p12 + parser.rb:770-810)
- **Quoted paragraph**: параграф, где строка 1 начинается `"`, предпоследняя
  кончается `"`, последняя — `-- credit` → quote-блок с ГОЛЫМ контентом
  (как [quote]-параграф, без `<p>`), кавычки стрипаются; credit =
  attribution[, citetitle] (split ', ' 2), получает apply_subs. Негативы:
  без `--`-строки — обычный параграф с кавычками; `-- ` пустой — НЕ credit
  (это open-block делимитер у asciidoctor!).
- **Markdown blockquote**: строки `> ...` (первая обязана `> `) — стрип
  ОДНОГО уровня (`>` → пусто, `> x` → x, прочее как есть), остаток парсится
  как COMPOUND (врапперы параграфов ЕСТЬ, в отличие от quoted paragraph!);
  `> >` → вложенный quote, `> *` → список; trailing `-- credit` →
  attribution.
- **Кавычки в attrlist**: `'...'`-значение защищает запятую И получает
  normal subs при использовании (link/strong в citetitle); `"..."` — только
  защита запятой, литерал; кавычка открывается только в начале значения
  (после `,`/`=`) — апостроф в `Dad's words` не кавычка (проба p12).

### Что сделано
- **ПАРСЕР** block.rs scan_paragraph: две новые ветки перед plain-para
  (см. TODO.md); BlockScanner::new_nested(lines, depth) — скан по готовым
  строкам в body-контексте; поле md_quote_depth (cap 16).
- **ПАРСЕР** attributes.rs: quote-aware split (`'` и `"`, открытие только
  после `,`/`=`), стрип кавычек позиционных, поле
  single_quoted_positionals (merge: флаги newer при захвате style-слота).
- **ПАРСЕР** block.rs emit_block_metadata: маркер-ключи
  attribution-subs/citetitle-subs в named (только quote/verse).
- **РЕНДЕРЕР**: quote_attribution/quote_citetitle → Option<(String,bool)>;
  хелпер render_quote_attribution (дедуп двух армов TagEnd quote/verse);
  флаг → render_inline_value, иначе html_escape.
- Тесты: +3 html (quoted paragraph 4 кейса, md blockquote 4 кейса,
  single-quoted subs 4 кейса).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (960).
- Пробы p11, p12 IDENTICAL; quote.adoc 109→0.
- **Корпус: Identical 297→298 (+1)**. Blast (base dd7cf69, вся сессия):
  3 флипа (subs, ordered, quote), sdr-004 312→314 и description 295→298 —
  позиционный сдвиг поверх pre-existing корней (sdr-004: наш md-quote
  фрагмент сверен = asciidoctor; description: пустой `<p></p>` в dd),
  **0 семантических регрессий**.

### Ограничения/пределы новые
- md-blockquote: вложенность глубже 16 уровней `>` — плоский параграф
  (защита от рекурсии).
- merge стопки attrlist: single-quoted флаги берутся от newer при захвате
  style-слота (приближение).

---

## Сессия (2026-06-12, тридцать пятая, часть 2) — Фаза 3: вложенность списков по стеку маркеров + стиль olist от маркера

Та же сессия, вторая задача. Ветка **`fix/mixed-marker-list-nesting`** —
ЗАКОММИЧЕНА (`a091988`), смержена в master (`83c71e4`), запушена, ветка
удалена. Baseline: Identical 296.

### Выбор задачи
nearmiss: **ordered.adoc (90 diff)** — давний pre-existing «nested-список с
другим маркером в li», один корень + попутный (стиль olist).

### Семантика asciidoctor (пробы /tmp/p_subs/p6, p8, p9 — все IDENTICAL)
- Маркер, матчащий ОТКРЫТЫЙ список в стеке (текущий/предок) → закрыть всё
  выше него (cross-type), sibling-item. НЕсматченный маркер — глубже, МЕЛЬЧЕ
  или другого типа — НИЧЕГО не закрывает: новый список вкладывается в самый
  внутренний открытый item. Quirk подтверждён: `** b` затем `* c` → `* c`
  ВКЛАДЫВАЕТСЯ в li от `** b` (не «возврат на уровень»).
- Стиль olist implicit — от числа ТОЧЕК маркера (`..` → loweralpha даже
  первым ol в документе, внутри ulist-item), не от вложенности `<ol>`.
- dlist+list interplay и indent-вариант (mix-alt) — не затронуты, совпадают.

### Что сделано
- **ПАРСЕР** block.rs: scan_ordered_list_item получил has_parent_list +
  close_to_parent_list(depth, false) — зеркало unordered (была асимметрия);
  else-ветки ОБОИХ сканов — Vec::new() (вложение, без закрытий);
  close_list_items_for_depth УДАЛЁН (мёртвый); BlockContext::ListItem
  потерял поле depth (не читалось).
- **ПАРСЕР** event.rs: Tag::OrderedList + `depth: u8` (число точек).
- **РЕНДЕРЕР** blocks.rs start_ordered_list: implicit-стиль от depth
  (1→arabic, 2→loweralpha, 3→lowerroman, 4→upperalpha, 5+→upperroman)
  вместо подсчёта открытых TagEnd::OrderedList в tag_stack.
- Тесты: +2 html (mixed-marker nesting: olist↔ulist, shallower-quirk;
  стиль от маркера), integration-тест обновлён (поле depth).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (957).
- Пробы p6/p8/p9 IDENTICAL; ordered.adoc 90→0.
- **Корпус: Identical 296→297 (+1)**. Blast: 2 файла — 1 флип (ordered),
  description.adoc 295→298 — позиционный сдвиг поверх pre-existing корня
  (пустой `<p></p>` в dd, держащем только вложенный `:::`-dlist);
  изолят p10 сверен: структура вложенности new = asciidoctor,
  **0 семантических регрессий**.

### Новые вскрытые pre-existing (вне этого фикса)
- Пустой dd с вложенным `:::`-dlist → лишний `<p></p>` в dd
  (description.adoc 298 — главный корень файла).
- `'''`/`<<<` после списка НЕ закрывают список-контексты (`<hr>` оказывается
  внутри `<p>` item'а; в base так же — scan_leaf_blocks без close-pre-step,
  в отличие от admonition/table/delimited/fence/comment).

---

## Сессия (2026-06-12, тридцать пятая) — Фаза 3: точный index-term + `\\`-unconstrained escape + attr-refs в attrlist

Запрос «продолжи». Ветка
**`fix/escaped-index-term-and-double-backslash-unconstrained`** —
ЗАКОММИЧЕНА (`2c80fb2`), смержена в master (`4b66e7a`), запушена, локальная
ветка удалена. Baseline: Identical 295, master `dd7cf69` (base-бинарь
/tmp/adoc_base пересобран с master через временный worktree).

### Выбор задачи
nearmiss на 295: replacements (4 — NCR, скип); **subs.adoc (76 diff)** —
оказалось ТРИ корня (третий обнажился после сдвига).

### Семантика asciidoctor (пробы /tmp/p_subs/p1..p5 + ИСХОДНИК gem'а)
Ключевой приём: модель не сходилась на 4-скобочном кейсе — вскрыл
`/usr/share/rubygems-integration/all/gems/asciidoctor-2.0.23/lib/asciidoctor/`
(substitutors.rb:439-514, rx.rb InlineIndextermMacroRx) — снимает все догадки.
- **Index term**: ОДИН regex `\\?\(\((.+?)\)\)(?!\))` — non-greedy закрытие
  «скользит» мимо `)))`-хвостов; скобки самого контента решают форму:
  `(..)` с обеих сторон → concealed (невидимый, comma-split), слева →
  литеральная `(` + flow-term, справа → flow-term + `)`, иначе flow.
  Эскейп: контент в скобках → `(` + ВИДИМЫЙ flow + `)` («escape concealed,
  but process nested flow»); иначе весь матч литерально минус `\`.
- **`\\` + unconstrained-пара**: НЕТ спец-правила — каскад gsub-пассов:
  unconstrained-pass матчит `\MM..MM`, снимает один `\`; constrained-pass
  матчит с lead `\`, снимает второй → `\\__func__` → литерал `__func__`,
  контент с обычными subs (`\\__a *b* c__` → `__a <strong>b</strong> c__`;
  mid-word `a*b*c` — литерал, у constrained нет границы).
- **attr-refs в attrlist**: `[source,subs="{markup}"]` — asciidoctor
  подставляет атрибуты в block-attrlist строках на парсинге (document-order);
  unknown — intact (attribute-missing=skip), определение ПОСЛЕ — не работает,
  внутри verbatim — не трогается (проба p4 все кейсы IDENTICAL).

### Что сделано
- **ПАРСЕР** inline.rs: index_term_close (скользящее закрытие) +
  try_index_term (формы по скобкам контента); try_concealed_index_term /
  try_flow_index_term УДАЛЕНЫ, арм `(((`+`((` схлопнут в один. Escape-арм
  `\((` в handle_inline_escape (формы по asciidoctor). Арм `\\`+`MM` (марки
  `* _ #` и backtick): оба `\` съедены, Text(марки) + inner-парсер контента +
  Text(марки).
- **ПАРСЕР** preprocessor.rs: шаг 5a' в preprocess_with_attrs —
  строка-«[..]» целиком → expand_attr_refs_in_attrlist (известные атрибуты,
  `\{`-escape скип); после verbatim-fence гейта (внутри фенсов не работает).
- Тесты: +3 parser (sliding/partial parens, escaped index term,
  double-backslash unconstrained), +1 preprocessor (6 сценариев).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 490, html 374).
- Пробы p1, p3, p4 IDENTICAL; p2 — корпусные кейсы ✓, остатки = пределы ниже.
- **Корпус: Identical 295→296 (+1)** (subs.adoc 76→0). Blast (base dd7cf69):
  ровно 1 файл изменился — 1 флип, **0 регрессий**.

### Новые известные пределы (вне корпуса)
- `\__one__` (одиночный `\` + unconstrained): asciidoctor → `<em>_one_</em>`
  (каскад: unconstrained снял `\`, constrained-em сматчил `_`+`_one_`+`_`);
  у нас литерал `__one__`. Аналогично `\**bold**` → `<strong>*bold</strong>*`.
- `` \\`mono` ``: asciidoctor хранит один `\` и НЕ форматирует (у code только
  constrained-pass); у нас `\`+`<code>mono</code>` (eager-`\\`-модель).

### Что дальше
- nearmiss пересчитать на 296; кандидаты по прошлому списку: replacements
  (4 — NCR, скип), ordered (90), part-with-special-sections (103),
  multipart-book (109), quote (109 — `-- Author` attribution), metadata (111),
  apply-subs-to-text (115), image (125), ts-url-format (125).
- Кандидаты-корни: `++…++` double-plus НЕ экранирует спецсимволы
  (block-name-table 431); syntax-quick-reference — file-level корень
  (нет `<div id="content">`).
- Pre-existing из прошлых сессий: blank в DEFAULT-ячейке → второй `<p>`,
  footnotes-div внутри a-ячейки, nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `[abstract]`-параграф → quoteblock, `:icons:`-colist,
  unknown-style в class на quote/sidebar, list-merge через
  continuation-attrlist, author-line после attr-entry в header, label
  block-anchor `[[id,label]]` над блоком не побеждает `.Title`,
  `\\https://…` двойной backslash, CSV drop incomplete row,
  eager-`\\`-escape ест первый backslash (`\\` в ячейке → `\`).

---

## Сессия (2026-06-12, тридцать четвёртая) — Фаза 3: escaped `\|` + table width + стилевые веса cols + passthrough-скип в unconstrained

Запрос «продолжи». Ветка **`fix/pass-macro-and-delimited`** — ЗАКОММИЧЕНА
(`8e9dbeb`), смержена в master (`dd7cf69`), запушена, локальная ветка удалена.
Baseline: Identical 292, master `0a1e5fc` (base-бинарь /tmp/adoc_base
пересобран с master через временный worktree).

### Выбор задачи
nearmiss на 292: **pass-macro (3 diff)** + **delimited (9 diff)**;
replacements (4) — NCR-кластер, скип. По дороге flip'нулся data-format (615→0).

### Семантика asciidoctor (пробы /tmp/p_pm/p1..p10, /tmp/p_dl/p1..p4)
- **Table width**: `tablepcwidth` = Ruby `to_i` от width-атрибута (ведущие
  цифры, иначе 0); вне (0..100] → 100, КРОМЕ literal `"0"`/`"0%"` (→ 0).
  pcw==100 → класс `stretch`; иначе `style="width: N%;"` и НИКАКОГО
  class-маркера. Явный width подавляет `fit-content` даже при `%autowidth`
  (p3/p10: autowidth+width=50% → style; autowidth+width=100% → stretch);
  colgroup при autowidth всегда голый `<col>`.
- **Веса колонок**: trailing-стилевая буква не часть веса —
  `cols="1m,3m"` → 25%/75% (у нас было 50/50: trailing-digits пуст → 1.0).
- **Passthrough в unconstrained-спане**: `**a+++**+++b**` → strong над
  `a**b` — passthrough извлекается ДО quotes; наш find_closing_unconstrained
  не скипал спаны (constrained уже умел). Закрыл `+++**+++` внутри listing
  с `subs="+quotes,+macros"` (pass-macro).
- **Escaped pipe**: `|` сразу после `\` — НЕ разделитель ячеек, ровно один
  `\` снимается (`\|`→`|`, `\\|`→`\|` в одной ячейке); работает и в
  continuation-строках (`tail \| more` → `tail | more` в открытой ячейке).
  Строка только с escaped-пайпами — чистая continuation.

### Что сделано
- **РЕНДЕРЕР** blocks.rs start_table: tablepcwidth (to_i-эмуляция со знаком),
  width_style → ` style="…"` после class в `<table>`; fit-content гейтится
  `width_value.is_none()`. parse_col_widths: strip_suffix [adehlmsv] перед
  выделением веса.
- **ПАРСЕР** inline.rs find_closing_unconstrained: скип passthrough_span_len
  / pass_macro_span_len (зеркало constrained), возврат search_start+i.
- **ПАРСЕР** scanner.rs: unescape_cell_pipes (Cow), find_unescaped_pipe,
  split_unescaped_pipes; parse_table_cells сплитит по unescaped-пайпам,
  unescape контента и continuation; `TableLineCells.continuation` →
  `Option<Cow>`. block.rs: append_cell_continuation берёт `&str`
  (else-ветка Owned), None-путь parse_table_cells → unescape_cell_pipes.
- Тесты: +1 scanner (escaped pipe, 5 кейсов), +4 html (width 8 кейсов,
  styled col weights, unconstrained-скип, escaped pipe 3 кейса);
  2 ассерта continuation → as_deref().

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 486, html 374).
- Пробы все IDENTICAL (p2-остаток `\\`→`\` — давний eager-`\\`-escape предел,
  не от этого фикса).
- **Корпус: Identical 292→295 (+3)**: pass-macro 3→0, delimited 9→0,
  data-format 615→0. Blast (base 0a1e5fc): ровно 4 файла — 3 флипа,
  character-replacement-ref 616→625 — позиционный сдвиг поверх pre-existing
  корня (vbar-строка `|\|` теперь рендерится `|` как asciidoctor, BASE давал
  `\`+пустую; первый diff-корень файла тот же), **0 регрессий**.

### Что дальше
- nearmiss пересчитать на 295; кандидаты по прошлому списку: replacements
  (4 — NCR, скип), subs (76), ordered (90), part-with-special-sections (103),
  multipart-book (109), quote (109 — `-- Author` attribution), metadata (111),
  apply-subs-to-text (115), image (125), ts-url-format (125).
- Кандидаты-корни: `++…++` double-plus НЕ экранирует спецсимволы
  (block-name-table 431, проба /tmp/p_acell/p11 прошлой сессии);
  syntax-quick-reference — file-level корень (нет `<div id="content">`).
- Pre-existing из прошлых сессий: blank в DEFAULT-ячейке → второй `<p>`,
  footnotes-div внутри a-ячейки, nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `[abstract]`-параграф → quoteblock, `:icons:`-colist,
  unknown-style в class на quote/sidebar, list-merge через
  continuation-attrlist, author-line после attr-entry в header, label
  block-anchor `[[id,label]]` над блоком не побеждает `.Title`,
  `\\https://…` двойной backslash, CSV drop incomplete row,
  eager-`\\`-escape ест первый backslash (`\\` в ячейке → `\`).

---

## Сессия (2026-06-12, тридцать третья) — Фаза 3: named-footnote reuse + a-ячейки (nested-парсинг) + наследование колоночных стилей + literal-ячейки + blank/indent в ячейках

Запрос «продолжи». ДВЕ ветки, обе СМЕРЖЕНЫ в master и запушены:
`fix/footnote-named-reuse` (`096bd8d`) и `fix/asciidoc-table-cell` (`b742c4b`),
локальные ветки удалены. Baseline старта: Identical 284, master `8c95141`.

### Задача 1: footnote examples (70 diff) — named-footnote reuse
Семантика (пробы /tmp/p_fnr/p1..p3): повторное использование footnote с уже
определённым id — ССЫЛКА (`<sup class="footnoteref">`, анкор БЕЗ id, номер
первого определения), текст повтора игнорируется, счётчик не бампится — даже
при `footnote:id[с текстом]`. Пустой `footnote:id[]` без определения —
`<sup class="footnoteref red" title="Unresolved footnote reference.">[id]</sup>`
(forward-ref НЕТ, строго document-order). Фикс: **РЕНДЕРЕР** events.rs — арм
Event::Footnote сначала lookup (ref-форма через хелпер push_footnote_ref),
арм FootnoteRef — ref-форма/unresolved-маркер. 1 тест переписан (фиксировал
неверную ref-форму), +1 тест. **Корпус 284→285** (footnote examples 70→0),
blast: ровно 1 файл, 0 регрессий.

### Задача 2: a-ячейки + кластер табличных стилей (bibliography 72 → весь кластер)
Семантика (пробы /tmp/p_acell/p1..p12):
- `a|`-ячейка (или `a`-колонка): `<td ...><div class="content">` + ПОЛНЫЙ
  nested block-парсинг контента (списки, listing, example...), trailing
  newline отстрижен перед `</div></td>`; пустая a-ячейка —
  `<div class="content"></div>`; в header-строке стили колонок ИГНОРИРУЮТСЯ
  (плоский th). Footnote из a-ячейки делит СЧЁТЧИК с внешним документом,
  xref на внешнюю секцию резолвится → nested-события надо гнать через ТОТ ЖЕ
  рендерер, не отдельный экземпляр.
- Колоночные стили наследуются ячейками без явного стиля: m/s/e-обёртки
  (inline subs работают внутри), `l` → `<div class="literal"><pre>` с
  VERBATIM-subs (`{empty}`/`*b*` литеральны, спецсимволы экранируются).
- НОВЫЕ спек-чары `d` (explicit default, ПОБЕЖДАЕТ колоночный стиль) и `v`
  (verse — рендерится как default). Без их распознавания `d|x` считался
  continuation-текстом и ломал header-промоушн.
- Blank-строки и отступы continuation-строк — ЧАСТЬ контента ячейки:
  структурны для `a` (два параграфа!), сохраняются в `l` (pre), для
  остальных стилей схлопываются (старое поведение). Края целого текста
  ячейки стрипаются.

### Что сделано (задача 2)
- **ПАРСЕР** parser.rs: стек `cell_subs_pushed` — Start(TableCell):
  AsciiDoc→subs NONE (сырой Text), Literal→VERBATIM; pop на End.
- **ПАРСЕР** scanner.rs: `style_explicit: bool` в CellSpec/ExactCellSpec;
  d/v в обоих style-парсерах; parse_cell_style_suffix возвращает 3-tuple;
  parse_table_cells сохраняет отступ continuation-текста (prefix от сырой
  строки, не trim_start).
- **ПАРСЕР** block.rs: resolve_style наследует ЛЮБОЙ колоночный стиль при
  `!style_explicit && style==Default`; blank-строка при открытых ячейках →
  append_cell_continuation(""); строка без `|` — trim_end (не trim);
  `cell_text(cell, style)` на эмиссии: a/l — trim краёв целиком, остальные —
  lines().map(trim).filter(non-empty).join("\n").
- **РЕНДЕРЕР** lib.rs: стек `acell_capture: Vec<String>`; blocks.rs
  start_table_cell AsciiDoc-арм: `<div class="content">` + push капчура;
  events.rs Text-арм: guard капчура (сырой текст в буфер, return);
  TagEnd::TableCell AsciiDoc-арм: pop ДО рендера, nested
  `adoc_parser::Parser::new(&raw)` события через self.push_event в тот же
  output, pop trailing `\n`, `</div>`; Literal-армы: `<div class="literal">
  <pre>`/`</pre></div>` (было `<p><code>`).
- Тесты: literal-тест переписан (фиксировал `<p><code>`), +3 новых
  (a-cell 4 кейса, column-inheritance 6 кейсов, literal blank/indent).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (946 passed).
- Пробы p1..p12 IDENTICAL, кроме p6 — задокументированный предел: footnote,
  ОПРЕДЕЛЁННАЯ в a-ячейке — asciidoctor эмитит nested footnotes-div ВНУТРИ
  ячейки, у нас уходит во внешнюю секцию (счётчик общий — совпадает).
- **Корпус: Identical 285→292 (+7)**: asciidoc-vs-markdown 988→0(!),
  blocks/index, ordered 90→0, footnote pages 165→0, bibliography 72→0,
  format-cell-content 154→0, format-column-content 195→0. Blast (base
  096bd8d): 18 файлов — 7 флипов, **0 регрессий**, сильно ближе: pass-macro
  241→3, table-ref 893→135, delimited 307→9, highlight-lines 286→185,
  document-attributes-ref 6583→6363, character-replacement-ref 641→616;
  рост syntax-quick-reference 2734→2828 / block-name-table 396→431 /
  image-svg 263→282 — позиционный сдвиг поверх pre-existing корней
  (сверено: syntax-quick-reference расходится с ПЕРВОГО токена — нет
  `<div id="content">`; block-name-table — `++[<LABEL>]++`-корень ниже;
  image-svg — frame/grid-атрибуты и SVG `<object>`).

### Новые кандидаты-корни (вскрыты этой сессией)
- **`++…++` double-plus pass НЕ экранирует спецсимволы** (pre-existing, НЕ от
  этого фикса): asciidoctor `++[<LABEL>]++` → `[&lt;LABEL&gt;]`, у нас сырой
  `[<LABEL>]`. Только `+++` (triple) пропускает без экранирования. Проба
  /tmp/p_acell/p11. Закрыл бы block-name-table (431) и часть других.
  Вероятный фикс: try_double_plus_passthrough → Event::Text вместо
  InlinePassthrough (рендерер Text экранирует).
- pass-macro остаток 3 diff: `stretch`-класс на таблице (ref без stretch) +
  `+++…+++` внутри ячейки.
- syntax-quick-reference: нет `<div id="content">` с первого токена —
  file-level корень, ВЕСЬ счётчик 2828 — сдвиг.
- table-ref остаток 135, cell.adoc 965 (blank в DEFAULT-ячейке → второй
  `<p class="tableblock">` — предел остался), table.adoc 597 (`|=== <1>` в
  параграфе → colist, корень прошлой сессии).

### Что дальше
- nearmiss пересчитать на 292; кандидаты: replacements (4 — NCR, скип),
  pass-macro (3!), delimited (9!), subs (76), part-with-special-sections
  (103), multipart-book (109), quote (109 — `-- Author` attribution),
  metadata (111), apply-subs-to-text (115).
- Pre-existing из прошлых сессий: blank в DEFAULT-ячейке → второй `<p>`,
  footnotes-div внутри a-ячейки (новый предел), nested-список с другим
  маркером в li, `[square]`-класс, компактный colist-`<li><p>`, `== heading`
  не прерывает параграф, `[abstract]`-параграф → quoteblock, `:icons:`-colist,
  unknown-style в class на quote/sidebar, list-merge через
  continuation-attrlist, author-line после attr-entry в header, label
  block-anchor `[[id,label]]` над блоком не побеждает `.Title`,
  `\\https://…` двойной backslash, CSV drop incomplete row.

---

## Сессия (2026-06-12, тридцать вторая) — Фаза 3: blank после `|===` гасит implicit header

Запрос «продолжи». Ветка **`fix/add-columns-nearmiss`** — ЗАКОММИЧЕНА
(`f5a9afd`), смержена в master (`7d9f2eb`), запушена, локальная ветка
удалена. Baseline: Identical 282, master `43f7ab1`
(base-бинарь /tmp/adoc_base пересобран с master).

### Выбор задачи
nearmiss: replacements.adoc (4 diff) — известный NCR-кластер, скип;
**add-columns.adoc (40 diff)** — один корень.

### Семантика asciidoctor (пробы /tmp/p_ac/p1..p8, t1 — все IDENTICAL)
- Blank-строка (одна или несколько) МЕЖДУ `|===` и первой data-строкой
  гасит implicit header promotion (p1/p3); явный `[%header]` всё равно
  промоутит (p4); colcount по-прежнему из первой строки (p3, 2 колонки).
- Comment-строка прозрачна: `|===`+comment+row+blank → header ЕСТЬ (p6);
  но blank до/после comment (до первой data-строки) — гасит (p7/p8).

### Что сделано (ПАРСЕР block.rs scan_table)
- Флаг `blank_before_first_data` — взводится на blank при
  `first_data_idx.is_none()`; добавлен в гейт `implicit_header` (`&& !…`).
- +1 html-тест `test_table_leading_blank_suppresses_implicit_header_html`
  (6 кейсов: blank/несколько blank/comment+blank/только comment/явный
  %header/colcount).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 485, html 366).
- Пробы p1..p8 и add-columns.adoc IDENTICAL.
- **Корпус: Identical 282→284 (+2)**; blast (base 43f7ab1): 4 файла —
  2 флипа (add-columns 40→0, column.adoc 172→0), cell.adoc 975→965 ближе,
  table.adoc 556→597 — позиционный шум поверх pre-existing корня
  (`|=== <1>` в параграфе → у нас colist; изолированная таблица из файла
  сверена: thead у обоих 0, BASE был неправ), **0 семантических регрессий**.
- Закоммичено (`f5a9afd`), смержено в master (`7d9f2eb`), запушено; локальная ветка удалена.

### Что дальше
- nearmiss на 284: replacements (4 — NCR-кластер, в одиночку бесполезен),
  footnote examples (70), bibliography (72), subs (76), ordered (90),
  part-with-special-sections (103), multipart-book (109), quote (109 —
  `-- Author` attribution), metadata (111), apply-subs-to-text (115).
- Кандидаты-корни прошлых сессий: `cols=2;2;3;3` `;`-разделитель
  (image-ref, image-svg); `l|`-ячейка → `<div class="literal"><pre>`
  (image-svg); `[frame=ends,grid=none]` (image-svg); НОВЫЙ: `|=== <1>` в
  параграфе не должен открывать colist (table.adoc — крупный позиционный
  корень).
- Pre-existing из прошлых сессий: ячейка `a|` nested-парсинг, nested-список
  с другим маркером в li, `[square]`-класс, компактный colist-`<li><p>`,
  `== heading` не прерывает параграф, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, label block-anchor `[[id,label]]` над блоком не
  побеждает `.Title`, `\\https://…` двойной backslash, blank в ячейке →
  второй `<p>`, CSV drop incomplete row.

---

## Сессия (2026-06-12, тридцать первая) — Фаза 3: таблицы — открытая модель ячейки (continuation/пустые/дупликация/спек-цепочки/drop-row/comments)

Запрос «продолжи». Ветка **`fix/align-by-column`** — ЗАКОММИЧЕНА (`0fe6e49`),
смержена в master (`0c5418a`), запушена, локальная ветка удалена.
Baseline: Identical 267, master `4099d62` (прошлая ветка смержена;
base-бинарь /tmp/adoc_base пересобран с master).

### Выбор задачи
nearmiss: **align-by-column.adoc (7 diff)** — один видимый корень
(continuation-строки ячеек), но фикс вскрыл кластер psv-семантики, добит
целиком (6 подкорней).

### Семантика asciidoctor (пробы /tmp/p_abc/p1..p17)
- **Continuation**: текст до первого `|` строки (или строка без `|` вовсе) —
  продолжение последней ячейки предыдущей строки, join `\n` в ОДНОМ
  `<p class="tableblock">` (p1/p2/p6); спек между текстом и `|` — спек
  следующей ячейки (`tail 2+|wide`, p8); без предыдущей ячейки текст
  открывает собственную (p3/p7).
- **Header**: implicit header ТОЛЬКО если blank сразу после первой строки И
  следующая non-blank строка начинается с ячейки — continuation до (p5) или
  после (p9) blank гасит промоушн.
- **Colcount**: имплицитное число колонок = ячейки первой строки, пока та
  «открыта»: ячейка, открытая mid-line на continuation-строке, считается
  (p6: `|a` + `mid |late` → 2 колонки; p1: `|cell two` с новой строки → 1).
- **Drop incomplete row**: ячейки неполной последней строки дропаются
  («dropping cells from incomplete row detected end of table», p7/p10);
  CSV-путь у asciidoctor тоже дропает (p11) — у нас НЕТ (предел).
- **Пустые ячейки**: `|a |` → 2 ячейки, `|a | |c` → mid-пустая; рендер —
  голый `<td></td>` без `<p>` (p12/p13).
- **Дупликация/цепочки**: `2*>m|x` → ячейка ×2 right+mono; `.2+^.>s|` —
  span+align+style цепочкой (CellSpecRx: factor, align, style; спек требует
  пробельной границы слева); копии дупликации несут ПОЛНЫЙ контент включая
  continuation-строки (p15, cell.adoc).
- **Comments**: line-comment в таблице невидим — дроп из контента ячейки, не
  влияет на header/colcount (p17; закрыл style-operators 1 diff и section-ref).
- Blank внутри ячейки → ВТОРОЙ `<p>` в той же ячейке (p9/p16) — НЕ сделано
  (у нас join `\n`), задокументированный предел.

### Что сделано
- **ПАРСЕР** scanner.rs: `parse_table_cells` → `Option<TableLineCells
  { continuation: Option<&str>, cells }>`; `CellSpec.content: Cow<str>`
  (+ поле `duplication: u8`, раскрывается потребителем); НОВЫЙ
  `parse_cell_spec_exact(s) -> Option<ExactCellSpec>` (вся строка = спек;
  префикс и whitespace-отделённый токен в non-last частях); пустые части
  всегда пушатся как ячейки; legacy-суффикс-цепочка осталась fallback'ом
  (квирк `x2+` без пробела сохранён).
- **ПАРСЕР** block.rs scan_table: цикл сбора — скип comment-строк,
  `append_cell_continuation` (join `\n`, в пустую — без `\n`, без ячеек —
  новая), first_row_width пока строка открыта (×duplication), header-гейт
  (blank at first+1 && post_blank_line_starts_cell && width==num_cols);
  экспансия дупликации ПОСЛЕ сбора (`repeat_n`); build_table_rows: последняя
  строка пушится только если заполняет грид (trailing rowspan-occupancy
  учитывается).
- **ПАРСЕР** parser.rs: арм `Event::Text(Cow::Owned)` → inline-парсинг с
  into_static (раньше Owned-текст шёл сырым: слитые ячейки и CSV-поля не
  получали typographic/quotes — отсюда флипы subs-файлов).
- **РЕНДЕРЕР** lib.rs/blocks.rs/events.rs: `cell_p_start_stack` — позиция
  после открывающего `<p class="tableblock">`; на TagEnd::TableCell пустая
  ячейка → truncate `<p>` → `<td></td>` (как asciidoctor).
- Тесты: +2 scanner (exact-spec, duplication unexpanded), +3 html
  (continuation 6 кейсов + comments, пустые ячейки, дупликация/цепочки),
  обновлены `| A | B |` (теперь пустая 3-я ячейка) и trailing-spec ассерты;
  тестовые вызовы переведены на хелпер `line_cells`.

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (941 passed).
- Пробы p1..p17 IDENTICAL, кроме пределов: p9/p16 (второй `<p>` после blank
  в ячейке), p11 (CSV drop incomplete row), p14 (`a|` nested-рендер — давний).
- **Корпус: Identical 267→282 (+15)**; blast (base 4099d62): 37 файлов —
  15 флипов (align-by-column 7→0, build-a-basic-table, add-cells-and-rows,
  row, style-operators 126→0, section-ref 626→0, header-ref, audio-and-video,
  link-macro-ref, unresolved-references, toc-ref, subs/attributes,
  post-replacements, quotes, special-characters), **0 регрессий**, остальные
  в основном ближе (table 612→556, subs-symbol-repl 226→165, replacements
  148→4 — остаток NCR-кластер, document-attributes-ref 6672→6538, ordered
  232→227); рост image-ref 659→746 / image-svg / cell / table-ref —
  позиционный шум поверх pre-existing корней, новые фрагменты сверены с
  эталоном (слитые ячейки = asciidoctor).
- Закоммичено (`0fe6e49`), смержено в master (`0c5418a`), запушено; локальная ветка удалена.

### Известные пределы (вне корпуса)
- Blank в ячейке → второй `<p class="tableblock">` (у нас один `<p>` с `\n`).
- CSV: неполная последняя строка не дропается (отдельный путь
  scan_delimited_format_table).
- `a|`-ячейка: нет nested-парсинга в `<div class="content">` (давний).
- `|e|x` без пробела: у нас `e` — спек (asciidoctor: контент, нужен
  whitespace перед спеком) — legacy-квирк, сохранён сознательно.

### Что дальше
- nearmiss на 282: **add-columns (40)**, footnote examples (70),
  bibliography (72), subs (76), ordered (90), part-with-special-sections
  (103), multipart-book (109), quote (109 — `-- Author` attribution),
  metadata (111), apply-subs-to-text (115).
- Новые кандидаты-корни из этой сессии: `cols=2;2;3;3` — `;`-разделитель
  cols не парсится (image-ref, image-svg); `l|`-ячейка должна рендериться
  `<div class="literal"><pre>` (image-svg); `[frame=ends,grid=none]` на
  таблице (image-svg); NCR-в-monospace кластер (replacements 4 diff —
  по памяти бесполезен в одиночку).
- Pre-existing из прошлых сессий: ячейка `a|` nested-парсинг, nested-список
  с другим маркером в li, `[square]`-класс, компактный colist-`<li><p>`,
  `== heading` не прерывает параграф, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, label block-anchor `[[id,label]]` над блоком не
  побеждает `.Title`, `\\https://…` двойной backslash.

---

## Сессия (2026-06-12, тридцатая) — Фаза 3: footnotes вне #content + merge стопки attrlist + cols-multiplier + trailing cell-spec + счётчики в verbatim

Запрос «продолжи». Ветка **`fix/pages-include-nearmiss`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 262, master `313a275`
(base-бинарь /tmp/adoc_base пересобран с master — ветка стартовала с него же).

### Выбор задачи
nearmiss: **pages/include.adoc (8 diff)** — один корень (footnotes); затем
**customize-title-label.adoc (66 diff)** — три корня, по дороге вскрыт и закрыт
четвёртый (pre-existing trailing cell-spec).

### Семантика asciidoctor (пробы /tmp/p_fn/p1, /tmp/p_ctl/p1..p11, m*, n*)
- **A (footnotes)**: `<div id="footnotes">` идёт ПОСЛЕ закрытия `</div>`
  `#content`, ПЕРЕД `<div id="footer">` (p_fn/p1).
- **B (стопка attrlist-строк)**: метаданные НАКАПЛИВАЮТСЯ, не заменяются:
  named — override по ключу; id — последний побеждает; roles/options —
  аккумулируются (`[#id1.r1]`+`[#id2.r2]` → id2, r1 r2, p8); позиционные —
  послотно: `[quote,Author]`+`[verse]` → verse + attribution (p9); пустой
  слот 1 не затирает стиль: `[source,ruby]`+`[,python]` → python (p10).
- **C (cols multiplier)**: `3*` → 3 колонки (33.3333/33.3333/33.3334 —
  последняя получает остаток), `2*1,3` → 20/20/60, `2*<.^2,>1` → 40/40/20
  со спеком на обеих (p2). caption= на таблице: verbatim-префикс, счётчик НЕ
  бампится (`Table A.`), пустой `[caption=]`/`[caption=""]` → голый title (p3).
- **D (trailing cell-spec)**: спек ячейки привязан к СЛЕДУЮЩЕМУ `|` — в конце
  строки это контент: `|a` → ячейка «a» (не AsciiDoc-style), `|d |e` хранит
  «e»; в середине строки `|one a|two` — спек следующей (проба n4).
- **E (счётчики в verbatim)**: include/conditionals — уровень READER (работают
  в listing!), счётчики `{counter:}`/attr-entries — уровень substitutions/блоков
  (в listing/literal/pass/comment/markdown-fence НЕ работают).

### Что сделано (5 точек)
- **РЕНДЕРЕР** finish.rs: render_footnotes гейтится `!standalone`; lib.rs run():
  footnotes эмитятся после `</div>` content, перед footer.
- **ПАРСЕР** attributes.rs: `BlockAttributes::merge(older, newer)` (id
  last-wins, roles/options extend, named override, позиционные послотно,
  выравнивание implied_source_lang при смешанных формах); block.rs ~615 —
  attrlist-арм мержит вместо замены.
- **РЕНДЕРЕР** blocks.rs `parse_col_widths`: multiplier `N*` раскрывается
  (зеркало parse_col_spec парсера, который уже умел).
- **ПАРСЕР** scanner.rs `parse_table_cells`: спек-суффиксы (style/span/align)
  парсятся только для НЕ-последней части строки (pre-existing: `|a` терял
  ячейку целиком, `|d |e` терял «e», `<.>` в конце строки ячейки съедался —
  этим был сломан и ряд corpus-таблиц).
- **ПАРСЕР** preprocessor.rs: трекинг verbatim-фенсов (`----`/`....`/`++++`/
  `////` с точной длиной закрытия + markdown ```) — внутри: счётчики не
  раскрываются, attr-entries не потребляются; conditionals/endif работают
  по-прежнему (reader-level, обрабатываются до фенс-проверки).
- Тесты: +1 html (footnotes вне content), +1 scanner (trailing cell-spec),
  +1 attributes (merge, 5 сценариев), +1 preprocessor (verbatim-фенсы,
  4 сценария), +3 html (стопка attrlist, multiplier-ширины, `|a`-контент),
  +1 html (counter literal в listing через preprocess).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (935 passed).
- Все пробы IDENTICAL кроме n4 — pre-existing предел: ячейка `a|` (AsciiDoc
  style) не рендерится как вложенный content-div (требует nested-парсинга).
- **Корпус: Identical 262→267 (+5)**; blast (base 313a275): 17 файлов —
  5 флипов (pages/include 8→0, customize-title-label 66→0, subs-group-table
  ×2, image-position), **0 регрессий**, 10 ближе (align-by-column 617→7!,
  row 310→81, add-columns 211→40, footnote 101→70, image-svg 312→263,
  pass-macro 249→241), column 168→172 и table 560→612 — позиционный шум,
  точечно сверено с эталоном (новые фрагменты = asciidoctor: 50/50-колонки,
  `<.>`-текст в ячейке сохранён).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Известные пределы (вне корпуса)
- Ячейка `a|` (AsciiDoc style): спек парсится, но рендерер не оборачивает в
  `<div class="content">` с nested-парсингом (давний архитектурный, n4).
- `[subs="+attributes"]` на listing: asciidoctor раскрыл бы счётчик при
  рендере — наш препроцессор внутрь фенса не заходит вовсе.
- merge позиционных при экзотике (newer со слотами 3+ без стиля) — послотное
  выравнивание приближённое (наша модель не хранит сырые слоты).

### Что дальше
- nearmiss на 267: **align-by-column (7 diff!)** — почти флип, разведать
  первым; add-columns (40), footnote (70), subs (76), bibliography (77),
  row (81), ordered (90), part-with-special-sections (103),
  multipart-book/quote (109 — quote: `-- Author` attribution не реализован),
  metadata (111 — позиционный шум).
- Pre-existing из прошлых сессий: ячейка `a|` nested-парсинг, nested-список
  с другим маркером в li, `[square]`-класс, компактный colist-`<li><p>`,
  `== heading` не прерывает параграф, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, label block-anchor `[[id,label]]` над блоком не
  побеждает `.Title`, `\\https://…` двойной backslash.

---

## Сессия (2026-06-12, двадцать девятая) — Фаза 3: include.adoc examples + links.adoc (форма include-директивы + comment в параграфах + autolink-границы/escape)

Запрос «продолжи». Ветка **`fix/include-directive-shape-and-mid-paragraph-comments`** —
НЕ закоммичена (рабочее дерево). Baseline: Identical 259, master `248d240`
(base-бинарь /tmp/adoc_base пересобран с master — тот же HEAD).

### Выбор задачи
nearmiss: **examples/include.adoc (52 diff)** — три корня (третий обнажился
по ходу); после него добит links.adoc (оставался 1 diff — escaped autolink).

### Семантика asciidoctor (пробы /tmp/p_inc: p1..p11, pA..pE, q1..q13, r1..r4)
- **A (include-shape)**: IncludeDirectiveRx заякорен — `include::` с колонки 0
  (индент → литерал/literal-блок, p9), `]` — ПОСЛЕДНИЙ символ строки (rstrip;
  `include::core.rb[tag=parse] <.>` → НЕ директива: сырой текст + conum, p1/p2);
  trailing-пробелы ок (p7); пробел ВНУТРИ target ок, на краях — нет.
  `\include::…[] tail` — не directive-shaped → НЕ escape, backslash остаётся (p10).
- **B (comment в параграфе)**: line-comment в середине параграфа дропается,
  строки сливаются в один `<p>` (p3/p5) — то же в admonition (pA), ulist (pB),
  dlist dd (pC), olist (pD); в verse/verbatim — контент (pE); comment+blank
  завершает параграф (p4); `////` рвёт (p6); «comment после blank рвёт списки»
  не затронуто.
- **C (autolink-границы)**: bare-URL линкуется только после старта строки,
  пробела или `<>()[];` — `:` (q1! — отсюда литеральная `include::https://…[]`
  линковалась), `-`(q3), `=`(q5), `,`(q6), straight `"`(q8/q9) блокируют;
  `'` у asciidoctor линкует НЕ из-за кавычки, а из-за `;` NCR `&#8217;` (q10).
  Trailing `)` никогда не входит в bare-URL — стрипаются ВСЕ (r1/r4, даже от
  `foo(bar)`), `;`/`:` тоже (r2/r3); но форма `URL[text]` — ДРУГОЙ альтернат
  regex: URL до `[` целиком, `)` сохраняется.

### Что сделано (ПАРСЕР, 4 файла)
- scanner.rs `is_include_directive`: без leading-trim, `strip_suffix(']')` после
  rstrip, path без краевых пробелов (по построению без `[`).
- preprocessor.rs: escaped-ветка — `strip_prefix('\\')` +
  `is_include_directive(rest)`-гейт (вместо безусловного starts_with).
- block.rs: skip-арм `is_line_comment` (advance+continue) в scan_paragraph
  (гейт `!verbatim_paragraph`, при пустом para_lines — break как раньше),
  scan_admonition, 3 цикла wrapped-строк (ulist/olist/colist, replace_all),
  dd-цикл dlist.
- inline.rs `try_autolink`: boundary-check prev-символа (старт/whitespace/
  `<>()[];`, хелперы `at_autolink_boundary`/`autolink_scheme_at`);
  trailing-стрип получил `)` и гейтится `!bracket_follows`
  (форма `URL[text]` идёт нестрипнутой — фикс регрессии key-concepts.adoc 0→3,
  пойманной первым blast'ом).
- inline.rs: НОВЫЙ escape-арм `\https://…` (handle_inline_escape) — backslash
  дропается, URL литерален; гейт: MACROS + autolink_scheme_at + валидная
  граница ПЕРЕД `\` (s-пробы: `word-\https` и `\\https` хранят backslash;
  сам URL не линкуется, т.к. prev для него — оставшийся в input `\`).
  Закрыл links.adoc (232→0, кейс `` `\https://…` `` в monospace).
- Тесты: test_line_comment_skipped переписан (фиксировал разрыв параграфа);
  +5 ассертов в test_is_include_directive; +1 preprocessor
  (non-directive verbatim, indent); +1 parser (comment в ulist-item);
  +2 html (merge параграф/admonition/dd/olist/verse/blank-негатив;
  autolink-границы + trailing-paren + escaped-autolink 3 кейса).

### Статус (верифицировано)
- clippy --workspace 0 (после touch — не кэш); cargo test --workspace зелёное
  (parser 480, html 356).
- Все 20+ проб IDENTICAL (нормализация compare_full), кроме s5 — известный
  pre-existing предел; examples/include.adoc 52→0.
- **Корпус: Identical 259→262 (+3)**; blast (base 248d240): 11 файлов —
  3 флипа (examples/include.adoc, document-attributes.adoc 284→0 — corpus-файл
  с массой comment-в-параграфах, links.adoc 232→0), **0 регрессий**, 5 ближе:
  pages/include.adoc 75→8, image-ref 686→659, subs.adoc 89→76, image 126→125,
  sdr-005 377→372; metadata.adoc 108→111 и outline.adoc 8664→8681 — позиционный
  сдвиговый шум, точечно сверено с эталоном (новый вывод = asciidoctor).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Известные пределы (вне корпуса)
- `a'https://…`: asciidoctor линкует (boundary = `;` от NCR `&#8217;` после
  replacements), мы — нет (сырой UTF-8 `'`).
- URL сразу после inline-спана (`*b*https://…`): asciidoctor линкует (`>` от
  `</strong>` в substituted-тексте), у нас prev=`*` → литерал (chunk-граница,
  родственно em-dash-пределу 28-й сессии).
- `\\https://…` (s5): asciidoctor хранит ОБА backslash; наш eager `\\`-escape
  съедает первый (pre-existing escape-модель, упоминалась в 23-й сессии).

### Что дальше
- nearmiss на 262: **pages/include.adoc (8 diff!)** — почти флип, разведать
  первым; customize-title-label (66), bibliography (77), subs (76),
  subs-group-table (90), ordered (90), footnote (101),
  part-with-special-sections (103), metadata (111 — позиционный шум, реально
  ближе). Кандидат-корень quote.adoc: `-- Author` attribution не реализован.
- Pre-existing из прошлых сессий: nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `cols="2*"` multiplier, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, label block-anchor `[[id,label]]` над блоком не
  побеждает `.Title`. («comment в середине dd-параграфа» — ЗАКРЫТ этой сессией.)

---

## Сессия (2026-06-12, двадцать восьмая) — Фаза 3: source.adoc (em-dash правила + include-строка = текст)

Запрос «продолжи». Ветка **`fix/source-block-nearmiss`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 258, master `6c5d1a3`
(base-бинарь /tmp/adoc_base пересобран через временный worktree).

### Выбор задачи
nearmiss: **source.adoc (63 diff)** — два корня. В файле `---- <.>` — не
делимитер → ВСЕ пары `----` смещены (asciidoctor так же! warning «unterminated
listing block»); расходились только: (A) em-dash в параграфе `---- <.>`,
(B) escaped-include строки.

### Семантика asciidoctor (пробы /tmp/p_src/p1..p7, все IDENTICAL после фикса)
- **A (em-dash)**: правила ровно `(\w)--(?=\w)` → em+ZWSP и
  `(^|\n| |\\)--( |\n|$)` → thin+em+thin (граничный пробел/`\n` ПОГЛОЩАЕТСЯ —
  строки сливаются; gsub: в `a -- -- b` второй `--` литерал). Правила `---`
  НЕТ: `a---b`, `g --- h`, `e----f`, `----` — литералы. `\--` — escape только
  там, где матчился бы unescaped (`\---` → backslash ОСТАЁТСЯ литералом).
- **B (include)**: include резолвится ТОЛЬКО в reader (наш препроцессор);
  строка `include::…[]`, дошедшая до парсера (от escaped `\include::`), —
  обычный ТЕКСТ: параграф, не рвёт параграфы/списки (пробы p5/p6).

### Что сделано (ПАРСЕР, 2 файла)
- inline.rs `apply_typographic_replacements`: арм `---` УДАЛЁН; spaced-арм —
  границы `^`/`\n`/пробел/конец с обеих сторон, граничный символ поглощается,
  guard `i > copied_up_to` (gsub-семантика); word-арм без изменений.
  `typographic_escape_len`: `\--` валиден только при (word-before+word-after)
  или (пробел/`\n`/EOL после) — `\---` больше не эскейпится.
  Пределы (вне корпуса): chunk-границы после inline-конструкций считаются
  line-границами (`*b*-- x` заменили бы, asciidoctor нет); merge строк через
  SoftBreak не делается (EOL `--` даёт em-dash, но `\n` остаётся).
- block.rs: include-арм УДАЛЁН из scan_directives + 4 break-условия
  (`is_include_directive`) из paragraph/list-сканов. Event::Include в enum
  остаётся (API), арм рендерера `<!-- include:: -->` — мёртвый, не тронут.
  scanner::is_include_directive жив (препроцессор).
- Тесты: 5 переписаны (2 block include → plain-text/не-рвёт; inline
  `hello---world`×2, mixed, `\---`-escape — фиксировали самодельную
  семантику), +кейсы в test_typographic_em_dash/spaced (literals, границы
  строк, gsub `a -- -- b` — probe-verified).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 478, html 354).
- Пробы p1..p7 IDENTICAL (нормализованные токены); source.adoc 63→0 diff.
- **Корпус: Identical 258→259 (+1)**; blast (base 6c5d1a3): 7 файлов —
  1 флип (source.adoc), **0 регрессий**, include.adoc 124→52 (сильно ближе),
  остальные 5 — равный счётчик (subs-symbol-repl/delimited: em-dash токены
  стали INREF; quote/data — noref-шум, другие корни).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 259: **include.adoc examples (52** — Unresolved-directive
  семантика?), customize-title-label (66), include pages (75),
  bibliography (77), subs (89), subs-group-table (90), ordered (90),
  footnote (101), part-with-special-sections (103), metadata (108).
- Замечен кандидат-корень quote.adoc (109): строка `-- Author` после
  кавычки-параграфа — attribution quote-блока не реализован.
- Pre-existing из прошлых сессий: nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `cols="2*"` multiplier, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, comment в середине dd-параграфа, label block-anchor
  `[[id,label]]` над блоком не побеждает `.Title`.

---

## Сессия (2026-06-12, двадцать седьмая) — Фаза 3: stem (4 корня: stem-эскейпы, block-macro catch-all, ++++, {n!})

Запрос «продолжи». Ветка **`fix/stem-block-macro-and-escapes`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 257, master `df05b5f`
(base-бинарь /tmp/adoc_base пересобран через временный worktree).

### Выбор задачи
nearmiss: **stem.adoc (56 diff)** — давно откладывался как «3-4 корня», но все
корни оказались малыми и хорошо локализуемыми.

### Семантика asciidoctor (пробы /tmp/p_st/p1..p5, все после фикса байт-в-байт)
- **A**: `stem:[…]` — `\]` НЕ закрывает макрос и unescape'ится в контенте
  (`stem:[[[a,b\],[c,d\]\]((n),(k))]` → `\$[[a,b],[c,d]]((n),(k))\$`;
  правило InlineStemMacroRx `(.*?[^\\])?\]`).
- **B**: блочные макросы матчатся ТОЛЬКО по зарегистрированным именам —
  `stem::[…]`, `foo::bar[baz]`, `chart::data.csv[w=100]` → литеральный параграф
  (`.Title` прикрепляется к нему); зеркало inline-правила 23-й сессии.
- **C**: `++++` в тексте = ПУСТОЙ `++`-passthrough (`++`+`++`) → рендерится в
  ничто; regex asciidoctor `(\+\+\+?)(.*?)\1` бэктрекает с `+++` на `++` с той
  же позиции.
- **D**: имя attr-ref — строго `\w[\w-]*`: `{n!}`/`{x!}`/`{name!fallback}` —
  НЕ референс, литерал (даже если `n` определён). Синтаксиса `!fallback` у
  asciidoctor НЕТ — был самодельный.

### Что сделано (ПАРСЕР, 4 точки)
- inline.rs: `parse_bracket_macro_escaped` (скан `]` с пропуском `\]`, unescape
  через Cow) — используется ТОЛЬКО в `try_stem_macro` (stem/latexmath/asciimath).
- block.rs: арм `scanner::is_custom_block_macro` УДАЛЁН из scan_block_macros;
  scanner.rs: `is_custom_block_macro`/`is_known_block_macro`/`is_valid_macro_name`
  удалены. Tag::CustomBlockMacro в enum остаётся (API), армы рендерера/compat —
  мёртвые, не тронуты.
- inline.rs: triple-plus-арм при провале пробует double-plus с ТОЙ ЖЕ позиции;
  в `try_double_plus_passthrough` close==0 разрешён (пустой → без события).
  Попутно `+++x++` теперь матчится как `++`+`+x`+`++` (бэктрек как asciidoctor).
- inline.rs: `!`-split в attr-ref удалён — content с `!` не парсится как реф;
  поле `fallback` в Event::AttributeReference остаётся (API), парсер всегда
  эмитит None; плюмбинг рендерера не тронут.
- Тесты: 2 parser + 4 html переписаны (фиксировали самодельную семантику
  fallback/custom-block-macro); +2 parser (stem-эскейпы; пустой `++++` +
  литеральный `stem::[…]`), +3 html (unknown block macro + title;
  stem-эскейпы; пустой `++++`).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (924: parser 479, html 354).
- Пробы p1..p5 байт-в-байт; stem.adoc 56→0 diff.
- **Корпус: Identical 257→258 (+1)**; blast (base df05b5f): ровно 1 файл
  изменился — 1 флип (stem.adoc), **0 регрессий** (удаление fallback и
  block-catch-all больше нигде в корпусе не стреляло).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 258: **source (63** — include-«Unresolved directive»-параграфы?,
  `----`→`—-` em-dash в callout-строке листинга), customize-title-label (66),
  include (75), bibliography (77), subs (89), subs-group-table (90),
  ordered (90), footnote (101), part-with-special-sections (103), metadata (108).
- Pre-existing из прошлых сессий: nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `cols="2*"` multiplier, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, comment в середине dd-параграфа должен слить строки
  в один `<p>`; label block-anchor `[[id,label]]` над блоком не побеждает
  `.Title`.

---

## Сессия (2026-06-12, двадцать шестая) — Фаза 3: lexicon (xreflabel/dt-терм → reftext)

Запрос «продолжи». Ветка **`fix/xreflabel-reftext-resolution`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 256, master `f2133db`
(base-бинарь /tmp/adoc_base пересобран через временный worktree).

### Выбор задачи
nearmiss: **lexicon.adoc (34 diff)** — давний кандидат-кластер «xreflabel →
reftext», один корень: все 34 diff'а — нерезолвленные `<<id>>` (у нас fallback
`[id]`, у asciidoctor — текст dt-терма / label).

### Семантика asciidoctor (пробы /tmp/p_xl/p1..7)
- `[[id]]term:: def` → `<<id>>` = текст терма; reftext по умолчанию даёт ТОЛЬКО
  leading-анкер dlist-терма. В параграфах и ulist-item'ах `[[id]]` без label —
  fallback `[id]`. Mid-term анкер (`middle [[jj]]term::`) — тоже fallback.
- `[[id,label]]` / `anchor:id[label]` → label побеждает терм; label
  форматируется при использовании (`label with *bold*` → `<strong>`).
- reftext — разметка: `[[hh]]term with *bold*::` → ссылка содержит
  `term with <strong>bold</strong>`.
- Forward-ref работает (резолв отложен до конца документа).
- Block-anchor `[[id,label]]` НАД блоком: label побеждает `.Title` (p4) —
  НЕ реализовано (предел, в корпусе нет; требует Event::BlockMetadata.reftext).

### Что сделано
- **ПАРСЕР** event.rs: `Tag::Anchor { id, label: Option<CowStr> }` (+into_static);
  inline.rs: `try_anchor` — label из `[[id,label]]` (trim_start, пустой → None),
  `try_anchor_macro` — label из bracket-контента. Тесты обновлены
  (test_anchor_with_reftext_still_works теперь ожидает label).
- **РЕНДЕРЕР** lib.rs: поля `anchor_reftexts: Vec<(String,String)>`,
  `dt_term_start: Option<usize>`, `pending_term_anchor: Option<(String,usize)>`.
  events.rs: Tag::DescriptionTerm — `dt_term_start = output.len()` после
  открывающей разметки (все 3 стиля); арм Anchor — label рендерится через
  render_inline_value → anchor_reftexts; leading-анкер в dt без label →
  pending_term_anchor (id, позиция после `</a>`); TagEnd::DescriptionTerm —
  захват `output[pos..]` как Markup-reftext, сброс dt_term_start.
  finish.rs: цикл `ctx.add_block(id, RefText::Markup)` после bibliography
  (add_block = or_insert, first-wins — секции/блоки/biblio выигрывают).
- +1 html-тест `test_anchor_reftext_xref_resolution` (7 кейсов: dt-терм,
  bold в терме, label с форматированием, anchor-макрос, forward-ref,
  негативы mid-term/параграф).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (923 total).
- Пробы p1..p7 сходятся (кроме документированного предела p4 `<<ee>>`).
- **Корпус: Identical 256→257 (+1)**; blast (base f2133db): ровно 1 файл —
  1 флип (lexicon.adoc 34→0), **0 регрессий**.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 257: stem (56 — 3-4 корня: `\$`-эскейп, `stem::`-макрос literal,
  `++++`+callout, `{n!}`), source (63), customize-title-label (66), include (75),
  bibliography (77), subs (89), subs-group-table (90), ordered (90),
  footnote (101).
- Новый известный предел: label block-anchor-строки `[[id,label]]` над блоком
  не побеждает `.Title` (нужен reftext в Event::BlockMetadata + BlockMeta +
  приоритет над block_ref_titles в finish).
- Pre-existing из прошлых сессий: nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `cols="2*"` multiplier, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, comment в середине dd-параграфа должен слить строки
  в один `<p>`.

---

## Сессия (2026-06-12, двадцать пятая) — Фаза 3: revision-information (had_blank_line не сбрасывался в dlist/colist-сканах)

Запрос «продолжи». Ветка **`fix/revision-information`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 255, master `8edb60d`
(base-бинарь /tmp/adoc_base пересобран через временный worktree).

### Выбор задачи
nearmiss: **revision-information.adoc (24 diff)** — один корень.

### Семантика asciidoctor (пробы /tmp/p_ri1..15)
- Comment-строка СРАЗУ после текста item'а (без blank перед ней) список НЕ рвёт —
  даже если ПЕРЕД этим item'ом была blank-строка (между entries одного dlist).
  Comment ПОСЛЕ blank — рвёт (поведение 18-й сессии, верно).
- Минимальный репро (p_ri13): `a:: x\n\nb:: y\n//c\n\nc:: z` → у asciidoctor
  ОДИН dlist; у нас был раскол после b. То же для colist (p_ri15).

### Корень
`scan_description_list_item` и `scan_callout_list_item` НЕ сбрасывали
`had_blank_line` (в отличие от scan_unordered/ordered_list_item). Blank перед
`b::` оставлял флаг взведённым → comment-handler (block.rs ~870, правило
«comment после blank разделяет списки») ошибочно закрывал список.

### Что сделано (ПАРСЕР block.rs, 2 строки)
- `self.had_blank_line = false` в конце `scan_description_list_item` (~2939) и
  `scan_callout_list_item` (~3161) — зеркало строки 3034 (unordered).
- +1 parser-тест `test_comment_after_dlist_entry_does_not_split_list`
  (позитив + негатив «после blank рвёт»), +1 html-тест
  `test_comment_after_list_entry_keeps_single_list` (dlist, colist, негатив).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 478, html 353).
- Пробы p_ri1..15 все сходятся; revision-information.adoc 24→0 diff.
- **Корпус: Identical 255→256 (+1)**; blast (base 8edb60d): ровно 2 файла —
  1 флип (revision-information.adoc), lexicon.adoc 376→34 (тот же корень рвал
  dlist по всему файлу), **0 регрессий**.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 256: **lexicon (34!** — остаток: xreflabel → reftext для
  xref-резолва, давний кандидат-кластер: label в Tag::Anchor + регистрация в
  XrefResolver + reftext из dt-терма), stem (56 — 3-4 корня: `\$`-эскейп,
  `stem::`-макрос literal, `++++`+callout, `{n!}`), source (63),
  customize-title-label (66), include (75), bibliography (77), subs (89),
  subs-group-table/ordered (90), footnote (101).
- Pre-existing из прошлых сессий: nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `cols="2*"` multiplier, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header, comment в СЕРЕДИНЕ dd-параграфа должен слить строки в
  один `<p>` (p_ri4: asciidoctor «text a\nstill a», у нас два блока).

---

## Сессия (2026-06-12, двадцать четвёртая) — Фаза 3: pass.adoc + revision-line (passthrough-`</div>` + doc-интринсики)

Запрос «продолжи». Ветка **`fix/passthrough-stray-div-and-doc-intrinsics`** —
НЕ закоммичена (рабочее дерево). Baseline: Identical 253, master `99fab03`
(base-бинарь /tmp/adoc_base пересобран через временный worktree).

### Выбор задачи
nearmiss: **pass.adoc (18 diff)** — два корня; попутно закрыт
revision-line-with-version-prefix (1 diff — `{docdate}`, ранее скипался,
оказался того же семейства doc-интринсиков).

### Семантика asciidoctor (пробы /tmp/p_pt1..3, /tmp/probedir/p_doc1..2, p_rev2..4)
- Standalone passthrough (`++++` и `[pass]`-параграф) — контент ГОЛЫЙ, без
  обёртки вовсе (нечего закрывать).
- Интринсики от входного файла: `docname` (stem), `docfile` (abs path),
  `docdir`, `docfilesuffix`; `docdate`/`doctime`/`docdatetime` из **mtime**
  (`%F`, `%T %Z` → `14:30:45 +0300`); `localdate`/… = now. При stdin:
  docname/docfile undefined, docdir=cwd, docdate=now. Header-entry
  ПЕРЕОПРЕДЕЛЯЕТ docdate, но НЕ docname (locked).
- Attr-refs в revision-line резолвятся при ЧТЕНИИ строки (read-time):
  атрибут, определённый ПОЗЖЕ в header, — литерал; undefined — литерал;
  `v{docname}` → strip `v` идёт по уже резолвленному значению.

### Что сделано
- **РЕНДЕРЕР** events.rs TagEnd::DelimitedBlock: армы Passthrough (только
  newline-guard, БЕЗ `</div>`) и Comment (ничего) вместо catch-all `</div>`
  (каждый `++++`-блок и `[pass]`-параграф оставлял лишний `</div>`).
- **CLI** main.rs: сидинг интринсиков в initial_attrs (препроцессор) +
  html_attrs (рендерер); явные `-a` (и unset-формы) не перетираются
  (`cli_attr_names`-guard).
- **РЕНДЕРЕР** finish.rs::render_author_details: resolve_attr_refs_text на
  revnumber/revdate/revremark (теперь Option<String>, if-let по ссылке).
  Резолв в арме Event::Revision НЕ работает — парсер следом эмитит
  дублирующие Event::Attribute с сырыми значениями (перетирают); поэтому
  резолв на точке рендера.
- +2 html-теста: test_revision_attr_refs_resolved_in_details (LPR-префикс
  стрипается → «version 55»), test_passthrough_block_bare_content_no_stray_div.

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (920: parser 477, html 352).
- Пробы p_pt1/2 и p_doc1 байт-в-байт; corpus-файлы — чисто (кроме NCR-шума).
- **Корпус: Identical 253→255 (+2)**; blast (base 99fab03): 3 файла — 2 флипа
  (pass.adoc, revision-line-with-version-prefix.adoc), **0 регрессий**,
  stem 56=56 (нейтрально).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Известные пределы (задокументированы, в корпусе нет)
- Резолв revision-refs по header-FINAL state (asciidoctor — read-time): ref на
  атрибут, определённый позже в header, у нас резолвится, у него — литерал.
- v-strip по сырому значению: `v{docname}` → «vp_rev» (asciidoctor «p_rev»).
- docname/docfile/docdir у asciidoctor locked от header-entry — у нас
  переопределяются.
- `outfilesuffix`/`filetype` не сеются (слой рендерера); Ruby `%Z` vs chrono
  `%z` может разойтись в TZ с именованной зоной (UTC и т.п.).
- Pre-existing (НЕ тронуто, base тоже): author-line после attr-entry в header
  не распознаётся вовсе (нет details).

### Что дальше
- nearmiss на 255: **revision-information (24)**, stem (56 — 3-4 корня:
  `\$`-эскейп, `stem::`-макрос literal, `++++`+callout, `{n!}`), source (63),
  customize-title-label (66), include (75), bibliography (77), subs (89),
  subs-group-table/ordered (90), footnote (101).
- Кандидат-кластер: xreflabel → reftext для xref-резолва (label в Tag::Anchor +
  регистрация в XrefResolver; p_id1/2/3 + lexicon-остаток).
- Pre-existing из прошлых сессий: nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `cols="2*"` multiplier, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, unknown-style в class на
  quote/sidebar, list-merge через continuation-attrlist, author-line после
  attr-entry в header.

---

## Сессия (2026-06-12, двадцать третья) — Фаза 3: literal-monospace (pass:SPEC + удаление custom-macro catch-all)

Запрос «продолжи». Ветка **`fix/inline-pass-spec-and-custom-macro-removal`** —
НЕ закоммичена (рабочее дерево). Baseline: Identical 250, master `7f05b9d`
(base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
stem (56 — 3-4 корня, снова отложен) → **literal-monospace (59 diff)**, один
корень: `` `\pass:c[]` `` → у нас `\p` + мусорный `<span custom-macro macro-ass>`.

### Семантика asciidoctor (пробы /tmp/p_ep1..5)
- `pass:SPEC[content]` — SPEC: одночар-алиасы `a c m n p q r v` + полные имена
  (`quotes`, `normal`…); перечисленные subs применяются к контенту
  (`pass:c[<b>]` → escaped, `pass:q[*b*]` → bold БЕЗ экранирования, `pass:n` —
  полный normal-набор). Без `[` после спека — НЕ макрос, литерал (`pass:c here`).
- `\pass:SPEC[…]` — backslash дропается, `pass:SPEC[` литерал, контент и
  хвостовой `]` идут через обычные subs (`\pass:c[*b*]` → `pass:c[<strong>b</strong>]`).
- `\\pass:SPEC[…]` — в escape участвует только ОДИН backslash, первый остаётся
  литералом (`\pass:c[abc]`).
- **Неизвестные inline-макросы НЕ матчатся вовсе** — литеральный текст,
  внутренность скобок идёт через обычные subs (`foo:bar[*b*]` →
  `foo:bar[<strong>b</strong>]`; `chart:sales[Q1,Q2]` — литерал).

### Что сделано (ПАРСЕР inline.rs + attributes.rs)
- `try_pass_macro`: optional spec (`pass_spec_len` — [a-z,_-]-ран строго до `[`;
  невалидный/без скобки → не макрос); `pass_spec_to_subs` (алиасы +
  `attributes::sub_name_to_flags`, теперь pub(crate)); `push_pass_spec_content` —
  ре-парс контента со спекнутым набором, Text→InlinePassthrough когда нет
  SPECIALCHARS (рендерер экранирует Text безусловно).
- Escape-армы: `\pass:SPEC[` (расширен с `pass:[`) + НОВЫЙ арм `\\pass:SPEC[`.
- `pass_macro_span_len` spec-aware (скип границ в constrained-спанах);
  `push_single_plus_content` — spec-aware границы, c-спек → Text (экранируется).
- **Catch-all custom-macro УДАЛЁН** (try_custom_inline_macro + dispatch-арм +
  scanner::is_known_inline_macro): был кошмарно жадный (target до `[` без
  ограничений — «Mono with content: `+abc+` [x]» матчился как макрос `content:`!).
  Tag::CustomInlineMacro остаётся в enum (API), блочный `name::` не тронут.
- Тесты: 3 html-теста переписаны (фиксировали неверную семантику custom-macro),
  +2 html (pass-spec 8 кейсов; escaped-pass 3 кейса), +1 parser (events).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (918: parser 477, html 350).
- Пробы p_ep1/2/4/5 байт-в-байт, кроме двух документированных пределов (ниже).
- **Корпус: Identical 250→253 (+3)**; blast (base 7f05b9d): 11 файлов — 3 флипа
  (literal-monospace, attribute-entries, revision-line), **0 регрессий**,
  8 changed-still-different — ВСЕ ближе: pass 133→18(!), footnote 260→101,
  revision-information 96→24, align-by-column 637→617, format-column-content
  218→198, apply-subs-to-text 119→115, syntax-quick-reference 2791→2735,
  outline 8718→8664.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Известные пределы (задокументированы в коде, в корпусе нет)
- `pass:c,q[…]`: asciidoctor гоняет q ПО уже экранированному тексту (`;` блокирует
  constrained-открытие) — bitflag-модель только membership, у нас `*x*` болдится.
- Spec'нутый pass внутри `+…+`: форматирующие subs не перегоняются (статик-хелпер),
  чтится только membership SPECIALCHARS.
- `foo:b\`ar[baz]`: наш eager-escape съедает backslash (asciidoctor хранит) —
  pre-existing разница escape-модели, не от этого фикса.

### Что дальше
- nearmiss на 253: **pass (18 diff!)**, **revision-information (24!)**, stem (56 —
  3-4 корня: `\$`-эскейп, `stem::`-макрос literal, `++++`+callout, `{n!}`),
  source (63), customize-title-label (66), include (75), bibliography (77),
  subs (89), subs-group-table/ordered (90), footnote (101);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
- Кандидат-кластер: xreflabel → reftext для xref-резолва (label в Tag::Anchor +
  регистрация в XrefResolver; p_id1/2/3 + lexicon-остаток).
- Pre-existing из прошлых сессий: nested-список с другим маркером в li,
  `[square]`-класс, компактный colist-`<li><p>`, `== heading` не прерывает
  параграф, `cols="2*"` multiplier, `[abstract]`-параграф → quoteblock,
  `:icons:`-colist, m/e/s-стили колонок, лишний `</div>` у standalone passthrough,
  unknown-style в class на quote/sidebar, list-merge через continuation-attrlist.

---

## Сессия (2026-06-12, двадцать вторая) — Фаза 3: block.adoc (`.Title` на списках)

Запрос «продолжи». Ветка **`fix/list-block-title`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 249, master `0e6808c` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
stem (56 — 3-4 независимых корня: `\$`-эскейп, `stem::`-макрос, `++++`+callout,
`{n!}`; отложен) → **block.adoc (57 diff)**, один корень: `.Title` на ulist.

### Семантика asciidoctor (пробы /tmp/p_lt1..6)
- `.Title` на списке → `<div class="title">` ВНУТРИ обёртки, ПЕРЕД
  `<ul>`/`<ol>`/`<dl>`/`<table>` (все формы: ulist/olist/dlist/horizontal/qanda/colist).
- `.Title` ПОСЛЕ blank в list-контексте закрывает списки (как block-attr/comment);
  title вешается на следующий блок. Двойной title — последний побеждает.
- `.Title`-строка БЕЗ blank внутри item/dd/параграфа/admonition-параграфа —
  обычный wrapped-текст (slurp): титулы НИКОГДА не прерывают параграф
  (прерывают attr-строки и делимитеры; `== heading` тоже НЕ прерывает — у нас
  прерывает, pre-existing, не тронуто).

### Что сделано
- **ПАРСЕР** block.rs: (1) `.Title`-handler в scan_block_metadata — close_list_contexts
  при had_blank_line в list-контексте (зеркало block-attr-ветки); (2) исключение
  `is_block_title` УБРАНО из `is_list_continuation_line`, `is_dlist_continuation_line`,
  break-условий `scan_paragraph` и `scan_admonition` (slurp как у asciidoctor).
- **РЕНДЕРЕР**: `emit_pending_block_title` после открытия обёртки в
  `start_unordered_list` (обе ветки), `start_ordered_list`, `start_description_list`
  (3 арма) — blocks.rs; arm `Tag::CalloutList` — events.rs.
- +3 теста: parser `test_block_title_after_blank_separates_lists` (2 кейса),
  parser `test_block_title_line_does_not_interrupt_paragraph`,
  html `test_list_block_title_html` (7 кейсов).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 476, html 348).
- Пробы p_lt1 байт-в-байт; p_lt2/4/5/6 — остатки только pre-existing другие корни
  (вложение списка с другим маркером внутрь li, `[square]`-класс на `<ul>`,
  компактный colist-`<li><p>`, heading не slurp'ится в параграф).
- **Корпус: Identical 249→250 (+1)**; blast (base 0e6808c): 6 файлов — 1 флип
  (block.adoc), **0 регрессий**, 5 changed-still-different — все ближе:
  ordered 223→90, unordered 298→145, release-and-progress-reviews 409→406,
  outline 8735→8718, admonition 197=197 (len ближе).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 250: stem (56 — 3-4 корня: инлайн `\$[[...]]`-эскейп ломает текст,
  `stem::[...]` должен остаться литеральным параграфом а не custom-macro,
  `++++ <.>` в callout-листинге, `{n!}` дропается в latexmath-параграфе),
  literal-monospace (59), source (63), customize-title-label (66), include (75),
  bibliography (77), subs (89), ordered (90 — стало ближе);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
- Кандидат-кластер: **xreflabel → reftext для xref-резолва** (label в Tag::Anchor
  + регистрация в XrefResolver; закрыл бы p_id1/2/3-строки и lexicon-остаток).
- Новые pre-existing находки (НЕ в корпусе как флип): `* x` после blank внутри
  `- y`-списка должен вкладываться как nested ulist в li (у нас — sibling);
  `[square]`-стиль не даёт класс на `<ul>`; colist-`<li><p>` компактен (нет
  переносов); `== heading` не прерывает параграф у asciidoctor (у нас прерывает).
- Прочее: `cols="2*"` multiplier (row.adoc), `[abstract]`-параграф → quoteblock,
  `:icons:`-colist (TODO), кластер `m`/`e`/`s` стиля колонок; pre-existing: лишний
  `</div>` у standalone passthrough, unknown-style течёт в class на quote/sidebar,
  пустые строки в пустых sectionbody, list-merge через continuation-attrlist (p_chk2).

---

## Сессия (2026-06-11, двадцать первая) — Фаза 3: assign-id + example-blocks (2 near-miss)

Запрос «продолжи». Ветка **`fix/example-caption-unset-and-positional-shorthand`** —
НЕ закоммичена (рабочее дерево). Baseline: Identical 247, master `172faf5`
(base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**assign-id.adoc (2 diff)** + **example-blocks.adoc (2 diff)** — оба почти-флипа
из прошлой сессии, два независимых корня, взяты вместе в одну ветку.

### Семантика asciidoctor (пробы /tmp/p_ec1..3, p_qa1..2, p_sh1, p_ln1..8)
- `:!example-caption:` → голый title (и mid-document); `:example-caption: Demo` →
  «Demo 1.» с общим счётчиком; дефолт «Example 1.».
- Shorthand attrlist — ТОЛЬКО в первой comma-части: `[quote#roads,Dr. Emmett
  Brown,Back to the Future]` — attribution целиком; `[quote,#bar]`/`[quote,.baz]` —
  verbatim positional; `[.r1,.r2]` → только r1; `[%header,%footer]` → только
  header (`%header%footer` — оба).
- 3-й позиционный СЛОТ source-блока = linenums: любое непустое позиционное
  значение включает (`linenums`/`%linenums`/`#code1`/`yaml`; implied
  `[,ruby,linenums]` тоже), named (`start=10`) слот НЕ занимает.
- linenums РЕНДЕРИТСЯ только под build-time подсветчиком (rouge/pygments/
  coderay); без подсветчика и под highlight.js — игнор целиком (ни класса,
  ни таблицы).

### Что сделано
- **РЕНДЕРЕР** lib.rs: `example-caption: Example` в дефолтных document_attrs;
  blocks.rs арм Example: label из document_attrs (как figure/table).
- **ПАРСЕР** attributes.rs::parse: обе shorthand-ветки гейтятся `idx == 0`;
  +правило linenums-слота по raw-parts (после implied_source_lang).
- **ПАРСЕР** block.rs::emit_block_metadata: style гейтится
  `first_positional_is_style` (позиционал слота 2+ не утекает в style/class).
- **РЕНДЕРЕР** blocks.rs::start_source_block: linenums гейтится
  `rouge|pygments|coderay` (закрыта регрессия db-migration.adoc — `[id=app,
  source, yaml]` слот 3 = `yaml` → linenums on, но подсветчика нет → игнор).
- Тесты: 4 старых переписаны (фиксировали неверное: parse_role, has_option,
  source_with_shorthand_id, table_header_footer_combined `[%header,%footer]`→
  `[%header%footer]`); linenums-тесты переведены на `:source-highlighter:
  rouge` + негативный test_source_block_linenums_needs_build_time_highlighter;
  +4 новых (example-caption, shorthand-first-position html+parser, linenums-слот).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 474, html 347).
- **Корпус: Identical 247→249 (+2)**; blast (base 172faf5): 3 файла — 2 флипа
  (assign-id.adoc, example-blocks.adoc), **0 регрессий**, add-title 252=252
  (семантически ближе: mid-document `:!example-caption:` теперь чтится).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 249: **stem (56 — MathJax-остатки?)**, block (57 — корень
  `.Title` на ulist теряется), literal-monospace (59), source (63),
  customize-title-label (66), include (75), bibliography (77), subs (89);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
- Кандидат-кластер: **xreflabel → reftext для xref-резолва** (label в
  Tag::Anchor + регистрация в XrefResolver; закрыл бы p_id1/2/3-строки и
  lexicon-остаток).
- Прочее: `.Title` на ulist (block.adoc), `cols="2*"` multiplier (row.adoc),
  `[abstract]`-параграф → quoteblock, `:icons:`-colist (TODO), кластер
  `m`/`e`/`s` стиля колонок; pre-existing: лишний `</div>` у standalone
  passthrough, unknown-style течёт в class на quote/sidebar, пустые строки
  в пустых sectionbody, list-merge через continuation-attrlist (p_chk2).
- Латентно (нет в корпусе): наша linenotable-разметка ≠ rouge байт-в-байт
  (нет server-side подсветки) — всплывёт, если в корпусе появится
  rouge+linenums файл.

---

## Сессия (2026-06-11, двадцатая) — Фаза 3: collapsible.adoc (masquerade-параграф — голый контент)

Запрос «продолжи». Ветка **`fix/collapsible-block`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 244, master `184b97d` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**collapsible.adoc (51 diff)**, один корень.

### Семантика asciidoctor (пробы /tmp/p_col1..3)
- Параграф, masquerade'нутый стилем (`[example]`, `[example%collapsible]`,
  `[sidebar]`, `[quote]`, `[open]`) → текст ГОЛЫЙ в `<div class="content">` /
  `<blockquote>` (без `<div class="paragraph"><p>`); multiline сохраняет строки.
- `[partintro]` — ИСКЛЮЧЕНИЕ: paragraph-обёртка внутри openblock сохраняется
  (p_col3, book-контекст; подтверждает сессию 12).
- `[open]`-параграф → `<div class="openblock">` (класс `open` в обёртку НЕ течёт);
  у нас не masquerade'ился вовсе (`paragraph open`).
- `[%collapsible]` без стиля — опция игнорируется, обычный параграф (было верно).
- partintro вне book-part → ERROR + exclude блока (НЕ реализовано, в корпусе нет).

### Что сделано (ПАРСЕР + newline-guard в рендерере)
- `block.rs::scan_paragraph`: арм `quote|example|sidebar|open` — Text без
  Tag::Paragraph (как verse/pass); `partintro` выделен в отдельный арм (с обёрткой).
- `attributes.rs::block_style_kind`: +`"open"`; `block.rs::emit_block_metadata`
  exclusion-список: +`"open"`.
- `events.rs` TagEnd::DelimitedBlock: newline-guard (`!ends_with('\n')`) в армах
  Quote / Example(details) / Example|Sidebar|Open; verse НЕ тронут (отсутствие
  `\n` перед `</pre>` намеренное).
- +1 html-тест `test_style_masqueraded_paragraph_bare_content` (7 кейсов: example,
  collapsible, sidebar, quote, open без утечки класса, multiline, guard настоящего
  delimited-блока).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (908 passed, html 345).
- Пробы p_col1 байт-в-байт; p_col2 — остатки только partintro-вне-book (не в корпусе)
  и trailing newline.
- **Корпус: Identical 244→247 (+3)**; blast (base 184b97d): 8 файлов — 3 флипа
  (collapsible.adoc, sidebars.adoc, release-plan.adoc), **0 регрессий**,
  5 changed-still-different: assign-id 84→2, example-blocks →2 (почти флипы!),
  quote 161→109, add-title 291→252, block 57=57 (нейтрально).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- **assign-id (2 diff!)** и **example-blocks (2 diff!)** — почти флипы, разведать
  первыми. Затем nearmiss: stem (56), block (57 — корень `.Title` на ulist
  теряется), literal-monospace (59), source (63), customize-title-label (66),
  include (75), bibliography (77), quote (109 — стало ближе);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
- Кандидат-кластер: **xreflabel → reftext для xref-резолва** (label в Tag::Anchor +
  регистрация в XrefResolver; закрыл бы p_id1/2/3-строки и lexicon-остаток).
- Прочее: `.Title` на ulist (block.adoc), `cols="2*"` multiplier (row.adoc),
  `[abstract]`-параграф → quoteblock, `:icons:`-colist (TODO), кластер `m`/`e`/`s`
  стиля колонок; pre-existing: лишний `</div>` у standalone passthrough,
  unknown-style течёт в class на quote/sidebar, пустые строки в пустых sectionbody,
  list-merge через continuation-attrlist (p_chk2).

---

## Сессия (2026-06-11, девятнадцатая) — Фаза 3: checklist.adoc (%interactive чекбоксы)

Запрос «продолжи». Ветка **`fix/checklist-rendering`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 243, master `715b17e` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**checklist.adoc (49 diff)**, один корень.

### Семантика asciidoctor (пробы /tmp/p_chk1..2)
- `[%interactive]` (и formal `options=interactive`) на checklist →
  `<input type="checkbox" data-item-complete="1" checked> ` для checked,
  `<input type="checkbox" data-item-complete="0"> ` для unchecked (вместо
  `&#10003;`/`&#10063;`); обычные item'ы списка — без изменений.
- На списке БЕЗ чекбоксов опция ни на что не влияет (нет и класса checklist).
- Вложенный список — свой узел, опцию НЕ наследует.
- Pre-existing (p_chk2, НЕ в корпусе): `+`-continuation с `[%interactive]`+новым
  `*`-item — asciidoctor вливает всё в ОДИН список, мы открываем второй.

### Что сделано (только РЕНДЕРЕР, 3 точки + поле)
- `lib.rs`: поле `interactive_ulist_stack: Vec<bool>` (параллельный стек, по
  образцу admonition_block_stack).
- `blocks.rs::start_unordered_list`: push флага из `meta.options` (interactive).
- `events.rs`: arm `Tag::ListItem` — match (checked, interactive) → input-формы;
  `TagEnd::UnorderedList` — pop.
- +1 html-тест `test_checklist_interactive_html` (4 кейса: shorthand, formal,
  не-наследование вложенным, NCR без опции).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (907 passed, html 344).
- Проба p_chk1 байт-в-байт; p_chk2 — только pre-existing list-merge edge.
- **Корпус: Identical 243→244 (+1)**; blast (base 715b17e): ровно 1 файл —
  1 флип (checklist.adoc), **0 регрессий**.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 244: **collapsible (51 diff)**, release-plan (56), stem (56),
  block (57), literal-monospace (59), source (63), customize-title-label (66),
  include (75), bibliography (77); revision-line-with-version-prefix (1 —
  `{docdate}`, скип).
- Кандидат-кластер: **xreflabel → reftext для xref-резолва** (label в Tag::Anchor +
  регистрация в XrefResolver; закрыл бы p_id1/2/3-строки и lexicon-остаток).
- Прочее: `cols="2*"` multiplier (row.adoc), `[abstract]`-параграф → quoteblock,
  `:icons:`-colist (TODO), кластер `m`/`e`/`s` стиля колонок; pre-existing: лишний
  `</div>` у standalone passthrough, unknown-style течёт в class на quote/sidebar,
  пустые строки в пустых sectionbody, list-merge через continuation-attrlist (p_chk2).

---

## Сессия (2026-06-11, восемнадцатая) — Фаза 3: id.adoc (anchor:-макрос, xreflabel, comment-разделитель списков)

Запрос «продолжи». Ветка **`fix/inline-anchor-macro-and-xreflabel`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 242, master `7e772f6` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**id.adoc (45 diff)**, четыре корня.

### Семантика asciidoctor (пробы /tmp/p_id1..9)
- `anchor:id[]`/`anchor:id[label]` → `<a id="id"></a>`; label НЕ рендерится in place,
  используется как reftext для xref. Target с пробелом — литерал; `\anchor:x[]` —
  литерал без backslash.
- `[[id,xreflabel]]` (inline И block) → id без label; label = reftext для xref
  (`<<bookmark-d>>` → «last paragraph»; block-anchor label ПОБЕЖДАЕТ .Title в xref).
- `<<id>>` на inline-анкер БЕЗ label → fallback `[id]`.
- `[[id]]image:...[]` (строка с хвостом после `]]`) — параграф с inline-анкором,
  НЕ block-attrlist (BlockAttributeListRx: первый символ inner — `[\w{,.#"'%]`).
- Comment-строка ПОСЛЕ blank разделяет смежные списки (даже однотипные, p_id7)
  и отрывает dlist от ulist-item; comment сразу после item (без blank) — НЕ рвёт
  (p_id5/8); dlist после голого blank ПРИКРЕПЛЯЕТСЯ к li (p_id4 — asciidoctor тоже).

### Что сделано (только ПАРСЕР; рендерер Tag::Anchor уже был готов)
- `inline.rs::try_anchor_macro` + dispatch-arm `b'a'`/`anchor:` (при провале
  `pos += 7` — иначе catch-all ел `nchor:`); `anchor:` в NAMES (11→12);
  `try_anchor` — split id по запятой.
- `scanner.rs::is_block_attribute` — ужесточение первого символа + ветка
  BlockAnchorRx для `[[...]]` (вся строка, interior без скобок).
- `attributes.rs` legacy-anchor — split по запятой.
- `block.rs` comment-handler — close_list_contexts при had_blank_line в
  list-контексте (зеркало block-attribute-ветки, строка ~600).
- +4 теста: inline `test_anchor_macro` (4 кейса) + обновлён
  `test_anchor_with_reftext_still_works` (фиксировал НЕВЕРНОЕ поведение);
  scanner `test_is_block_attribute` (+10 ассертов); attributes
  `test_legacy_anchor_xreflabel_stripped`; block
  `test_comment_after_blank_separates_lists`; html
  `test_inline_anchor_macro_and_xreflabel_html` (6 кейсов).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 472, html 343, core 13).
- Пробы: p_id4/5/6/8/9 байт-в-байт; p_id7 — только trailing-newline (норм.);
  p_id1/2/3 — остаток ТОЛЬКО xref-reftext строки (не нужны для флипа).
- **Корпус: Identical 242→243 (+1)**; blast (base 7e772f6): 9 файлов — 1 флип
  (id.adoc), **0 регрессий**, 8 changed-still-different (list-файлы ближе к
  эталону: complex.adoc ulist 1→5 при 13 в ref; checklist 49=49,
  revision-information 94→96 — позиционный шум).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 243: **checklist (49 diff)**, collapsible (51), release-plan (56),
  stem (56), block (57), literal-monospace (59), source (63),
  customize-title-label (66), include (75), bibliography (77);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
- Новый кандидат-кластер: **xreflabel → reftext для xref-резолва** (label в
  Tag::Anchor + регистрация в XrefResolver; закрыл бы p_id1/2/3-строки и
  родственный lexicon-остаток «reftext из dt-терма»).
- Прочее: `cols="2*"` multiplier (row.adoc), `[abstract]`-параграф → quoteblock,
  `:icons:`-colist (TODO), кластер `m`/`e`/`s` стиля колонок; pre-existing:
  лишний `</div>` у standalone passthrough, unknown-style течёт в class на
  quote/sidebar, пустые строки в пустых sectionbody.

---

## Сессия (2026-06-11, семнадцатая) — Фаза 3: author-атрибуты из attribute-entries

Запрос «продолжи». Ветка **`fix/author-attr-entries`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 241, master `2d07b0b` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**reference-author.adoc (37 diff)**, три корня.

### Семантика asciidoctor (пробы /tmp/p_au1..16; источник parser.rb/document.rb читан)
- End-of-header rescan (parse_header_metadata): если `author`-атрибут задан и ≠
  значения от author-line — names-only парсинг значения (split ≤3 whitespace-сегментов,
  4+ слов → хвост в lastname, `_`→пробел в каждом сегменте, initials = первые символы,
  fullname РЕКОМПОЗИРУЕТСЯ) → клоббер firstname/middlename/lastname (даже явных
  entries!); явный `:authorinitials:`, отличный от line-derived, ВЫЖИВАЕТ;
  authorcount → 1 («do not allow multiple»). Email из значения НЕ извлекается
  (`<...>` в attr-entry уже проэкранирован header-subs → ветка sanitize мертва;
  lastname получает `Jones <m@x.org>` verbatim).
- `Document#authors` — полностью attribute-backed: спаны details из `author`/`email`
  + `author_N`/`email_N` (гейт `authorcount`). `:email:` без author → НЕТ details;
  `:!author:` после author-line — details ПОДАВЛЕН (но firstname от line остаётся).
- `:author_2:` attr-entry второго автора НЕ создаёт; mid-document `:author:` ничего
  не дериватит и details не открывает; `:firstname:`+`:lastname:` БЕЗ author author
  не композируют.
- Section auto-id: attr-refs в заголовке резолвятся ДО генерации id
  (`== About {author}` → `_about_kismet_r_lee`); значения entries резолвятся at
  definition (`:nested: x {foo} y`); undefined — литерал (скобки дропает санация id).

### Что сделано
- **CORE** `Author::from_attribute_value(value)` — names-only дериватор (+1 юнит-тест).
- **РЕНДЕРЕР** `finish.rs::finalize_header_authors` (зов в events.rs на TagEnd::Header
  ДО render_author_details, в обоих режимах — derived attrs нужны body-refs);
  `render_author_details` — author-спаны attribute-backed (цикл по authorcount,
  name_suffix/id_suffix из AuthorRegistry); guard details: `author`-attr вместо
  registry. events.rs Event::Author — +`authorcount` в document_attrs (= len реестра).
- **ПАРСЕР** `block.rs`: поле `doc_attrs: HashMap` (имена lowercase, значения
  definition-time resolved); `record_attribute_entry` (unset-формы `!n`/`n!` —
  remove) на всех 5 точках attr-entry (body, header×3, revision) + запись
  author-line-атрибутов (suffix `_N`); `resolve_title_attr_refs` перед
  `generate_id` на всех 4 точках (section/discrete/doc-header×2).
- +1 html-тест `test_author_attrs_from_attribute_entries` (6 кейсов),
  +1 parser-тест `test_section_id_resolves_attr_refs` (5 ids).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (902 passed: parser 469,
  html 342, core 13).
- Пробы: p_au1 (standalone+embedded) байт-в-байт кроме известной NCR-нормализации;
  p_au2..16 OK (p_au16 body — pre-existing пустые строки в пустых sectionbody).
- **Корпус: Identical 241→242 (+1)**; blast (base 2d07b0b): ровно 1 файл — 1 флип
  (reference-author.adoc), **0 регрессий**.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 242: **id (45 diff)**, checklist (49), collapsible (51),
  release-plan (56), stem (56), block (57), literal-monospace (59), source (63),
  customize-title-label (66), include (75), bibliography (77);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
  Прочее: `cols="2*"` multiplier (row.adoc), `[abstract]`-параграф → quoteblock,
  `:icons:`-colist (TODO), кластер `m`/`e`/`s` стиля колонок; pre-existing: лишний
  `</div>` у standalone passthrough, unknown-style течёт в class на quote/sidebar,
  пустые строки в пустых sectionbody. Известные пределы фикса: parser-карта не
  дериватит firstname из entry-`:author:` для ids (нет в корпусе); `:authors:`-атрибут
  (множественный) не поддержан (нет в корпусе).

---

## Сессия (2026-06-11, шестнадцатая) — Фаза 3: subs trailing-plus + attr-value pass-макрос

Запрос «продолжи». Ветка **`fix/subs-trailing-plus-and-attr-pass-macro`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 240, master `1a13391` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**listing.adoc (34 diff)**, два корня.

### Семантика asciidoctor (пробы /tmp/p_subs1..6, p_rec)
- `subs=` (resolve_subs): модификаторы — `+x` append, `x+` PREPEND (trailing plus!),
  `-x` remove; первый МОДИФИКАТОР сидит дефолты блока, первый PLAIN-токен сидит
  ПУСТОЙ набор (замена) — `"quotes,+attributes"` ДРОПАЕТ specialchars; составные
  имена (`verbatim+`/`-normal`) допустимы. ПОРЯДОК применения (prepend = sub ДО
  specialchars → двойное экранирование значения) в bitflag-модели непредставим —
  только membership; два известных edge-предела (p_subs5 case1, p_subs3 case2),
  в корпусе их нет.
- Attr-entry значение `pass:SUBS[content]` (full-value, apply_attribute_value_subs):
  subs применяются при ОПРЕДЕЛЕНИИ; `pass:a[{ref}]` — undefined ref остаётся
  литералом и при использовании НЕ ре-сканится (`:x: pass:a[{x}]` → литерал `{x}`).
- ПОПУТНЫЙ pre-existing КРАШ: `:x: {x}` + `{x}` → stack overflow (рекурсия
  events.rs AttributeReference → render_inline_value). Asciidoctor — литерал.

### Что сделано
- **ПАРСЕР** `attributes.rs::parse_subs_value`: детекция модификаторов +trailing `+`;
  логика asciidoctor (acc: Option<SubstitutionSet>, get_or_insert(default) у
  модификаторов / get_or_insert(NONE) у plain); +`sub_name_to_flags` (составные
  normal/verbatim/none). 2 юнит-теста переписаны под верную семантику (probe-verified),
  +1 `test_subs_parse_trailing_plus`.
- **РЕНДЕРЕР** `lib.rs::apply_attr_value_pass_macro` (зов из apply_attribute):
  full-value `pass:SPEC[content]` — обёртка стрипается, `a`/`attributes` в SPEC →
  definition-time резолв через core `resolve_attr_refs_text`; ПУСТОЙ SPEC (`pass:[…]`)
  НЕ трогается (inline pass-макрос обрабатывает at use, verbatim-вставка).
- **РЕНДЕРЕР** guard рекурсии: поле `attr_refs_in_progress: Vec<String>`;
  arm AttrRefOutcome::Document — повторный вход по тому же (lowercase) имени →
  литерал `{name}` (закрыт краш `:x: {x}` и взаимная рекурсия `:a: {b}`/`:b: {a}`).
- +2 html-теста: `test_subs_trailing_plus_and_attr_value_pass_macro` (5 кейсов),
  `test_self_referential_attribute_no_recursion` (2 кейса).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 467→468, html 339→341).
- Пробы p_subs1/2/6 байт-в-байт; p_rec — литерал как asciidoctor (был abort).
- **Корпус: Identical 240→241 (+1)**; blast (base 1a13391): 4 файла — 1 флип
  (listing.adoc, 0 diffs), **0 регрессий**, 3 changed-still-different:
  include 125→124, subs 92→89, footnote 245→260 (СЕМАНТИЧЕСКИ ЛУЧШЕ:
  `:fn-disclaimer: pass:c,q[footnote:…]` теперь даёт настоящие footnote-`<sup>`
  вместо мусорного custom-macro; рост счётчика — позиционный шум от появившихся
  footnote-определений).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 241: **reference-author (37 diff)**, id (45), checklist (49),
  collapsible (51), release-plan (56), stem (56), block (57), literal-monospace (59),
  source (63), customize-title-label (66), include (75);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
  Прочее: `cols="2*"` multiplier (row.adoc), `[abstract]`-параграф → quoteblock,
  `:icons:`-colist (TODO), кластер `m`/`e`/`s` стиля колонок; pre-existing: лишний
  `</div>` у standalone passthrough, unknown-style течёт в class на quote/sidebar.
  Новый известный предел: порядок subs (prepend/append) не представим bitflag'ом —
  если встретится в корпусе, потребуется упорядоченный Vec<Sub> вместо маски.

---

## Сессия (2026-06-11, пятнадцатая) — Фаза 3: revision-атрибуты из attribute-entries

Запрос «продолжи». Ветка **`fix/revision-attrs-from-entries`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 239, master `77b6302` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**reference-revision-attributes.adoc (31 diff)**, один корень.

### Семантика asciidoctor (пробы /tmp/p_rev1..8, после фикса все 8 байт-в-байт)
- Revision-спаны в `<div class="details">` — attribute-driven (html5.rb смотрит
  document-атрибуты `revnumber`/`revdate`/`revremark`): attr-entries в header дают
  спаны БЕЗ revision-line; автор не обязателен.
- Значение verbatim: `:revnumber: v8.3` → «version v8.3» (`v` стрипается ТОЛЬКО при
  парсинге revision-line).
- attr-entry ПОБЕЖДАЕТ revision-line (later-wins в header); `:!revdate:` снимает
  спан и запятую после version; set-but-empty `:revnumber:` → спан «version ».
- Body-атрибуты (после blank за header'ом / mid-document) в details НЕ попадают.

### Что сделано (только РЕНДЕРЕР)
- `finish.rs::render_author_details`: revision-часть читает
  `document_attrs.get("revnumber"/"revdate"/"revremark")` (метод зовётся на
  `TagEnd::Header` — документ-атрибуты в этот момент = ровно header-состояние);
  guard пустоты details расширен на эти три ключа; запятая после version — по
  наличию revdate; display_version больше не зовётся (verbatim).
- `lib.rs`: поле `revision: Option<Revision>` удалено; `events.rs` arm
  Event::Revision только вливает `attr_entries()` в document_attrs (precedence
  с attr-entries — порядком стрима).
- +1 html-тест `test_revision_attrs_from_attribute_entries` (4 кейса; негативные
  ассерты — по `<span id=…`, т.к. голые имена есть в default-stylesheet).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (html 338→339, parser 467).
- Пробы p_rev1..8 — header-секции байт-в-байт с asciidoctor.
- **Корпус: Identical 239→240 (+1)**; blast (base 77b6302): ровно 1 файл — 1 флип
  (reference-revision-attributes.adoc), **0 регрессий**.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 240: **listing (34 diff)**, reference-author (37), id (45),
  checklist (49), collapsible (51), release-plan (56), stem (56), block (57),
  literal-monospace (59), source (63), customize-title-label (66);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
  Прочее: `cols="2*"` multiplier (row.adoc), `[abstract]`-параграф → quoteblock,
  `:icons:`-colist (TODO), кластер `m`/`e`/`s` стиля колонок; pre-existing: лишний
  `</div>` у standalone passthrough, unknown-style течёт в class на quote/sidebar.

---

## Сессия (2026-06-11, четырнадцатая) — Фаза 3: admonition block-форма (параграф-обёртки)

Запрос «продолжи». Ветка **`fix/admonition-block-paragraph-wrappers`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 235, master `3dfe796` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**apply-subs-to-blocks.adoc (31 diff)**, один корень (len_delta=8 = 2 параграфа × 4
строки обёртки).

### Семантика asciidoctor (пробы /tmp/p_adm1..13)
- paragraph-форма (`NOTE: text` И `[NOTE]` на параграфе) — голый текст в
  `<td class="content">`.
- block-форма (`[NOTE]` на `====` example или `--` open) — compound: дети с обычными
  обёртками (`<div class="paragraph"><p>`, ulist, вложенные admonition и т.д.).
- admonition-стиль чтится ТОЛЬКО на example/open; на listing/literal/sidebar/quote/
  passthrough — ИГНОРИРУЕТСЯ, блок остаётся родным, стиль дропается (как и unknown
  `[foo]` — но у нас на quote/sidebar unknown-стиль ТЕЧЁТ в class, pre-existing).
- Попутно: голый `++++` passthrough у нас даёт лишний `</div>` (pre-existing, есть в
  base; p_adm12 поэтому единственная не-байт-в-байт проба из 13).

### Что сделано
- **ПАРСЕР** `event.rs`: `Tag::Admonition` +поле `block: bool` (+doc-комментарий,
  into_static). `block.rs`: paragraph-точки (scan_paragraph ~1814, scan_admonition
  ~2091) → `block: false`; ранний перехват «admonition style on any delimited block»
  (~2222) УДАЛЁН; в structural-ветке гейт `matches!(delim_type, Example|Open)` →
  `block: true` (verbatim-типы теперь падают в родную ветку, стиль дропается).
- **РЕНДЕРЕР** `lib.rs`: поле `admonition_block_stack: Vec<bool>`; `blocks.rs`:
  start_admonition(+block) пушит; `events.rs`: TagEnd::Admonition попит;
  `is_direct_child_of_admonition` → подавление `<p>` только при `!block`;
  `is_inside_compact_context` arm Admonition → компактность только при `!block`
  (block-форма → полные обёртки; вложенность paragraph-в-block работает: ближайший
  Admonition в tag_stack = вершина параллельного стека).
- Тесты: html `test_block_admonition_html`/`test_note_style_on_listing_delimiter`
  переписаны под верную семантику, +1 `test_admonition_block_vs_paragraph_forms`
  (open-форма, bare-формы, игнор на sidebar/quote, вложенный admonition);
  parser `test_block_admonition`/`_warning` → `block: true`; integration 2 места;
  builder.rs (compat) — паттерн `{ kind, .. }`.

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (parser 467, html 337→338).
- Пробы 11/12 байт-в-байт (искл. p_adm12 — pre-existing passthrough-`</div>`).
- **Корпус: Identical 235→239 (+4)**; blast (base 3dfe796): 10 файлов — 4 флипа
  (header.adoc, icon-macro.adoc, apply-subs-to-blocks.adoc, validation.adoc),
  **0 регрессий**; 6 changed-still-different: ordered 420→232, admonition 223→197,
  special-characters 150→148, cookbook 2604→2582, java/index 2313=2313,
  syntax-quick-reference 2759→2791 (позиционный шум — admonition-сегмент проверен
  локальным diff'ом байт-в-байт).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 239: **reference-revision-attributes (31 diff)**, listing (34),
  reference-author (37), id (45), checklist (49), collapsible (51), release-plan (56),
  stem (56), block (57), literal-monospace (59), source (63);
  revision-line-with-version-prefix (1 — `{docdate}`, скип).
  Прочее: `cols="2*"` multiplier (row.adoc), `[abstract]`-параграф → quoteblock,
  `:icons:`-colist (TODO), кластер `m`/`e`/`s` стиля колонок; новые pre-existing
  находки: лишний `</div>` у standalone passthrough-блока, unknown-style течёт в
  class на quote/sidebar (asciidoctor дропает).

---

## Сессия (2026-06-11, тринадцатая) — Фаза 3: add-header-row.adoc (noheader + formal options=)

Запрос «продолжи». Ветка **`fix/table-noheader-option`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 234, master `1c22959` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**add-header-row.adoc (29 diff)**, один корень + попутный пробел.

### Семантика asciidoctor (пробы /tmp/p_nh1..7.adoc)
- `noheader` (shorthand `%noheader` И formal `options=noheader`) подавляет ТОЛЬКО
  implicit-промоушен первой строки в header; явный `header` побеждает
  (`%header%noheader` → `<thead>`).
- `opts=` — alias `options=`; значение comma-separated (`options="header,footer"`).
- Попутно обнаружено: formal `options=header` у нас ВООБЩЕ не работал — в корпусе
  маскировался implicit-правилом (blank после первой строки в formal-таблицах).

### Что сделано (только ПАРСЕР, 3 точки)
- `attributes.rs::parse`: named `options`/`opts` промотируются в вектор `options`
  (split по `,`, trim, тот же путь, что shorthand `%`; named["options"] никто не читал).
- `block.rs`: оба места has_header (psv ~1379, csv/dsv ~1627) —
  `&& !block_attrs.has_option("noheader")` в implicit-ветке.
- +1 html-тест `test_table_noheader_option_html` (5 кейсов: shorthand/formal noheader,
  конфликт, formal header без implicit-layout, opts-alias).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (html 336→337).
- Все 7 проб байт-в-байт (кроме p_nh4 CSV — остаточный pre-existing `<colgroup>`-diff,
  НЕ про header; thead подавлен верно).
- **Корпус: Identical 234→235 (+1)**; blast (base 1c22959): 2 файла — 1 флип
  (add-header-row.adoc), **0 регрессий**; row.adoc 312→310 (changed-still-different,
  доминирует корень `cols="2*"` multiplier — НЕ поддержан, потенциальная задача).
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 235: **apply-subs-to-blocks (31 diff)**, reference-revision-attributes (31),
  listing (34), reference-author (37), icon-macro (41), id (45), checklist (49),
  collapsible (51); revision-line-with-version-prefix (1 — `{docdate}`, скип).
  Новое: `cols="2*"` multiplier-синтаксис (row.adoc 310 diff — крупный, но один корень?).
  Прочее: `[abstract]`-параграф → quoteblock, `:icons:`-colist (TODO),
  кластер `m`/`e`/`s` стиля колонок.

---

## Сессия (2026-06-11, двенадцатая) — Фаза 3: part.adoc ([partintro]-параграф → open block)

Запрос «продолжи». Ветка **`fix/partintro-paragraph-openblock`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 233, master `6f82f8a` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**part.adoc (18 diff)**, один корень.

### Семантика asciidoctor (пробы /tmp/p_pi1..4.adoc)
- `[partintro]` на параграфе — masquerade в open block:
  `<div class="openblock partintro"><div class="content"><div class="paragraph"><p>…`.
- Вне book-part — ERROR + exclude всего блока (НЕ реализовано: в корпусе нет таких).
- `[partintro]` на `--`-блоке — у нас уже работало (фолбэк `_ => {}`).
- `[abstract]`-параграф → `<div class="quoteblock abstract"><blockquote>текст` (БЕЗ
  paragraph-обёртки) — НЕ сделано, отдельный potential-кластер (abstract-block 5 diff).

### Что сделано (только ПАРСЕР, 2 точки)
- `attributes.rs::block_style_kind`: +`"partintro"`.
- `block.rs::scan_paragraph`: arm `quote|example|sidebar` → `…|partintro`,
  kind `DelimitedBlockKind::Open`; style не исключён в emit_block_metadata →
  класс `openblock partintro` собирает рендерер.
- +1 html-тест `test_partintro_paragraph_masquerades_as_open_block` (masquerade +
  guard явного open-блока).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (html 335→336).
- **Корпус: Identical 233→234 (+1)**; blast (base 6f82f8a): ровно 1 файл — 1 флип
  (part.adoc, 0 diffs), **0 регрессий**.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 234: **add-header-row (29 diff)**, apply-subs-to-blocks (31),
  reference-revision-attributes (31), listing (34), reference-author (37),
  icon-macro (41), id (45); revision-line-with-version-prefix (1 — `{docdate}`, скип).
  Прочее: `[abstract]`-параграф → quoteblock (см. выше), `:icons:`-colist (TODO),
  кластер `m`/`e`/`s` стиля колонок.

---

## Сессия (2026-06-11, одиннадцатая) — Фаза 3: url.adoc (irc-схема, link role=, mailto query)

Запрос «продолжи». Ветка **`fix/url-macro-irc-role-mailto`** — НЕ закоммичена
(рабочее дерево). Baseline: Identical 232, master `4c62625` (base-бинарь пересобран).

### Выбор задачи
nearmiss: revision-line-with-version-prefix (1 diff — `{docdate}`, скип) →
**url.adoc (7 diff)**, три корня.

### Семантика asciidoctor (пробы /tmp/p_url1..3.adoc, /tmp/p_u_a..d.adoc)
- `irc://` и `ftp://` — автолинк-схемы как http(s); голые → `class="bare"`.
- `role=green` на link/url/mailto-макросах → class на `<a>`; пустой текст →
  `class="bare green"` (bare первым). Raw-порядок атрибутов: href, class, target, rel.
- mailto positional 2/3 → `?subject=&body=`, percent-encode ERB-стиля (литеральны
  `A-Za-z0-9_.~-`, пробел `%20`, hex UPPERCASE), кавычки снимаются.
  `mailto:a@b[T,,body]` (пустой subject) — asciidoctor ПАДАЕТ (nil) → поведение
  свободно, у нас пустые компоненты опускаются.

### Что сделано
- **ПАРСЕР** `event.rs`: `Tag::Link` +поле `role: Option<CowStr>`.
- **ПАРСЕР** `attributes.rs::parse_link_attrs`: +role/subject/body; named-ветка
  гейтится валидным именем ключа; латентный баг закрыт — named-only attrlist
  (`[role=x]`/`[window=_blank]`) теперь даёт ПУСТОЙ text (→ bare), а не весь
  bracket_content.
- **ПАРСЕР** `inline.rs`: +2 dispatch-арма ftp://+irc:// → try_autolink;
  `url_encode_into` (ERB-стиль); mailto строит query-URL (Cow::Owned).
- **РЕНДЕРЕР** `events.rs` arm Tag::Link: class = bare+role сразу после href.
- +1 parser-тест `test_link_role_mailto_query_irc_scheme` (5 кейсов), +1 html-тест
  `test_link_role_and_mailto_query_html` (6 ассертов). Тестовые инициализаторы
  Tag::Link дополнены `role: None` (perl one-liner).

### Статус (верифицировано)
- clippy --workspace 0; cargo test --workspace зелёное (893: parser 467, html 335).
- **Корпус: Identical 232→233 (+1)**; blast (base 4c62625): ровно 1 файл — 1 флип
  (url.adoc), **0 регрессий**. Все 5 проб байт-в-байт с asciidoctor.
- НЕ закоммичено — коммит/мерж по запросу пользователя.

### Что дальше
- nearmiss на 233: **part.adoc (18 diff, len_delta=4)**, add-header-row (29),
  apply-subs-to-blocks (31), reference-revision-attributes (31), listing (34),
  reference-author (37), icon-macro (41), id (45), checklist (49);
  revision-line-with-version-prefix (1 — `{docdate}`, скип). Прочее: `:icons:`-colist
  (TODO), кластер `m`/`e`/`s` стиля колонок.

---

## Методика (каноническая, действует во всех сессиях)

- **Git**: никогда не коммитить в master напрямую; `git checkout master && git pull` →
  новая ветка `fix/...`. Коммит/мерж/пуш — ТОЛЬКО по запросу пользователя.
  session.md обычно пишется ДО мержа — статус «НЕ закоммичено» прошлой сессии означает
  «смотри git log: следующая сессия начинается с уже смерженного master».
- **НЕ запускать cargo fmt.**
- **Корпус**: `/mnt/c/tmp/adoc-test/` (344 файла), `python3 compare_full.py`
  (нужен release-бинарь: `cargo build --release -p adoc-cli`).
- **blast**: `/tmp/blast.py` — пофайловое сравнение с `/tmp/adoc_base` (release-бинарь
  чистого master; пересобирать в начале сессии: build → `cp target/release/adoc
  /tmp/adoc_base`). Показывает флипы/регрессии/changed-still-different.
- **fdiff**: `/tmp/fdiff.py <relpath>` — позиционный diff одного файла.
- **nearmiss**: `/tmp/nearmiss.py` — ранжирует Different-файлы по числу diff'ов;
  берём ближайший к флипу. (revision-line-with-version-prefix закрыт в 24-й
  сессии — CLI сидит `docdate` из mtime файла.)
- **Семантику asciidoctor проверять пробами** (`asciidoctor -o - [-s] /tmp/p_*.adoc`,
  установлен в /usr/bin/asciidoctor) ДО фикса; фиксировать выводы в session.md/TODO.md.
- CLI: `adoc [--no-standalone] file` (флага `-e` НЕТ).
- Перед коммитом: `cargo clippy --workspace` (0 warnings) + `cargo test --workspace`
  (всё зелёное). После фикса: корпус + blast (0 регрессий — обязательное условие).

---

## Архив сессий (сжато; полные детали каждого фикса — в TODO.md и git log)

Формат: тема — ветка; корпус-дельта. Все смержены в master.

### 2026-06-11 (Фаза 3 + R9)
- **одиннадцатая** — url.adoc: irc/ftp-автолинк, link `role=`→class, mailto subject/body
  query (см. выше); 232→233.
- **десятая** — multi-author `author_2`: name_suffix `_2`/`_3` для attr-entries,
  id_suffix без сепаратора для span-id (CORE AuthorRegistry) —
  `fix/multi-author-attr-underscore`; 231→232 (multiple-authors.adoc).
- **девятая** — email-автолинк без `class="bare"` — `fix/email-autolink-no-bare-class`;
  230→231 (header.adoc). bare — только URL-автолинки и `link:`/URL-макросы с пустым текстом.
- **восьмая** — version-label в revnumber-span + attr-entry внутри текстового блока =
  литерал (в dlist wrapped — дроп) — `fix/version-label-revnumber`; 229→230.
- **седьмая** — toc2/toc-left/toc-right: классы на body (только header-`:toc:` c
  left/right), div — голый `class="toc2"` — `fix/toc2-body-class`; 228→229.
- **шестая** — п.41 header после ведущих комментариев —
  `fix/header-after-leading-comments`; **210→228 (+18)**.
- **пятая** — sect0-heading standalone (без div-обёртки) + admonition image-иконки при
  `:icons:` (не-font) — `fix/callout-rendering`; 208→210. Остаток: `:icons:`-colist
  таблицей (TODO).
- **четвёртая** — QUOTES/ATTRIBUTES в метках link/xref/mailto (inner-reparse
  `subs.without(MACROS)` в `push_macro_label`) — `fix/macro-label-inline-formatting`;
  206→208. Остаток: `\` `` в метке съедает оба backslash (pre-existing).
- **третья** — `pass:[…]` извлекается ДО `+…+` (случай A) —
  `fix/pass-macro-in-single-plus`; 205→206.
- **вторая** — YouTube-плейлисты в video (target `id/list`, `id1,id2`, голый loop →
  `&playlist={id}`; порядок query-параметров) — `fix/youtube-playlist-params`; 204→205.
- **первая** — **R9**: `InlineOptions` — общий канал document-attrs → inline-парсер
  (streaming `apply_attribute` + snapshot `from_attr_lookup`) —
  `refactor/inline-doc-attrs-channel`; нейтрально (байт-в-байт). АУДИТ R1–R9 ЗАКРЫТ.

### 2026-06-10 (аудит рендерера R1–R8)
- **восьмая** — **R8**: распил adoc-html/src/lib.rs (6220 строк) на модули (events,
  blocks, inline, media, finish, escape, tests) — `refactor/html-modules`; байт-в-байт.
- **седьмая** — **R7-5 (финал)**: Author/AuthorRegistry + Revision в adoc-render-core —
  `refactor/render-core-author-revision`; байт-в-байт.
- **шестая** — **R7-4**: CaptionCounters + FootnoteRegistry в core —
  `refactor/render-core-captions`; байт-в-байт.
- **пятая** — **R7-3**: SectionNumberer + TocBuilder (toc_steps) в core —
  `refactor/render-core-section-toc`; байт-в-байт.
- **четвёртая** — **R7-2**: XrefResolver (RefText::{Plain,Markup}, precedence) в core —
  `refactor/render-core-xref-resolver`; байт-в-байт.
- **третья** — **R7-1**: крейт **adoc-render-core** (интринсики
  IntrinsicAttribute{text,html}, resolve_attribute_reference, resolve_attr_refs_text);
  закрыт дрейф builder.rs (apos/pp/quot) — `refactor/render-core-attr-resolver`.
- **вторая** — **R5**: ResolutionContext + однопроходный резолв `\x00`-сентинелей
  (рекурсия depth 8; стресс 2000 xref 807ms→33ms) —
  `refactor/finish-single-pass-resolution`; байт-в-байт + багфикс вложенных сентинелей.
- **первая** — **R1/R2/R4/R6 + частично R3/R5**: figure-caption (title ПОСЛЕ content,
  счётчик, parse_image_attrs caption=/title=), video/stem title-leak, хелпер
  `open_block_with_title` (новые block-arm'ы писать через него!),
  `push_media_time_fragment`, li-paragraph хелперы — `fix/block-image-figure-caption`;
  204 (0 флипов, улучшения diff'ов).

### 2026-06-09 (марафон Фазы 3, поздняя-1…29; 145→204)
- **29** — аудит рендерера (БЕЗ правок): находки R1–R9, верифицированы агентами.
- **28** — MathJax-loader при `:stem:` (const MATHJAX_DOCINFO в write_document_tail) —
  `fix/stem-mathjax-docinfo`; 203→204. Остаток: `eqnums` (не в корпусе).
- **27** — rowspan: двойной декремент occupancy → ячейки уезжали в спанированную
  колонку — `fix/rowspan-row-placement`; 202→203. Остаток: col_idx в emit_row_cells
  не учитывает rowspan-сдвиг (латентно).
- **26** — continuation-блок в callout-элементе (li_p_open для CalloutListItem) + сдвиг
  позиционных слотов ведущим named/shorthand-атрибутом (`[id=app, source, yaml]`) —
  `fix/callout-item-block-and-shifted-source-lang`; 200→202.
- **25** — audio: `opts=` alias, `#t=start,end`, `.Title` —
  `fix/audio-start-opts-and-title`; 199→200.
- **24** — intrinsic `{quot}`/`{apos}`/`{pp}` + `pass:[…]` в constrained-matching
  (случай G, pass_macro_span_len) — `fix/intrinsic-quot-apos-and-pass-constrained`;
  198→199.
- **23** — UI-макросы kbd:/btn:/menu: за `:experimental:` —
  `fix/gate-experimental-ui-macros`; 194→198.
- **22** — revnumber strip нецифрового префикса + `[%hardbreaks]` —
  `fix/revision-prefix-and-hardbreaks`; 193→194. Отложенный баг: trailing ` +` в
  reparsed monospace → ложный `<br>` (pre-existing, outline.adoc).
- **21** — `.Title` на отступном literal-параграфе — `fix/literal-paragraph-block-title`;
  192→193.
- **20** — голый `{name}` на счётчик в document-order (препроцессор) —
  `fix/counter-bare-reference`; 191→192. Остаток: счётчики в verbatim (counters.adoc).
- **19** — section-id: точки-разделители + дедуп `_2` — `fix/section-id-dots-and-dedup`;
  190→191.
- **18** — escaped inline-макрос `\name:target[…]` — `fix/escaped-inline-macro`; 189→190.
- **17** — single-plus `+…+` как constrained-пара —
  `fix/single-plus-passthrough-constrained`; 188→189.
- **16** — passthrough внутри monospace/quote (`` `++`++` ``) —
  `fix/passthrough-inside-monospace`; 186→188.
- **15** — путь между `}` и `[` в attr-ref (`{url}/issues[text]`) —
  `fix/attr-ref-path-before-brackets`; 185→186.
- **14** — значение `{attr-ref}` уважает subs блока — `fix/attr-ref-respect-block-subs`;
  184→185.
- **13** — verbatim-параграф сохраняет `//`-комментарий — `fix/verbatim-paragraph-comment`;
  182→184.
- **12** — header-style колонка `h` → `<th>` — `fix/table-header-column-style`; 180→182.
  Остаток: `m`/`e`/`s`/`a`/`l` стили не наследуются (кластер, TODO).
- **11** — `{attr-ref}[text]` как ссылка (subs-order; render_inline_value) —
  `fix/attr-ref-link-macro`; **175→180 (+5)**.
- **10** — trailing whitespace строк параграфа (rstrip_line_trailing_ws) —
  `fix/paragraph-trailing-whitespace`; 173→175.
- **9** — `table-caption` document-атрибут — `fix/table-caption-doc-attr`; 172→173.
- **8** — `link:url[]` пустой текст → `class="bare"` (п.14) — link-macro-empty-bare;
  171→172.
- **7** — preserve bare char-ref `&#167;` (п.15) — bare-char-reference-preserved;
  170→171. Остаток: char-ref внутри `` `…` `` (Event::Code).
- **6** — неизвестный verbatim-style → class — `fix/literal-unknown-style-class`; 169→170.
- **5** — custom caption на admonition — admonition-custom-caption; 168→169.
- **4** — REPLACEMENTS в тексте макроса (остаток п.37) — macro-text-replacements;
  165→168.
- **3** — xref fallback `[id]` + bibliography reftext — xref-fallback-bracketed-id;
  162→165. Родственный остаток: inline-anchor reftext из dt-терма (lexicon.adoc).
- **2** — link blank-window `^` (п.14) — link-blank-window-caret; 158→162.
- **1** — п.19 xref-id нормализация (natural cross reference) — 157→158.
- **(дневная)** — п.18 image alt двойные кавычки; 153→157.

### 2026-05-31 и ранее
- em-dash границы + ZWSP; 149→153. Escaped preprocessor-директива; 145→149.
- Ранняя история (Фазы 1–2, аудиты D1–D6, xref авто-текст 79→135 и пр.) — в TODO.md,
  разделы «Сделано».
