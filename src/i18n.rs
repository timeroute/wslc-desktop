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
    pub overview: &'static str,
    pub containers: &'static str,
    pub images: &'static str,
    pub volumes: &'static str,
    pub settings: &'static str,

    pub overview_intro: &'static str,
    pub overview_engine: &'static str,
    pub overview_resources: &'static str,
    pub overview_session: &'static str,
    pub overview_app_version: &'static str,
    pub overview_sdk_version: &'static str,
    pub overview_backend: &'static str,
    pub overview_session_name: &'static str,
    pub overview_storage: &'static str,
    pub overview_running: &'static str,
    pub overview_containers: &'static str,
    pub overview_images: &'static str,
    pub overview_volumes: &'static str,
    pub overview_status_live: &'static str,
    pub overview_status_off: &'static str,
    pub overview_na: &'static str,
    pub overview_persist_hint: &'static str,

    pub refresh: &'static str,
    pub working: &'static str,
    pub refreshing: &'static str,
    pub ready: &'static str,

    pub all: &'static str,
    pub running: &'static str,
    pub start: &'static str,
    pub stop: &'static str,
    pub restart: &'static str,
    pub remove: &'static str,
    pub create: &'static str,
    pub pull: &'static str,
    pub pulling: &'static str,
    pub close: &'static str,
    pub cancel: &'static str,
    pub logs: &'static str,
    pub inspect: &'static str,
    pub exec: &'static str,

    pub confirm_remove_title: &'static str,
    pub confirm_remove_container: &'static str,
    pub confirm_remove_image: &'static str,
    pub confirm_remove_volume: &'static str,

    pub containers_count: &'static str,
    pub images_count: &'static str,
    pub volumes_count: &'static str,
    pub refreshed_summary: &'static str,
    pub mode_live: &'static str,
    pub mode_unavailable: &'static str,

    pub no_containers_title: &'static str,
    pub no_containers_detail: &'static str,
    pub no_images_title: &'static str,
    pub no_images_detail: &'static str,
    pub no_volumes_title: &'static str,
    pub no_volumes_detail: &'static str,

    pub create_volume_title: &'static str,
    pub create_volume_intro: &'static str,
    pub volume_name_label: &'static str,
    pub volume_name_empty: &'static str,

    pub pull_image: &'static str,
    pub pull_hint: &'static str,
    pub pull_logs_placeholder: &'static str,
    pub pull_complete: &'static str,
    pub pull_finished_ok: &'static str,
    pub pull_channel_disconnected: &'static str,
    pub pull_worker_disconnected: &'static str,
    pub image_ref_empty: &'static str,
    pub pulled_image: &'static str,

    pub sdk_setup_title: &'static str,
    pub sdk_setup_intro: &'static str,
    pub sdk_setup_retry: &'static str,
    pub sdk_setup_installing: &'static str,
    pub sdk_setup_failed: &'static str,
    pub sdk_setup_logs_placeholder: &'static str,
    pub sdk_setup_phase_running: &'static str,
    pub sdk_setup_phase_failed: &'static str,
    pub sdk_setup_channel_disconnected: &'static str,
    pub sdk_locked: &'static str,

    pub create_container_title: &'static str,
    pub create_container_intro: &'static str,
    pub image_label: &'static str,
    pub name_optional: &'static str,
    pub name_placeholder: &'static str,
    pub ports_label: &'static str,
    pub ports_placeholder: &'static str,
    pub bind_mounts_label: &'static str,
    pub bind_mounts_placeholder: &'static str,
    pub named_volumes_label: &'static str,
    pub named_volumes_placeholder: &'static str,
    pub auto_remove_label: &'static str,
    pub auto_remove_on: &'static str,
    pub auto_remove_off: &'static str,
    pub auto_remove_badge: &'static str,
    pub command_label: &'static str,
    pub command_placeholder: &'static str,
    pub volume_placeholder: &'static str,
    pub volume_size_label: &'static str,

    pub container_logs_title: &'static str,
    pub container_inspect_title: &'static str,
    pub container_exec_title: &'static str,
    pub container_exec_intro: &'static str,
    pub container_exec_placeholder: &'static str,
    pub container_exec_run: &'static str,
    pub logs_empty: &'static str,

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
    pub settings_session: &'static str,
    pub settings_session_hint: &'static str,
    pub settings_cpu: &'static str,
    pub settings_memory: &'static str,
    pub settings_prefs_saved: &'static str,

    pub overview_cpu: &'static str,
    pub overview_memory: &'static str,

    pub list_containers_failed: &'static str,
    pub list_images_failed: &'static str,
    pub list_volumes_failed: &'static str,
    pub error_prefix: &'static str,
    pub started: &'static str,
    pub stopped: &'static str,
    pub restarted: &'static str,
    pub removed: &'static str,
    pub created_container: &'static str,
    pub created_volume: &'static str,
    pub pulling_image: &'static str,
    pub pull_failed_msg: &'static str,
    pub parse_ports_failed: &'static str,
    pub parse_bind_mounts_failed: &'static str,
    pub parse_named_volumes_failed: &'static str,
    pub container_logs_failed: &'static str,
    pub container_inspect_failed: &'static str,
    pub container_exec_failed: &'static str,
    pub image_empty: &'static str,
    pub op_channel_disconnected: &'static str,

    pub engine_connected: &'static str,
    pub engine_unusable: &'static str,
    pub engine_missing: &'static str,
    pub engine_missing_components: &'static str,
    pub engine_sdk_outdated: &'static str,
}

pub static EN: Strings = Strings {
    app_subtitle: "WSL Containers",
    browse: "Browse",
    overview: "Overview",
    containers: "Containers",
    images: "Images",
    volumes: "Volumes",
    settings: "Settings",

    overview_intro: "WSLc Desktop status, versions, and resource summary.",
    overview_engine: "Engine",
    overview_resources: "Resources",
    overview_session: "Session",
    overview_app_version: "App version",
    overview_sdk_version: "SDK version",
    overview_backend: "Backend",
    overview_session_name: "Session name",
    overview_storage: "Storage",
    overview_running: "Running",
    overview_containers: "Containers",
    overview_images: "Images",
    overview_volumes: "Volumes",
    overview_status_live: "Connected (LIVE)",
    overview_status_off: "Unavailable (OFF)",
    overview_na: "—",
    overview_persist_hint: "Containers and volumes are restored after app restart (session is not terminated on exit).",
    overview_cpu: "CPU",
    overview_memory: "Memory",

    refresh: "Refresh",
    working: "Working…",
    refreshing: "Refreshing…",
    ready: "Ready",

    all: "All",
    running: "Running",
    start: "Start",
    stop: "Stop",
    restart: "Restart",
    remove: "Remove",
    create: "Create",
    pull: "Pull",
    pulling: "Pulling…",
    close: "Close",
    cancel: "Cancel",
    logs: "Logs",
    inspect: "Inspect",
    exec: "Exec",

    confirm_remove_title: "Confirm remove",
    confirm_remove_container: "Remove container “{name}”? This cannot be undone.",
    confirm_remove_image: "Remove image “{name}”? This cannot be undone.",
    confirm_remove_volume: "Remove volume “{name}”? This cannot be undone.",

    containers_count: "{total} total · {running} running",
    images_count: "{count} images",
    volumes_count: "{count} volumes",
    refreshed_summary: "Refreshed · {containers} containers · {images} images · {volumes} volumes · {mode}",
    mode_live: "live",
    mode_unavailable: "unavailable",

    no_containers_title: "No containers",
    no_containers_detail: "Pull an image, create a container, then refresh.",
    no_images_title: "No images",
    no_images_detail: "Use Pull to download an image. Create or remove from each row.",
    no_volumes_title: "No volumes",
    no_volumes_detail: "Use Create to add a VHD volume. Remove from each row.",

    create_volume_title: "Create volume",
    create_volume_intro: "Creates a dynamic VHD in this desktop session. Pick a size, then create.",
    volume_name_label: "Name",
    volume_name_empty: "Volume name is empty",

    pull_image: "Pull image",
    pull_hint: "Enter an image reference and pull",
    pull_logs_placeholder: "Pull logs will appear here.",
    pull_complete: "Pull complete",
    pull_finished_ok: "✓ Pull finished successfully",
    pull_channel_disconnected: "Pull channel disconnected",
    pull_worker_disconnected: "✗ pull worker disconnected",
    image_ref_empty: "Image reference is empty",
    pulled_image: "Pulled image",

    sdk_setup_title: "WSL Containers SDK",
    sdk_setup_intro: "Install or update the SDK to continue. The app stays locked until a live session starts. If this keeps failing, install or update WSL yourself (`wsl --install` / `wsl --update`), then retry.",
    sdk_setup_retry: "Retry install SDK",
    sdk_setup_installing: "Installing / connecting…",
    sdk_setup_failed: "SDK setup failed",
    sdk_setup_logs_placeholder: "Setup logs will appear here.",
    sdk_setup_phase_running: "SETUP",
    sdk_setup_phase_failed: "FAILED",
    sdk_setup_channel_disconnected: "SDK setup channel disconnected",
    sdk_locked: "Waiting for WSL Containers SDK…",

    create_container_title: "Create container",
    create_container_intro: "Map host ports (e.g. 8080:80) to reach the container. Leave command empty to use the image entrypoint.",
    image_label: "Image",
    name_optional: "Name (optional)",
    name_placeholder: "optional container name",
    ports_label: "Ports",
    ports_placeholder: "8080:80, 443:443",
    bind_mounts_label: "Bind mounts",
    bind_mounts_placeholder: "C:\\data:/data or D:\\logs:/var/log:ro",
    named_volumes_label: "Named volumes",
    named_volumes_placeholder: "data:/data, cache:/var/cache:ro",
    auto_remove_label: "Remove on exit",
    auto_remove_on: "On",
    auto_remove_off: "Off",
    auto_remove_badge: "auto-remove",
    command_label: "Command (optional)",
    command_placeholder: "optional; leave empty for image default",
    volume_placeholder: "my-volume",
    volume_size_label: "Size",

    container_logs_title: "Container logs",
    container_inspect_title: "Container inspect",
    container_exec_title: "Exec",
    container_exec_intro: "Run a one-shot command (non-interactive). Example: uname -a",
    container_exec_placeholder: "uname -a",
    container_exec_run: "Run",
    logs_empty: "(no log output)",

    binary_label: "sdk: {name}",

    pull_idle: "IDLE",
    pull_phase_pulling: "PULLING",
    pull_ok: "OK",
    pull_failed: "FAILED",

    settings_intro: "Language, appearance, and session resources. Language defaults from the system locale (Chinese → 中文, otherwise English).",
    language: "Language",
    language_hint: "UI language for menus, buttons, and status text.",
    theme: "Theme",
    theme_hint: "Window color scheme.",
    theme_dark: "Dark",
    theme_light: "Light",
    settings_session: "Session resources",
    settings_session_hint: "CPU and memory for the wslc-desktop session. Saved immediately; applied the next time a new session is created (existing sessions keep their original limits).",
    settings_cpu: "CPU count",
    settings_memory: "Memory (MiB)",
    settings_prefs_saved: "Session preferences saved",

    list_containers_failed: "List containers failed: {err}",
    list_images_failed: "List images failed: {err}",
    list_volumes_failed: "List volumes failed: {err}",
    error_prefix: "Error: {err}",
    started: "Started {name}",
    stopped: "Stopped {name}",
    restarted: "Restarted {name}",
    removed: "Removed {name}",
    created_container: "Created {name}",
    created_volume: "Created volume {name}",
    pulling_image: "Pulling {name}…",
    pull_failed_msg: "Pull failed: {err}",
    parse_ports_failed: "Invalid ports: {err}",
    parse_bind_mounts_failed: "Invalid bind mounts: {err}",
    parse_named_volumes_failed: "Invalid named volumes: {err}",
    container_logs_failed: "Logs failed: {err}",
    container_inspect_failed: "Inspect failed: {err}",
    container_exec_failed: "Exec failed: {err}",
    image_empty: "Image is required",
    op_channel_disconnected: "Background operation disconnected",

    engine_connected: "Connected via WSL Containers SDK {bin}",
    engine_unusable: "SDK unavailable ({err}). Retry install in the setup dialog, or install/update WSL (`wsl --install` / `wsl --update`).",
    engine_missing: "wslcsdk.dll not available. Need network to download the SDK, plus WSL (`wsl --update`). Use Retry in the setup dialog.",
    engine_missing_components: "WSL Containers runtime incomplete ({err}). Run `wsl --install` / `wsl --update`, then reboot if prompted, and retry.",
    engine_sdk_outdated: "Cached wslcsdk.dll is too old for this WSL. Use Retry to re-download a matching SDK, or run `wsl --update`.",
};

pub static ZH: Strings = Strings {
    app_subtitle: "WSL 容器",
    browse: "浏览",
    overview: "总览",
    containers: "容器",
    images: "镜像",
    volumes: "卷",
    settings: "设置",

    overview_intro: "WSLc Desktop 状态、版本与资源摘要。",
    overview_engine: "引擎",
    overview_resources: "资源",
    overview_session: "会话",
    overview_app_version: "应用版本",
    overview_sdk_version: "SDK 版本",
    overview_backend: "后端",
    overview_session_name: "会话名称",
    overview_storage: "存储路径",
    overview_running: "运行中",
    overview_containers: "容器",
    overview_images: "镜像",
    overview_volumes: "卷",
    overview_status_live: "已连接（LIVE）",
    overview_status_off: "不可用（OFF）",
    overview_na: "—",
    overview_persist_hint: "退出应用不会终止会话；重启后会自动恢复容器与卷。",
    overview_cpu: "CPU",
    overview_memory: "内存",

    refresh: "刷新",
    working: "处理中…",
    refreshing: "刷新中…",
    ready: "就绪",

    all: "全部",
    running: "运行中",
    start: "启动",
    stop: "停止",
    restart: "重启",
    remove: "删除",
    create: "创建",
    pull: "拉取",
    pulling: "拉取中…",
    close: "关闭",
    cancel: "取消",
    logs: "日志",
    inspect: "检查",
    exec: "执行",

    confirm_remove_title: "确认删除",
    confirm_remove_container: "删除容器「{name}」？此操作不可撤销。",
    confirm_remove_image: "删除镜像「{name}」？此操作不可撤销。",
    confirm_remove_volume: "删除卷「{name}」？此操作不可撤销。",

    containers_count: "共 {total} · 运行中 {running}",
    images_count: "{count} 个镜像",
    volumes_count: "{count} 个卷",
    refreshed_summary: "已刷新 · {containers} 容器 · {images} 镜像 · {volumes} 卷 · {mode}",
    mode_live: "实时",
    mode_unavailable: "不可用",

    no_containers_title: "暂无容器",
    no_containers_detail: "拉取镜像并创建容器后刷新。",
    no_images_title: "暂无镜像",
    no_images_detail: "点击「拉取」下载镜像。在列表行内创建容器或删除。",
    no_volumes_title: "暂无卷",
    no_volumes_detail: "点击「创建」添加 VHD 卷。在列表行内删除。",

    create_volume_title: "创建卷",
    create_volume_intro: "在本桌面会话中创建动态 VHD。选择容量后创建。",
    volume_name_label: "名称",
    volume_name_empty: "卷名称为空",

    pull_image: "拉取镜像",
    pull_hint: "输入镜像引用后拉取",
    pull_logs_placeholder: "拉取日志将显示在这里。",
    pull_complete: "拉取完成",
    pull_finished_ok: "✓ 拉取成功",
    pull_channel_disconnected: "拉取通道已断开",
    pull_worker_disconnected: "✗ 拉取任务已断开",
    image_ref_empty: "镜像引用为空",
    pulled_image: "已拉取镜像",

    sdk_setup_title: "WSL Containers SDK",
    sdk_setup_intro: "请安装或更新 SDK 后才能继续。在成功建立实时会话前，应用将保持锁定。若反复失败，请自行安装/更新 WSL（`wsl --install` / `wsl --update`），然后再重试。",
    sdk_setup_retry: "重新安装 SDK",
    sdk_setup_installing: "正在安装 / 连接…",
    sdk_setup_failed: "SDK 安装失败",
    sdk_setup_logs_placeholder: "安装日志将显示在这里。",
    sdk_setup_phase_running: "安装中",
    sdk_setup_phase_failed: "失败",
    sdk_setup_channel_disconnected: "SDK 安装通道已断开",
    sdk_locked: "等待 WSL Containers SDK…",

    create_container_title: "创建容器",
    create_container_intro: "请映射主机端口（如 8080:80）才能从本机访问。命令留空则使用镜像默认入口。",
    image_label: "镜像",
    name_optional: "名称（可选）",
    name_placeholder: "可选容器名称",
    ports_label: "端口",
    ports_placeholder: "8080:80, 443:443",
    bind_mounts_label: "绑定挂载",
    bind_mounts_placeholder: "C:\\data:/data 或 D:\\logs:/var/log:ro",
    named_volumes_label: "命名卷",
    named_volumes_placeholder: "data:/data, cache:/var/cache:ro",
    auto_remove_label: "退出后删除",
    auto_remove_on: "开",
    auto_remove_off: "关",
    auto_remove_badge: "退出删除",
    command_label: "命令（可选）",
    command_placeholder: "可选；留空则用镜像默认命令",
    volume_placeholder: "我的卷",
    volume_size_label: "容量",

    container_logs_title: "容器日志",
    container_inspect_title: "容器检查",
    container_exec_title: "执行命令",
    container_exec_intro: "运行一次性命令（非交互）。例如：uname -a",
    container_exec_placeholder: "uname -a",
    container_exec_run: "运行",
    logs_empty: "（无日志输出）",

    binary_label: "sdk: {name}",

    pull_idle: "空闲",
    pull_phase_pulling: "拉取中",
    pull_ok: "成功",
    pull_failed: "失败",

    settings_intro: "语言、外观与会话资源。语言默认跟随系统区域设置（中文 → 中文，否则英文）。",
    language: "语言",
    language_hint: "菜单、按钮与状态栏使用的界面语言。",
    theme: "主题",
    theme_hint: "窗口配色方案。",
    theme_dark: "深色",
    theme_light: "浅色",
    settings_session: "会话资源",
    settings_session_hint: "wslc-desktop 会话的 CPU 与内存。立即保存；仅在新建会话时生效（已有会话仍沿用原限额）。",
    settings_cpu: "CPU 数量",
    settings_memory: "内存（MiB）",
    settings_prefs_saved: "会话偏好已保存",

    list_containers_failed: "列出容器失败: {err}",
    list_images_failed: "列出镜像失败: {err}",
    list_volumes_failed: "列出卷失败: {err}",
    error_prefix: "错误: {err}",
    started: "已启动 {name}",
    stopped: "已停止 {name}",
    restarted: "已重启 {name}",
    removed: "已删除 {name}",
    created_container: "已创建 {name}",
    created_volume: "已创建卷 {name}",
    pulling_image: "正在拉取 {name}…",
    pull_failed_msg: "拉取失败: {err}",
    parse_ports_failed: "端口格式无效: {err}",
    parse_bind_mounts_failed: "绑定挂载格式无效: {err}",
    parse_named_volumes_failed: "命名卷格式无效: {err}",
    container_logs_failed: "获取日志失败: {err}",
    container_inspect_failed: "检查失败: {err}",
    container_exec_failed: "执行失败: {err}",
    image_empty: "请填写镜像",
    op_channel_disconnected: "后台操作已断开",

    engine_connected: "已通过 WSL Containers SDK {bin} 连接",
    engine_unusable: "SDK 不可用（{err}）。请在安装弹窗中重试，或自行安装/更新 WSL（`wsl --install` / `wsl --update`）。",
    engine_missing: "无法使用 wslcsdk.dll。需联网下载 SDK，并已安装/更新 WSL（`wsl --update`）。请在安装弹窗中重试。",
    engine_missing_components: "WSL Containers 运行时不完整（{err}）。请执行 `wsl --install` / `wsl --update`，必要时重启后再重试。",
    engine_sdk_outdated: "缓存的 wslcsdk.dll 过旧。请点击重试以下载匹配 SDK，或先 `wsl --update`。",
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
