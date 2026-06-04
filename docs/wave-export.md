# 波形导出

`stcjudge` 可以在正常执行 `run`, `repl`, `dump` 时附带导出波形, 不需要切到单独的 probe 入口.

## CLI 参数

- `--wave-html <path>`: 导出单文件 HTML 查看器.
- `--wave-json <path>`: 导出原始 JSON 数据.
- `--wave-msgpack <path>`: 导出原始二进制 MessagePack 数据.
- `--wave-start <time>`: 仅保留从该时刻开始的波形. 支持 `ns/us/ms/s` 后缀, 例如 `1ns`, `250us`, `12ms`, `1s`. 不带后缀时仍按 `ns` 解释.
- `--wave-end <time>`: 仅保留到该时刻结束的波形. 支持 `ns/us/ms/s` 后缀, 不带后缀时仍按 `ns` 解释.

可以同时导出 HTML, JSON 和 MessagePack. 例如:

```bash
stcjudge run \
  --hex sample/led_pwm/prj/Objects/led_pwm.hex \
  --stdin \
  --wave-start 20ms \
  --wave-end 180ms \
  --wave-html /tmp/led_pwm_wave.html \
  --wave-json /tmp/led_pwm_wave.json \
  --wave-msgpack /tmp/led_pwm_wave.msgpack <<'EOF'
run_ms(100);
tap_key(S9, 80);
run_ms(100);
EOF
```

## 先看调试时间, 再裁剪波形

如果还不确定要抓哪一段时间窗, 可以先用 `RUST_LOG=debug` 跑一次脚本. 评测脚本逐语句日志里现在会带 `sim_time_ns`, 也就是该时刻的仿真时间, 单位为 `ns`.

一个简单流程如下:

1. 先执行:

```bash
RUST_LOG=debug stcjudge run \
  --hex sample/led_pwm/prj/Objects/led_pwm.hex \
  --script sample/led_pwm/judge/smoke.rhai
```

2. 在日志里找到目标现象附近的 `sim_time_ns`, 比如某次按键后显示异常, 某次 `IIC` 读写, 某次中断进入.
3. 围绕这个时间留一点前后余量, 例如前后各 `100us` 或 `1ms`.
4. 再执行一次并导出波形:

```bash
stcjudge run \
  --hex sample/led_pwm/prj/Objects/led_pwm.hex \
  --script sample/led_pwm/judge/smoke.rhai \
  --wave-start 12.4ms \
  --wave-end 13.6ms \
  --wave-html /tmp/led_pwm_wave.html
```

如果日志里看到的是 `12400000 ns`, 那么可以直接写成 `--wave-start 12400000`, 也可以换成更容易读写的 `12.4ms`. 两种写法等价.

## 查看器特性

HTML 查看器是一个自包含文件, 直接用浏览器打开即可.

- HTML 内部默认嵌入的是 base64 编码的 MessagePack 载荷, 而不是原始大 JSON. 这样导出体积更小, 浏览器加载时的字符串解析开销也更低.
- `--wave-msgpack` 导出的原始二进制和 HTML 内嵌载荷使用同一份结构定义, 适合后续脚本化处理或接到其他可视化工具.

- 左侧按 `category / group` 分类列出信号, 可以自由勾选组合显示.
- 搜索框会同时过滤侧边栏和主视图中的信号. 对未勾选但命中的信号会以 preview 形式临时显示.
- 搜索同时支持 alias. 例如 `i2c` / `iic`, `uart1` / `serial1`, `onewire` / `1-wire`, `adc` / `ad`, `dac` / `da` 都可以互相命中.
- 左侧筛选区支持收起和再次展开, 收起后主波形区会自动扩宽并立即重绘.
- 顶部工具栏提供预设快速筛选下拉框, 可以一键切到 `IIC / UART / LED / Keys / SEG / ADC/DAC / 1-Wire / CPU / Pins` 等常用视图. 手动勾选到不再对应某个整套预设时, 下拉框会显示 `Custom`.
- 协议类预设不是只保留单一协议轨. 例如 `IIC / UART / ADC/DAC / 1-Wire` 这类预设会同时带上 `cpu events` 和相关端口上下文, 方便直接排查完整时序.
- 预设切换时还会自动调整信号排序. 一般会把更高层的语义轨放在前面, 例如协议事件, 协议抽象波形, 数码管显示内容, 字节级端口值, 最后才是具体 pin 和 latch bit, 方便从现象一路看到底层细节.
- 在预设基础上手动隐藏或勾选信号时, 查看器会切到 `Custom`, 但会继承当前预设下的显示顺序继续编辑, 不会因为进入 `Custom` 就突然打乱整页排序.
- 支持滚轮缩放, 鼠标拖拽平移, 并可通过预设下拉框快速恢复默认可见轨道或切到其他常用信号集合.
  `Shift + 滚轮` 平移当前时间窗, `Alt + 滚轮` 以当前视窗中心缩放.
  在信号区还支持 `Ctrl + 滚轮` 按光标位置缩放, 以及鼠标中键拖动平移.
  主信号区的无修饰键普通滚轮会保持浏览器默认的上下滚动.
- 信号区每一行最左侧都有一个拖动手柄区域, 可以直接拖动调整信号显示顺序.
- 信号区每一行都可以直接点击标签区右侧的小圆减号, 效果等同于左侧取消勾选该信号.
- 顶部工具栏会固定在视口中, 当前时间范围不会随着波形区滚动而移出视野.
- 顶部还会显示一条和信号区同宽的矩形 coverage 范围条, 用来拖边界, 平移和缩放当前观察窗口.
- viewer 顶部支持临时 marker. 可以输入时间和可选标签来添加, 也可以使用当前悬停 cursor 时间来添加.
- marker 会同时显示在主信号区和 minimap 中. 需要先点击聚焦 active marker, 之后再拖动移动位置, 也可以点击列表项聚焦并直接删除.
- 在主信号区悬停时按 `m` 键, 可以直接在当前鼠标时间点添加 marker.
- Rhai 脚本也可以通过 `add_marker(...)` 预先写入导出 marker. 这些 marker 会作为 viewer 打开时的初始 marker 载入.
- 前端手动增删拖动的 marker 仍然只存在于当前查看器页面中, 不会回写导出的 HTML, JSON 或 MessagePack 数据.
- 左侧筛选区和右侧波形区各自独立滚动, 页面本身不会再出现整体滚动条.
- 鼠标悬停时会显示当前时间点的信号值, 对事件轨则显示事件标签和细节; 不同事件会使用不同颜色区分.
- 时间轴会按视图跨度自动切换 `ns/us/ms/s`, 并使用动态刻度间隔.
- `seg.text` 和 `seg.d1.text .. seg.d8.text` 会直接按字符显示, 适合观察数码管刷新结果.

## 主要信号分类

常见分类如下:

- `protocol / i2c`: `i2c.bus_scl`, `i2c.bus_sda`, `event.i2c`.
- `protocol / onewire`: `onewire.bus_high`, `event.onewire`.
- `protocol / uart1`, `protocol / uart2`: `uart1.tx`, `uart1.rx`, `event.uart1`, `event.uart2`.
- `protocol / ds1302`: `ds1302.ce`, `ds1302.clk`, `ds1302.io`, `event.ds1302`.
- `pins / Pn pins`: 各 MCU 引脚的实际电平, 形如 `pin.p3.4`.
- `port_latches / Pn latches`: 端口锁存器值, 形如 `latch.p2.1`.
- `board_latches`: 板级锁存的有效值, 端口写入值和 XDATA 镜像值.
- `board_signals / jumpers`: 板上关键信号和跳帽状态, 例如 `signal.sig_out`, `jumper.net_sig_sig_out`.
- `keys / matrix_keys`: 按键本体的按下状态, 例如 `key.s4`, `key.s19`.
- `outputs / leds`: `led.l1 .. led.l8`.
- `outputs / board_outputs`: `output.relay`, `output.motor`, `output.buzzer`.
- `display / seg`: `seg.text`.
- `display / seg_digits`: 每一位的字符, 例如 `seg.d1.text`.
- `display / seg_raw`: 每一位的原始段码, 例如 `seg.d1.raw`.
- `analog / pcf8591`: ADC/DAC 相关数值和事件, 包括 `pcf8591.adc_code`, `pcf8591.dac_v`, `event.adc_dac`.
- `analog / pcf8591_ne555`: 板上模拟量和 NE555, 包括 `analog.rd1_v`, `analog.rb2_v`, `ne555.level`, `ne555.frequency_hz`.
- `cpu / interrupts`: 当前仅输出 `event.cpu`, 用来标记中断进入时刻.

## 事件轨语义

事件轨只做语义标注, 不承载持续电平.

- `event.i2c`: `START`, `REPEATED START`, `STOP`, `ADDR`, `TX`, `RX`, `ACK`, `NACK`.
  其中 `ADDR` 会保留总线首字节的原始值, 例如 `ADDR 0xA0 W`, 不会右移掉 R/W bit.
- `event.onewire`: 复位, ROM command, Function command, 发出字节, 发送温度数据等.
- `event.uart1`, `event.uart2`: UART 收发帧起点.
- `event.adc_dac`: PCF8591 控制字, ADC 采样, DAC 写入.
- `event.ds1302`: DS1302 命令, 读寄存器, 写寄存器.
- `event.cpu`: 中断入口, 如 `T0 enter`, `T1 enter`, `UART enter`.

## JSON 结构

JSON 顶层字段:

- `start_ns`
- `end_ns`
- `signals`
- `samples`
- `events`
- `markers`

其中:

- `signals` 描述每条轨道的元信息, 包括 `id`, `label`, `category`, `group`, `aliases`, `kind`, `format`, `default_visible`, `unit`.
- `samples[signal_id]` 是该轨道的变化点数组, 每项形如 `{ "t": 123, "v": ... }`.
- `events` 是事件数组, 每项形如 `{ "track_id": "event.i2c", "t": 456, "label": "START", "detail": null }`.
- `markers` 是 marker 数组, 每项形如 `{ "t": 789, "label": "boot" }` 或 `{ "t": 790, "label": null }`.

数字波形和文本波形都只在数值变化时记录一个点, 查看器会按阶梯方式展开.

## MessagePack 结构

`--wave-msgpack` 导出的顶层字段包括:

- `version`
- `start_ns`
- `end_ns`
- `signals`
- `samples`
- `events`
- `markers`

其中:

- `signals` 使用定长数组编码, 顺序为 `id`, `label`, `category`, `group`, `aliases`, `kind`, `format`, `unit`, `default_visible`.
- `samples` 是按 signal 顺序排列的二维数组, 每个采样点编码为 `[time_ns, value]`.
- `events` 是事件数组, 每项编码为 `[track_signal_index, time_ns, label, detail]`.
- `markers` 是 marker 数组, 每项编码为 `[time_ns, label_or_null]`.

这种格式会把字段名和 signal id 这类重复字符串集中保留在元数据中, 降低运行期和导出阶段的字符串处理开销.
