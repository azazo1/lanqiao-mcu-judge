#include "utils.h"
#include "init.h"
#include "key.h"
#include "seg.h"
#include "led.h"
#include "uart.h"
#include "us.h"
#include "pcf8591.h"
#include "eeprom.h"
#include "ds18b20.h"
#include "ds1302.h"
#include <stdio.h>
#include <string.h>

#define PAGE_CLOCK 0
#define PAGE_TEMP 1
#define PAGE_LIGHT 2
#define PAGE_DIST_FREQ 3

#define CLOCK_SUB_TIME 0
#define CLOCK_SUB_DATE 1

#define LIGHT_SUB_ADC 0
#define LIGHT_SUB_DAC 1

#define LIGHT_MODE_AUTO 0
#define LIGHT_MODE_MANUAL 1

#define EEPROM_LOCK0 0x16
#define EEPROM_LOCK1 0x71
#define EEPROM_DEFAULT_CFG 0x00
#define EEPROM_DEFAULT_SPEED_OFFSET 20
#define EEPROM_DEFAULT_MANUAL_DAC 96

#define KEY_SCAN_MS 10
#define DISPLAY_MS 50
#define SENSOR_MS 120
#define DOUBLE_CLICK_MS 260
#define SAVE_HOLD_MS 2000
#define HEARTBEAT_HALF_MS 500
#define DIST_ALARM_HALF_MS 250
#define FREQ_ALARM_HALF_MS 125
#define FREQ_WINDOW_MS 100
#define PWM_PERIOD 8

#define UART_BUF_SIZE 80

#define is_down(x) ((key_down >> ((x) - 4)) & 1)
#define is_up(x) ((key_up >> ((x) - 4)) & 1)
#define is_pressing(x) ((key_val >> ((x) - 4)) & 1)

idata u8 sg_pos = 0;
pdata u8 sg_buf[8] = {10, 10, 10, 10, 10, 10, 10, 10};
pdata u8 led_buf[8] = {0, 0, 0, 0, 0, 0, 0, 0};

idata u8 key_sd = 0;
idata u8 display_sd = 0;
idata u8 sensor_sd = 0;

idata uint key_val = 0;
idata uint key_old = 0;
idata uint key_down = 0;
idata uint key_up = 0;

idata u8 main_page = PAGE_CLOCK;
idata u8 clock_subpage = CLOCK_SUB_TIME;
idata u8 light_subpage = LIGHT_SUB_ADC;

idata u8 clock_mode_12h = 0;
idata u8 temp_precision = 0;
idata u8 light_mode = LIGHT_MODE_AUTO;
idata uint sound_speed = 340;
idata u8 manual_dac = EEPROM_DEFAULT_MANUAL_DAC;

idata float temp_c = 0.0;
idata long temp_milli = 0;
idata u8 adc_ain1 = 0;
idata u8 adc_ain3 = 0;
idata u8 effective_dac = 32;
idata uint distance_cm = 0;
idata uint fan_freq = 0;

pdata u8 rtc_time[3] = {0, 0, 0};
pdata u8 rtc_date[4] = {0, 0, 0, 0};
pdata u8 eeprom_cache[6] = {0, 0, 0, 0, 0, 0};

idata u8 heartbeat_led = 0;
idata uint heartbeat_tick = 0;
idata u8 distance_alarm_led = 0;
idata uint distance_alarm_tick = 0;
idata u8 freq_alarm_led = 0;
idata uint freq_alarm_tick = 0;
idata u8 pwm_phase = 0;

idata uint freq_window_tick = 0;

idata u8 s12_click_count = 0;
idata uint s12_click_timer = 0;
idata u8 combo_active = 0;
idata u8 combo_saved = 0;
idata uint combo_hold_ms = 0;

pdata char uart_buf[UART_BUF_SIZE] = {0};
idata u8 uart_idx = 0;
idata u8 uart_frame_ready = 0;

pdata char page_text[20] = {0};

u8 calc_checksum(u8 byte0, u8 byte1, u8 byte2, u8 byte3, u8 byte4) {
    return byte0 ^ byte1 ^ byte2 ^ byte3 ^ byte4 ^ 0x5A;
}

u8 build_config_byte() {
    return (clock_mode_12h ? 0x01 : 0x00)
        | ((temp_precision & 0x03) << 1)
        | ((light_mode & 0x01) << 3);
}

void update_effective_dac() {
    if (light_mode == LIGHT_MODE_AUTO) {
        if (adc_ain1 < adc_ain3) {
            effective_dac = 224;
        } else {
            effective_dac = 32;
        }
    } else {
        effective_dac = manual_dac;
    }
}

long quantize_temp_milli(float temp_value, u8 precision) {
    long raw_milli;
    uint step_milli;

    raw_milli = (long)(temp_value * 1000);
    if (temp_value < 0 && precision < 3) {
        return ((long)temp_value) * 1000;
    }

    if (precision == 0) {
        step_milli = 500;
    } else if (precision == 1) {
        step_milli = 250;
    } else if (precision == 2) {
        step_milli = 125;
    } else {
        return raw_milli;
    }

    return (raw_milli / step_milli) * step_milli;
}

void rebuild_eeprom_cache() {
    eeprom_cache[0] = EEPROM_LOCK0;
    eeprom_cache[1] = EEPROM_LOCK1;
    eeprom_cache[2] = build_config_byte();
    eeprom_cache[3] = sound_speed - 320;
    eeprom_cache[4] = manual_dac;
    eeprom_cache[5] = calc_checksum(
        eeprom_cache[0],
        eeprom_cache[1],
        eeprom_cache[2],
        eeprom_cache[3],
        eeprom_cache[4]
    );
}

void write_cached_params() {
    eeprom_write(0x00, eeprom_cache, 6);
}

void sync_eeprom_cache_runtime() {
    eeprom_cache[2] = build_config_byte();
    eeprom_cache[3] = sound_speed - 320;
    eeprom_cache[4] = manual_dac;
    eeprom_cache[5] = calc_checksum(
        eeprom_cache[0],
        eeprom_cache[1],
        eeprom_cache[2],
        eeprom_cache[3],
        eeprom_cache[4]
    );
}

void persist_clock_mode_only() {
    sync_eeprom_cache_runtime();
    eeprom_write(0x02, eeprom_cache + 2, 4);
}

void save_params() {
    rebuild_eeprom_cache();
    write_cached_params();
}

void apply_runtime_settings() {
    rtc_set_12h(clock_mode_12h);
    tempe_set_res(temp_precision);
    update_effective_dac();
    dac(effective_dac);
}

void load_default_params() {
    clock_mode_12h = 0;
    temp_precision = 0;
    light_mode = LIGHT_MODE_AUTO;
    sound_speed = 340;
    manual_dac = EEPROM_DEFAULT_MANUAL_DAC;
    rebuild_eeprom_cache();
    write_cached_params();
    apply_runtime_settings();
}

void load_saved_params() {
    eeprom_read(0x00, eeprom_cache, 6);
    if (eeprom_cache[0] != EEPROM_LOCK0
        || eeprom_cache[1] != EEPROM_LOCK1
        || eeprom_cache[5] != calc_checksum(
            eeprom_cache[0],
            eeprom_cache[1],
            eeprom_cache[2],
            eeprom_cache[3],
            eeprom_cache[4]
        )
        || eeprom_cache[3] > 59) {
        load_default_params();
        return;
    }

    clock_mode_12h = eeprom_cache[2] & 0x01;
    temp_precision = (eeprom_cache[2] >> 1) & 0x03;
    light_mode = (eeprom_cache[2] >> 3) & 0x01;
    sound_speed = 320 + eeprom_cache[3];
    manual_dac = eeprom_cache[4];
    sync_eeprom_cache_runtime();
    apply_runtime_settings();
}

u8 seg_char(char ch) {
    if (ch >= '0' && ch <= '9') {
        return ch - '0';
    }
    if (ch == '-') {
        return 11;
    }
    if (ch == 'P') {
        return 12;
    }
    if (ch == 'E') {
        return 13;
    }
    return 10;
}

void sg_set_text(char *text) {
    u8 i;
    u8 j;

    for (i = 0; i < 8; ++i) {
        sg_buf[i] = 10;
    }

    i = 0;
    j = 0;
    while (text[i] != 0 && j < 8) {
        if (text[i] == '.') {
            if (j > 0 && sg_buf[j - 1] < ',') {
                sg_buf[j - 1] += ',';
            }
        } else {
            sg_buf[j++] = seg_char(text[i]);
        }
        ++i;
    }
}

void build_time_page() {
    rtc_get_time(rtc_time);
    sprintf(page_text, "%02u-%02u-%02u", rtc_time[0], rtc_time[1], rtc_time[2]);
}

void build_date_page() {
    rtc_get_date(rtc_date);
    sprintf(page_text, "%02u-%02u-%02u", rtc_date[0], rtc_date[1], rtc_date[2]);
}

void build_temp_page() {
    unsigned long abs_milli;
    uint integer_part;
    uint frac_part;

    abs_milli = temp_milli >= 0 ? temp_milli : -temp_milli;
    integer_part = abs_milli / 1000;
    frac_part = abs_milli % 1000;

    if (temp_milli < 0) {
        sprintf(page_text, "-%u.%03u-%u", integer_part, frac_part, temp_precision);
    } else {
        sprintf(page_text, "%u.%03u-%u", integer_part, frac_part, temp_precision);
    }
}

void build_light_page() {
    if (light_subpage == LIGHT_SUB_ADC) {
        sprintf(page_text, "%03u-%03u", adc_ain1, adc_ain3);
    } else {
        sprintf(page_text, "%u-%03u", light_mode, effective_dac);
    }
}

void build_dist_freq_page() {
    sprintf(page_text, "%03u-%04u", distance_cm, fan_freq);
}

void display_proc() {
    if (display_sd < DISPLAY_MS) {
        return;
    }
    display_sd = 0;

    if (main_page == PAGE_CLOCK) {
        if (clock_subpage == CLOCK_SUB_TIME) {
            build_time_page();
        } else {
            build_date_page();
        }
    } else if (main_page == PAGE_TEMP) {
        build_temp_page();
    } else if (main_page == PAGE_LIGHT) {
        build_light_page();
    } else {
        build_dist_freq_page();
    }

    sg_set_text(page_text);
}

void sensor_proc() {
    if (sensor_sd < SENSOR_MS) {
        return;
    }
    sensor_sd = 0;

    temp_c = tempe_get();
    temp_milli = quantize_temp_milli(temp_c, temp_precision);
    adc_ain1 = adc(0x41);
    adc_ain3 = adc(0x43);
    update_effective_dac();
    dac(effective_dac);
    distance_cm = us_dist(sound_speed);
}

void set_clock_mode(u8 new_mode) {
    clock_mode_12h = new_mode ? 1 : 0;
    rtc_set_12h(clock_mode_12h);
}

void handle_s12_double() {
    if (main_page == PAGE_CLOCK) {
        set_clock_mode(!clock_mode_12h);
        persist_clock_mode_only();
    } else if (main_page == PAGE_LIGHT) {
        light_mode = !light_mode;
        sync_eeprom_cache_runtime();
        update_effective_dac();
        dac(effective_dac);
    }
}

void handle_short_keys() {
    if (is_down(4)) {
        main_page = (main_page + 1) % 4;
    }

    if (is_down(5)) {
        if (main_page == PAGE_CLOCK) {
            clock_subpage = !clock_subpage;
        } else if (main_page == PAGE_LIGHT) {
            light_subpage = !light_subpage;
        }
    }

    if (is_down(8)) {
        if (main_page == PAGE_TEMP) {
            temp_precision = (temp_precision + 1) % 4;
            tempe_set_res(temp_precision);
            sync_eeprom_cache_runtime();
        } else if (main_page == PAGE_LIGHT
            && light_subpage == LIGHT_SUB_DAC
            && light_mode == LIGHT_MODE_MANUAL) {
            if (manual_dac <= 247) {
                manual_dac += 8;
            } else {
                manual_dac = 255;
            }
            sync_eeprom_cache_runtime();
            update_effective_dac();
            dac(effective_dac);
        }
    }

    if (is_down(9)) {
        if (main_page == PAGE_TEMP) {
            temp_precision = (temp_precision + 3) % 4;
            tempe_set_res(temp_precision);
            sync_eeprom_cache_runtime();
        } else if (main_page == PAGE_LIGHT
            && light_subpage == LIGHT_SUB_DAC
            && light_mode == LIGHT_MODE_MANUAL) {
            if (manual_dac >= 8) {
                manual_dac -= 8;
            } else {
                manual_dac = 0;
            }
            sync_eeprom_cache_runtime();
            update_effective_dac();
            dac(effective_dac);
        }
    }
}

void process_s12_state() {
    if (is_pressing(12) && is_pressing(13)) {
        if (!combo_active) {
            combo_active = 1;
            combo_saved = 0;
            combo_hold_ms = 0;
            s12_click_count = 0;
            s12_click_timer = 0;
        } else if (!combo_saved) {
            combo_hold_ms += KEY_SCAN_MS;
            if (combo_hold_ms >= SAVE_HOLD_MS) {
                save_params();
                combo_saved = 1;
            }
        }
        return;
    }

    if (combo_active) {
        if (!is_pressing(12) && !is_pressing(13)) {
            combo_active = 0;
            combo_saved = 0;
            combo_hold_ms = 0;
        }
        return;
    }

    if (s12_click_timer > 0) {
        if (s12_click_timer > KEY_SCAN_MS) {
            s12_click_timer -= KEY_SCAN_MS;
        } else {
            s12_click_timer = 0;
            s12_click_count = 0;
        }
    }

    if (is_up(12) && !is_pressing(13)) {
        if (s12_click_count == 0) {
            s12_click_count = 1;
            s12_click_timer = DOUBLE_CLICK_MS;
        } else {
            s12_click_count = 0;
            s12_click_timer = 0;
            handle_s12_double();
        }
    }
}

void key_proc() {
    if (key_sd < KEY_SCAN_MS) {
        return;
    }
    key_sd = 0;

    key_old = key_val;
    key_val = key_read();
    key_down = key_val & (key_val ^ key_old);
    key_up = ~key_val & (key_val ^ key_old);

    process_s12_state();
    if (combo_active) {
        return;
    }
    handle_short_keys();
}

u8 is_digit_char(char ch) {
    return ch >= '0' && ch <= '9';
}

u8 parse_u8_text(char *text, uint *value) {
    uint result;
    u8 i;

    if (text[0] == 0) {
        return 0;
    }

    result = 0;
    i = 0;
    while (text[i] != 0) {
        if (!is_digit_char(text[i])) {
            return 0;
        }
        result = result * 10 + (text[i] - '0');
        ++i;
    }
    *value = result;
    return 1;
}

u8 parse_two_digits(char *text, u8 *value) {
    if (!is_digit_char(text[0]) || !is_digit_char(text[1]) || text[2] != 0) {
        return 0;
    }
    *value = (text[0] - '0') * 10 + (text[1] - '0');
    return 1;
}

u8 is_leap_year(u8 year) {
    uint full_year;

    full_year = 2000 + year;
    if ((full_year % 400) == 0) {
        return 1;
    }
    if ((full_year % 100) == 0) {
        return 0;
    }
    return (full_year % 4) == 0;
}

u8 valid_date_value(u8 year, u8 month, u8 date) {
    u8 max_date;

    if (month == 0 || month > 12 || date == 0) {
        return 0;
    }

    if (month == 2) {
        max_date = is_leap_year(year) ? 29 : 28;
    } else if (month == 4 || month == 6 || month == 9 || month == 11) {
        max_date = 30;
    } else {
        max_date = 31;
    }

    return date <= max_date;
}

u8 parse_date_text(char *text, u8 *year, u8 *month, u8 *date) {
    char part[3];

    if (text[2] != '-' || text[5] != '-' || text[8] != 0) {
        return 0;
    }

    part[2] = 0;
    part[0] = text[0];
    part[1] = text[1];
    if (!parse_two_digits(part, year)) {
        return 0;
    }

    part[0] = text[3];
    part[1] = text[4];
    if (!parse_two_digits(part, month)) {
        return 0;
    }

    part[0] = text[6];
    part[1] = text[7];
    if (!parse_two_digits(part, date)) {
        return 0;
    }

    return valid_date_value(*year, *month, *date);
}

u8 parse_time_text(char *text, u8 *hour, u8 *minute, u8 *second) {
    char part[3];

    if (text[2] != ':' || text[5] != ':' || text[8] != 0) {
        return 0;
    }

    part[2] = 0;
    part[0] = text[0];
    part[1] = text[1];
    if (!parse_two_digits(part, hour)) {
        return 0;
    }

    part[0] = text[3];
    part[1] = text[4];
    if (!parse_two_digits(part, minute)) {
        return 0;
    }

    part[0] = text[6];
    part[1] = text[7];
    if (!parse_two_digits(part, second)) {
        return 0;
    }

    if (*hour > 23 || *minute > 59 || *second > 59) {
        return 0;
    }
    return 1;
}

u8 split_tokens(char *text, char **tokens, u8 max_tokens) {
    u8 count;
    u8 i;

    if (text[0] == 0) {
        return 0;
    }

    count = 1;
    tokens[0] = text;
    i = 0;
    while (text[i] != 0) {
        if (text[i] == '|') {
            text[i] = 0;
            if (count >= max_tokens || text[i + 1] == 0) {
                return 0;
            }
            tokens[count++] = text + i + 1;
        }
        ++i;
    }
    return count;
}

void uart_reply_cfg() {
    printf(
        "$CFG|CLK:%u|TP:%u|LM:%u|SV:%u|DAC:%03u;",
        clock_mode_12h ? 12 : 24,
        temp_precision,
        light_mode,
        sound_speed,
        manual_dac
    );
}

void uart_reply_env() {
    unsigned long abs_milli;
    uint integer_part;
    uint frac_part;

    abs_milli = temp_milli >= 0 ? temp_milli : -temp_milli;
    integer_part = abs_milli / 1000;
    frac_part = abs_milli % 1000;

    printf(
        "$ENV|TMP:%c%u.%03u|L:%03u|VR:%03u|DIST:%03u|FREQ:%04u;",
        temp_milli < 0 ? '-' : '+',
        integer_part,
        frac_part,
        adc_ain1,
        adc_ain3,
        distance_cm,
        fan_freq
    );
}

void uart_reply_frame_error() {
    printf("$ERR|FRAME;");
}

void uart_reply_value_error() {
    printf("$ERR|VALUE;");
}

void uart_reply_set_ok() {
    printf("$OK|SET;");
}

void uart_reply_rtc_ok() {
    printf("$OK|RTC;");
}

u8 apply_set_token(
    char *token,
    u8 *new_clock_mode,
    u8 *new_precision,
    u8 *new_light_mode,
    uint *new_speed,
    u8 *new_manual_dac
) {
    uint value;

    if (strncmp(token, "CLK:", 4) == 0) {
        if (strcmp(token + 4, "12") == 0) {
            *new_clock_mode = 1;
            return 1;
        }
        if (strcmp(token + 4, "24") == 0) {
            *new_clock_mode = 0;
            return 1;
        }
        return 0;
    }

    if (strncmp(token, "TP:", 3) == 0) {
        if (!parse_u8_text(token + 3, &value) || value > 3) {
            return 0;
        }
        *new_precision = value;
        return 1;
    }

    if (strncmp(token, "LM:", 3) == 0) {
        if (!parse_u8_text(token + 3, &value) || value > 1) {
            return 0;
        }
        *new_light_mode = value;
        return 1;
    }

    if (strncmp(token, "SV:", 3) == 0) {
        if (!parse_u8_text(token + 3, &value) || value < 320 || value > 379) {
            return 0;
        }
        *new_speed = value;
        return 1;
    }

    if (strncmp(token, "DAC:", 4) == 0) {
        if (!parse_u8_text(token + 4, &value) || value > 255) {
            return 0;
        }
        *new_manual_dac = value;
        return 1;
    }

    return 0;
}

u8 process_set_frame(char **tokens, u8 token_count) {
    u8 new_clock_mode;
    u8 new_precision;
    u8 new_light_mode;
    uint new_speed;
    u8 new_manual_dac;
    u8 i;

    if (token_count < 2) {
        return 0;
    }

    new_clock_mode = clock_mode_12h;
    new_precision = temp_precision;
    new_light_mode = light_mode;
    new_speed = sound_speed;
    new_manual_dac = manual_dac;

    for (i = 1; i < token_count; ++i) {
        if (!apply_set_token(
            tokens[i],
            &new_clock_mode,
            &new_precision,
            &new_light_mode,
            &new_speed,
            &new_manual_dac
        )) {
            return 0;
        }
    }

    set_clock_mode(new_clock_mode);
    temp_precision = new_precision;
    tempe_set_res(temp_precision);
    light_mode = new_light_mode;
    sound_speed = new_speed;
    manual_dac = new_manual_dac;
    sync_eeprom_cache_runtime();
    update_effective_dac();
    dac(effective_dac);
    return 1;
}

u8 process_rtc_frame(char **tokens, u8 token_count) {
    u8 year;
    u8 month;
    u8 date;
    u8 hour;
    u8 minute;
    u8 second;
    u8 has_date;
    u8 has_time;
    u8 i;
    u8 new_date[4];
    u8 new_time[3];

    if (token_count < 3) {
        return 0;
    }

    has_date = 0;
    has_time = 0;
    year = 0;
    month = 0;
    date = 0;
    hour = 0;
    minute = 0;
    second = 0;

    for (i = 1; i < token_count; ++i) {
        if (strncmp(tokens[i], "DATE:", 5) == 0) {
            if (!parse_date_text(tokens[i] + 5, &year, &month, &date)) {
                return 0;
            }
            has_date = 1;
        } else if (strncmp(tokens[i], "TIME:", 5) == 0) {
            if (!parse_time_text(tokens[i] + 5, &hour, &minute, &second)) {
                return 0;
            }
            has_time = 1;
        } else {
            return 0;
        }
    }

    if (!has_date || !has_time) {
        return 0;
    }

    new_date[0] = year;
    new_date[1] = month;
    new_date[2] = date;
    new_date[3] = 1;

    new_time[0] = hour;
    new_time[1] = minute;
    new_time[2] = second;

    rtc_set_12h(0);
    rtc_set_date(new_date);
    rtc_set_time(new_time);
    if (clock_mode_12h) {
        rtc_set_12h(1);
    }
    return 1;
}

void process_uart_frame() {
    char *tokens[8];
    u8 token_count;

    if (!uart_frame_ready) {
        return;
    }
    uart_frame_ready = 0;

    if (uart_idx < 2 || uart_buf[0] != '$' || uart_buf[uart_idx - 1] != ';') {
        uart_reply_frame_error();
        uart_idx = 0;
        uart_buf[0] = 0;
        return;
    }

    uart_buf[uart_idx - 1] = 0;
    token_count = split_tokens(uart_buf + 1, tokens, 8);
    if (token_count == 0) {
        uart_reply_frame_error();
    } else if (strcmp(tokens[0], "Q") == 0) {
        if (token_count != 2) {
            uart_reply_frame_error();
        } else if (strcmp(tokens[1], "CFG") == 0) {
            uart_reply_cfg();
        } else if (strcmp(tokens[1], "ENV") == 0) {
            uart_reply_env();
        } else {
            uart_reply_value_error();
        }
    } else if (strcmp(tokens[0], "SET") == 0) {
        if (process_set_frame(tokens, token_count)) {
            uart_reply_set_ok();
        } else {
            uart_reply_value_error();
        }
    } else if (strcmp(tokens[0], "RTC") == 0) {
        if (process_rtc_frame(tokens, token_count)) {
            uart_reply_rtc_ok();
        } else {
            uart_reply_value_error();
        }
    } else {
        uart_reply_value_error();
    }

    uart_idx = 0;
    uart_buf[0] = 0;
}

void refresh_leds() {
    u8 pwm_high_steps;
    u8 dist_alarm;
    u8 freq_alarm;

    led_buf[0] = main_page == PAGE_CLOCK;
    led_buf[1] = main_page == PAGE_TEMP;
    led_buf[2] = main_page == PAGE_LIGHT;
    led_buf[3] = main_page == PAGE_DIST_FREQ;
    led_buf[4] = heartbeat_led;

    pwm_high_steps = effective_dac == 255 ? PWM_PERIOD : (effective_dac * PWM_PERIOD) / 255;
    led_buf[5] = pwm_high_steps > pwm_phase;

    dist_alarm = distance_cm < 15 || distance_cm > 70;
    if (dist_alarm) {
        led_buf[6] = distance_alarm_led;
    } else {
        led_buf[6] = 0;
        distance_alarm_led = 0;
        distance_alarm_tick = 0;
    }

    freq_alarm = fan_freq < 1000 || fan_freq > 2000;
    if (freq_alarm) {
        led_buf[7] = freq_alarm_led;
    } else {
        led_buf[7] = 0;
        freq_alarm_led = 0;
        freq_alarm_tick = 0;
    }
}

void Timer0_Init(void) {
    TMOD &= 0xF0;
    TMOD |= 0x05;
    TL0 = 0x00;
    TH0 = 0x00;
    TF0 = 0;
    TR0 = 1;
}

void Timer1_Init(void) {
    AUXR &= 0xBF;
    TMOD &= 0x0F;
    TL1 = 0x18;
    TH1 = 0xFC;
    TF1 = 0;
    TR1 = 1;
    ET1 = 1;
    EA = 1;
}

void Uart1_Isr(void) interrupt 4 {
    if (RI) {
        RI = 0;
        if (uart_idx < UART_BUF_SIZE - 1) {
            uart_buf[uart_idx++] = SBUF;
            uart_buf[uart_idx] = 0;
            if (SBUF == ';') {
                uart_frame_ready = 1;
            }
        } else {
            uart_idx = 0;
            uart_buf[0] = 0;
        }
    }
}

void Timer1_Isr(void) interrupt 3 {
    ++key_sd;
    ++display_sd;
    ++sensor_sd;

    if (++heartbeat_tick >= HEARTBEAT_HALF_MS) {
        heartbeat_tick = 0;
        heartbeat_led = !heartbeat_led;
    }

    if (distance_cm < 15 || distance_cm > 70) {
        if (++distance_alarm_tick >= DIST_ALARM_HALF_MS) {
            distance_alarm_tick = 0;
            distance_alarm_led = !distance_alarm_led;
        }
    }

    if (fan_freq < 1000 || fan_freq > 2000) {
        if (++freq_alarm_tick >= FREQ_ALARM_HALF_MS) {
            freq_alarm_tick = 0;
            freq_alarm_led = !freq_alarm_led;
        }
    }

    if (++pwm_phase >= PWM_PERIOD) {
        pwm_phase = 0;
    }

    if (++freq_window_tick >= FREQ_WINDOW_MS) {
        freq_window_tick = 0;
        fan_freq = ((TH0 << 8) | TL0) * (1000 / FREQ_WINDOW_MS);
        TH0 = 0;
        TL0 = 0;
    }

    refresh_leds();

    if (++sg_pos == 8) {
        sg_pos = 0;
    }
    if (sg_buf[sg_pos] >= ',') {
        sg_disp(sg_pos, sg_buf[sg_pos] - ',', 1);
    } else {
        sg_disp(sg_pos, sg_buf[sg_pos], 0);
    }

    led_disp(led_buf);
}

void main() {
    sys_init();
    Timer0_Init();
    Timer1_Init();
    Uart1_Init();

    load_saved_params();
    sensor_proc();
    display_proc();

    while (1) {
        key_proc();
        sensor_proc();
        process_uart_frame();
        display_proc();
    }
}
