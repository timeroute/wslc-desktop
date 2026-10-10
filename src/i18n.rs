//! Chinese / English UI strings with system-locale detection.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    En,
    Zh,
}

impl Locale {
    /// Prefer Chinese when the OS locale starts with `zh`; otherwise English.
    pub fn from_system() -> Self {
        match sys_locale::get_locale() {
            Some(loc) => {
                let normalized = loc.to_ascii_lowercase().replace('_', "-");
                if normalized.starts_with("zh") {
                    Self::Zh
                } else {
                    Self::En
                }
            }
            None => Self::En,
        }
    }

    pub fn strings(self) -> &'static Strings {
        match self {
            Self::En => &EN,
            Self::Zh => &ZH,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Zh => "中文",
        }
    }
}

pub struct Strings {
    pub app_subtitle: &'static str,
    pub browse: &'static str,
    pub preferences: &'static str,
    pub containers: &'static str,
    pub images: &'static str,
    pub volumes: &'static str,
    pub run: &'static str,
    pub settings: &'static str,

    pub refresh: &'static str,
    pub working: &'static str,
    pub refreshing: &'static str,
    pub ready: &'static str,

    pub all: &'static str,
    pub running: &'static str,
    pub start: &'static str,
    pub stop: &'static str,
    pub remove: &'static str,
    pub create: &'static str,
    pub pull: &'static str,
    pub pulling: &'static str,
    pub run_detached: &'static str,

    pub containers_count: &'static str,
    pub images_count: &'static str,
    pub volumes_count: &'static str,
    pub refreshed_summary: &'static str,
    pub mode_live: &'static str,
    pub mode_demo: &'static str,

    pub no_containers_title: &'static str,
    pub no_containers_detail: &'static str,
    pub no_images_title: &'static str,
    pub no_images_detail: &'static str,
    pub no_volumes_title: &'static str,
    pub no_volumes_detail: &'static str,

    pub pull_image: &'static str,
    pub pull_hint: &'static str,
    pub pull_logs_placeholder: &'static str,
    pub pull_complete: &'static str,
    pub pull_finished_ok: &'static str,
    pub pull_channel_disconnected: &'static str,
    pub pull_worker_disconnected: &'static str,
    pub image_ref_empty: &'static str,
    pub pulled_image: &'static str,

    pub run_intro: &'static str,
    pub image_label: &'static str,
    pub name_optional: &'static str,
    pub cli_equivalents: &'static str,
    pub name_placeholder: &'static str,
    pub volume_placeholder: &'static str,

    pub binary_label: &'static str,

    pub pull_idle: &'static str,
    pub pull_phase_pulling: &'static str,
    pub pull_ok: &'static str,
    pub pull_failed: &'static str,

    pub settings_intro: &'static str,
    pub language: &'static str,
    pub language_hint: &'static str,
    pub theme: &'static str,
    pub theme_hint: &'static str,
    pub theme_dark: &'static str,
    pub theme_light: &'static str,

    pub list_containers_failed: &'static str,
    pub list_images_failed: &'static str,
    pub list_volumes_failed: &'static str,
    pub error_prefix: &'static str,
    pub started: &'static str,
    pub stopped: &'static str,
    pub removed: &'static str,
    pub launched: &'static str,
    pub created_volume: &'static str,
    pub pulling_image: &'static str,
    pub pull_failed_msg: &'static str,

    pub engine_connected: &'static str,
    pub engine_unusable: &'static str,
    pub engine_missing: &'static str,
}

pub static EN: Strings = Strings {
    app_subtitle: "WSL Containers",
    browse: "Browse",
    preferences: "Preferences",
    containers: "Containers",
    images: "Images",
    volumes: "Volumes",
    run: "Run",
    settings: "Settings",

    refresh: "Refresh",
    working: "Working…",
    refreshing: "Refreshing…",
    ready: "Ready",

    all: "All",
    running: "Running",
    start: "Start",
    stop: "Stop",
    remove: "Remove",
    create: "Create",
    pull: "Pull",
    pulling: "Pulling…",
    run_detached: "Run detached",

    containers_count: "{total} total · {running} running",
    images_count: "{count} images",
    volumes_count: "{count} volumes",
    refreshed_summary: "Refreshed · {containers} containers · {images} images · {volumes} volumes · {mode}",
    mode_live: "live",
    mode_demo: "demo",

    no_containers_title: "No containers",
    no_containers_detail: "Run an image from the Run tab, or refresh after starting one with wslc.",
    no_images_title: "No images",
    no_images_detail: "Type an image name above and pull it, then refresh.",
    no_volumes_title: "No volumes",
    no_volumes_detail: "Create a named volume above, or refresh after creating one with wslc volume create.",

    pull_image: "Pull image",
    pull_hint: "Enter an image reference and pull",
    pull_logs_placeholder: "Pull logs will appear here.",
    pull_complete: "Pull complete",
    pull_finished_ok: "✓ Pull finished successfully",
    pull_channel_disconnected: "Pull channel disconnected",
    pull_worker_disconnected: "✗ pull worker disconnected",
    image_ref_empty: "Image reference is empty",
    pulled_image: "Pulled image",

    run_intro: "Start a detached container through the local wslc CLI. Without WSLc, demo mode only updates the in-memory list.",
    image_label: "Image",
    name_optional: "Name (optional)",
    cli_equivalents: "CLI equivalents",
    name_placeholder: "optional container name",
    volume_placeholder: "my-volume",

    binary_label: "binary: {name}",

    pull_idle: "IDLE",
    pull_phase_pulling: "PULLING",
    pull_ok: "OK",
    pull_failed: "FAILED",

    settings_intro: "Language and appearance. Language defaults from the system locale (Chinese → 中文, otherwise English).",
    language: "Language",
    language_hint: "UI language for menus, buttons, and status text.",
    theme: "Theme",
    theme_hint: "Window color scheme.",
    theme_dark: "Dark",
    theme_light: "Light",

    list_containers_failed: "List containers failed: {err}",
    list_images_failed: "List images failed: {err}",
    list_volumes_failed: "List volumes failed: {err}",
    error_prefix: "Error: {err}",
    started: "Started {name}",
    stopped: "Stopped {name}",
    removed: "Removed {name}",
    launched: "Launched {name}",
    created_volume: "Created volume {name}",
    pulling_image: "Pulling {name}…",
    pull_failed_msg: "Pull failed: {err}",

    engine_connected: "Connected via `{bin}`",
    engine_unusable: "CLI present but unusable ({err}); using demo data",
    engine_missing: "WSLc CLI not found — showing demo data (install WSL ≥ 2.9.3 on Windows)",
};

pub static ZH: Strings = Strings {
    app_subtitle: "WSL 容器",
    browse: "浏览",
    preferences: "偏好",
    containers: "容器",
    images: "镜像",
    volumes: "卷",
    run: "运行",
    settings: "设置",

    refresh: "刷新",
    working: "处理中…",
    refreshing: "刷新中…",
    ready: "就绪",

    all: "全部",
    running: "运行中",
    start: "启动",
    stop: "停止",
    remove: "删除",
    create: "创建",
    pull: "拉取",
    pulling: "拉取中…",
    run_detached: "后台运行",

    containers_count: "共 {total} · 运行中 {running}",
    images_count: "{count} 个镜像",
    volumes_count: "{count} 个卷",
    refreshed_summary: "已刷新 · {containers} 容器 · {images} 镜像 · {volumes} 卷 · {mode}",
    mode_live: "实时",
    mode_demo: "演示",

    no_containers_title: "暂无容器",
    no_containers_detail: "可在「运行」页启动镜像，或用 wslc 创建后再刷新。",
    no_images_title: "暂无镜像",
    no_images_detail: "在上方输入镜像名并拉取，然后刷新。",
    no_volumes_title: "暂无卷",
    no_volumes_detail: "在上方创建命名卷，或用 wslc volume create 创建后再刷新。",

    pull_image: "拉取镜像",
    pull_hint: "输入镜像引用后拉取",
    pull_logs_placeholder: "拉取日志将显示在这里。",
    pull_complete: "拉取完成",
    pull_finished_ok: "✓ 拉取成功",
    pull_channel_disconnected: "拉取通道已断开",
    pull_worker_disconnected: "✗ 拉取任务已断开",
    image_ref_empty: "镜像引用为空",
    pulled_image: "已拉取镜像",

    run_intro: "通过本机 wslc CLI 启动分离式容器。未安装 WSLc 时，演示模式仅更新内存列表。",
    image_label: "镜像",
    name_optional: "名称（可选）",
    cli_equivalents: "等效 CLI 命令",
    name_placeholder: "可选容器名称",
    volume_placeholder: "我的卷",

    binary_label: "二进制: {name}",

    pull_idle: "空闲",
    pull_phase_pulling: "拉取中",
    pull_ok: "成功",
    pull_failed: "失败",

    settings_intro: "语言与外观。语言默认跟随系统区域设置（中文 → 中文，否则英文）。",
    language: "语言",
    language_hint: "菜单、按钮与状态栏使用的界面语言。",
    theme: "主题",
    theme_hint: "窗口配色方案。",
    theme_dark: "深色",
    theme_light: "浅色",

    list_containers_failed: "列出容器失败: {err}",
    list_images_failed: "列出镜像失败: {err}",
    list_volumes_failed: "列出卷失败: {err}",
    error_prefix: "错误: {err}",
    started: "已启动 {name}",
    stopped: "已停止 {name}",
    removed: "已删除 {name}",
    launched: "已启动 {name}",
    created_volume: "已创建卷 {name}",
    pulling_image: "正在拉取 {name}…",
    pull_failed_msg: "拉取失败: {err}",

    engine_connected: "已通过 `{bin}` 连接",
    engine_unusable: "检测到 CLI 但不可用（{err}）；使用演示数据",
    engine_missing: "未找到 WSLc CLI — 显示演示数据（请在 Windows 安装 WSL ≥ 2.9.3）",
};

pub fn fmt1(template: &str, key: &str, value: impl AsRef<str>) -> String {
    template.replace(&format!("{{{key}}}"), value.as_ref())
}

pub fn fmt2(
    template: &str,
    k1: &str,
    v1: impl AsRef<str>,
    k2: &str,
    v2: impl AsRef<str>,
) -> String {
    fmt1(&fmt1(template, k1, v1), k2, v2)
}

pub fn fmt4(
    template: &str,
    k1: &str,
    v1: impl AsRef<str>,
    k2: &str,
    v2: impl AsRef<str>,
    k3: &str,
    v3: impl AsRef<str>,
    k4: &str,
    v4: impl AsRef<str>,
) -> String {
    fmt2(&fmt2(template, k1, v1, k2, v2), k3, v3, k4, v4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_locale_prefixes() {
        for sample in ["zh", "zh-CN", "zh_TW", "zh-Hans-CN"] {
            let normalized = sample.to_ascii_lowercase().replace('_', "-");
            assert!(normalized.starts_with("zh"), "{sample}");
        }
        for sample in ["en-US", "ja-JP", "fr"] {
            let normalized = sample.to_ascii_lowercase().replace('_', "-");
            assert!(!normalized.starts_with("zh"), "{sample}");
        }
    }

    #[test]
    fn fmt_helpers() {
        assert_eq!(fmt1("Hello {name}", "name", "WSLc"), "Hello WSLc");
        assert_eq!(
            fmt2("{a}/{b}", "a", "1", "b", "2"),
            "1/2"
        );
    }
}
