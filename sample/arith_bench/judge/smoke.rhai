let phase_names = [
    "u8加法",
    "u8减法",
    "u8乘法",
    "u8除法",
    "u8取模",
    "char加法",
    "char减法",
    "char乘法",
    "char除法",
    "char取模",
    "int加法",
    "int减法",
    "int乘法",
    "int除法",
    "int取模",
    "uint加法",
    "uint减法",
    "uint乘法",
    "uint除法",
    "uint取模",
    "long加法",
    "long减法",
    "long乘法",
    "long除法",
    "long取模",
    "ulong加法",
    "ulong减法",
    "ulong乘法",
    "ulong除法",
    "ulong取模",
    "float加法",
    "float减法",
    "float乘法",
    "float除法",
    "double加法",
    "double减法",
    "double乘法",
    "double除法",
];

let unit_ops_byte = 384;
let unit_ops_int = 384;
let unit_ops_long = 256;
let unit_ops_fp = 384;

let unit_op_counts = [
    unit_ops_byte,
    unit_ops_byte,
    unit_ops_byte,
    unit_ops_byte,
    unit_ops_byte,
    unit_ops_byte,
    unit_ops_byte,
    unit_ops_byte,
    unit_ops_byte,
    unit_ops_byte,
    unit_ops_int,
    unit_ops_int,
    unit_ops_int,
    unit_ops_int,
    unit_ops_int,
    unit_ops_int,
    unit_ops_int,
    unit_ops_int,
    unit_ops_int,
    unit_ops_int,
    unit_ops_long,
    unit_ops_long,
    unit_ops_long,
    unit_ops_long,
    unit_ops_long,
    unit_ops_long,
    unit_ops_long,
    unit_ops_long,
    unit_ops_long,
    unit_ops_long,
    unit_ops_fp,
    unit_ops_fp,
    unit_ops_fp,
    unit_ops_fp,
    unit_ops_fp,
    unit_ops_fp,
    unit_ops_fp,
    unit_ops_fp,
];

fn assert_phase_ns(actual, label) {
    assert_in(actual, 5_000..=800_000_000, label + ", 高电平脉宽应可测");
}

fn assert_gap_ns(actual, label) {
    assert_in(actual, 100_000..=20_000_000, label + ", 相邻阶段之间应存在稳定低电平间隔");
}

fn assert_repeat_ns(actual, baseline, label) {
    let tolerance = baseline / 2 + 50_000;
    assert(
        actual >= baseline - tolerance && actual <= baseline + tolerance,
        label + ", 同一阶段的两轮脉宽应保持接近"
    );
}

fn measure_cycle(cycle_label, first_phase_already_high, phase_names) {
    let samples = [];
    let index = 0;

    while index < len(phase_names) {
        let phase = phase_names[index];

        if index > 0 || !first_phase_already_high {
            let rise_ns = run_to(P34, UP, 20_000_000);
            assert_gap_ns(rise_ns, cycle_label + " " + phase + " 前");
        }

        add_marker(cycle_label + "_" + phase + "_上升沿");
        let pulse_ns = run_to(P34, DOWN, 800_000_000);
        add_marker(cycle_label + "_" + phase + "_下降沿");
        assert_phase_ns(pulse_ns, cycle_label + " " + phase);
        samples.push(pulse_ns);

        index += 1;
    }

    samples
}

fn format_fixed_2(value) {
    let scaled = (value * 100.0 + 0.5).to_int();
    let integer_part = scaled / 100;
    let frac_part = scaled % 100;

    if frac_part < 10 {
        "" + integer_part + ".0" + frac_part
    } else {
        "" + integer_part + "." + frac_part
    }
}

fn repeat_text(text, count) {
    let out = "";
    let index = 0;

    while index < count {
        out = out + text;
        index += 1;
    }

    out
}

fn display_width(text) {
    let width = 0;
    let index = 0;

    while index < len(text) {
        let ch = text[index..index + 1];
        if regex_is_match(ch, "^[ -~]$") {
            width += 1;
        } else {
            width += 2;
        }
        index += 1;
    }

    width
}

fn pad_right(text, width) {
    let extra = width - display_width(text);

    if extra > 0 {
        text + repeat_text(" ", extra)
    } else {
        text
    }
}

fn pad_left(text, width) {
    let extra = width - display_width(text);

    if extra > 0 {
        repeat_text(" ", extra) + text
    } else {
        text
    }
}

fn print_summary_table(cycle0, cycle1, phase_names, unit_op_counts) {
    print("算术运算耗时统计表");
    print(
        "| " + pad_right("阶段", 12)
        + " | " + pad_left("单位运算数", 10)
        + " | " + pad_left("第1轮总耗时_ns", 16)
        + " | " + pad_left("第2轮总耗时_ns", 16)
        + " | " + pad_left("平均总耗时_ns", 16)
        + " | " + pad_left("平均单次耗时_ns", 16)
        + " |"
    );
    print(
        "| " + repeat_text("-", 12)
        + " | " + repeat_text("-", 10)
        + " | " + repeat_text("-", 16)
        + " | " + repeat_text("-", 16)
        + " | " + repeat_text("-", 16)
        + " | " + repeat_text("-", 16)
        + " |"
    );

    let index = 0;
    while index < len(phase_names) {
        let phase = phase_names[index];
        let first_ns = cycle0[index];
        let second_ns = cycle1[index];
        let first_ns_f = first_ns.to_float();
        let second_ns_f = second_ns.to_float();
        let avg_ns = (first_ns_f + second_ns_f) / 2.0;
        let avg_ns_int = (first_ns + second_ns) / 2;
        let unit_ops = unit_op_counts[index];
        let avg_ns_per_op = avg_ns / unit_ops.to_float();

        print(
            "| " + pad_right(phase, 12)
            + " | " + pad_left(unit_ops.to_string(), 10)
            + " | " + pad_left(first_ns.to_string(), 16)
            + " | " + pad_left(second_ns.to_string(), 16)
            + " | " + pad_left(avg_ns_int.to_string(), 16)
            + " | " + pad_left(format_fixed_2(avg_ns_per_op), 16)
            + " |"
        );

        index += 1;
    }
}

// 场景: 等待上电后的首个运算阶段开始.
add_marker("上电");
let startup_ns = run_to(P34, UP, 2_000_000);
add_marker("第1轮_u8加法_上升沿");
assert_in(startup_ns, 0..=2_000_000, "上电后应很快进入第 1 个运算阶段");

// 场景: 测量第 1 轮完整阶段序列.
let cycle0 = measure_cycle("第1轮", true, phase_names);

// 场景: 再测量第 2 轮, 验证相同阶段的耗时稳定性.
let cycle1 = measure_cycle("第2轮", false, phase_names);

// 场景: 逐阶段比较两轮脉宽, 防止实现顺序正确但耗时抖动过大.
let index = 0;
while index < len(phase_names) {
    assert_repeat_ns(cycle1[index], cycle0[index], phase_names[index]);
    index += 1;
}

print_summary_table(cycle0, cycle1, phase_names, unit_op_counts);
