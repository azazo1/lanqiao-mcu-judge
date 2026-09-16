#include "filtering.h"
#define N 5 // 滤波窗口大小

pdata float median_data_array[N] = {0};    // 用于中值滤波的窗口数据
pdata unsigned char median_index = 0;      // 当前数据的索引（中值滤波）
pdata unsigned char median_data_count = 0; // 中值滤波器当前已有的数据数量

// 中值滤波器函数
// 参数：new_data - 新的输入数据
// 返回值：滤波后的数据
float Median_Filter(float new_data)
{
  float sorted_data[N];
  float temp;
  unsigned char i, j;

  median_data_array[median_index] = new_data; // 存储新数据到中值滤波窗口
  median_index = (median_index + 1) % N;      // 更新索引
  if (median_data_count < N)
    median_data_count++; // 增加中值滤波器内已有数据的数量

  // 复制数据到临时数组
  for (i = 0; i < median_data_count; i++)
  {
    sorted_data[i] = median_data_array[i];
  }

  // 冒泡排序
  for (i = 0; i < median_data_count - 1; i++)
  {
    for (j = 0; j < median_data_count - i - 1; j++)
    {
      if (sorted_data[j] > sorted_data[j + 1])
      {
        // 交换元素位置
        temp = sorted_data[j];
        sorted_data[j] = sorted_data[j + 1];
        sorted_data[j + 1] = temp;
      }
    }
  }
  return sorted_data[median_data_count / 2]; // 返回中值
}

pdata unsigned char mean_data_array[N] = {0}; // 用于滑动平均滤波的窗口数据
pdata float mean_sum = 0.0f;                  // 滑动平均滤波器当前数据和
pdata unsigned char mean_index = 0;           // 当前数据的索引（滑动平均滤波）
pdata unsigned char mean_data_count = 0;      // 滑动平均滤波器当前已有的数据数量

// 滑动平均滤波器函数
// 参数：new_data - 新的输入数据
// 返回值：滤波后的数据
float Moving_Average_Filter(unsigned char new_data)
{
  mean_sum -= mean_data_array[mean_index]; // 减去滑动平均窗口中被替换的数据值
  mean_data_array[mean_index] = new_data;  // 将新数据存入滑动平均窗口
  mean_sum += new_data;                    // 更新窗口内数据的总和
  mean_index = (mean_index + 1) % N;       // 更新索引
  if (mean_data_count < N)
    mean_data_count++;               // 增加滑动平均滤波器内已有数据的数量
  return mean_sum / mean_data_count; // 返回平均值
}
