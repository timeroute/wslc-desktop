# Roadmap

## 0.1.0：最小可用版本

目标：跑通 “pull alpine -> create container -> echo -> capture output -> stop/delete”。

功能：

- `Service::missing_components`
- `Service::version`
- `Service::ensure_available`
- `SessionBuilder`
- `Session::pull_image`
- `ContainerBuilder`
- `ProcessOptions::new`
- `Container::start`
- `Process::wait`
- `Container::start_and_wait`
- `Container::stop`
- `Container::delete`
- `Session::terminate`
- 基础错误模型
- 基础集成测试

不做：

- async；
- streaming stdin；
- exec；
- port mapping；
- volume；
- GPU；
- registry auth。

## 0.2.0：容器管理补齐

- `Container::inspect`
- `Container::state`
- `Container::id`
- `Container::exec`
- `Process::pid`
- `Process::signal`
- `Process::exit_code`
- port mapping
- basic volume mount
- named volume mount

## 0.3.0：镜像与存储

- `Session::list_images`
- `Session::delete_image`
- `Session::tag_image`
- `Session::push_image`
- `Session::authenticate`
- `Session::create_vhd_volume`
- `Session::delete_vhd_volume`
- `ImageInfo`

## 0.4.0：IO 与异步

- stdout/stderr streaming
- stdin write
- `wait_async`
- `pull_image_async`
- optional Tokio feature
- callback panic isolation
- worker thread runtime

## 0.5.0：高级能力

- GPU feature flag
- privileged container
- crash dump callback
- session termination event
- dynamic loading mode
- CLI fallback
- richer diagnostics

## 1.0.0：稳定版

前提：

- 官方 WSLC API 达到稳定或接近稳定；
- 已覆盖 x64 / ARM64；
- CI 可在 Windows runner 上跑集成测试；
- FFI 与安全策略经过 review；
- 文档完善；
- 公开 API 基本不再频繁破坏。
