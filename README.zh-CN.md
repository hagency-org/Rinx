# Rinx

[English](README.md) | 简体中文

Rinx 是原生 Matrix 客户端，包含聊天、联系人、发现、朋友圈和文章编辑器，支持独立运行及作为 OctoSense 原生模块运行。完整构建与兼容性说明见 [英文文档](README.md)。

## 多账号

桌面点击左下角账号菜单，手机进入“我 → 设置 → 账号”，可以添加账号并切换已保存的登录。切换保留各账号的设备与加密数据库；只有当前选中的账号同步，退出登录只撤销当前账号。登录页也能选择已有账号。详见[多账号流程与本地数据隔离](docs/multi-account.md)。

## Hagency 快速开始

打开 **小程序 → Palpo**（移动布局中在 **发现** 下），使用当前登录的 Matrix 账号。Palpo 小程序负责服务器关联审批、资源、项目和 agent。Hagency 单独运行在已登录本机 Codex 的机器上。

1. 在 Hagency 中发起 **新建服务器关联**，填写 Matrix 地址以及已有的资源所有者和协调者账号。
2. 资源所有者在 Rinx 的 **Palpo → Inbox** 确认，再由 Palpo 管理员批准。配置自动交付给 Hagency；所有者点 **验证连接**，等待 **连接已验证**。
3. 在 Hagency 的 **我的资源 → 新建资源配置** 为关联创建资源，设置模型、预算和可申请项目的用户。每个资源只有一份预算，无需再分配第二层资源池。
4. 有资格的项目负责人在 Rinx 的 **资源 → 在此申请项目** 提交申请。项目获批后，在 **项目 → 申请 agent** 提交 agent 申请。协调者在 Palpo Inbox 处理这两类申请。
5. 私聊邀请出现后先接受，等待 **执行（Execution）· ready** 再发送消息。所有者私聊无需 @，共享项目房间需要 @。

[Hagency 快速开始指南](docs/hagency-quickstart.zh-CN.md)介绍角色分工、首次资源配置、已有项目房间、加密和常见问题。[Agent-chat 开发说明](docs/agent-chat.md)介绍客户端协议。

## 设置与主题

桌面和移动布局共用 **设置**，都有 **账号、偏好设置、隐私、关于**。移动布局从 **我 → 设置** 进入。

主题入口是 **设置 → 偏好设置 → 应用外观**：选择浅色／深色、在支持的平台上跟随系统、修改强调色，或打开 **自定义外观**。托管 Rinx 可能显示外观由 OctoSense 管理。

默认构建已启用 `agent_chat`。**偏好设置 → Hagency** 只控制工作流命令建议；关闭该开关仍可使用审批卡片和 Palpo 小程序。Rinx 不会自行启动 Hagency 服务。

## 内置应用

`system-apps.json` 定义随 Rinx 发布的原生和 OctoScript 应用。原生文章编辑器位于 `apps/article-editor/`；共享文档和 Makepad 控件库保留在 `crates/`。

Palpo 小程序位于 `apps/palpo/`，随 Rinx 内置；权限由当前 Matrix 账号及服务器授权决定。

独立版由 Rinx 管理 Octos 运行时和提供商配置；托管版使用 OctoSense 注入的服务、共享内核与配置。小程序通过受限的宿主接口访问 Matrix 和 Octos，不启动自己的内核。

应用目录构建验证、开发和发布流程见 [应用开发指南](apps/README.zh-CN.md)；架构决策见 [ADR 0008](docs/adr/0008-rinx-system-app-catalog.md)。
