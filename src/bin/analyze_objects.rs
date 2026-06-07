use std::{
    fs,
    path::{Path, PathBuf},
    process,
};

use anyhow::{Context, Result, bail};
use clap::Parser;
use regex::Regex;

#[derive(Debug, Parser)]
#[command(
    name = "analyze-objects",
    version,
    about = "分析 Keil Listings 中适合 peek_* / poke_* 使用的固定地址符号"
)]
struct Cli {
    sample: String,
    pattern: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SymbolKind {
    Dseg,
    Iseg,
    Pseg,
    Xseg,
    Sfr,
    Bit,
    Sbit,
}

impl SymbolKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Dseg => "DSEG",
            Self::Iseg => "ISEG",
            Self::Pseg => "PSEG",
            Self::Xseg => "XSEG",
            Self::Sfr => "SFR",
            Self::Bit => "BIT",
            Self::Sbit => "SBIT",
        }
    }
}

#[derive(Debug, Clone)]
struct XRange {
    start: i32,
    end: i32,
    kind: SymbolKind,
}

#[derive(Debug, Clone)]
struct SymbolRecord {
    kind: SymbolKind,
    sort_key: String,
    display_addr: String,
    size: String,
    name: String,
    module: String,
    base_hex: String,
    bit_index: String,
    raw_line: String,
}

#[derive(Debug, Clone)]
struct DeclarationInfo {
    file: String,
    line_number: usize,
    text: String,
    size: String,
}

#[derive(Debug, Clone)]
struct SearchHit {
    path: String,
    line_number: usize,
    text: String,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err:#}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let pattern = cli.pattern.unwrap_or_default();

    let sample_dir = PathBuf::from("sample").join(&cli.sample);
    let prj_dir = sample_dir.join("prj");
    let objects_dir = prj_dir.join("Objects");
    let listings_dir = prj_dir.join("Listings");

    if !sample_dir.is_dir() {
        bail!("sample 不存在: {}", display_path(&sample_dir));
    }

    if !listings_dir.is_dir() {
        bail!(
            "Listings 目录不存在: {}\n请先构建 sample, 例如 just build-sample {}",
            display_path(&listings_dir),
            cli.sample
        );
    }

    let mut m51_files = collect_files_with_ext(&listings_dir, "m51")
        .with_context(|| format!("读取目录失败: {}", display_path(&listings_dir)))?;
    let mut lst_files = collect_files_with_ext(&listings_dir, "lst")
        .with_context(|| format!("读取目录失败: {}", display_path(&listings_dir)))?;

    sort_paths(&mut m51_files);
    sort_paths(&mut lst_files);

    if m51_files.is_empty() {
        bail!(
            "未找到可分析的 m51 文件: {}\n请先确认 Keil 构建已经生成 Listings/*.m51.",
            display_path(&listings_dir)
        );
    }

    let mut all_symbols = Vec::new();
    for map_file in &m51_files {
        all_symbols.extend(extract_public_symbols_m51(map_file)?);
    }

    let filtered_symbols = filter_symbols(&all_symbols, &pattern);

    println!("==> Keil 编译产物自动分析");
    println!("sample: {}", cli.sample);
    println!("prj: {}", display_path(&prj_dir));
    println!("listings: {}", display_path(&listings_dir));
    if objects_dir.is_dir() {
        println!("objects: {}", display_path(&objects_dir));
    }

    print_section("内存总览");
    println!("m51: {}", display_path(&m51_files[0]));
    for line in print_memory_overview(&m51_files[0])? {
        println!("{line}");
    }

    print_section("固定地址符号");
    println!("来源: Listings/*.m51 的 PUBLIC 符号和 LINK MAP.");
    println!("默认只列出更像用户自定义的符号, 避免把整套 SFR 全部刷出来.");

    if filtered_symbols.is_empty() {
        if pattern.is_empty() {
            println!("未找到可直接用于 peek_* / poke_* 的固定地址符号.");
        } else {
            println!("未找到匹配关键字 \"{pattern}\" 的固定地址符号.");
        }
    } else {
        for kind in [
            SymbolKind::Dseg,
            SymbolKind::Iseg,
            SymbolKind::Pseg,
            SymbolKind::Xseg,
            SymbolKind::Sfr,
            SymbolKind::Bit,
            SymbolKind::Sbit,
        ] {
            print_rows(kind, &filtered_symbols, &listings_dir, &lst_files)?;
        }
    }

    print_section("使用提示");
    println!("- m51 的 PUBLIC 更适合找全局变量, sbit, SFR, pdata, xdata.");
    println!("- m51 的 PROC 里的 SYMBOL 多数是 overlay 或临时分配, 不适合写死到长期 judge.");
    println!("- lst 更适合对照源码声明, m51 更适合确认最终地址.");
    println!("- 当前项目定位地址时, 统一以 Listings/*.m51 和 Listings/*.lst 为准.");
    println!(
        "- 如需按名字缩小范围, 可执行: just analyze-objects {} 关键字",
        cli.sample
    );

    if !pattern.is_empty() {
        print_section("关键字命中");
        println!("pattern: {pattern}");

        let hits = collect_search_hits(&m51_files, &lst_files, &pattern)?;
        if hits.is_empty() {
            println!("没有找到更多命中.");
        } else {
            for hit in hits {
                println!("{}:{}:{}", hit.path, hit.line_number, hit.text);
            }
        }
    }

    Ok(())
}

fn collect_files_with_ext(dir: &Path, ext: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case(ext))
        {
            files.push(path);
        }
    }
    Ok(files)
}

fn read_text_lossy(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("读取文件失败: {}", display_path(path)))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn sort_paths(paths: &mut [PathBuf]) {
    paths.sort_by_key(|path| display_path(path));
}

fn display_path(path: &Path) -> String {
    let relative = std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(&cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf());
    relative.to_string_lossy().replace('\\', "/")
}

fn normalize_hex_token(raw: &str) -> String {
    let mut value = raw.to_ascii_uppercase();
    if let Some(index) = value.find(['H', 'h']) {
        value.truncate(index);
    }
    if let Some(index) = value.find('.') {
        value.truncate(index);
    }
    value
}

fn normalize_bit_token(raw: &str) -> String {
    raw.to_ascii_uppercase().replace(['H', 'h'], "")
}

fn format_hex_addr(raw: &str) -> String {
    let trimmed = normalize_hex_token(raw).trim_start_matches('0').to_string();
    if trimmed.is_empty() {
        "0x0".to_string()
    } else {
        format!("0x{trimmed}")
    }
}

fn hex_to_i32(raw: &str) -> Option<i32> {
    let normalized = normalize_hex_token(raw);
    if normalized.is_empty() {
        return None;
    }
    i32::from_str_radix(&normalized, 16).ok()
}

fn is_user_symbol(name: &str) -> bool {
    name.chars().any(|ch| ch.is_ascii_lowercase() || ch == '_')
}

fn classify_xdata(addr: i32, ranges: &[XRange]) -> SymbolKind {
    for range in ranges {
        if addr >= range.start && addr < range.end {
            return range.kind;
        }
    }
    SymbolKind::Xseg
}

fn extract_public_symbols_m51(map_file: &Path) -> Result<Vec<SymbolRecord>> {
    let default_module = map_file
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown")
        .to_string();
    let content = read_text_lossy(map_file)?;

    let mut current_module = default_module.clone();
    let mut in_link_map = false;
    let mut in_symbol_table = false;
    let mut x_ranges = Vec::new();
    let mut symbols = Vec::new();

    for line in content.lines() {
        if line.starts_with("LINK MAP OF MODULE:") {
            in_link_map = true;
            in_symbol_table = false;
            continue;
        }

        if line.starts_with("OVERLAY MAP OF MODULE:") {
            in_link_map = false;
            continue;
        }

        if line.starts_with("SYMBOL TABLE OF MODULE:") {
            in_link_map = false;
            in_symbol_table = true;
            current_module = default_module.clone();
            continue;
        }

        if in_link_map {
            let trimmed = line.trim_start();
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.first().copied() == Some("XDATA") && parts.len() >= 4 {
                let start = hex_to_i32(parts[1]);
                let len = hex_to_i32(parts[2]);
                if let (Some(start), Some(len)) = (start, len) {
                    let kind = if parts[3] == "INPAGE" {
                        SymbolKind::Pseg
                    } else {
                        SymbolKind::Xseg
                    };
                    x_ranges.push(XRange {
                        start,
                        end: start + len,
                        kind,
                    });
                }
            }
            continue;
        }

        if !in_symbol_table {
            continue;
        }

        let trimmed = line.trim_start();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 3 && parts[0] == "-------" && parts[1] == "MODULE" {
            current_module = parts[2].to_string();
            continue;
        }
        if parts.len() >= 2 && parts[0] == "-------" && parts[1] == "ENDMOD" {
            current_module = default_module.clone();
            continue;
        }

        let Some(raw_token) = parts.first().copied() else {
            continue;
        };
        if !matches!(raw_token.chars().next(), Some('D' | 'I' | 'X' | 'B' | 'C'))
            || !raw_token.contains(':')
        {
            continue;
        }
        if parts.len() < 3 || parts[1] != "PUBLIC" {
            continue;
        }

        let name = parts[2];
        if !is_user_symbol(name) {
            continue;
        }

        let prefix = raw_token.chars().next().unwrap_or_default();
        match prefix {
            'D' => {
                let raw_hex = normalize_hex_token(&raw_token[2..]);
                let Some(addr) = hex_to_i32(&raw_hex) else {
                    continue;
                };
                let kind = if addr < 128 {
                    SymbolKind::Dseg
                } else {
                    SymbolKind::Sfr
                };
                symbols.push(new_symbol_record(
                    kind,
                    raw_hex.clone(),
                    format_hex_addr(&raw_hex),
                    "?".to_string(),
                    name.to_string(),
                    current_module.clone(),
                    (raw_hex, String::new()),
                ));
            }
            'I' => {
                let raw_hex = normalize_hex_token(&raw_token[2..]);
                if hex_to_i32(&raw_hex).is_none() {
                    continue;
                }
                symbols.push(new_symbol_record(
                    SymbolKind::Iseg,
                    raw_hex.clone(),
                    format_hex_addr(&raw_hex),
                    "?".to_string(),
                    name.to_string(),
                    current_module.clone(),
                    (raw_hex, String::new()),
                ));
            }
            'X' => {
                let raw_hex = normalize_hex_token(&raw_token[2..]);
                let Some(addr) = hex_to_i32(&raw_hex) else {
                    continue;
                };
                let kind = classify_xdata(addr, &x_ranges);
                symbols.push(new_symbol_record(
                    kind,
                    raw_hex.clone(),
                    format_hex_addr(&raw_hex),
                    "?".to_string(),
                    name.to_string(),
                    current_module.clone(),
                    (raw_hex, String::new()),
                ));
            }
            'B' => {
                let bit_raw = normalize_bit_token(&raw_token[2..]);
                let Some((base_hex_raw, bit_index)) = bit_raw.split_once('.') else {
                    continue;
                };
                let base_hex = normalize_hex_token(base_hex_raw);
                let Some(addr) = hex_to_i32(&base_hex) else {
                    continue;
                };
                let kind = if addr < 128 {
                    SymbolKind::Bit
                } else {
                    SymbolKind::Sbit
                };
                let sort_key = format!("{base_hex}.{bit_index}");
                let display_addr = format!("{}.{bit_index}", format_hex_addr(&base_hex));
                symbols.push(new_symbol_record(
                    kind,
                    sort_key,
                    display_addr,
                    "1bit".to_string(),
                    name.to_string(),
                    current_module.clone(),
                    (base_hex, bit_index.to_string()),
                ));
            }
            _ => {}
        }
    }

    Ok(symbols)
}

fn new_symbol_record(
    kind: SymbolKind,
    sort_key: String,
    display_addr: String,
    size: String,
    name: String,
    module: String,
    location: (String, String),
) -> SymbolRecord {
    let (base_hex, bit_index) = location;
    let raw_line = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        kind.as_str(),
        sort_key,
        display_addr,
        size,
        name,
        module,
        base_hex,
        bit_index
    );

    SymbolRecord {
        kind,
        sort_key,
        display_addr,
        size,
        name,
        module,
        base_hex,
        bit_index,
        raw_line,
    }
}

fn print_memory_overview(map_file: &Path) -> Result<Vec<String>> {
    let content = read_text_lossy(map_file)?;
    let mut in_link_map = false;
    let mut lines = Vec::new();

    for line in content.lines() {
        if line.starts_with("LINK MAP OF MODULE:") {
            in_link_map = true;
            continue;
        }
        if line.starts_with("OVERLAY MAP OF MODULE:") {
            in_link_map = false;
            continue;
        }
        if !in_link_map {
            continue;
        }

        let trimmed = line.trim_start();
        if matches_memory_overview(trimmed) {
            lines.push(trimmed.to_string());
        }
    }

    Ok(lines)
}

fn matches_memory_overview(line: &str) -> bool {
    ["REG", "DATA", "IDATA", "BIT", "XDATA"]
        .iter()
        .any(|prefix| {
            line.starts_with(prefix) && line[prefix.len()..].starts_with(char::is_whitespace)
        })
}

fn filter_symbols(symbols: &[SymbolRecord], pattern: &str) -> Vec<SymbolRecord> {
    if pattern.is_empty() {
        return symbols.to_vec();
    }

    let pattern_lower = pattern.to_lowercase();
    symbols
        .iter()
        .filter(|symbol| symbol.raw_line.to_lowercase().contains(&pattern_lower))
        .cloned()
        .collect()
}

fn print_section(title: &str) {
    println!();
    println!("[{title}]");
}

fn print_rows(
    kind: SymbolKind,
    symbols: &[SymbolRecord],
    listings_dir: &Path,
    lst_files: &[PathBuf],
) -> Result<()> {
    let mut rows: Vec<&SymbolRecord> = symbols.iter().filter(|row| row.kind == kind).collect();
    rows.sort_by(|left, right| {
        left.sort_key
            .cmp(&right.sort_key)
            .then_with(|| left.name.cmp(&right.name))
    });

    if rows.is_empty() {
        return Ok(());
    }

    println!();
    println!("{}:", kind.as_str());
    for row in rows {
        let decl_info = find_declaration_info(&row.name, &row.module, listings_dir, lst_files)?;
        let mut resolved_size = row.size.clone();
        if resolved_size == "?"
            && let Some(info) = &decl_info
            && info.size != "?"
        {
            resolved_size = info.size.clone();
        }

        println!(
            "  {} size={} {:<24} module={} -> {}",
            row.display_addr,
            resolved_size,
            row.name,
            row.module,
            access_hint(row)
        );

        if let Some(info) = decl_info {
            println!("    lst={}:{} {}", info.file, info.line_number, info.text);
        }
    }

    Ok(())
}

fn access_hint(symbol: &SymbolRecord) -> String {
    match symbol.kind {
        SymbolKind::Dseg => format!(
            "peek_data({}) / poke_data({}, value)",
            symbol.display_addr, symbol.display_addr
        ),
        SymbolKind::Iseg => format!(
            "peek_idata({}) / poke_idata({}, value)",
            symbol.display_addr, symbol.display_addr
        ),
        SymbolKind::Pseg => format!(
            "peek_pdata({}) / poke_pdata({}, value)",
            symbol.display_addr, symbol.display_addr
        ),
        SymbolKind::Xseg => format!(
            "peek_xdata({}) / poke_xdata({}, value)",
            symbol.display_addr, symbol.display_addr
        ),
        SymbolKind::Sfr => format!(
            "peek_sfr({}) / poke_sfr({}, value)",
            symbol.display_addr, symbol.display_addr
        ),
        SymbolKind::Bit => format!(
            "peek_data({}) & {}",
            format_hex_addr(&symbol.base_hex),
            bit_mask_hex(&symbol.bit_index)
        ),
        SymbolKind::Sbit => format!(
            "peek_sfr({}) & {}",
            format_hex_addr(&symbol.base_hex),
            bit_mask_hex(&symbol.bit_index)
        ),
    }
}

fn bit_mask_hex(bit_index: &str) -> &'static str {
    match bit_index {
        "0" => "0x01",
        "1" => "0x02",
        "2" => "0x04",
        "3" => "0x08",
        "4" => "0x10",
        "5" => "0x20",
        "6" => "0x40",
        "7" => "0x80",
        _ => "?",
    }
}

fn find_declaration_info(
    name: &str,
    module: &str,
    listings_dir: &Path,
    lst_files: &[PathBuf],
) -> Result<Option<DeclarationInfo>> {
    let mut candidates = Vec::new();
    if !module.is_empty() && module != "-" {
        let primary = listings_dir.join(format!("{}.lst", module.to_ascii_lowercase()));
        if primary.is_file() {
            candidates.push(primary);
        }
    }

    for file in lst_files {
        if candidates
            .iter()
            .any(|candidate: &PathBuf| same_path(candidate, file))
        {
            continue;
        }
        candidates.push(file.clone());
    }

    for prefer_decl in [true, false] {
        for file in &candidates {
            if let Some((line_number, text)) = search_declaration_in_file(file, name, prefer_decl)?
            {
                return Ok(Some(DeclarationInfo {
                    file: file
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or_default()
                        .to_string(),
                    line_number,
                    size: infer_decl_size(&text),
                    text,
                }));
            }
        }
    }

    Ok(None)
}

fn same_path(left: &Path, right: &Path) -> bool {
    display_path(left).eq_ignore_ascii_case(&display_path(right))
}

fn search_declaration_in_file(
    file: &Path,
    name: &str,
    prefer_decl: bool,
) -> Result<Option<(usize, String)>> {
    let content = read_text_lossy(file)?;

    for (index, raw_line) in content.lines().enumerate() {
        if !raw_line.contains(name) {
            continue;
        }
        if prefer_decl && !looks_like_decl(raw_line, name) {
            continue;
        }
        return Ok(Some((index + 1, raw_line.trim_end().to_string())));
    }

    Ok(None)
}

fn looks_like_decl(line: &str, needle: &str) -> bool {
    let Some(pos) = line.find(needle) else {
        return false;
    };
    if !line.contains(';') {
        return false;
    }

    let prefix = line[..pos].to_lowercase();
    [
        "idata", "pdata", "xdata", "bdata", "data", "code", "bit", "sbit", "volatile", "static",
        "const", "unsigned", "signed", "char", "int", "long", "float", "double", "u8", "u16",
        "u32", "u64",
    ]
    .iter()
    .any(|keyword| prefix.contains(keyword))
}

fn infer_decl_size(line: &str) -> String {
    let lower = line.to_lowercase();
    let base = if contains_token(&lower, "sbit") || contains_token(&lower, "bit") {
        "1bit".to_string()
    } else if contains_token(&lower, "double")
        || contains_token(&lower, "float")
        || contains_token(&lower, "u32")
        || contains_token(&lower, "long")
    {
        "4".to_string()
    } else if contains_token(&lower, "u16") || contains_token(&lower, "int") {
        "2".to_string()
    } else if contains_token(&lower, "u8") || contains_token(&lower, "char") {
        "1".to_string()
    } else {
        "?".to_string()
    };

    let array_re = Regex::new(r"\[\s*(\d+)\s*\]").expect("array regex");
    if let Some(captures) = array_re.captures(&lower)
        && let Some(count) = captures
            .get(1)
            .and_then(|value| value.as_str().parse::<usize>().ok())
    {
        if base == "1bit" {
            return format!("{count}bit");
        }
        if let Ok(base_num) = base.parse::<usize>() {
            return (base_num * count).to_string();
        }
    }

    base
}

fn contains_token(line: &str, needle: &str) -> bool {
    let mut start = 0;
    while let Some(offset) = line[start..].find(needle) {
        let index = start + offset;
        let prev = line[..index].chars().next_back();
        let next = line[index + needle.len()..].chars().next();
        let prev_ok = prev.is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_');
        let next_ok = next.is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_');
        if prev_ok && next_ok {
            return true;
        }
        start = index + 1;
    }
    false
}

fn collect_search_hits(
    m51_files: &[PathBuf],
    lst_files: &[PathBuf],
    pattern: &str,
) -> Result<Vec<SearchHit>> {
    let mut hits = Vec::new();
    let pattern_lower = pattern.to_lowercase();

    for file in m51_files.iter().chain(lst_files.iter()) {
        let content = read_text_lossy(file)?;
        for (index, line) in content.lines().enumerate() {
            if !line.to_lowercase().contains(&pattern_lower) {
                continue;
            }
            hits.push(SearchHit {
                path: display_path(file),
                line_number: index + 1,
                text: line.to_string(),
            });
        }
    }

    Ok(hits)
}
