use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use regex::{Captures, Regex};
use tempfile::TempDir;
use tracing::{debug, info, warn};
use tracing_subscriber::{EnvFilter, fmt};
use which::which;

const DEFAULT_IRAM_SIZE: u32 = 256;
const DEFAULT_XRAM_SIZE: u32 = 1792;
const DEFAULT_CODE_SIZE: u32 = 0xEFF9;

const STC15_COMPAT_HEADER: &str = r#"#ifndef STC15F2K60S2_SDCC_COMPAT_H
#define STC15F2K60S2_SDCC_COMPAT_H

#include <stc12.h>

__sfr __at (0xD6) T2H;
__sfr __at (0xD7) T2L;

#define P00 P0_0
#define P01 P0_1
#define P02 P0_2
#define P03 P0_3
#define P04 P0_4
#define P05 P0_5
#define P06 P0_6
#define P07 P0_7

#define P10 P1_0
#define P11 P1_1
#define P12 P1_2
#define P13 P1_3
#define P14 P1_4
#define P15 P1_5
#define P16 P1_6
#define P17 P1_7

#define P20 P2_0
#define P21 P2_1
#define P22 P2_2
#define P23 P2_3
#define P24 P2_4
#define P25 P2_5
#define P26 P2_6
#define P27 P2_7

#define P30 P3_0
#define P31 P3_1
#define P32 P3_2
#define P33 P3_3
#define P34 P3_4
#define P35 P3_5
#define P36 P3_6
#define P37 P3_7

#define P40 P4_0
#define P41 P4_1
#define P42 P4_2
#define P43 P4_3
#define P44 P4_4
#define P45 P4_5
#define P46 P4_6
#define P47 P4_7

#define P50 P5_0
#define P51 P5_1
#define P52 P5_2
#define P53 P5_3

#endif
"#;

const INTRINS_COMPAT_HEADER: &str = r#"#ifndef INTRINS_SDCC_COMPAT_H
#define INTRINS_SDCC_COMPAT_H

#define _nop_() __asm__("nop")

#endif
"#;

const SDCC_STDIO_COMPAT_HEADER: &str = r#"#ifndef SDCC_STDIO_COMPAT_H
#define SDCC_STDIO_COMPAT_H

#include <stdio.h>

int sscanf(const char *input, const char *format, ...);

#endif
"#;

const SDCC_STDIO_COMPAT_SOURCE: &str = r#"#include "sdcc_stdio_compat.h"

#include <stdarg.h>

static int compat_is_space(char ch) {
    return ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n' || ch == '\f' || ch == '\v';
}

static int compat_is_digit(char ch) {
    return ch >= '0' && ch <= '9';
}

static const char *compat_parse_unsigned_long(const char *src, unsigned int width, unsigned char has_width, unsigned long *out) {
    unsigned long value = 0;
    unsigned char seen = 0;
    unsigned int consumed = 0;

    while (compat_is_space(*src)) {
        ++src;
    }
    while ((!has_width || consumed < width) && compat_is_digit(*src)) {
        seen = 1;
        value = value * 10 + (unsigned long)(*src - '0');
        ++src;
        ++consumed;
    }
    if (!seen) {
        return 0;
    }
    *out = value;
    return src;
}

static const char *compat_parse_signed_long(const char *src, unsigned int width, unsigned char has_width, long *out) {
    unsigned long value = 0;
    unsigned char negative = 0;
    unsigned char seen = 0;
    unsigned int consumed = 0;

    while (compat_is_space(*src)) {
        ++src;
    }
    if ((!has_width || consumed < width) && (*src == '+' || *src == '-')) {
        negative = (*src == '-');
        ++src;
        ++consumed;
    }
    while ((!has_width || consumed < width) && compat_is_digit(*src)) {
        seen = 1;
        value = value * 10 + (unsigned long)(*src - '0');
        ++src;
        ++consumed;
    }
    if (!seen) {
        return 0;
    }
    *out = negative ? -(long)value : (long)value;
    return src;
}

static const char *compat_parse_float(const char *src, unsigned int width, unsigned char has_width, float *out) {
    float value = 0.0f;
    float scale = 0.1f;
    char sign = 1;
    unsigned char seen = 0;
    unsigned int consumed = 0;

    while (compat_is_space(*src)) {
        ++src;
    }
    if ((!has_width || consumed < width) && *src == '+') {
        ++src;
        ++consumed;
    } else if ((!has_width || consumed < width) && *src == '-') {
        sign = -1;
        ++src;
        ++consumed;
    }
    while ((!has_width || consumed < width) && compat_is_digit(*src)) {
        seen = 1;
        value = value * 10.0f + (float)(*src - '0');
        ++src;
        ++consumed;
    }
    if ((!has_width || consumed < width) && *src == '.') {
        ++src;
        ++consumed;
        while ((!has_width || consumed < width) && compat_is_digit(*src)) {
            seen = 1;
            value = value + (float)(*src - '0') * scale;
            scale = scale * 0.1f;
            ++src;
            ++consumed;
        }
    }
    if (!seen) {
        return 0;
    }
    *out = sign < 0 ? -value : value;
    return src;
}

int sscanf(const char *input, const char *format, ...) {
    const char *src = input;
    const char *fmt = format;
    int assigned = 0;
    va_list args;

    va_start(args, format);
    while (*fmt != '\0') {
        if (compat_is_space(*fmt)) {
            while (compat_is_space(*fmt)) {
                ++fmt;
            }
            while (compat_is_space(*src)) {
                ++src;
            }
            continue;
        }

        if (*fmt != '%') {
            if (*src != *fmt) {
                break;
            }
            ++src;
            ++fmt;
            continue;
        }

        ++fmt;
        if (*fmt == '%') {
            if (*src != '%') {
                break;
            }
            ++src;
            ++fmt;
            continue;
        }

        {
            unsigned char has_width = 0;
            unsigned int width = 0;

            while (compat_is_digit(*fmt)) {
                has_width = 1;
                width = width * 10 + (unsigned int)(*fmt - '0');
                ++fmt;
            }
            if (*fmt == '.') {
                ++fmt;
                while (compat_is_digit(*fmt)) {
                    ++fmt;
                }
            }

        if (*fmt == 'l' && fmt[1] == 'u') {
            const char *end = 0;
            unsigned long value = 0;
            unsigned long *out;

            end = compat_parse_unsigned_long(src, width, has_width, &value);
            if (!end) {
                break;
            }
            out = va_arg(args, unsigned long *);
            *out = value;
            src = end;
            fmt += 2;
            ++assigned;
            continue;
        }

        if (*fmt == 'b' && fmt[1] == 'u') {
            const char *end = 0;
            unsigned long value = 0;
            unsigned char *out;

            end = compat_parse_unsigned_long(src, width, has_width, &value);
            if (!end) {
                break;
            }
            out = va_arg(args, unsigned char *);
            *out = (unsigned char)value;
            src = end;
            fmt += 2;
            ++assigned;
            continue;
        }

        if (*fmt == 'l' && fmt[1] == 'd') {
            const char *end = 0;
            long value = 0;
            long *out;

            end = compat_parse_signed_long(src, width, has_width, &value);
            if (!end) {
                break;
            }
            out = va_arg(args, long *);
            *out = value;
            src = end;
            fmt += 2;
            ++assigned;
            continue;
        }

        if (*fmt == 'b' && fmt[1] == 'd') {
            const char *end = 0;
            long value = 0;
            signed char *out;

            end = compat_parse_signed_long(src, width, has_width, &value);
            if (!end) {
                break;
            }
            out = va_arg(args, signed char *);
            *out = (signed char)value;
            src = end;
            fmt += 2;
            ++assigned;
            continue;
        }

        if (*fmt == 'u') {
            const char *end = 0;
            unsigned long value = 0;
            unsigned int *out;

            end = compat_parse_unsigned_long(src, width, has_width, &value);
            if (!end) {
                break;
            }
            out = va_arg(args, unsigned int *);
            *out = (unsigned int)value;
            src = end;
            ++fmt;
            ++assigned;
            continue;
        }

        if (*fmt == 'd') {
            const char *end = 0;
            long value = 0;
            int *out;

            end = compat_parse_signed_long(src, width, has_width, &value);
            if (!end) {
                break;
            }
            out = va_arg(args, int *);
            *out = (int)value;
            src = end;
            ++fmt;
            ++assigned;
            continue;
        }

        if (*fmt == 'f') {
            const char *end = 0;
            float value = 0.0f;
            float *out;

            end = compat_parse_float(src, width, has_width, &value);
            if (!end) {
                break;
            }
            out = va_arg(args, float *);
            *out = value;
            src = end;
            ++fmt;
            ++assigned;
            continue;
        }

        if (*fmt == 'n') {
            unsigned int *out = va_arg(args, unsigned int *);
            *out = (unsigned int)(src - input);
            ++fmt;
            continue;
        }
        }

        assigned = 0;
        break;
    }
    va_end(args);

    return assigned;
}
"#;

const KEYWORD_REPLACEMENTS: [(&str, &str); 6] = [
    ("idata", "__idata"),
    ("pdata", "__pdata"),
    ("xdata", "__xdata"),
    ("bdata", "__data"),
    ("code", "__code"),
    ("bit", "__bit"),
];

const BUILD_PROFILES: [BuildProfile; 3] = [
    BuildProfile {
        label: "small",
        flags: &["--model-small"],
    },
    BuildProfile {
        label: "small-stack-auto",
        flags: &["--model-small", "--stack-auto"],
    },
    BuildProfile {
        label: "large-stack-auto",
        flags: &["--model-large", "--stack-auto"],
    },
];

#[derive(Debug, Parser)]
#[command(author, version, about = "使用 sdcc + packihx 构建 sample hex")]
struct Cli {
    sample: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MemoryConfig {
    iram_size: u32,
    xram_size: u32,
    code_size: u32,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            iram_size: DEFAULT_IRAM_SIZE,
            xram_size: DEFAULT_XRAM_SIZE,
            code_size: DEFAULT_CODE_SIZE,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct BuildProfile {
    label: &'static str,
    flags: &'static [&'static str],
}

#[derive(Debug)]
struct ProfileFailure {
    stage: &'static str,
    label: &'static str,
    log: String,
}

#[derive(Debug)]
struct BuildContext<'a> {
    sdcc: &'a Path,
    memory: &'a MemoryConfig,
    include_dirs: &'a [PathBuf],
    c_files: &'a [PathBuf],
    rel_dir: &'a Path,
    ihx_path: &'a Path,
    temp_root: &'a Path,
}

#[derive(Debug)]
struct TempWorkspace {
    path: TempDir,
}

impl TempWorkspace {
    fn new(prefix: &str) -> Result<Self> {
        let path = tempfile::Builder::new()
            .prefix(prefix)
            .tempdir()
            .with_context(|| format!("创建临时目录失败, 前缀: {prefix}"))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        self.path.path()
    }
}

fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    run(cli)
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).try_init();
}

fn run(cli: Cli) -> Result<()> {
    let workspace_root = env::current_dir().context("读取当前工作目录失败")?;
    let sample_dir = workspace_root.join("sample").join(&cli.sample);
    let src_dir = sample_dir.join("src");
    let prj_dir = sample_dir.join("prj");
    let objects_dir = prj_dir.join("Objects");

    ensure!(
        sample_dir.is_dir(),
        "sample directory not found: {}",
        sample_dir.display()
    );
    ensure!(
        src_dir.is_dir(),
        "source directory not found: {}",
        src_dir.display()
    );

    let sdcc = which("sdcc").context("sdcc not found in PATH")?;
    let packihx = which("packihx").context("packihx not found in PATH")?;
    debug!("sdcc path: {}", sdcc.display());
    debug!("packihx path: {}", packihx.display());

    fs::create_dir_all(&objects_dir)
        .with_context(|| format!("创建输出目录失败: {}", objects_dir.display()))?;

    let memory = load_memory_config(&prj_dir)?;
    println!(
        "memory: iram={} xram={} code={}",
        memory.iram_size, memory.xram_size, memory.code_size
    );
    info!(
        sample = %cli.sample,
        iram_size = memory.iram_size,
        xram_size = memory.xram_size,
        code_size = memory.code_size,
        "loaded memory layout"
    );

    let temp = TempWorkspace::new(&format!("sdcc-build-{}", sanitize_name(&cli.sample)))?;
    let compat_dir = temp.path().join("compat");
    let transformed_root = temp.path().join("source");
    let rel_dir = temp.path().join("rel");

    fs::create_dir_all(&compat_dir)
        .with_context(|| format!("创建兼容目录失败: {}", compat_dir.display()))?;
    fs::create_dir_all(&transformed_root)
        .with_context(|| format!("创建转换目录失败: {}", transformed_root.display()))?;
    fs::create_dir_all(&rel_dir)
        .with_context(|| format!("创建目标目录失败: {}", rel_dir.display()))?;

    write_compat_headers(&compat_dir)?;

    let source_files = collect_source_files(&src_dir, &["c", "h"])?;
    ensure!(
        !source_files.is_empty(),
        "no C or header files found under {}",
        src_dir.display()
    );

    let mut include_dirs = vec![compat_dir.clone()];
    let mut c_files = Vec::new();

    for source_file in &source_files {
        let relative = source_file
            .strip_prefix(&src_dir)
            .with_context(|| format!("计算相对路径失败: {}", source_file.display()))?;
        let output_file = transformed_root.join(relative);

        if let Some(parent) = output_file.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("创建转换目录失败: {}", parent.display()))?;
            push_unique_path(&mut include_dirs, parent);
        }

        transform_source(source_file, &output_file)?;

        if source_file.extension().and_then(|ext| ext.to_str()) == Some("c") {
            c_files.push(output_file);
        }
    }

    c_files.push(compat_dir.join("sdcc_stdio_compat.c"));

    ensure!(
        !c_files.is_empty(),
        "no C source files found under {}",
        src_dir.display()
    );

    let output_stem = sample_dir
        .file_name()
        .and_then(|name| name.to_str())
        .context("sample 名称无效")?;
    let ihx_path = objects_dir.join(format!("{output_stem}.ihx"));
    let hex_path = objects_dir.join(format!("{output_stem}.hex"));

    let mut last_failure = None;
    let build_context = BuildContext {
        sdcc: &sdcc,
        memory: &memory,
        include_dirs: &include_dirs,
        c_files: &c_files,
        rel_dir: &rel_dir,
        ihx_path: &ihx_path,
        temp_root: temp.path(),
    };
    for profile in BUILD_PROFILES {
        match build_with_profile(&build_context, profile)? {
            None => {
                println!("profile: {}", profile.label);
                run_packihx(&packihx, &ihx_path, &hex_path, temp.path())?;
                println!("ihx: {}", ihx_path.display());
                println!("hex: {}", hex_path.display());
                return Ok(());
            }
            Some(failure) => {
                warn!(
                    profile = failure.label,
                    stage = failure.stage,
                    "build profile failed, trying next profile"
                );
                last_failure = Some(failure);
            }
        }
    }

    let failure = last_failure.context("没有可用的构建 profile")?;
    eprintln!("{} failed for profile: {}", failure.stage, failure.label);
    eprintln!("{}", failure.log);
    bail!("构建失败")
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '_',
        })
        .collect()
}

fn write_compat_headers(compat_dir: &Path) -> Result<()> {
    fs::write(compat_dir.join("STC15F2K60S2.H"), STC15_COMPAT_HEADER.as_bytes())
        .with_context(|| {
            format!(
                "写入兼容头文件失败: {}",
                compat_dir.join("STC15F2K60S2.H").display()
            )
        })?;
    fs::write(compat_dir.join("intrins.h"), INTRINS_COMPAT_HEADER.as_bytes())
        .with_context(|| {
            format!(
                "写入兼容头文件失败: {}",
                compat_dir.join("intrins.h").display()
            )
        })?;
    fs::write(
        compat_dir.join("sdcc_stdio_compat.h"),
        SDCC_STDIO_COMPAT_HEADER.as_bytes(),
    )
    .with_context(|| {
        format!(
            "写入兼容头文件失败: {}",
            compat_dir.join("sdcc_stdio_compat.h").display()
        )
    })?;
    fs::write(
        compat_dir.join("sdcc_stdio_compat.c"),
        SDCC_STDIO_COMPAT_SOURCE.as_bytes(),
    )
    .with_context(|| {
        format!(
            "写入兼容源码失败: {}",
            compat_dir.join("sdcc_stdio_compat.c").display()
        )
    })?;
    Ok(())
}

fn collect_source_files(root: &Path, extensions: &[&str]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_source_files_inner(root, extensions, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_source_files_inner(
    root: &Path,
    extensions: &[&str],
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("读取目录失败: {}", root.display()))? {
        let entry = entry.with_context(|| format!("读取目录项失败: {}", root.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("读取文件类型失败: {}", path.display()))?;

        if file_type.is_dir() {
            collect_source_files_inner(&path, extensions, files)?;
            continue;
        }

        if file_type.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| extensions.iter().any(|candidate| candidate == &ext))
        {
            files.push(path);
        }
    }

    Ok(())
}

fn load_memory_config(prj_dir: &Path) -> Result<MemoryConfig> {
    let mut uvproj_files = Vec::new();

    if prj_dir.is_dir() {
        for entry in
            fs::read_dir(prj_dir).with_context(|| format!("读取目录失败: {}", prj_dir.display()))?
        {
            let entry = entry.with_context(|| format!("读取目录项失败: {}", prj_dir.display()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("读取文件类型失败: {}", path.display()))?;
            if file_type.is_file()
                && path.extension().and_then(|ext| ext.to_str()) == Some("uvproj")
            {
                uvproj_files.push(path);
            }
        }
    }

    uvproj_files.sort();
    let Some(uvproj_path) = uvproj_files.into_iter().next() else {
        return Ok(MemoryConfig::default());
    };

    let project = fs::read_to_string(&uvproj_path)
        .with_context(|| format!("读取 uvproj 失败: {}", uvproj_path.display()))?;
    let cpu_regex =
        Regex::new(r"(?s)<Cpu>(.*?)</Cpu>").context("编译 Cpu 提取正则失败")?;

    let Some(captures) = cpu_regex.captures(&project) else {
        warn!(path = %uvproj_path.display(), "uvproj 中未找到 Cpu 字段, 使用默认内存方案");
        return Ok(MemoryConfig::default());
    };

    let cpu_spec = captures
        .get(1)
        .map(|matched| matched.as_str())
        .unwrap_or_default();
    debug!(path = %uvproj_path.display(), cpu_spec, "parsed cpu spec");
    Ok(parse_cpu_spec(cpu_spec))
}

fn parse_cpu_spec(cpu_spec: &str) -> MemoryConfig {
    let mut memory = MemoryConfig::default();

    for (regex, target) in [
        (r"IRAM\(0-0x([0-9A-Fa-f]+)\)", MemoryField::Iram),
        (r"XRAM\(0-0x([0-9A-Fa-f]+)\)", MemoryField::Xram),
        (r"IROM\(0-0x([0-9A-Fa-f]+)\)", MemoryField::Code),
    ] {
        let regex = Regex::new(regex).expect("memory regex must compile");
        if let Some(captures) = regex.captures(cpu_spec) {
            let Some(hex) = captures.get(1).map(|matched| matched.as_str()) else {
                continue;
            };
            if let Ok(limit) = u32::from_str_radix(hex, 16) {
                match target {
                    MemoryField::Iram => memory.iram_size = limit + 1,
                    MemoryField::Xram => memory.xram_size = limit + 1,
                    MemoryField::Code => memory.code_size = limit + 1,
                }
            }
        }
    }

    memory
}

#[derive(Debug, Clone, Copy)]
enum MemoryField {
    Iram,
    Xram,
    Code,
}

fn transform_source(input: &Path, output: &Path) -> Result<()> {
    let source = fs::read(input).with_context(|| format!("读取源码失败: {}", input.display()))?;
    let text = latin1_bytes_to_string(&source);
    let file_name = input.file_name().and_then(|name| name.to_str());
    let text = replace_ascii_identifier_tokens(&text, &KEYWORD_REPLACEMENTS);
    let text = replace_keil_data_storage(&text)?;
    let text = replace_interrupt_and_using(&text)?;
    let text = replace_putchar_signatures(&text)?;
    let text = inject_stdio_compat_include(&text);
    let text = replace_i2c_delay_linkage(file_name, &text)?;
    let text = inject_i2c_delay_prototype(file_name, &text);
    let text = replace_sbit_declarations(&text)?;
    let output_bytes = latin1_string_to_bytes(&text)?;

    fs::write(output, output_bytes)
        .with_context(|| format!("写入转换结果失败: {}", output.display()))
}

fn latin1_bytes_to_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| char::from(*byte)).collect()
}

fn latin1_string_to_bytes(text: &str) -> Result<Vec<u8>> {
    text.chars()
        .map(|ch| {
            let code = u32::from(ch);
            u8::try_from(code).context("转换后的源码包含无法写回 Latin-1 的字符")
        })
        .collect()
}

fn replace_ascii_identifier_tokens(input: &str, replacements: &[(&str, &str)]) -> String {
    let mut result = String::with_capacity(input.len());
    let mut token = String::new();

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            token.push(ch);
            continue;
        }

        flush_ascii_token(&mut result, &mut token, replacements);
        result.push(ch);
    }

    flush_ascii_token(&mut result, &mut token, replacements);
    result
}

fn flush_ascii_token(result: &mut String, token: &mut String, replacements: &[(&str, &str)]) {
    if token.is_empty() {
        return;
    }

    if let Some((_, replacement)) = replacements.iter().find(|(source, _)| *source == token) {
        result.push_str(replacement);
    } else {
        result.push_str(token);
    }

    token.clear();
}

fn replace_interrupt_and_using(input: &str) -> Result<String> {
    let interrupt_regex = Regex::new(r"(?m)(^|[^A-Za-z0-9_])interrupt[ \t]+(\d+)")
        .context("编译 interrupt 转换正则失败")?;
    let using_regex = Regex::new(r"(?m)(^|[^A-Za-z0-9_])using[ \t]+(\d+)")
        .context("编译 using 转换正则失败")?;

    let text = interrupt_regex.replace_all(input, "${1}__interrupt($2)");
    let text = using_regex.replace_all(&text, "${1}__using($2)");
    Ok(text.into_owned())
}

fn replace_keil_data_storage(input: &str) -> Result<String> {
    let regex = Regex::new(
        r"(?m)^([ \t]*(?:(?:unsigned|signed|static|const|volatile|register|char|short|int|long|float|double|u8|u16|u32|u64|s8|s16|s32|s64|uint|ulong|uchar)(?:[ \t]+(?:unsigned|signed|static|const|volatile|register|char|short|int|long|float|double|u8|u16|u32|u64|s8|s16|s32|s64|uint|ulong|uchar))*))[ \t]+data([ \t]+(?:\*+[ \t]*)?[A-Za-z_]\w*)",
    )
    .context("编译 data 存储类转换正则失败")?;
    Ok(regex.replace_all(input, "${1}${2}").into_owned())
}

fn replace_putchar_signatures(input: &str) -> Result<String> {
    let prototype_regex = Regex::new(
        r"(?m)^([ \t]*)(?:extern[ \t]+)?char[ \t]+putchar[ \t]*\([ \t]*char(?:[ \t]+([A-Za-z_]\w*))?[ \t]*\)[ \t]*;",
    )
    .context("编译 putchar 声明转换正则失败")?;
    let definition_regex = Regex::new(
        r"(?m)^([ \t]*)(?:extern[ \t]+)?char[ \t]+putchar[ \t]*\([ \t]*char[ \t]+([A-Za-z_]\w*)[ \t]*\)[ \t]*\{",
    )
    .context("编译 putchar 定义转换正则失败")?;

    let text = prototype_regex.replace_all(input, |captures: &Captures<'_>| {
        let indent = captures.get(1).map(|matched| matched.as_str()).unwrap_or("");
        let parameter = captures
            .get(2)
            .map(|matched| format!(" {}", matched.as_str()))
            .unwrap_or_default();
        format!("{indent}int putchar(int{parameter});")
    });
    let text = definition_regex.replace_all(&text, |captures: &Captures<'_>| {
        let indent = captures.get(1).map(|matched| matched.as_str()).unwrap_or("");
        let name = captures.get(2).map(|matched| matched.as_str()).unwrap_or("ch");
        format!("{indent}int putchar(int {name}) {{")
    });
    Ok(text.into_owned())
}

fn inject_stdio_compat_include(input: &str) -> String {
    if !input.contains("stdio.h") || input.contains("sdcc_stdio_compat.h") {
        return input.to_owned();
    }

    let mut output = String::with_capacity(input.len() + 48);
    for line in input.split_inclusive('\n') {
        output.push_str(line);
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.contains("stdio.h") {
            let newline = if line.ends_with("\r\n") {
                "\r\n"
            } else if line.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            output.push_str("#include \"sdcc_stdio_compat.h\"");
            output.push_str(newline);
        }
    }

    if !input.ends_with('\n') && output.ends_with("sdcc_stdio_compat.h") {
        output.push('\n');
    }

    output
}

fn replace_i2c_delay_linkage(file_name: Option<&str>, input: &str) -> Result<String> {
    if file_name != Some("iic.c") {
        return Ok(input.to_owned());
    }

    let regex = Regex::new(r"(?m)^([ \t]*)static[ \t]+void[ \t]+I2C_Delay[ \t]*\(")
        .context("编译 I2C_Delay 可见性转换正则失败")?;
    Ok(regex
        .replace_all(input, "${1}void I2C_Delay(")
        .into_owned())
}

fn inject_i2c_delay_prototype(file_name: Option<&str>, input: &str) -> String {
    if file_name != Some("iic.h") || input.contains("I2C_Delay(") {
        return input.to_owned();
    }

    let newline = if input.contains("\r\n") { "\r\n" } else { "\n" };
    let mut output = input.to_owned();
    if !output.ends_with('\n') {
        output.push_str(newline);
    }
    output.push_str("void I2C_Delay(unsigned char n);");
    output.push_str(newline);
    output
}

fn replace_sbit_declarations(input: &str) -> Result<String> {
    let regex = Regex::new(
        r"(?m)^([ \t]*)sbit[ \t]+([A-Za-z_]\w*)[ \t]*=[ \t]*(P[0-5])[ \t]*\^[ \t]*([0-7])[ \t]*;",
    )
    .context("编译 sbit 转换正则失败")?;

    let text = regex.replace_all(input, |captures: &Captures<'_>| {
        let indent = captures.get(1).map(|matched| matched.as_str()).unwrap_or("");
        let name = captures.get(2).map(|matched| matched.as_str()).unwrap_or("");
        let port = captures.get(3).map(|matched| matched.as_str()).unwrap_or("");
        let bit = captures
            .get(4)
            .and_then(|matched| matched.as_str().parse::<u8>().ok())
            .unwrap_or(0);
        let base = match port {
            "P0" => 0x80_u8,
            "P1" => 0x90_u8,
            "P2" => 0xA0_u8,
            "P3" => 0xB0_u8,
            "P4" => 0xC0_u8,
            "P5" => 0xC8_u8,
            _ => 0x80_u8,
        };
        format!("{indent}__sbit __at (0x{:02X}) {name};", base + bit)
    });

    Ok(text.into_owned())
}

fn build_with_profile(
    context: &BuildContext<'_>,
    profile: BuildProfile,
) -> Result<Option<ProfileFailure>> {
    let compile_log_path = context
        .temp_root
        .join(format!("{}.compile.log", profile.label));
    let link_log_path = context.temp_root.join(format!("{}.link.log", profile.label));
    let mut rel_files = Vec::new();

    for source_file in context.c_files {
        let stem = source_file
            .strip_prefix(context.temp_root.join("source"))
            .unwrap_or(source_file)
            .to_string_lossy()
            .replace(['/', '\\'], "_");
        let stem = stem.strip_suffix(".c").unwrap_or(&stem).to_owned();
        let rel_file = context.rel_dir.join(format!("{stem}.rel"));

        let mut command = Command::new(context.sdcc);
        command.current_dir(context.temp_root);
        command.args([
            OsString::from("-mmcs51"),
            OsString::from("--std=c99"),
            OsString::from("--iram-size"),
            OsString::from(context.memory.iram_size.to_string()),
            OsString::from("--xram-size"),
            OsString::from(context.memory.xram_size.to_string()),
            OsString::from("--code-size"),
            OsString::from(context.memory.code_size.to_string()),
            OsString::from("--opt-code-size"),
        ]);
        for flag in profile.flags {
            command.arg(flag);
        }
        command.arg("-c");
        for include_dir in context.include_dirs {
            command.arg("-I");
            command.arg(include_dir);
        }
        command.arg("-o");
        command.arg(&rel_file);
        command.arg(source_file);

        debug!(
            profile = profile.label,
            source = %source_file.display(),
            rel = %rel_file.display(),
            "compiling C source"
        );
        let output = command
            .output()
            .with_context(|| format!("调用 sdcc 编译失败: {}", source_file.display()))?;
        let log = command_output_text(&output);
        fs::write(&compile_log_path, log.as_bytes())
            .with_context(|| format!("写入编译日志失败: {}", compile_log_path.display()))?;

        if !output.status.success() {
            return Ok(Some(ProfileFailure {
                stage: "compile",
                label: profile.label,
                log,
            }));
        }

        rel_files.push(rel_file);
    }

    let mut command = Command::new(context.sdcc);
    command.current_dir(context.temp_root);
    command.args([
        OsString::from("-mmcs51"),
        OsString::from("--out-fmt-ihx"),
        OsString::from("--iram-size"),
        OsString::from(context.memory.iram_size.to_string()),
        OsString::from("--xram-size"),
        OsString::from(context.memory.xram_size.to_string()),
        OsString::from("--code-size"),
        OsString::from(context.memory.code_size.to_string()),
        OsString::from("--opt-code-size"),
    ]);
    for flag in profile.flags {
        command.arg(flag);
    }
    command.arg("-o");
    command.arg(context.ihx_path);
    for rel_file in &rel_files {
        command.arg(rel_file);
    }

    debug!(
        profile = profile.label,
        ihx = %context.ihx_path.display(),
        "linking ihx"
    );
    let output = command
        .output()
        .with_context(|| format!("调用 sdcc 链接失败: {}", context.ihx_path.display()))?;
    let log = command_output_text(&output);
    fs::write(&link_log_path, log.as_bytes())
        .with_context(|| format!("写入链接日志失败: {}", link_log_path.display()))?;

    if !output.status.success() {
        return Ok(Some(ProfileFailure {
            stage: "link",
            label: profile.label,
            log,
        }));
    }

    Ok(None)
}

fn run_packihx(packihx: &Path, ihx_path: &Path, hex_path: &Path, temp_root: &Path) -> Result<()> {
    let output = Command::new(packihx)
        .current_dir(temp_root)
        .arg(ihx_path)
        .output()
        .with_context(|| format!("调用 packihx 失败: {}", ihx_path.display()))?;

    if !output.status.success() {
        bail!("packihx failed: {}", command_output_text(&output).trim());
    }

    if !output.stderr.is_empty() {
        warn!(
            stderr = %String::from_utf8_lossy(&output.stderr),
            "packihx emitted stderr output"
        );
    }

    fs::write(hex_path, &output.stdout)
        .with_context(|| format!("写入 hex 失败: {}", hex_path.display()))
}

fn command_output_text(output: &Output) -> String {
    let mut text = String::new();

    if !output.stdout.is_empty() {
        text.push_str(&String::from_utf8_lossy(&output.stdout));
    }

    if !output.stderr.is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&String::from_utf8_lossy(&output.stderr));
    }

    if text.is_empty() {
        text.push_str("(no output)\n");
    }

    text
}

fn push_unique_path(paths: &mut Vec<PathBuf>, candidate: &Path) {
    if !paths.iter().any(|path| path == candidate) {
        paths.push(candidate.to_path_buf());
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MemoryConfig, inject_i2c_delay_prototype, parse_cpu_spec, replace_ascii_identifier_tokens,
        replace_i2c_delay_linkage, replace_interrupt_and_using, replace_keil_data_storage,
        replace_putchar_signatures, replace_sbit_declarations,
    };

    #[test]
    fn parse_cpu_spec_reads_stc15_layout() {
        let memory = parse_cpu_spec("IRAM(0-0xFF) XRAM(0-0x6FF) IROM(0-0xEFF8) CLOCK(35000000)");
        assert_eq!(
            memory,
            MemoryConfig {
                iram_size: 256,
                xram_size: 1792,
                code_size: 61433,
            }
        );
    }

    #[test]
    fn transform_rewrites_putchar_signature() {
        let source = "extern char putchar(char ch) {\r\n\treturn ch;\r\n}\r\n";
        let transformed = replace_putchar_signatures(source).expect("rewrite putchar");
        assert!(transformed.contains("int putchar(int ch) {"));
    }

    #[test]
    fn transform_rewrites_sbit_and_keywords() {
        let source = "sbit sda = P2^1;\r\nidata u8 value;\r\nvoid isr(void) interrupt 4 using 1\r\n";
        let transformed =
            replace_sbit_declarations(&replace_interrupt_and_using(&replace_ascii_identifier_tokens(
                source,
                &[("idata", "__idata"), ("bit", "__bit")],
            ))
            .expect("rewrite interrupt"))
            .expect("rewrite sbit");
        assert!(transformed.contains("__sbit __at (0xA1) sda;"));
        assert!(transformed.contains("__idata u8 value;"));
        assert!(transformed.contains("__interrupt(4) __using(1)"));
    }

    #[test]
    fn transform_rewrites_keil_data_storage() {
        let source = "void Delay10ms(void)\n{\n    unsigned char data i, j;\n}\n";
        let transformed = replace_keil_data_storage(source).expect("rewrite data storage");
        assert!(transformed.contains("unsigned char i, j;"));
        assert!(!transformed.contains(" data "));
    }

    #[test]
    fn transform_exports_i2c_delay_for_cross_file_calls() {
        let source = "static void I2C_Delay(unsigned char n)\n{\n}\n";
        let transformed =
            replace_i2c_delay_linkage(Some("iic.c"), source).expect("rewrite i2c delay");
        assert!(transformed.contains("void I2C_Delay(unsigned char n)"));
        assert!(!transformed.contains("static void I2C_Delay"));
    }

    #[test]
    fn transform_injects_i2c_delay_prototype_when_missing() {
        let source = "#include \"utils.h\"\n\nvoid I2CStart(void);\n";
        let transformed = inject_i2c_delay_prototype(Some("iic.h"), source);
        assert!(transformed.contains("void I2C_Delay(unsigned char n);"));
    }

    #[test]
    fn stdio_compat_lists_bu_support() {
        assert!(super::SDCC_STDIO_COMPAT_SOURCE.contains("*fmt == 'b' && fmt[1] == 'u'"));
    }

    #[test]
    fn stdio_compat_lists_width_and_signed_support() {
        assert!(super::SDCC_STDIO_COMPAT_SOURCE.contains("while (compat_is_digit(*fmt))"));
        assert!(super::SDCC_STDIO_COMPAT_SOURCE.contains("*fmt == 'd'"));
        assert!(super::SDCC_STDIO_COMPAT_SOURCE.contains("fmt[1] == 'd'"));
        assert!(super::SDCC_STDIO_COMPAT_SOURCE.contains("if (*fmt == '.')"));
    }
}
