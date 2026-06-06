#include <STC15F2K60S2.H>

#define LOOP_BYTE 96
#define LOOP_INT 96
#define LOOP_LONG 64
#define LOOP_FP 96

#define RUN_STAGE(fn) \
    bench_pin = 1; \
    fn(); \
    bench_pin = 0; \
    delay_gap();

#define DEFINE_BENCH_U8(name, setup, init_a, init_b, init_c, init_d, expr_a, expr_b, expr_c, expr_d, sink_expr) \
void name(void) { \
    unsigned char i; \
    unsigned char a = init_a; \
    unsigned char b = init_b; \
    unsigned char c = init_c; \
    unsigned char d = init_d; \
    setup \
    for (i = 0; i < LOOP_BYTE; i++) { \
        a = (unsigned char)(expr_a); \
        b = (unsigned char)(expr_b); \
        c = (unsigned char)(expr_c); \
        d = (unsigned char)(expr_d); \
    } \
    sink_u8 = (unsigned char)(sink_expr); \
}

#define DEFINE_BENCH_CHAR(name, setup, init_a, init_b, init_c, init_d, expr_a, expr_b, expr_c, expr_d, sink_expr) \
void name(void) { \
    unsigned char i; \
    char a = init_a; \
    char b = init_b; \
    char c = init_c; \
    char d = init_d; \
    setup \
    for (i = 0; i < LOOP_BYTE; i++) { \
        a = (char)(expr_a); \
        b = (char)(expr_b); \
        c = (char)(expr_c); \
        d = (char)(expr_d); \
    } \
    sink_char = (char)(sink_expr); \
}

#define DEFINE_BENCH_INT(name, setup, init_a, init_b, init_c, init_d, expr_a, expr_b, expr_c, expr_d, sink_expr) \
void name(void) { \
    unsigned char i; \
    int a = init_a; \
    int b = init_b; \
    int c = init_c; \
    int d = init_d; \
    setup \
    for (i = 0; i < LOOP_INT; i++) { \
        a = (int)(expr_a); \
        b = (int)(expr_b); \
        c = (int)(expr_c); \
        d = (int)(expr_d); \
    } \
    sink_int = (int)(sink_expr); \
}

#define DEFINE_BENCH_UINT(name, setup, init_a, init_b, init_c, init_d, expr_a, expr_b, expr_c, expr_d, sink_expr) \
void name(void) { \
    unsigned char i; \
    unsigned int a = init_a; \
    unsigned int b = init_b; \
    unsigned int c = init_c; \
    unsigned int d = init_d; \
    setup \
    for (i = 0; i < LOOP_INT; i++) { \
        a = (unsigned int)(expr_a); \
        b = (unsigned int)(expr_b); \
        c = (unsigned int)(expr_c); \
        d = (unsigned int)(expr_d); \
    } \
    sink_uint = (unsigned int)(sink_expr); \
}

#define DEFINE_BENCH_LONG(name, setup, init_a, init_b, init_c, init_d, expr_a, expr_b, expr_c, expr_d, sink_expr) \
void name(void) { \
    unsigned char i; \
    long a = init_a; \
    long b = init_b; \
    long c = init_c; \
    long d = init_d; \
    setup \
    for (i = 0; i < LOOP_LONG; i++) { \
        a = (long)(expr_a); \
        b = (long)(expr_b); \
        c = (long)(expr_c); \
        d = (long)(expr_d); \
    } \
    sink_long = (long)(sink_expr); \
}

#define DEFINE_BENCH_ULONG(name, setup, init_a, init_b, init_c, init_d, expr_a, expr_b, expr_c, expr_d, sink_expr) \
void name(void) { \
    unsigned char i; \
    unsigned long a = init_a; \
    unsigned long b = init_b; \
    unsigned long c = init_c; \
    unsigned long d = init_d; \
    setup \
    for (i = 0; i < LOOP_LONG; i++) { \
        a = (unsigned long)(expr_a); \
        b = (unsigned long)(expr_b); \
        c = (unsigned long)(expr_c); \
        d = (unsigned long)(expr_d); \
    } \
    sink_ulong = (unsigned long)(sink_expr); \
}

#define DEFINE_BENCH_FLOAT(name, init_a, init_b, init_c, init_d, expr_a, expr_b, expr_c, expr_d, sink_expr) \
void name(void) { \
    unsigned char i; \
    float a = init_a; \
    float b = init_b; \
    float c = init_c; \
    float d = init_d; \
    for (i = 0; i < LOOP_FP; i++) { \
        a = (float)(expr_a); \
        b = (float)(expr_b); \
        c = (float)(expr_c); \
        d = (float)(expr_d); \
    } \
    sink_float = (float)(sink_expr); \
}

#define DEFINE_BENCH_DOUBLE(name, init_a, init_b, init_c, init_d, expr_a, expr_b, expr_c, expr_d, sink_expr) \
void name(void) { \
    unsigned char i; \
    double a = init_a; \
    double b = init_b; \
    double c = init_c; \
    double d = init_d; \
    for (i = 0; i < LOOP_FP; i++) { \
        a = (double)(expr_a); \
        b = (double)(expr_b); \
        c = (double)(expr_c); \
        d = (double)(expr_d); \
    } \
    sink_double = (double)(sink_expr); \
}

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

DEFINE_BENCH_U8(bench_u8_add, ;, 17, 29, 47, 61, a + 3, b + 5, c + 7, d + 11, a ^ b ^ c ^ d)
DEFINE_BENCH_U8(bench_u8_sub, ;, 210, 180, 150, 120, a - 3, b - 5, c - 7, d - 11, a ^ b ^ c ^ d)
DEFINE_BENCH_U8(
    bench_u8_mul,
    unsigned char m1 = 3; unsigned char m2 = 5; unsigned char m3 = 7; unsigned char m4 = 9;,
    17, 29, 47, 61,
    a * m1, b * m2, c * m3, d * m4,
    a ^ b ^ c ^ d
)
DEFINE_BENCH_U8(
    bench_u8_div,
    unsigned char q1 = 3; unsigned char q2 = 5; unsigned char q3 = 7; unsigned char q4 = 9;,
    243, 225, 217, 198,
    a / q1, b / q2, c / q3, d / q4,
    a ^ b ^ c ^ d
)
DEFINE_BENCH_U8(
    bench_u8_mod,
    unsigned char r1 = 13; unsigned char r2 = 11; unsigned char r3 = 9; unsigned char r4 = 7;,
    241, 223, 205, 187,
    a % r1, b % r2, c % r3, d % r4,
    a ^ b ^ c ^ d
)

DEFINE_BENCH_CHAR(bench_char_add, ;, -96, -64, -48, -32, a + 1, b + 1, c + 1, d + 1, a + b - c - d)
DEFINE_BENCH_CHAR(bench_char_sub, ;, 96, 64, 48, 32, a - 1, b - 1, c - 1, d - 1, a + b - c - d)
DEFINE_BENCH_CHAR(
    bench_char_mul,
    char m1 = -1; char m2 = -1; char m3 = -1; char m4 = -1;,
    63, -47, 31, -15,
    a * m1, b * m2, c * m3, d * m4,
    a + b - c - d
)
DEFINE_BENCH_CHAR(
    bench_char_div,
    char q1 = -1; char q2 = -1; char q3 = -1; char q4 = -1;,
    63, -47, 31, -15,
    a / q1, b / q2, c / q3, d / q4,
    a + b - c - d
)
DEFINE_BENCH_CHAR(
    bench_char_mod,
    char r1 = 11; char r2 = 13; char r3 = 17; char r4 = 19;,
    97, 81, 65, 49,
    a % r1, b % r2, c % r3, d % r4,
    a + b - c - d
)

DEFINE_BENCH_INT(bench_int_add, ;, 301, 509, 701, 907, a + 103, b + 97, c + 89, d + 83, a + b - c + d)
DEFINE_BENCH_INT(bench_int_sub, ;, 15001, 13009, 11003, 9001, a - 73, b - 79, c - 83, d - 89, a + b - c + d)
DEFINE_BENCH_INT(
    bench_int_mul,
    int m1 = -1; int m2 = -1; int m3 = -1; int m4 = -1;,
    3001, -2003, 1009, -509,
    a * m1, b * m2, c * m3, d * m4,
    a + b - c + d
)
DEFINE_BENCH_INT(
    bench_int_div,
    int q1 = -1; int q2 = -1; int q3 = -1; int q4 = -1;,
    3001, -2003, 1009, -509,
    a / q1, b / q2, c / q3, d / q4,
    a + b - c + d
)
DEFINE_BENCH_INT(
    bench_int_mod,
    int r1 = 97; int r2 = 89; int r3 = 83; int r4 = 79;,
    15001, 13009, 11003, 9001,
    a % r1, b % r2, c % r3, d % r4,
    a + b - c + d
)

DEFINE_BENCH_UINT(bench_uint_add, ;, 301U, 509U, 701U, 907U, a + 103U, b + 97U, c + 89U, d + 83U, a + b + c + d)
DEFINE_BENCH_UINT(bench_uint_sub, ;, 15001U, 13009U, 11003U, 9001U, a - 73U, b - 79U, c - 83U, d - 89U, a + b + c + d)
DEFINE_BENCH_UINT(
    bench_uint_mul,
    unsigned int m1 = 3U; unsigned int m2 = 5U; unsigned int m3 = 7U; unsigned int m4 = 9U;,
    17U, 29U, 47U, 61U,
    a * m1, b * m2, c * m3, d * m4,
    a + b + c + d
)
DEFINE_BENCH_UINT(
    bench_uint_div,
    unsigned int q1 = 3U; unsigned int q2 = 5U; unsigned int q3 = 7U; unsigned int q4 = 9U;,
    60000U, 50000U, 45000U, 40000U,
    a / q1, b / q2, c / q3, d / q4,
    a + b + c + d
)
DEFINE_BENCH_UINT(
    bench_uint_mod,
    unsigned int r1 = 251U; unsigned int r2 = 241U; unsigned int r3 = 239U; unsigned int r4 = 233U;,
    60000U, 50000U, 45000U, 40000U,
    a % r1, b % r2, c % r3, d % r4,
    a + b + c + d
)

DEFINE_BENCH_LONG(
    bench_long_add,
    ;,
    100003L, 200009L, 300007L, 400031L,
    a + 1009L, b + 1013L, c + 1019L, d + 1021L,
    a + b - c + d
)
DEFINE_BENCH_LONG(
    bench_long_sub,
    ;,
    2000003L, 1800001L, 1600007L, 1400009L,
    a - 911L, b - 919L, c - 929L, d - 937L,
    a + b - c + d
)
DEFINE_BENCH_LONG(
    bench_long_mul,
    long m1 = -1L; long m2 = -1L; long m3 = -1L; long m4 = -1L;,
    1000003L, -700001L, 300007L, -110003L,
    a * m1, b * m2, c * m3, d * m4,
    a + b - c + d
)
DEFINE_BENCH_LONG(
    bench_long_div,
    long q1 = -1L; long q2 = -1L; long q3 = -1L; long q4 = -1L;,
    1000003L, -700001L, 300007L, -110003L,
    a / q1, b / q2, c / q3, d / q4,
    a + b - c + d
)
DEFINE_BENCH_LONG(
    bench_long_mod,
    long r1 = 10007L; long r2 = 9001L; long r3 = 8009L; long r4 = 7001L;,
    200000003L, 180000017L, 160000001L, 140000009L,
    a % r1, b % r2, c % r3, d % r4,
    a + b - c + d
)

DEFINE_BENCH_ULONG(
    bench_ulong_add,
    ;,
    100003UL, 200009UL, 300007UL, 400031UL,
    a + 1009UL, b + 1013UL, c + 1019UL, d + 1021UL,
    a + b + c + d
)
DEFINE_BENCH_ULONG(
    bench_ulong_sub,
    ;,
    2000003UL, 1800001UL, 1600007UL, 1400009UL,
    a - 911UL, b - 919UL, c - 929UL, d - 937UL,
    a + b + c + d
)
DEFINE_BENCH_ULONG(
    bench_ulong_mul,
    unsigned long m1 = 3UL; unsigned long m2 = 5UL; unsigned long m3 = 7UL; unsigned long m4 = 9UL;,
    100003UL, 200009UL, 7001UL, 3701UL,
    a * m1, b * m2, c * m3, d * m4,
    a + b + c + d
)
DEFINE_BENCH_ULONG(
    bench_ulong_div,
    unsigned long q1 = 3UL; unsigned long q2 = 5UL; unsigned long q3 = 7UL; unsigned long q4 = 9UL;,
    3000000001UL, 2800000001UL, 2600000001UL, 2400000001UL,
    a / q1, b / q2, c / q3, d / q4,
    a + b + c + d
)
DEFINE_BENCH_ULONG(
    bench_ulong_mod,
    unsigned long r1 = 10007UL; unsigned long r2 = 9001UL; unsigned long r3 = 8009UL; unsigned long r4 = 7001UL;,
    4000000003UL, 3800000001UL, 3600000007UL, 3400000009UL,
    a % r1, b % r2, c % r3, d % r4,
    a + b + c + d
)

DEFINE_BENCH_FLOAT(
    bench_float_add,
    0.125f, 0.25f, 0.375f, 0.5f,
    a + 0.0001f, b + 0.0002f, c + 0.0003f, d + 0.0004f,
    a + b + c + d
)
DEFINE_BENCH_FLOAT(
    bench_float_sub,
    1.125f, 1.25f, 1.375f, 1.5f,
    a - 0.0001f, b - 0.0002f, c - 0.0003f, d - 0.0004f,
    a + b + c + d
)
DEFINE_BENCH_FLOAT(
    bench_float_mul,
    0.875f, 1.125f, 0.9375f, 1.0625f,
    a * 1.0001f, b * 0.9999f, c * 1.0002f, d * 0.9998f,
    a + b + c + d
)
DEFINE_BENCH_FLOAT(
    bench_float_div,
    1.875f, 1.625f, 1.4375f, 1.3125f,
    a / 1.0001f, b / 1.0002f, c / 1.0003f, d / 1.0004f,
    a + b + c + d
)

DEFINE_BENCH_DOUBLE(
    bench_double_add,
    0.125, 0.25, 0.375, 0.5,
    a + 0.0001, b + 0.0002, c + 0.0003, d + 0.0004,
    a + b + c + d
)
DEFINE_BENCH_DOUBLE(
    bench_double_sub,
    1.125, 1.25, 1.375, 1.5,
    a - 0.0001, b - 0.0002, c - 0.0003, d - 0.0004,
    a + b + c + d
)
DEFINE_BENCH_DOUBLE(
    bench_double_mul,
    0.875, 1.125, 0.9375, 1.0625,
    a * 1.0001, b * 0.9999, c * 1.0002, d * 0.9998,
    a + b + c + d
)
DEFINE_BENCH_DOUBLE(
    bench_double_div,
    1.875, 1.625, 1.4375, 1.3125,
    a / 1.0001, b / 1.0002, c / 1.0003, d / 1.0004,
    a + b + c + d
)

void main(void) {
    bench_pin = 0;

    while (1) {
        RUN_STAGE(bench_u8_add);
        RUN_STAGE(bench_u8_sub);
        RUN_STAGE(bench_u8_mul);
        RUN_STAGE(bench_u8_div);
        RUN_STAGE(bench_u8_mod);

        RUN_STAGE(bench_char_add);
        RUN_STAGE(bench_char_sub);
        RUN_STAGE(bench_char_mul);
        RUN_STAGE(bench_char_div);
        RUN_STAGE(bench_char_mod);

        RUN_STAGE(bench_int_add);
        RUN_STAGE(bench_int_sub);
        RUN_STAGE(bench_int_mul);
        RUN_STAGE(bench_int_div);
        RUN_STAGE(bench_int_mod);

        RUN_STAGE(bench_uint_add);
        RUN_STAGE(bench_uint_sub);
        RUN_STAGE(bench_uint_mul);
        RUN_STAGE(bench_uint_div);
        RUN_STAGE(bench_uint_mod);

        RUN_STAGE(bench_long_add);
        RUN_STAGE(bench_long_sub);
        RUN_STAGE(bench_long_mul);
        RUN_STAGE(bench_long_div);
        RUN_STAGE(bench_long_mod);

        RUN_STAGE(bench_ulong_add);
        RUN_STAGE(bench_ulong_sub);
        RUN_STAGE(bench_ulong_mul);
        RUN_STAGE(bench_ulong_div);
        RUN_STAGE(bench_ulong_mod);

        RUN_STAGE(bench_float_add);
        RUN_STAGE(bench_float_sub);
        RUN_STAGE(bench_float_mul);
        RUN_STAGE(bench_float_div);

        RUN_STAGE(bench_double_add);
        RUN_STAGE(bench_double_sub);
        RUN_STAGE(bench_double_mul);
        RUN_STAGE(bench_double_div);
    }
}
