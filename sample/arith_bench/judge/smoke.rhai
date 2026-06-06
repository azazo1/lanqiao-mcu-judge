let phase_names = [
    "u8_mix",
    "char_mix",
    "int_mix",
    "uint_mix",
    "long_mix",
    "ulong_mix",
    "float_add",
    "float_sub",
    "float_mul",
    "float_div",
    "double_add",
    "double_sub",
    "double_mul",
    "double_div",
];

let unit_op_counts = [
    0,
    0,
    0,
    0,
    0,
    0,
    384,
    384,
    384,
    384,
    384,
    384,
    384,
    384,
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

fn report_unit_fp_ns(phase, pulse_ns, unit_ops) {
    if unit_ops > 0 {
        let ns_per_op = pulse_ns.to_float() / unit_ops.to_float();
        print(phase + " pulse_ns");
        print(pulse_ns);
        print(phase + " ns_per_op");
        print(ns_per_op);
    }
}

fn measure_cycle(cycle_label, first_phase_already_high) {
    let samples = [];
    let index = 0;

    while index < len(phase_names) {
        let phase = phase_names[index];

        if index > 0 || !first_phase_already_high {
            let rise_ns = run_to(P34, UP, 20_000_000);
            assert_gap_ns(rise_ns, cycle_label + " " + phase + " 前");
        }

        add_marker(cycle_label + "_" + phase + "_up");
        let pulse_ns = run_to(P34, DOWN, 800_000_000);
        add_marker(cycle_label + "_" + phase + "_down");
        assert_phase_ns(pulse_ns, cycle_label + " " + phase);
        report_unit_fp_ns(phase, pulse_ns, unit_op_counts[index]);
        samples.push(pulse_ns);

        index += 1;
    }

    samples
}

// 场景: 等待上电后的首个 bench 阶段开始.
add_marker("boot");
let startup_ns = run_to(P34, UP, 2_000_000);
add_marker("cycle0_u8_mix_up");
assert_in(startup_ns, 0..=2_000_000, "上电后应很快进入第 1 个 bench 阶段");

// 场景: 测量第 1 轮完整阶段序列.
let cycle0 = measure_cycle("cycle0", true);

// 场景: 再测量第 2 轮, 验证相同阶段的耗时稳定性.
let cycle1 = measure_cycle("cycle1", false);

// 场景: 逐阶段比较两轮脉宽, 防止实现顺序正确但耗时抖动过大.
let index = 0;
while index < len(phase_names) {
    assert_repeat_ns(cycle1[index], cycle0[index], phase_names[index]);
    index += 1;
}
