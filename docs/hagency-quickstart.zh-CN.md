[English](hagency-quickstart.md) | [简体中文](hagency-quickstart.zh-CN.md)

# 在 Rinx 使用 Hagency：快速开始

通过 Rinx 批准 Hagency 连接、申请项目与 agent，并与 agent 聊天。Hagency 在
运维者的机器上运行 Codex；Rinx 是 Matrix 客户端。本指南对应当前源码中的
Palpo 小程序与 Hagency 服务器关联流程。

## 准备账号与本机运行环境

| 角色 | 负责什么 |
| --- | --- |
| 资源所有者／Hagency 运维者 | 运行 Hagency、确认连接、创建资源、委托协调者。 |
| Palpo 管理员 | 批准服务器关联。 |
| 协调者 | 审查关联中的项目、agent 和追加 token 申请。 |
| 项目／agent 所有者 | 申请项目和 agent、聊天、批准受保护操作。 |

一个人可以担任多个角色。使用同一 Matrix 服务器上已有的账号，完整 ID 例如
`@owner:example.org`。协调者是 Matrix 用户，不需要通过 Hagency 创建一个机器人。
同机测试多个账号时使用独立的 Rinx 配置目录，见[数据与兼容性](../README.md#data-and-compatibility)。

运维者在运行 agent 的机器上安装并登录 Codex，启动 Hagency，打开本地控制台：

```sh
codex login
hagency start
```

通过 **设置 → 编程代理** 检查检测到的 Codex 及运行配置。agent 工作期间，机器
和服务需保持运行。安装、服务模式和首次来源配置详见
[Hagency 使用指南](https://github.com/hagency-org/hagency-rs/blob/main/docs/user-guide/README.zh-CN.md)。

在 Rinx 登录 Palpo，设置交叉签名并验证会话，以便使用加密审批室。打开
**小程序 → Palpo**（移动布局中在 **发现** 下）。首次使用时检查权限并点击
**运行（Run）**。小程序使用当前 Matrix 账号，无需在其中填写密码或 Codex 密钥。

主要标签为 **Inbox、Projects、Agents、Resources**。在 **More（更多）** 中打开
**Engagements（服务器关联）**、**Notifications（通知）** 和 **My Actions（我的待办）**；
管理员工具仅在当前账号有权限时显示。

## 1. 建立服务器关联

1. **运维者，在 Hagency 中**：在 **设置 → 连接 Palpo** 或 **服务器关联** 页面
   点 **新建服务器关联**。填写 HTTPS Matrix 地址、资源所有者和协调者的 Matrix
   ID、名称和授权期限。只有管理员提供单独的管理服务地址时，才填写
   **独立的 Palpo 地址**。点击 **发起关联**。
2. **资源所有者，在 Rinx 中**：打开 **Palpo → Inbox**，确认是自己发起的连接后
   批准。Hagency 的 **在 Rinx 打开请求** 也会进入同一条请求。
3. **管理员，在 Rinx 中**：在自己的 Palpo Inbox 批准服务器配置。
4. Hagency 自动接收已批准的配置。**通过这个入口发起的新连接无需下载／导入 JSON。**
5. **资源所有者，在 Rinx 中**：打开已批准的请求，点击一次 **验证连接（Verify
   connection）**。验证期间按钮禁用，等待两个应用都显示 **连接已验证**。

已有正常关联时，直接在下方流程添加资源即可，无需撤销再重连。委托表单中的
**取消** 只是丢弃未保存的编辑；撤销是另一项变更，不会删除已有 agent、历史或
预留额度。

## 2. 在 Hagency 创建资源

1. 打开 **我的资源 → 新建资源配置**。
2. 选择已验证的 **服务器关联** 和本地 **来源配置**。来源提供框架、提供方和
   账号；搜索框按模型／框架／推理档位过滤，不是填写 agent 或项目名。
3. 选择模型、推理档位，填写每月 token 预算。
4. 填写 **可申请项目的 Matrix 用户**，使用同一服务器上的完整 ID，以空格或
   换行分隔，例如 `@project-owner:example.org`。
5. 点击 **创建资源**，等待它出现在有资格用户的 Rinx **Palpo → 资源** 目录中。

每个资源在关联下只有一份预算。一个关联可以有多个资源，之后无需再分配第二层
资源池。共享账户额度未知不代表实测为零，也不代表服务商保证的容量。

全新 Hagency 状态目录中，向导目前需要运维者先通过文档中的
[运维 API](https://github.com/hagency-org/hagency-rs/blob/main/README.zh-CN.md#用运维-api-创建资源)
创建第一个本地来源。设置页面会配置 Codex 运行环境，但不会创建这个来源。已有
来源后，关联资源统一通过上述向导创建。

## 3. 在 Rinx 申请项目和 agent

1. **项目所有者**：在 **Palpo → 资源** 中，点某个资源的 **在此申请项目（Request
   project here）**，填写项目名称和用途。
2. 让 Palpo 创建新房间，或点 **选择已有房间（Choose an existing room）**。
   已有房间必须是你创建的、私密的（仅邀请加入）、未加密且不是 Space。
   Palpo 会验证所选房间并邀请关联的代表账号。
3. 提交。**协调者** 在 **Palpo → Inbox** 批准或拒绝项目；**所有者** 在
   **项目** 中等待配置完成。
4. **所有者** 在项目上点 **申请 agent（Request agent）**，选择资源和角色，
   填写 agent 名称、初始 token 数和每日速率，然后提交。
5. **协调者** 在自己的 Palpo Inbox 审查并批准或拒绝申请额度。Hagency 收到决策
   后检查当前授权和容量。
6. **所有者** 查看 **Agents → 打开最新结果（Open latest result）**。
   **已批准（Approved）** 是审批结果；**Waiting for Hagency（等待 Hagency）／
   Preparing agent（正在配置 agent）** 表示仍在配置运行环境。
   测试聊天前等待 **Ready to chat（可以聊天）**。

项目审批通过代表获得资源使用资格，并未预留 agent 的 token。项目房间用于共享
讨论和 @ 提及 agent；本地工作目录、加密所有者私聊和加密审批室是各自独立的。

## 4. 聊天与操作审批

- 接受 agent 的私聊邀请，发一条简短消息。**所有者私聊无需 @，第一条也不需要。**
  私聊里只保留你和 agent。
- 在共享项目房间中 @ 提及 agent，后续消息放在同一讨论串。agent 不在与其他人
  共享的加密群聊中工作。
- 在所有者审批室用卡片按钮批准受保护操作。文字回复不算批准。这与 Palpo Inbox
  中的项目和 token 审批是不同决策。
- 需要时使用 **Palpo → Agents → 申请更多 token（Request more tokens）**，由
  协调者审批。在 **Manage agent（管理 agent）** 中打开重命名、暂停、恢复和移除；
  这些操作只在当前账号有权限时显示，操作后查看最新运行结果。

## 排查问题，保留原请求

| 看到什么 | 检查什么 |
| --- | --- |
| Inbox 为空 | 当前 Matrix 账号及筛选。先由所有者确认，再由管理员批准；项目和 agent 由协调者审批。 |
| 资源为空 | 连接已验证、资源已发布、可申请人 ID 正确、委托有效。 |
| 项目没有就绪 | 查看配置说明。只有服务器提供入口时，使用 **重试项目配置**。 |
| 已批准但 Preparing agent | 配置仍在进行。私聊邀请出现后接受，并继续查看执行状态。 |
| Needs attention，配置结果不确定 | 请 Hagency 运维者检查原始尝试、日志及 Matrix 状态。账号／房间可能已创建，token 可能仍预留，不要重复申请。 |
| Needs attention，运行环境不可用 | agent 已存在，但运行环境或 Matrix 连接需要在 Hagency 主机处理。 |
| Ready to chat 但私聊无回复 | 确认你是所有者、没有其他成员；请运维者检查密钥交换和消息接收。 |
| 审批卡片无法解密 | 验证当前 Rinx 会话，或使用其他已验证会话。 |

## 设置和主题

桌面与移动布局共用 **设置 → 账号／偏好设置／隐私／关于**。移动布局从
**我 → 设置** 进入。主题在 **偏好设置 → 应用外观**；工作流命令建议在
**偏好设置 → Hagency**。审批卡片和 Palpo 小程序无需开启工作流命令建议。
详见[设置与主题](../README.zh-CN.md#设置与主题)。
