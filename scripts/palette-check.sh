#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

THEME="src/ui/theme.css"
PALETTE="scripts/palette.txt"
BRAND="docs/wiki/brand.md"
END='([^0-9A-Za-z_-]|$)'
HEX="#[0-9A-Fa-f]{3,8}${END}"
FUNCS='(^|[^A-Za-z-])(rgba?|hsla?|hwb|lab|lch|oklab|oklch|color|color-mix)\('
NAMED='aliceblue|antiquewhite|aqua|aquamarine|azure|beige|bisque|black|blanchedalmond|blue|blueviolet|brown|burlywood|cadetblue|chartreuse|chocolate|coral|cornflowerblue|cornsilk|crimson|cyan|darkblue|darkcyan|darkgoldenrod|darkgray|darkgreen|darkgrey|darkkhaki|darkmagenta|darkolivegreen|darkorange|darkorchid|darkred|darksalmon|darkseagreen|darkslateblue|darkslategray|darkslategrey|darkturquoise|darkviolet|deeppink|deepskyblue|dimgray|dimgrey|dodgerblue|firebrick|floralwhite|forestgreen|fuchsia|gainsboro|ghostwhite|gold|goldenrod|gray|green|greenyellow|grey|honeydew|hotpink|indianred|indigo|ivory|khaki|lavender|lavenderblush|lawngreen|lemonchiffon|lightblue|lightcoral|lightcyan|lightgoldenrodyellow|lightgray|lightgreen|lightgrey|lightpink|lightsalmon|lightseagreen|lightskyblue|lightslategray|lightslategrey|lightsteelblue|lightyellow|lime|limegreen|linen|magenta|maroon|mediumaquamarine|mediumblue|mediumorchid|mediumpurple|mediumseagreen|mediumslateblue|mediumspringgreen|mediumturquoise|mediumvioletred|midnightblue|mintcream|mistyrose|moccasin|navajowhite|navy|oldlace|olive|olivedrab|orange|orangered|orchid|palegoldenrod|palegreen|paleturquoise|palevioletred|papayawhip|peachpuff|peru|pink|plum|powderblue|purple|rebeccapurple|red|rosybrown|royalblue|saddlebrown|salmon|sandybrown|seagreen|seashell|sienna|silver|skyblue|slateblue|slategray|slategrey|snow|springgreen|steelblue|tan|teal|thistle|tomato|turquoise|violet|wheat|white|whitesmoke|yellow|yellowgreen'
CSS_NAMED="^[[:space:]]*[a-z-]+[[:space:]]*:[^{]*[^A-Za-z-](${NAMED})${END}"
SVG_NAMED="(fill|stroke|stop-color|flood-color|lighting-color|color)=\"(${NAMED})\""
TS_NAMED="(style\\.[A-Za-z]+[[:space:]]*=|setProperty\\()[^;]*[\"'\`](${NAMED})[\"'\`]"
status=0

fail() {
  echo "error: $*" >&2
  status=1
}

tracked() {
  git ls-files -- "$@"
}

report() {
  local label="$1" line
  while IFS= read -r line; do
    fail "${label}: ${line}"
  done
}

normalize() {
  local hex
  hex="$(tr '[:upper:]' '[:lower:]' <<<"${1#\#}")"
  if [ "${#hex}" -le 4 ]; then
    hex="$(sed -E 's/(.)/\1\1/g' <<<"$hex")"
  fi
  printf '#%s\n' "${hex:0:6}"
}

in_palette() {
  grep -qixF "$(normalize "$1")" "$PALETTE"
}

scan_sources() {
  local pattern="$1" label="$2" ext file files=()
  shift 2
  for ext in "$@"; do
    while IFS= read -r file; do
      files+=("$file")
    done < <(tracked "src/*.${ext}" | grep -vxF "$THEME" || true)
  done
  [ "${#files[@]}" -eq 0 ] && return 0
  report "$label" < <(grep -nHiE "$pattern" "${files[@]}" || true)
}

check_sources() {
  scan_sources "$HEX|$FUNCS" "raw color" css html ts
  scan_sources "$CSS_NAMED" "named color" css
  scan_sources "$SVG_NAMED" "named color" html
  scan_sources "$TS_NAMED" "color set in code" ts
}

check_theme() {
  local hex
  report "theme.css must use palette hex values" < <(grep -nHiE "$FUNCS|$CSS_NAMED" "$THEME" || true)
  while IFS= read -r hex; do
    in_palette "$hex" || fail "$THEME uses $hex, which is not in $PALETTE"
  done < <(grep -oE '#[0-9A-Fa-f]{3,8}' "$THEME")
}

svg_colors() {
  grep -oiE '(fill|stroke|stop-color|flood-color|lighting-color)[=:][[:space:]]*"?[^";)> ]+' "$1" |
    sed -E 's/^[^=:]+[=:][[:space:]]*"?//'
}

check_svg_value() {
  local file="$1" value="$2"
  case "$(tr '[:upper:]' '[:lower:]' <<<"$value")" in
    none | currentcolor | transparent | inherit | url\(*) return 0 ;;
    \#*) in_palette "$value" && return 0 ;;
  esac
  fail "$file uses $value, which is not in $PALETTE"
}

check_svgs() {
  local file value
  while IFS= read -r file; do
    while IFS= read -r value; do
      check_svg_value "$file" "$value"
    done < <(svg_colors "$file")
  done < <(tracked 'assets/brand/*.svg' 'src/ui/assets/*.svg')
}

check_named_in_brand() {
  local hex
  while IFS= read -r hex; do
    grep -qiF "$hex" "$BRAND" || fail "$PALETTE lists $hex, but $BRAND does not name it"
  done <"$PALETTE"
}

check_sources
check_theme
check_svgs
check_named_in_brand
exit "$status"
