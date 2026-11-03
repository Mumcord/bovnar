syn clear

hi BovnarComment      ctermfg=240 guifg=#6e7a86 gui=none
hi BovnarKeySigil     ctermfg=135 guifg=#c084fc
hi BovnarKeyName      ctermfg=39  guifg=#67d1f4
hi BovnarSemicolon    ctermfg=245 guifg=#828d9a gui=none
hi BovnarAssign       ctermfg=203 guifg=#f47067
hi BovnarType         ctermfg=255 guifg=#e8e8e8 gui=bold
hi BovnarTypeDelim    ctermfg=186 guifg=#a09040 gui=none
hi BovnarTypeSep      ctermfg=246 guifg=#8a94a1 gui=none
hi BovnarTypeParam    ctermfg=209 guifg=#da844c gui=none
hi BovnarTypeBitWidth ctermfg=205 guifg=#ff5faf gui=none
hi BovnarTypeBase     ctermfg=158 guifg=#afffd7 gui=none
hi BovnarNoUnit       ctermfg=114 guifg=#87d88c gui=none
hi BovnarSpecialFloat ctermfg=170 guifg=#c084fc
hi BovnarFloat        ctermfg=220 guifg=#f0c64e gui=none
hi BovnarInteger      ctermfg=220 guifg=#f0c64e gui=none
hi BovnarNegative     ctermfg=203 guifg=#f47067 gui=none
hi BovnarString       ctermfg=114 guifg=#87d88c gui=none
hi BovnarEscape       ctermfg=72  guifg=#5ab06e gui=bold
hi BovnarInvalidEsc   ctermfg=203 guifg=#f47067 gui=undercurl
hi BovnarSymbol       ctermfg=39  guifg=#67d1f4 gui=none
hi BovnarRefOp        ctermfg=203 guifg=#f47067 gui=bold
hi BovnarRefPath      ctermfg=168 guifg=#d4737f gui=none
hi BovnarDirective    ctermfg=170 guifg=#c084fc gui=bold
hi BovnarDateTime     ctermfg=80  guifg=#5eead4 gui=none
hi BovnarArrayDelim1  ctermfg=209 guifg=#da844c gui=none
hi BovnarArrayDelim2  ctermfg=135 guifg=#c084fc gui=none
hi BovnarArrayDelim3  ctermfg=204 guifg=#ff5f96 gui=none
hi BovnarArrayDelim4  ctermfg=214 guifg=#ffa500 gui=none
hi BovnarArrayDelim5  ctermfg=46  guifg=#87d88c gui=none
hi BovnarArraySep     ctermfg=246 guifg=#8a94a1 gui=none
hi BovnarRowSep       ctermfg=246 guifg=#8a94a1 gui=none
hi BovnarUnitSep      ctermfg=246 guifg=#8a94a1 gui=none
hi BovnarUnitPrefix   ctermfg=80  guifg=#5eead4 gui=none
hi BovnarUnitTilde    ctermfg=246 guifg=#8a94a1 gui=none
hi BovnarUnitExp      ctermfg=80  guifg=#5eead4 gui=none
hi BovnarTypeUnit     ctermfg=80  guifg=#5eead4 gui=none
hi BovnarTypeUnitSep  ctermfg=246 guifg=#8a94a1 gui=none
hi BovnarTypeUnitExp  ctermfg=80  guifg=#5eead4 gui=none
hi BovnarTypePrefix   ctermfg=80  guifg=#5eead4 gui=none
hi BovnarOctetData    ctermfg=102 guifg=#6272a4 gui=italic
hi BovnarNull         ctermfg=240 guifg=#4c5661 gui=italic
hi BovnarBoolean      ctermfg=170 guifg=#c084fc gui=bold

" 8 cycling colors for { } depth — levels 1,9,17,… share color 1; 2,10,18,… color 2; etc.
hi BovnarStructDelim1 ctermfg=209 guifg=#da844c gui=none
hi BovnarStructDelim2 ctermfg=135 guifg=#c084fc gui=none
hi BovnarStructDelim3 ctermfg=204 guifg=#ff5f96 gui=none
hi BovnarStructDelim4 ctermfg=214 guifg=#ffa500 gui=none
hi BovnarStructDelim5 ctermfg=46  guifg=#87d88c gui=none
hi BovnarStructDelim6 ctermfg=80  guifg=#5eead4 gui=none
hi BovnarStructDelim7 ctermfg=220 guifg=#f0c64e gui=none
hi BovnarStructDelim8 ctermfg=39  guifg=#67d1f4 gui=none

syn match   BovnarOctetData    '\\x[0-9A-Fa-f]\{2\}'

syn region  BovnarComment start=/#/ end=/$/
" Spec version directive (spec 1.1): #!bovnar <major>.<minor>. Defined AFTER the
" comment region so vim's last-defined-wins rule picks it over the '#' comment at
" the start of a directive line. Only meaningful as the first line.
syn match   BovnarDirective '^#!bovnar\s\+\d\+\.\d\+\s*$'

syn match   BovnarArraySep    ','
syn match   BovnarRowSep      '/'
syn match   BovnarKeySigil    '\.'

syn match   BovnarNegative '-\(\d\+\.\d*\([eE][+-]\=\d\+\)\=\|\.\d\+\([eE][+-]\=\d\+\)\=\|\d\+[eE][+-]\=\d\+\|\d\+\)' contains=BovnarFloat,BovnarInteger nextgroup=BovnarInlineProfile,BovnarInlineUnit skipwhite

syn match   BovnarAssign    '='
syn match   BovnarSemicolon ';'

syn match   BovnarNull '=\s*;'

syn match   BovnarInteger '\d\+' nextgroup=BovnarInlineProfile,BovnarInlineUnit skipwhite
syn match   BovnarFloat   '\d\+\.\d*\([eE][+-]\=\d\+\)\?' nextgroup=BovnarInlineProfile,BovnarInlineUnit skipwhite
syn match   BovnarFloat   '\d\+[eE][+-]\=\d\+' nextgroup=BovnarInlineProfile,BovnarInlineUnit skipwhite
syn match   BovnarFloat   '\.\d\+\([eE][+-]\=\d\+\)\?' nextgroup=BovnarInlineProfile,BovnarInlineUnit skipwhite

" ISO-8601 datetime literal (spec 1.1): YYYY-MM-DD with an optional T time,
" fractional seconds, and a Z or ±HH:MM offset. Defined AFTER BovnarInteger on
" purpose: at the shared leading-YYYY position vim resolves equal-start matches
" in favour of the item defined LAST (not the longest), so this more specific
" match must come after BovnarInteger to win — otherwise a date highlights as a
" bare integer (and, inside arrays/structs, splits into integer-dash-integer).
syn match   BovnarDateTime '\d\{4}-\d\{2}-\d\{2}\(T\d\{2}:\d\{2}:\d\{2}\(\.\d\+\)\=\(Z\|[+-]\d\{2}:\d\{2}\)\=\)\='

syn region  BovnarString
      \ start=/"/ skip=/\\./ end=/"/
      \ contains=BovnarEscape,BovnarInvalidEsc
      \ nextgroup=BovnarInlineProfile,BovnarInlineUnit skipwhite
" InvalidEsc is defined BEFORE Escape so that on the \t \n \v \f \r \" \\ valid
" escapes — which both patterns match with equal length — the later-defined
" BovnarEscape wins vim's last-defined-wins tie-break. Only genuinely illegal
" sequences (e.g. \q) fall through to BovnarInvalidEsc.
syn match   BovnarInvalidEsc '\\.'
syn match   BovnarEscape     '\\[tnvfr"\\]'
" spec 1.1 escapes: \u{1-6 hex} (Unicode scalar) and \xHH (one byte). Marked
" `contained` so they only apply inside strings and never shadow the global
" BovnarOctetData '\xHH' octet-stream delimiter.
syn match   BovnarEscape     '\\u{[0-9A-Fa-f]\{1,6}}' contained
syn match   BovnarEscape     '\\x[0-9A-Fa-f]\{2}' contained

syn match   BovnarRefOp   '&'
" The whole '&.seg.seg' is matched as one item (the '&' included) so it wins over
" BovnarKeyName, which would otherwise grab each '.seg' and colour the path like a
" key. (A '&\zs' split leaves the path exposed to BovnarKeyName, so the sigil is
" kept inside the path match here; web/sublime/vscode split it via captures.)
" spec 1.1: a reference path may carry array indices, e.g. &.matrix[0][1].
syn match   BovnarRefPath '&\(\.[A-Za-z_\d192-\d65535][A-Za-z0-9_+\-\d128-\d65535]*\|\[\d\+\]\)\+'

syn region  BovnarTypeAnn
      \ start=/</ end=/>/
      \ contains=BovnarType,BovnarTypeParam,BovnarNoUnit,
      \          BovnarTypeDelim,BovnarTypeSep,BovnarComment,
      \          BovnarTypeBitWidth,BovnarTypeBase,
      \          BovnarTypeUnit,BovnarTypeUnitSep,
      \          BovnarTypeUnitExp,BovnarTypePrefix,
      \          BovnarUnitPrefix,BovnarUnitSep,
      \          BovnarProfileUnit,
      \          BovnarString,
      \          BovnarRefPath,BovnarArraySep
      \ keepend

syn match   BovnarUnitTilde /\~/ contained
syn match   BovnarSymbol    '\<[A-Za-z_\d192-\d65535][A-Za-z0-9_+\-\d128-\d65535]*\>'
" Reserved value keywords — defined after BovnarSymbol so they win the tie and
" are coloured as keywords, not symbols. on==true, off==false; null is the null
" keyword. All reserved keywords share the keyword colour and differ from symbols.
" The trailing '\%([+-]\)\@!' is required because '+' and '-' are valid
" symbol-body chars that vim's \> boundary does NOT exclude: without it,
" 'on-failure' would colour 'on' as a keyword (it wins the same-start tie). The
" guard makes the keyword fail there so BovnarSymbol matches the whole word.
syn match   BovnarBoolean   '\<\(true\|false\|on\|off\)\>\%([+-]\)\@!'
syn match   BovnarNullKw    '\<null\>\%([+-]\)\@!'
hi          BovnarNullKw    ctermfg=170 guifg=#c084fc gui=bold
" nan / inf / ninf — bare reserved special-float keywords (no sigil). Defined
" after BovnarSymbol too, so they win the tie; \< \> keep 'infinity'/'nana' out,
" and the '\%([+-]\)\@!' guard keeps 'inf-loop' etc. a single symbol.
syn match   BovnarSpecialFloat '\<\(nan\|inf\|ninf\)\>\%([+-]\)\@!'
" No \< \> word boundaries: vim treats the leading °, µ, Ω of a unit as non-word
" characters, so a leading \< never matches a unit that starts with one (e.g.
" '°C'). The group is 'contained' (only reached inside <…> or a BovnarInlineUnit),
" so the boundaries are unnecessary anyway.
syn match   BovnarTypeUnit  '[A-Za-zµΩΩ°Δ][A-Za-z0-9_·()\-\+\/\^°Δ]*' contained
syn match   BovnarUnitSep   '[()]' contained
syn match   BovnarTypeUnit  '[%‰‱]' contained
" Currency component carries a mandatory '$' sigil (spec 1.0): $USD, $BTC.
syn match   BovnarTypeUnit  '\$[A-Za-z][A-Za-z0-9]*' contained
syn match   BovnarUnitPrefix '[A-Za-zµ][A-Za-z0-9]*\~\$\=[A-Za-z0-9_·\-\+\/*\^]*' contained
syn match   BovnarUnitSep   '[*\/]' contained
syn match   BovnarUnitExp   '[⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻]' contained
syn match   BovnarUnitExp   '\^[+-]\=\d\+' contained

" Inline unit suffix -- only valid immediately after a number (reached via the
" numeric matches' nextgroup). Keeps value-position barewords as BovnarSymbol and
" stops illegal symbol chars (* / ^ ( ) ?) being coloured as units/separators.
" '#' is excluded from both char classes so a trailing comment after a value
" ('.x = 5 # note') falls through to BovnarComment instead of being swallowed as
" a bogus unit by the nextgroup.
syn match BovnarInlineUnit '[^ \t\r;,#0-9+*/^()~\-\]}][^ \t\r;,#\]}]*' contained contains=BovnarUnitPrefix,BovnarTypeUnit,BovnarUnitSep,BovnarUnitExp,BovnarUnitTilde
hi link BovnarInlineUnit BovnarTypeUnit

" A unit PROFILE (doc/11): "namespace:code", e.g. ucum:mm[Hg], ucum:10*3/uL{WBC},
" udunits:kg m-2 s-1. The code is the other vocabulary's own notation, so none of
" the native classes apply -- its digits are not a bit-width, its braces not a
" struct -- and it is coloured whole. Both are defined AFTER the native unit
" matches so they win where both start at the same column.
"
" In an annotation it may only begin a parameter (after the family ':' or a ','),
" which is what keeps "float:64" from reading as a namespace; a ',' inside {...}
" and a space followed by more code both belong to it.
syn match BovnarProfileUnit '\%([,:]\s*\)\@<=[a-z][a-z0-9-]*:\%({[^}>]*}\|[^,>{[:space:]]\|[ \t]\+\%([^,>[:space:]]\)\@=\)\+' contained
hi link BovnarProfileUnit BovnarTypeUnit
" As an inline suffix it has no space and no comma, and may end in ']' or '}'.
syn match BovnarInlineProfile '[a-z][a-z0-9-]*:[^ \t\r;,#]\+' contained
hi link BovnarInlineProfile BovnarTypeUnit

syn match   BovnarType     '\v(float_fix|float_dec|float|uint|sint|utf8|bool|datetime)'
syn match   BovnarNoUnit   'no_unit'
syn match   BovnarTypeDelim '[<>]' contained
syn match   BovnarTypeSep  ':' contained
syn match   BovnarTypeParam    'q[0-9]\+' contained
syn match   BovnarTypeBitWidth '[0-9]\+'  contained
syn match   BovnarTypeBase     '_[0-9]\+' contained

syn match   BovnarTypeUnitSep '[*\/]'    contained
syn match   BovnarTypeUnitExp '[⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻]' contained
syn match   BovnarTypeUnitExp '\^[+-]\=\d\+' contained
syn match   BovnarTypeUnitExp '\/[1-9]'      contained
syn match   BovnarTypePrefix  '[A-Za-zµ][A-Za-z0-9]*\~' contained

syn match   BovnarKeyName '\.[A-Za-z_\d192-\d65535][A-Za-z0-9_+\-\d128-\d65535]*'

" Terminal items only — no struct or array regions.
" Both BovnarArray1 and BovnarStruct1 are non-contained and are added explicitly
" to the contains lists of each array/struct region below.
syn cluster BovnarContent contains=
      \ BovnarComment,BovnarKeySigil,BovnarKeyName,
      \ BovnarSemicolon,BovnarAssign,BovnarTypeAnn,
      \ BovnarNegative,BovnarFloat,BovnarInteger,BovnarDateTime,
      \ BovnarString,BovnarEscape,BovnarInvalidEsc,
      \ BovnarSymbol,BovnarBoolean,BovnarNullKw,
      \ BovnarRefOp,BovnarRefPath,
      \ BovnarArraySep,BovnarRowSep,
      \ BovnarNull,BovnarSpecialFloat,BovnarOctetData,
      \ BovnarType

" 5-level array region chain.
" BovnarArray1 is not contained — it starts at the top level and inside struct regions.
" Each level's matchgroup colors both [ and ] with the same highlight group.
" Struct depth resets to 1 when entering an array context.
syn region BovnarArray1 matchgroup=BovnarArrayDelim1 start=/\[/ end=/\]/ contains=@BovnarContent,BovnarArray2,BovnarStruct1
syn region BovnarArray2 matchgroup=BovnarArrayDelim2 start=/\[/ end=/\]/ contained contains=@BovnarContent,BovnarArray3,BovnarStruct1
syn region BovnarArray3 matchgroup=BovnarArrayDelim3 start=/\[/ end=/\]/ contained contains=@BovnarContent,BovnarArray4,BovnarStruct1
syn region BovnarArray4 matchgroup=BovnarArrayDelim4 start=/\[/ end=/\]/ contained contains=@BovnarContent,BovnarArray5,BovnarStruct1
syn region BovnarArray5 matchgroup=BovnarArrayDelim5 start=/\[/ end=/\]/ contained contains=@BovnarContent,BovnarStruct1

" 64-level struct region chain, 8-color cycling.
" BovnarArray1 is listed explicitly so arrays inside structs are highlighted.
syn region BovnarStruct1  matchgroup=BovnarStructDelim1 start=/{/ end=/}/ contains=@BovnarContent,BovnarStruct2,BovnarArray1
syn region BovnarStruct2  matchgroup=BovnarStructDelim2 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct3,BovnarArray1
syn region BovnarStruct3  matchgroup=BovnarStructDelim3 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct4,BovnarArray1
syn region BovnarStruct4  matchgroup=BovnarStructDelim4 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct5,BovnarArray1
syn region BovnarStruct5  matchgroup=BovnarStructDelim5 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct6,BovnarArray1
syn region BovnarStruct6  matchgroup=BovnarStructDelim6 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct7,BovnarArray1
syn region BovnarStruct7  matchgroup=BovnarStructDelim7 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct8,BovnarArray1
syn region BovnarStruct8  matchgroup=BovnarStructDelim8 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct9,BovnarArray1
syn region BovnarStruct9  matchgroup=BovnarStructDelim1 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct10,BovnarArray1
syn region BovnarStruct10 matchgroup=BovnarStructDelim2 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct11,BovnarArray1
syn region BovnarStruct11 matchgroup=BovnarStructDelim3 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct12,BovnarArray1
syn region BovnarStruct12 matchgroup=BovnarStructDelim4 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct13,BovnarArray1
syn region BovnarStruct13 matchgroup=BovnarStructDelim5 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct14,BovnarArray1
syn region BovnarStruct14 matchgroup=BovnarStructDelim6 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct15,BovnarArray1
syn region BovnarStruct15 matchgroup=BovnarStructDelim7 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct16,BovnarArray1
syn region BovnarStruct16 matchgroup=BovnarStructDelim8 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct17,BovnarArray1
syn region BovnarStruct17 matchgroup=BovnarStructDelim1 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct18,BovnarArray1
syn region BovnarStruct18 matchgroup=BovnarStructDelim2 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct19,BovnarArray1
syn region BovnarStruct19 matchgroup=BovnarStructDelim3 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct20,BovnarArray1
syn region BovnarStruct20 matchgroup=BovnarStructDelim4 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct21,BovnarArray1
syn region BovnarStruct21 matchgroup=BovnarStructDelim5 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct22,BovnarArray1
syn region BovnarStruct22 matchgroup=BovnarStructDelim6 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct23,BovnarArray1
syn region BovnarStruct23 matchgroup=BovnarStructDelim7 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct24,BovnarArray1
syn region BovnarStruct24 matchgroup=BovnarStructDelim8 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct25,BovnarArray1
syn region BovnarStruct25 matchgroup=BovnarStructDelim1 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct26,BovnarArray1
syn region BovnarStruct26 matchgroup=BovnarStructDelim2 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct27,BovnarArray1
syn region BovnarStruct27 matchgroup=BovnarStructDelim3 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct28,BovnarArray1
syn region BovnarStruct28 matchgroup=BovnarStructDelim4 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct29,BovnarArray1
syn region BovnarStruct29 matchgroup=BovnarStructDelim5 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct30,BovnarArray1
syn region BovnarStruct30 matchgroup=BovnarStructDelim6 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct31,BovnarArray1
syn region BovnarStruct31 matchgroup=BovnarStructDelim7 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct32,BovnarArray1
syn region BovnarStruct32 matchgroup=BovnarStructDelim8 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct33,BovnarArray1
syn region BovnarStruct33 matchgroup=BovnarStructDelim1 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct34,BovnarArray1
syn region BovnarStruct34 matchgroup=BovnarStructDelim2 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct35,BovnarArray1
syn region BovnarStruct35 matchgroup=BovnarStructDelim3 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct36,BovnarArray1
syn region BovnarStruct36 matchgroup=BovnarStructDelim4 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct37,BovnarArray1
syn region BovnarStruct37 matchgroup=BovnarStructDelim5 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct38,BovnarArray1
syn region BovnarStruct38 matchgroup=BovnarStructDelim6 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct39,BovnarArray1
syn region BovnarStruct39 matchgroup=BovnarStructDelim7 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct40,BovnarArray1
syn region BovnarStruct40 matchgroup=BovnarStructDelim8 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct41,BovnarArray1
syn region BovnarStruct41 matchgroup=BovnarStructDelim1 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct42,BovnarArray1
syn region BovnarStruct42 matchgroup=BovnarStructDelim2 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct43,BovnarArray1
syn region BovnarStruct43 matchgroup=BovnarStructDelim3 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct44,BovnarArray1
syn region BovnarStruct44 matchgroup=BovnarStructDelim4 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct45,BovnarArray1
syn region BovnarStruct45 matchgroup=BovnarStructDelim5 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct46,BovnarArray1
syn region BovnarStruct46 matchgroup=BovnarStructDelim6 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct47,BovnarArray1
syn region BovnarStruct47 matchgroup=BovnarStructDelim7 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct48,BovnarArray1
syn region BovnarStruct48 matchgroup=BovnarStructDelim8 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct49,BovnarArray1
syn region BovnarStruct49 matchgroup=BovnarStructDelim1 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct50,BovnarArray1
syn region BovnarStruct50 matchgroup=BovnarStructDelim2 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct51,BovnarArray1
syn region BovnarStruct51 matchgroup=BovnarStructDelim3 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct52,BovnarArray1
syn region BovnarStruct52 matchgroup=BovnarStructDelim4 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct53,BovnarArray1
syn region BovnarStruct53 matchgroup=BovnarStructDelim5 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct54,BovnarArray1
syn region BovnarStruct54 matchgroup=BovnarStructDelim6 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct55,BovnarArray1
syn region BovnarStruct55 matchgroup=BovnarStructDelim7 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct56,BovnarArray1
syn region BovnarStruct56 matchgroup=BovnarStructDelim8 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct57,BovnarArray1
syn region BovnarStruct57 matchgroup=BovnarStructDelim1 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct58,BovnarArray1
syn region BovnarStruct58 matchgroup=BovnarStructDelim2 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct59,BovnarArray1
syn region BovnarStruct59 matchgroup=BovnarStructDelim3 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct60,BovnarArray1
syn region BovnarStruct60 matchgroup=BovnarStructDelim4 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct61,BovnarArray1
syn region BovnarStruct61 matchgroup=BovnarStructDelim5 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct62,BovnarArray1
syn region BovnarStruct62 matchgroup=BovnarStructDelim6 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct63,BovnarArray1
syn region BovnarStruct63 matchgroup=BovnarStructDelim7 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarStruct64,BovnarArray1
syn region BovnarStruct64 matchgroup=BovnarStructDelim8 start=/{/ end=/}/ contained contains=@BovnarContent,BovnarArray1

" Parse from the start of the file. Bovnar arrays and structs are regions that
" routinely span blank lines (e.g. multi-row matrices, blank-separated struct
" members). Vim's default sync resynchronises at blank lines and loses the
" enclosing [ … ] / { … } nesting there, leaving the matching ] or } after a
" blank line uncoloured. 'fromstart' guarantees correct nesting at any depth.
syntax sync fromstart
