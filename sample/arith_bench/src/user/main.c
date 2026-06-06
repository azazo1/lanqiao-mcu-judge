#include <STC15F2K60S2.H>

#define LOOP_INT 120
#define LOOP_LONG 80
#define LOOP_FP 96

sbit bench_pin = P3^4;

volatile unsigned char sink_u8 = 0;
volatile char sink_char = 0;
volatile int sink_int = 0;
volatile unsigned int sink_uint = 0;
volatile long sink_long = 0;
volatile unsigned long sink_ulong = 0;
volatile float sink_float = 0.0f;
volatile double sink_double = 0.0;

void delay_gap(void) {
    unsigned char i;
    unsigned char j;

    i = 32;
    j = 180;
    do {
        while (--j) {
        }
        j = 180;
    } while (--i);
}

void bench_u8_mix(void) {
    unsigned char i;
    unsigned char a = 17;
    unsigned char b = 29;
    unsigned char c = 7;
    unsigned char d = 27;
    unsigned char e = 19;

    for (i = 0; i < LOOP_INT; i++) {
        a = (unsigned char)(a + 3);
        b = (unsigned char)(b - 2);
        c = (unsigned char)(c * 3);
        d = (unsigned char)(d / 3 + 1);
        e = (unsigned char)((e % 11) + a);
    }

    sink_u8 = (unsigned char)(a ^ b ^ c ^ d ^ e);
}

void bench_char_mix(void) {
    unsigned char i;
    char a = 17;
    char b = 29;
    char c = 7;
    char d = 27;
    char e = 19;

    for (i = 0; i < LOOP_INT; i++) {
        a = (char)((a + 3) % 90);
        b = (char)((b - 2 + 90) % 90);
        c = (char)((c * 3) % 90);
        d = (char)((d / 3) + 1);
        e = (char)((e % 11) + 1);
    }

    sink_char = (char)(a + b - c + d - e);
}

void bench_int_mix(void) {
    unsigned char i;
    int a = 301;
    int b = 509;
    int c = 71;
    int d = 37;
    int e = 23;

    for (i = 0; i < LOOP_INT; i++) {
        a = (a + 103) % 5000;
        b = (b - 77 + 5000) % 5000;
        c = (c * 3) % 5000;
        d = (d / 3) + 1;
        e = (e % 31) + 7;
    }

    sink_int = a + b - c + d - e;
}

void bench_uint_mix(void) {
    unsigned char i;
    unsigned int a = 301U;
    unsigned int b = 509U;
    unsigned int c = 71U;
    unsigned int d = 37U;
    unsigned int e = 23U;

    for (i = 0; i < LOOP_INT; i++) {
        a = (a + 103U) % 12000U;
        b = (b + 12000U - 77U) % 12000U;
        c = (c * 3U) % 12000U;
        d = (d / 3U) + 1U;
        e = (e % 31U) + 7U;
    }

    sink_uint = a + b + c + d + e;
}

void bench_long_mix(void) {
    unsigned char i;
    long a = 100003L;
    long b = 200009L;
    long c = 7001L;
    long d = 3701L;
    long e = 2303L;

    for (i = 0; i < LOOP_LONG; i++) {
        a = (a + 103L) % 200000000L;
        b = (b - 77L + 200000000L) % 200000000L;
        c = (c * 3L) % 200000000L;
        d = (d / 3L) + 1L;
        e = (e % 31L) + 7L;
    }

    sink_long = a + b - c + d - e;
}

void bench_ulong_mix(void) {
    unsigned char i;
    unsigned long a = 100003UL;
    unsigned long b = 200009UL;
    unsigned long c = 7001UL;
    unsigned long d = 3701UL;
    unsigned long e = 2303UL;

    for (i = 0; i < LOOP_LONG; i++) {
        a = (a + 103UL) % 400000000UL;
        b = (b + 400000000UL - 77UL) % 400000000UL;
        c = (c * 3UL) % 400000000UL;
        d = (d / 3UL) + 1UL;
        e = (e % 31UL) + 7UL;
    }

    sink_ulong = a + b + c + d + e;
}

void bench_float_add(void) {
    unsigned char i;
    float a = 0.125f;
    float b = 0.25f;
    float c = 0.375f;
    float d = 0.5f;

    for (i = 0; i < LOOP_FP; i++) {
        a = a + 0.0001f;
        b = b + 0.0002f;
        c = c + 0.0003f;
        d = d + 0.0004f;
    }

    sink_float = a + b + c + d;
}

void bench_float_sub(void) {
    unsigned char i;
    float a = 1.125f;
    float b = 1.25f;
    float c = 1.375f;
    float d = 1.5f;

    for (i = 0; i < LOOP_FP; i++) {
        a = a - 0.0001f;
        b = b - 0.0002f;
        c = c - 0.0003f;
        d = d - 0.0004f;
    }

    sink_float = a + b + c + d;
}

void bench_float_mul(void) {
    unsigned char i;
    float a = 0.875f;
    float b = 1.125f;
    float c = 0.9375f;
    float d = 1.0625f;

    for (i = 0; i < LOOP_FP; i++) {
        a = a * 1.0001f;
        b = b * 0.9999f;
        c = c * 1.0002f;
        d = d * 0.9998f;
    }

    sink_float = a + b + c + d;
}

void bench_float_div(void) {
    unsigned char i;
    float a = 1.875f;
    float b = 1.625f;
    float c = 1.4375f;
    float d = 1.3125f;

    for (i = 0; i < LOOP_FP; i++) {
        a = a / 1.0001f;
        b = b / 1.0002f;
        c = c / 1.0003f;
        d = d / 1.0004f;
    }

    sink_float = a + b + c + d;
}

void bench_double_add(void) {
    unsigned char i;
    double a = 0.125;
    double b = 0.25;
    double c = 0.375;
    double d = 0.5;

    for (i = 0; i < LOOP_FP; i++) {
        a = a + 0.0001;
        b = b + 0.0002;
        c = c + 0.0003;
        d = d + 0.0004;
    }

    sink_double = a + b + c + d;
}

void bench_double_sub(void) {
    unsigned char i;
    double a = 1.125;
    double b = 1.25;
    double c = 1.375;
    double d = 1.5;

    for (i = 0; i < LOOP_FP; i++) {
        a = a - 0.0001;
        b = b - 0.0002;
        c = c - 0.0003;
        d = d - 0.0004;
    }

    sink_double = a + b + c + d;
}

void bench_double_mul(void) {
    unsigned char i;
    double a = 0.875;
    double b = 1.125;
    double c = 0.9375;
    double d = 1.0625;

    for (i = 0; i < LOOP_FP; i++) {
        a = a * 1.0001;
        b = b * 0.9999;
        c = c * 1.0002;
        d = d * 0.9998;
    }

    sink_double = a + b + c + d;
}

void bench_double_div(void) {
    unsigned char i;
    double a = 1.875;
    double b = 1.625;
    double c = 1.4375;
    double d = 1.3125;

    for (i = 0; i < LOOP_FP; i++) {
        a = a / 1.0001;
        b = b / 1.0002;
        c = c / 1.0003;
        d = d / 1.0004;
    }

    sink_double = a + b + c + d;
}

void main(void) {
    bench_pin = 0;

    while (1) {
        bench_pin = 1;
        bench_u8_mix();
        bench_pin = 0;
        delay_gap();

        bench_pin = 1;
        bench_char_mix();
        bench_pin = 0;
        delay_gap();

        bench_pin = 1;
        bench_int_mix();
        bench_pin = 0;
        delay_gap();

        bench_pin = 1;
        bench_uint_mix();
        bench_pin = 0;
        delay_gap();

        bench_pin = 1;
        bench_long_mix();
        bench_pin = 0;
        delay_gap();

        bench_pin = 1;
        bench_ulong_mix();
        bench_pin = 0;
        delay_gap();

        bench_pin = 1;
        bench_float_add();
        bench_pin = 0;
        delay_gap();

        bench_pin = 1;
        bench_float_sub();
        bench_pin = 0;
        delay_gap();

        bench_pin = 1;
        bench_float_mul();
        bench_pin = 0;
        delay_gap();

        bench_pin = 1;
        bench_float_div();
        bench_pin = 0;
        delay_gap();

        bench_pin = 1;
        bench_double_add();
        bench_pin = 0;
        delay_gap();

        bench_pin = 1;
        bench_double_sub();
        bench_pin = 0;
        delay_gap();

        bench_pin = 1;
        bench_double_mul();
        bench_pin = 0;
        delay_gap();

        bench_pin = 1;
        bench_double_div();
        bench_pin = 0;
        delay_gap();
    }
}
