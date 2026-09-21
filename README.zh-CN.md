# rgbcx

[English](README.md) | 简体中文

Rust 编写的 BC1、BC3、BC4、BC5 纹理编解码库，提供可选的并行编码和 GPU 端点精修。默认构建无第三方依赖。

## 安装

```toml
[dependencies]
rgbcx = "0.1"
```

| Cargo 功能 | 作用                                |
| ---------- | ----------------------------------- |
| 默认（无） | CPU 编码、解码及精修，无第三方依赖  |
| `parallel` | 通过 Rayon 提供 `encode_parallel`   |
| `gpu`      | 通过 wgpu 提供可复用的 GPU 精修实例 |

两个可选功能可以分别启用，也可以同时启用。

## 编码与解码

```rust
use rgbcx::{decode, encode, Format, Quality};

fn main() -> Result<(), rgbcx::Error> {
    let rgba = [120, 160, 200, 255].repeat(8 * 8);
    let blocks = encode(8, 8, &rgba, Format::Bc3, Quality::Balanced)?;
    let preview = decode(8, 8, &blocks, Format::Bc3)?;
    Ok(())
}
```

输入为紧密排列的 RGBA8 像素，输出为逐行排列的 4×4 压缩块，不包含 DDS、KTX 等容器头。
支持非零尺寸，缓冲区大小须能由当前平台寻址。边缘不满一个块时复制最后一行或列的像素；解码时裁回原始尺寸。
`Format::encoded_len` 计算压缩块的存储大小；`encode_into` 和 `decode_into` 可写入调用者提供的精确大小缓冲区，便于批量处理时复用内存。
输入错误通过 `rgbcx::Error` 返回；尺寸或长度无效时不会修改输出。

| 格式 | 使用的输入通道              | 每块大小 |
| ---- | --------------------------- | -------- |
| BC1  | RGB，忽略 Alpha，输出不透明 | 8 字节   |
| BC3  | RGBA                        | 16 字节  |
| BC4  | R                           | 8 字节   |
| BC5  | R、G                        | 16 字节  |

BC4、BC5 使用无符号归一化通道，解码时未使用的颜色通道填零、Alpha 填 255。
BC1 解码也支持外部编码的单比特透明块；BC3 不论端点顺序，颜色部分始终使用四种插值颜色。

## 质量与并行

- `Fast`：主轴拟合与最小二乘调整。
- `Balanced`：增加聚类拟合，对误差较大的块尝试 `High`。
- `High`：搜索更多分区与邻近端点。

各档位使用相同的 Alpha 编码。颜色误差按整数 RGB 计算；库不执行色彩空间转换、预乘 Alpha、抖动或图片缩放。
质量档位表示搜索开销，不保证每张图片的感知质量，也不承诺不同硬件架构之间逐字节一致。

启用 `parallel` 后可使用 `encode_parallel`，通过 Rayon 当前线程池并行处理块，同一构建中与串行结果一致。
已有图片级并行调度的程序可以使用 `encode`，避免嵌套调度。

```rust
fn main() -> Result<(), rgbcx::Error> {
    #[cfg(feature = "parallel")]
    {
        use rgbcx::{encode_parallel, Format, Quality};

        let rgba = [120, 160, 200, 255].repeat(8 * 8);
        let blocks = encode_parallel(8, 8, &rgba, Format::Bc3, Quality::Balanced)?;
        assert_eq!(blocks.len(), Format::Bc3.encoded_len(8, 8)?);
    }
    Ok(())
}
```

## 可选 GPU 精修

启用 `gpu` 后，`gpu::Refiner` 使用整数 WGSL 搜索每块的 729 组邻近 RGB565 端点。
建议复用同一个实例，以摊薄设备与管线初始化开销。

```rust,no_run
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "gpu")]
    {
        use rgbcx::{encode, gpu::Refiner, Format, Quality};

        let rgba = [120, 160, 200, 255].repeat(8 * 8);
        let mut blocks = encode(8, 8, &rgba, Format::Bc3, Quality::Balanced)?;
        let mut worker = Refiner::new()?;
        worker.refine(8, 8, &rgba, Format::Bc3, &mut blocks)?;
    }
    Ok(())
}
```

`refine` 提供相同的 CPU 搜索。两者保持 BC3 Alpha 不变，误差相同时保留原块，且不会增加 RGB 平方误差。
BC1 精修只接受不透明块。GPU 加速的是额外精修阶段，初始颜色拟合仍在 CPU 上完成。

`gpu::Error` 区分输入错误、硬件不可用和设备执行失败。输入错误不会修改输出；
设备执行失败时，输出可能只更新了一部分。如需回退到 CPU，应保留原始压缩块。
数值误差不增加，并不等同于必然改善观感。

## 开发

需要 Rust 1.98.1 或更新版本。

```sh
cargo test --features parallel
cargo clippy --all-targets --all-features -- -D warnings
cargo test --release --all-features --tests -- --include-ignored
```

最后一条命令需要硬件计算适配器。测试包含 Alpha 参考样本、非整块尺寸、并发调用和 CPU/GPU 一致性检查。
本地性能测量使用紧密排列的原始 RGBA8 文件（不是 PNG）：

```sh
cargo run --release --all-features --example profile_encoder -- 512 512 image.rgba
cargo run --release --example encode_file -- 512 512 image.rgba bc3 balanced image.bc
```

计时示例预热后报告五次测量的中位数，分别列出编码与额外精修耗时；GPU 初始化单独计时。
精修计时包含重置输出缓冲区，GPU 精修计时也包含上传、执行和读回。

## 许可

Rust 实现采用 [MPL-2.0](./LICENSE)。第三方部分的原始版权和许可声明保留在 [licenses/](licenses/README.md)。

## 鸣谢

颜色拟合算法、排序表和 Alpha 编码参考了 [Richard Geldreich 的 rgbcx](https://github.com/richgel999/bc7enc_rdo) 1.13。
