#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF' >&2
用法:
  bash scripts/analyze-objects.sh <sample> [pattern]

说明:
  基于 Keil C51 和 BL51 的 Listings 产物,
  自动分析适合 peek_* / poke_* 使用的固定地址符号.

参数:
  sample   sample 名称, 例如 ad_da
  pattern  可选关键字. 提供后会额外输出 m51 和 lst 里的命中行.
EOF
    exit 1
}

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    usage
fi

sample="$1"
pattern="${2:-}"
sample_dir="sample/$sample"
prj_dir="$sample_dir/prj"
objects_dir="$prj_dir/Objects"
listings_dir="$prj_dir/Listings"

if [ ! -d "$sample_dir" ]; then
    echo "sample 不存在: $sample_dir" >&2
    exit 1
fi

if [ ! -d "$listings_dir" ]; then
    echo "Listings 目录不存在: $listings_dir" >&2
    echo "请先构建 sample, 例如 just build-sample $sample" >&2
    exit 1
fi

m51_files=()
while IFS= read -r path; do
    m51_files+=("$path")
done < <(find "$listings_dir" -maxdepth 1 -type f -name '*.m51' | LC_ALL=C sort)

lst_files=()
while IFS= read -r path; do
    lst_files+=("$path")
done < <(find "$listings_dir" -maxdepth 1 -type f -name '*.lst' | LC_ALL=C sort)

if [ "${#m51_files[@]}" -eq 0 ]; then
    echo "未找到可分析的 m51 文件: $listings_dir" >&2
    echo "请先确认 Keil 构建已经生成 Listings/*.m51." >&2
    exit 1
fi

tmp_base="${TMPDIR:-/tmp}"
tmp_base="${tmp_base%/}"
if [ -z "$tmp_base" ] || [ ! -d "$tmp_base" ]; then
    tmp_base="/tmp"
fi

tmp_symbols="$(mktemp "$tmp_base/analyze-objects-symbols.XXXXXX")"
tmp_filtered="$(mktemp "$tmp_base/analyze-objects-filtered.XXXXXX")"
trap 'rm -f "$tmp_symbols" "$tmp_filtered"' EXIT

extract_public_symbols_m51() {
    local map_file="$1"
    local default_module
    default_module="$(basename "$map_file")"
    default_module="${default_module%.m51}"
    LC_ALL=C awk -v default_module="$default_module" '
        function trim_hex(raw, value) {
            value = toupper(raw)
            sub(/[Hh].*$/, "", value)
            sub(/\..*$/, "", value)
            sub(/^0+/, "", value)
            if (value == "") {
                value = "0"
            }
            return "0x" value
        }

        function normalize_hex(raw, value) {
            value = toupper(raw)
            sub(/[Hh].*$/, "", value)
            sub(/\..*$/, "", value)
            return value
        }

        function normalize_bit(raw, value) {
            value = toupper(raw)
            sub(/[Hh]/, "", value)
            return value
        }

        function hex_to_dec(hex, i, digit, value, ch) {
            value = 0
            hex = toupper(hex)
            for (i = 1; i <= length(hex); i++) {
                ch = substr(hex, i, 1)
                digit = index("0123456789ABCDEF", ch) - 1
                if (digit < 0) {
                    return -1
                }
                value = value * 16 + digit
            }
            return value
        }

        function is_user_symbol(name) {
            return name ~ /[a-z_]/
        }

        function classify_xdata(addr_dec, i) {
            for (i = 1; i <= x_range_count; i++) {
                if (addr_dec >= x_range_start[i] && addr_dec < x_range_end[i]) {
                    return x_range_kind[i]
                }
            }
            return "XSEG"
        }

        /^LINK MAP OF MODULE:/ {
            in_link_map = 1
            next
        }

        /^OVERLAY MAP OF MODULE:/ {
            in_link_map = 0
            next
        }

        /^SYMBOL TABLE OF MODULE:/ {
            in_link_map = 0
            in_symbol_table = 1
            current_module = default_module
            next
        }

        in_link_map {
            line = $0
            sub(/^[[:space:]]+/, "", line)
            if (line ~ /^XDATA[[:space:]]+/) {
                split(line, parts, /[[:space:]]+/)
                start_hex = normalize_hex(parts[2])
                len_hex = normalize_hex(parts[3])
                start_dec = hex_to_dec(start_hex)
                len_dec = hex_to_dec(len_hex)
                if (start_dec >= 0 && len_dec >= 0) {
                    x_range_count++
                    x_range_start[x_range_count] = start_dec
                    x_range_end[x_range_count] = start_dec + len_dec
                    if (parts[4] == "INPAGE") {
                        x_range_kind[x_range_count] = "PSEG"
                    } else {
                        x_range_kind[x_range_count] = "XSEG"
                    }
                }
            }
            next
        }

        in_symbol_table && /^[[:space:]]*-------[[:space:]]+MODULE[[:space:]]+/ {
            line = $0
            sub(/^[[:space:]-]+MODULE[[:space:]]+/, "", line)
            sub(/[[:space:]]+$/, "", line)
            if (line != "") {
                current_module = line
            }
            next
        }

        in_symbol_table && /^[[:space:]]*-------[[:space:]]+ENDMOD/ {
            current_module = default_module
            next
        }

        in_symbol_table {
            line = $0
            sub(/^[[:space:]]+/, "", line)
            if (line !~ /^[DIXBC]:/) {
                next
            }

            split(line, parts, /[[:space:]]+/)
            if (parts[2] != "PUBLIC") {
                next
            }

            raw_token = parts[1]
            prefix = substr(raw_token, 1, 1)
            name = parts[3]
            if (!is_user_symbol(name)) {
                next
            }

            if (prefix == "D") {
                raw_hex = normalize_hex(substr(raw_token, 3))
                addr_dec = hex_to_dec(raw_hex)
                if (addr_dec < 0) {
                    next
                }
                if (addr_dec < 128) {
                    printf "DSEG\t%s\t%s\t?\t%s\t%s\t%s\t-\n", raw_hex, trim_hex(raw_hex), name, current_module, raw_hex, ""
                } else {
                    printf "SFR\t%s\t%s\t?\t%s\t%s\t%s\t-\n", raw_hex, trim_hex(raw_hex), name, current_module, raw_hex, ""
                }
                next
            }

            if (prefix == "I") {
                raw_hex = normalize_hex(substr(raw_token, 3))
                if (hex_to_dec(raw_hex) < 0) {
                    next
                }
                printf "ISEG\t%s\t%s\t?\t%s\t%s\t%s\t-\n", raw_hex, trim_hex(raw_hex), name, current_module, raw_hex, ""
                next
            }

            if (prefix == "X") {
                raw_hex = normalize_hex(substr(raw_token, 3))
                addr_dec = hex_to_dec(raw_hex)
                if (addr_dec < 0) {
                    next
                }
                kind = classify_xdata(addr_dec)
                printf "%s\t%s\t%s\t?\t%s\t%s\t%s\t-\n", kind, raw_hex, trim_hex(raw_hex), name, current_module, raw_hex, ""
                next
            }

            if (prefix == "B") {
                bit_raw = normalize_bit(substr(raw_token, 3))
                split(bit_raw, bit_parts, /\./)
                base_hex = normalize_hex(bit_parts[1])
                bit_index = bit_parts[2]
                addr_dec = hex_to_dec(base_hex)
                if (addr_dec < 0 || bit_index == "") {
                    next
                }
                if (addr_dec < 128) {
                    printf "BIT\t%s.%s\t%s.%s\t1bit\t%s\t%s\t%s\t%s\n", base_hex, bit_index, trim_hex(base_hex), bit_index, name, current_module, base_hex, bit_index
                } else {
                    printf "SBIT\t%s.%s\t%s.%s\t1bit\t%s\t%s\t%s\t%s\n", base_hex, bit_index, trim_hex(base_hex), bit_index, name, current_module, base_hex, bit_index
                }
                next
            }
        }
    ' "$map_file"
}

print_memory_overview() {
    local map_file="$1"
    LC_ALL=C awk '
        /^LINK MAP OF MODULE:/ {
            in_link_map = 1
            next
        }

        /^OVERLAY MAP OF MODULE:/ {
            in_link_map = 0
            next
        }

        in_link_map {
            line = $0
            sub(/^[[:space:]]+/, "", line)
            if (line ~ /^(REG|DATA|IDATA|BIT|XDATA)[[:space:]]+/) {
                print line
            }
        }
    ' "$map_file"
}

for map_file in "${m51_files[@]}"; do
    extract_public_symbols_m51 "$map_file" >> "$tmp_symbols"
done

if [ -n "$pattern" ]; then
    if ! LC_ALL=C rg --ignore-case --fixed-strings --no-line-number --no-filename -- "$pattern" "$tmp_symbols" > "$tmp_filtered"; then
        : > "$tmp_filtered"
    fi
else
    cp "$tmp_symbols" "$tmp_filtered"
fi

print_section() {
    local title="$1"
    printf '\n[%s]\n' "$title"
}

search_declaration_in_file() {
    local file="$1"
    local name="$2"
    local prefer_decl="$3"
    LC_ALL=C awk -v needle="$name" -v prefer_decl="$prefer_decl" '
        function looks_like_decl(line, pos, prefix, lower_prefix) {
            pos = index(line, needle)
            if (pos == 0) {
                return 0
            }
            prefix = substr(line, 1, pos - 1)
            lower_prefix = tolower(prefix)
            if (line !~ /;/) {
                return 0
            }
            return lower_prefix ~ /(idata|pdata|xdata|bdata|data|code|bit|sbit|volatile|static|const|unsigned|signed|char|int|long|float|double|u8|u16|u32|u64)/
        }

        index($0, needle) == 0 {
            next
        }

        prefer_decl == "1" && !looks_like_decl($0) {
            next
        }

        {
            print FNR "\t" $0
            exit
        }
    ' "$file"
}

infer_decl_size() {
    local decl_line="$1"
    LC_ALL=C awk -v line="$decl_line" '
        function trim(value) {
            sub(/^[[:space:]]+/, "", value)
            sub(/[[:space:]]+$/, "", value)
            return value
        }

        function type_size(lower_line, size) {
            if (lower_line ~ /(^|[^[:alnum:]_])sbit([^[:alnum:]_]|$)/) {
                return "1bit"
            }
            if (lower_line ~ /(^|[^[:alnum:]_])bit([^[:alnum:]_]|$)/) {
                return "1bit"
            }
            if (lower_line ~ /(^|[^[:alnum:]_])double([^[:alnum:]_]|$)/) {
                return 4
            }
            if (lower_line ~ /(^|[^[:alnum:]_])float([^[:alnum:]_]|$)/) {
                return 4
            }
            if (lower_line ~ /(^|[^[:alnum:]_])u32([^[:alnum:]_]|$)/) {
                return 4
            }
            if (lower_line ~ /(^|[^[:alnum:]_])long([^[:alnum:]_]|$)/) {
                return 4
            }
            if (lower_line ~ /(^|[^[:alnum:]_])u16([^[:alnum:]_]|$)/) {
                return 2
            }
            if (lower_line ~ /(^|[^[:alnum:]_])int([^[:alnum:]_]|$)/) {
                return 2
            }
            if (lower_line ~ /(^|[^[:alnum:]_])u8([^[:alnum:]_]|$)/) {
                return 1
            }
            if (lower_line ~ /(^|[^[:alnum:]_])char([^[:alnum:]_]|$)/) {
                return 1
            }
            return "?"
        }

        BEGIN {
            lower_line = tolower(line)
            base = type_size(lower_line)
            if (match(lower_line, /\[[[:space:]]*[0-9]+[[:space:]]*\]/)) {
                count_text = substr(lower_line, RSTART + 1, RLENGTH - 2)
                count_text = trim(count_text)
                if (base == "1bit") {
                    print count_text "bit"
                    exit
                }
                if (base != "?") {
                    print base * count_text
                    exit
                }
            }
            print base
        }
    '
}

find_declaration_info() {
    local name="$1"
    local module="$2"
    local module_lc
    local file
    local result
    local decl_size
    local decl_file
    local -a candidate_files

    candidate_files=()
    module_lc="$(printf '%s' "$module" | tr '[:upper:]' '[:lower:]')"
    if [ -n "$module_lc" ] && [ "$module_lc" != "-" ]; then
        file="$listings_dir/$module_lc.lst"
        if [ -f "$file" ]; then
            candidate_files+=("$file")
        fi
    fi

    for file in "${lst_files[@]}"; do
        if [ "${#candidate_files[@]}" -gt 0 ] && [ "$file" = "${candidate_files[0]}" ]; then
            continue
        fi
        candidate_files+=("$file")
    done

    for file in "${candidate_files[@]}"; do
        result="$(search_declaration_in_file "$file" "$name" 1)"
        if [ -n "$result" ]; then
            IFS=$'\t' read -r decl_line_no decl_line_text <<< "$result"
            decl_size="$(infer_decl_size "$decl_line_text")"
            decl_file="$(basename "$file")"
            printf '%s\t%s\t%s\t%s\n' "$decl_file" "$decl_line_no" "$decl_line_text" "$decl_size"
            return 0
        fi
    done

    for file in "${candidate_files[@]}"; do
        result="$(search_declaration_in_file "$file" "$name" 0)"
        if [ -n "$result" ]; then
            IFS=$'\t' read -r decl_line_no decl_line_text <<< "$result"
            decl_size="$(infer_decl_size "$decl_line_text")"
            decl_file="$(basename "$file")"
            printf '%s\t%s\t%s\t%s\n' "$decl_file" "$decl_line_no" "$decl_line_text" "$decl_size"
            return 0
        fi
    done
}

bit_mask_hex() {
    local bit_index="$1"
    case "$bit_index" in
        0) printf '0x01' ;;
        1) printf '0x02' ;;
        2) printf '0x04' ;;
        3) printf '0x08' ;;
        4) printf '0x10' ;;
        5) printf '0x20' ;;
        6) printf '0x40' ;;
        7) printf '0x80' ;;
        *) printf '?' ;;
    esac
}

format_hex_addr() {
    local raw="$1"
    local value="$raw"
    value="${value#x}"
    value="${value#X}"
    while [ -n "$value" ] && [ "${value#0}" != "$value" ]; do
        value="${value#0}"
    done
    if [ -z "$value" ]; then
        value="0"
    fi
    printf '0x%s' "$value"
}

access_hint() {
    local kind="$1"
    local addr="$2"
    local base_hex="$3"
    local bit_index="$4"
    local base_addr
    case "$kind" in
        DSEG)
            printf 'peek_data(%s) / poke_data(%s, value)' "$addr" "$addr"
            ;;
        ISEG)
            printf 'peek_idata(%s) / poke_idata(%s, value)' "$addr" "$addr"
            ;;
        PSEG)
            printf 'peek_pdata(%s) / poke_pdata(%s, value)' "$addr" "$addr"
            ;;
        XSEG)
            printf 'peek_xdata(%s) / poke_xdata(%s, value)' "$addr" "$addr"
            ;;
        SFR)
            printf 'peek_sfr(%s) / poke_sfr(%s, value)' "$addr" "$addr"
            ;;
        BIT)
            base_addr="$(format_hex_addr "$base_hex")"
            printf 'peek_data(%s) & %s' "$base_addr" "$(bit_mask_hex "$bit_index")"
            ;;
        SBIT)
            base_addr="$(format_hex_addr "$base_hex")"
            printf 'peek_sfr(%s) & %s' "$base_addr" "$(bit_mask_hex "$bit_index")"
            ;;
        *)
            printf '-'
            ;;
    esac
}

print_rows() {
    local kind="$1"
    local title="$2"
    local rows
    local decl_info
    local decl_file
    local decl_line_no
    local decl_line_text
    local decl_size
    local resolved_size
    rows="$(LC_ALL=C awk -F '\t' -v kind="$kind" '$1 == kind { print }' "$tmp_filtered" | LC_ALL=C sort -t $'\t' -k2,2 -k5,5)"
    if [ -z "$rows" ]; then
        return
    fi

    printf '\n%s:\n' "$title"
    while IFS=$'\t' read -r row_kind sort_key display_addr size name module base_hex bit_index; do
        decl_info="$(find_declaration_info "$name" "$module" || true)"
        resolved_size="$size"
        if [ -n "$decl_info" ]; then
            IFS=$'\t' read -r decl_file decl_line_no decl_line_text decl_size <<< "$decl_info"
            if [ "$resolved_size" = "?" ] && [ -n "$decl_size" ] && [ "$decl_size" != "?" ]; then
                resolved_size="$decl_size"
            fi
        fi
        printf '  %s size=%s %-24s module=%s -> %s\n' \
            "$display_addr" \
            "$resolved_size" \
            "$name" \
            "$module" \
            "$(access_hint "$row_kind" "$display_addr" "$base_hex" "$bit_index")"
        if [ -n "$decl_info" ]; then
            printf '    lst=%s:%s %s\n' "$decl_file" "$decl_line_no" "$decl_line_text"
        fi
    done <<< "$rows"
}

printf '==> Keil 编译产物自动分析\n'
printf 'sample: %s\n' "$sample"
printf 'prj: %s\n' "$prj_dir"
printf 'listings: %s\n' "$listings_dir"
if [ -d "$objects_dir" ]; then
    printf 'objects: %s\n' "$objects_dir"
fi

print_section '内存总览'
printf 'm51: %s\n' "${m51_files[0]}"
print_memory_overview "${m51_files[0]}"

print_section '固定地址符号'
printf '来源: Listings/*.m51 的 PUBLIC 符号和 LINK MAP.\n'
printf '默认只列出更像用户自定义的符号, 避免把整套 SFR 全部刷出来.\n'

if [ ! -s "$tmp_filtered" ]; then
    if [ -n "$pattern" ]; then
        printf '未找到匹配关键字 "%s" 的固定地址符号.\n' "$pattern"
    else
        printf '未找到可直接用于 peek_* / poke_* 的固定地址符号.\n'
    fi
else
    print_rows 'DSEG' 'DSEG'
    print_rows 'ISEG' 'ISEG'
    print_rows 'PSEG' 'PSEG'
    print_rows 'XSEG' 'XSEG'
    print_rows 'SFR' 'SFR'
    print_rows 'BIT' 'BIT'
    print_rows 'SBIT' 'SBIT'
fi

print_section '使用提示'
printf -- '- m51 的 PUBLIC 更适合找全局变量, sbit, SFR, pdata, xdata.\n'
printf -- '- m51 的 PROC 里的 SYMBOL 多数是 overlay 或临时分配, 不适合写死到长期 judge.\n'
printf -- '- lst 更适合对照源码声明, m51 更适合确认最终地址.\n'
printf -- '- 当前项目定位地址时, 统一以 Listings/*.m51 和 Listings/*.lst 为准.\n'
printf -- '- 如需按名字缩小范围, 可执行: just analyze-objects %s 关键字\n' "$sample"

if [ -n "$pattern" ]; then
    files_to_search=()
    for file in "${m51_files[@]}" "${lst_files[@]}"; do
        if [ -n "$file" ]; then
            files_to_search+=("$file")
        fi
    done

    print_section '关键字命中'
    printf 'pattern: %s\n' "$pattern"
    if [ "${#files_to_search[@]}" -eq 0 ]; then
        printf '没有可搜索的 m51 或 lst 文件.\n'
    else
        if ! LC_ALL=C rg -n --ignore-case --fixed-strings -- "$pattern" "${files_to_search[@]}"; then
            printf '没有找到更多命中.\n'
        fi
    fi
fi
