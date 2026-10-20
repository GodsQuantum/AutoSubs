# AutoSubs — 自动字幕与视频烧录

[English](README.md) · [Français](README.fr.md) · **简体中文**

**导入视频，调整文字和时间轴，一次完成字幕样式与视频渲染。** AutoSubs 采用 Rust、SvelteKit 和 FFmpeg/libass，支持 SRT、逐词时间修正、视频预览以及文件夹监听自动化。语音识别可以连接自托管 Speaches 或其他兼容 OpenAI API 的服务。

## 下载与三步上手

1. 下载 [Linux 发行版](https://github.com/GodsQuantum/AutoSubs/releases) 或 [Windows x64 / macOS Intel、Apple Silicon 安装候选版](https://github.com/GodsQuantum/AutoSubs/releases/tag/desktop-v3.6.3-rc.2)。Windows/macOS **没有正式签名或 Apple 公证**；请核对相邻的 SHA256 文件。
2. 使用 Docker 时，先复制 .env.example 为 .env，再执行以下命令。默认 HTTP 绑定本机 127.0.0.1，远程访问需身份验证和 HTTPS。

~~~bash
docker compose -f compose.example.yaml up -d
~~~

3. 在设置中填写语音服务的 /v1 地址，使用探测/配对功能确认模型可用。先用短视频测试 SRT、逐词时间及字幕烧录，成功后再启用监听文件夹。

## 自动检测与性能起点

~~~bash
python3 scripts/auto-tune.py
python3 scripts/auto-tune.py --env
~~~

脚本检测 CPU 亲和性和容器内存，只提供推荐值，**不会自动改动生产配置**。2–4 线程或不足 8 GiB 时建议 1 个渲染和 1 个转录任务；Ryzen 7 8845HS / 16 线程 / 32 GiB 共享容器建议各 2 个。只有通过真实 FFmpeg 视频编码及字幕质量测试后才使用 VAAPI/NVENC，不能把找到编码器当成已加速。

对于逐词字幕，Whisper、Qwen3-ASR 和 Nemotron 的时间戳能力并不相同；要测试真实输出与对齐器。应用中已保存的设置可能优先于初始 .env。

## 文档与社区

[完整英文使用说明](README.md) · [法语版](README.fr.md) · [发布版本](https://github.com/GodsQuantum/AutoSubs/releases) · [报告问题](https://github.com/GodsQuantum/AutoSubs/issues)。MIT 许可证。跨平台候选版的 CI 通过不等于官方代码签名或所有硬件认证。
