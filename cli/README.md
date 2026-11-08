# islo CLI

在 **设置 → 通用 → 命令行接口** 中开启。islo 会把安装目录加入当前用户 PATH，无需管理员权限；重新打开终端后可直接运行 `islo-cli`。开关关闭时拒绝应用控制请求，并移除由 islo 添加的 PATH 项；用户原先已有的 PATH 项会保留。可点“重新注册命令”修复注册。卸载时自动撤销注册，升级保留开关状态。

卸载清理由本版安装包生成的卸载器支持。如果从旧版仅用 ZIP/应用内更新替换了程序，旧卸载器仍可能保留 PATH 项；可以先关闭 CLI 开关，或运行本版安装包刷新卸载器。

```powershell
islo-cli capabilities
islo-cli app status
islo-cli tasks add --text "提交报告" --due "2026-11-07 15:00"
islo-cli tasks list --limit 50 --offset 0
```

`capabilities` 返回实际支持的命令、参数类型、必填字段、离线能力和时间规则，开启接口前也可以查看。Agent 应先发现能力，执行后检查退出码与 `ok`，再查询确认。`result` 是领域操作结果；设置命令继续使用原有 `settings` 字段。

## 功能命令

| 领域 | 操作 | 参数 |
| --- | --- | --- |
| 待办 | `tasks list` | 可选 `--limit 1..100 --offset N`，默认 50 条 |
| 待办 | `tasks add` | `--text TEXT`，可选 `--due "YYYY-MM-DD HH:MM" --repeat 0..127` |
| 待办 | `tasks update` | `--reference REF`，以及至少一个 `--text` / `--due` / `--repeat` |
| 待办 | `tasks complete` / `reopen` / `remove` | `--reference REF` |
| 本机日历 | `calendar list` | 可选分页参数 |
| 本机日历 | `calendar add` | `--title TITLE --start "YYYY-MM-DD HH:MM" --end "YYYY-MM-DD HH:MM"`；可选 `--reminder MINUTES` |
| 本机日历 | `calendar remove` | `--reference REF` |
| 日历连接 | `calendar status` / `sync` | 查询连接状态 / 请求同步远端日历 |
| 专注 | `focus status` / `start` / `pause` / `reset` | 状态 / 继续或开始 / 暂停 / 重置当前阶段 |
| 专注 | `focus phase` | `--phase focus` 或 `--phase rest`；切换后暂停 |
| 饮水 | `water status` / `drink` / `undo` | `drink --ml 1..6000`；撤销最近一次记录 |
| 文件架 | `shelf list` / `add` / `remove` | `add` / `remove` 使用 `--path PATH`，`list` 支持分页 |
| 媒体 | `music status` / `toggle` / `next` / `previous` | 查询 / 播放暂停切换 / 上一首下一首 |
| 媒体 | `music seek` / `volume` / `mute` | `seek --seconds N`；`volume --percent 0..100`；切换静音 |
| 邮件、通知 | `mail status` / `notifications status` | 账号连接状态、最近邮件条数 / 通知队列条数 |
| AI 额度 | `quota status` / `refresh` | 查询各账号额度快照 / 请求刷新 |
| 应用 | `app status` | 版本、本地时间、数据条数、编辑器忙碌状态 |

- 时间按电脑的本地时区解析，`app status` 返回包含 UTC 偏移的当前时间。Agent 负责把“明天下午三点”换算成明确的本地日期时间。无效日期、夏令时歧义或不存在的时间会被拒绝。
- 一次性待办不能新增过去的提醒。重复待办 `--repeat` 使用星期位：周一为 1、周二为 2，直到周日为 64；每天 127、工作日 31、周末 96、一次性 0。重复提醒只使用 `due` 的时分。
- `tasks update --due none` 清除提醒；不传的字段保留。完成重复待办只完成当天；重新打开不会自动重发已经发过的提醒。
- `reference` 是查询快照引用，不能长期保存。待办列表有任何变化（包括提醒状态更新）后应重新查询；过期引用会被拒绝，避免列表重排时改错项目。
- 全天日程使用 `--all-day true --start YYYY-MM-DD --end YYYY-MM-DD`，结束日包含在内。省略 `--reminder` 表示不提醒；允许 0..10080 分钟。全天日程启用提醒后于当天 09:00 提醒。
- 日历列表管理本机日程。远端日历通过 `calendar status` 查询连接、`sync` 请求同步，不修改远端日程。
- 文件架只记录文件路径，不复制或删除原文件。相对路径由 CLI 按调用者的当前目录解析。
- 专注计时、饮水记录、媒体、邮件、通知、额度和连接状态需要应用运行。待办、本机日程、文件架和设置支持离线保存。**提醒触发需要 islo 运行**。
- GUI 正在编辑待办/日历时，相应写操作返回忙碌错误，保留未保存内容。保存失败不会将 GUI 内存改成未落盘的数据。
- 媒体控制、日历同步、额度刷新返回 `accepted: true, completed: false`，表示已加入应用处理队列；目标应用可能不支持操作、刷新可能被限流，必须再查询状态。
- 密码、验证码内容、登录文件、Cookie 和令牌不出现在状态输出中；通知只返回计数。设置接口不能修改 `cli_enabled`，请使用界面开关。
- 命令超时表示结果未知，**先查询再决定是否重试写操作**，避免重复创建任务。

## 全屏待办提醒

在设置的通知页面开启“全屏待办提醒”，也可运行 `islo-cli settings set fullscreen_task_reminders 1`。到点后，会在 islo 所在屏幕显示独立的置顶全屏提示，直到选择“完成待办”“10 分钟后提醒”或“暂不处理”；Esc/关闭窗口等同于暂不处理，不会把任务标为完成。它单独于岛体的“全屏时隐藏”设置生效，仍遵循总通知开关。

未处理状态会保存，重启后恢复；稍后提醒不改变任务的原定时间或每周重复周期。`tasks list` 返回 `reminder_pending`、`snoozed_until`；`notifications status` 返回待处理数量和全屏开关状态。通过 CLI 完成、删除任务或修改提醒时间，会关闭对应的待处理提示。首次开启不会重播已经提醒过的旧任务。

全屏页随机使用内置 Meteocons 晴天、晴间多云或星夜动画及相应背景，运行时离线播放。动态安全输入框开启期间，待办保留待处理状态，暂不覆盖输入窗口。

## 所有 Agent / CLI 配置的输入规范

普通设置、地址、服务类型等由 Agent 通过 CLI 提供。缺失普通字段时返回字段要求或校验错误，由 Agent 补齐，不打开完整设置表单。只有确实需要密码、授权码、API Key、令牌等敏感值，才显示独立的小型本地弹框：用途与连接目标、一个遮蔽输入框，以及取消／确认。Enter 确认，Esc／关闭取消；复用已有凭据时不弹框。

`capabilities.interaction_policy` 声明这条全局规范。它不代表尚未实现的配置命令已经可用；不支持的敏感配置须明确报告，不能改为在聊天、参数、stdin、JSON 中收集秘密或绕过应用写凭据。开发新领域时复用 `src/agent_input.rs` 的弹框、独占输入保护和剪贴板记录暂停／恢复；敏感值留在本机凭据管理与目标操作之间，Agent 只查询状态。

## 邮箱配置与本地敏感输入

```powershell
islo-cli mail list
islo-cli mail configure
islo-cli mail configure --email user@example.com --provider exmail
islo-cli mail configure --email user@example.com --enabled false
islo-cli mail configure --email user@example.com --replace-secret true
islo-cli mail input-status --id REQUEST_ID
islo-cli mail input-cancel --id REQUEST_ID
islo-cli mail remove --email user@example.com
```

`mail list` 支持 `--limit` / `--offset`，也可离线读取账号元数据。其余账号操作需要应用运行并开启 CLI。配置参数全部可选：`--email`、`--provider`、`--enabled true|false`，以及自定义 IMAP 的 `--host`、`--port`、`--username`；服务键以 `capabilities` 中的 `mail_providers` 为准。仅支持与 GUI 相同的 IMAP over TLS。

缺少邮箱地址、服务类型或自定义主机时，返回 `status: needs_metadata` 和 `required_fields`，不弹框、不创建输入请求。Agent 补齐普通字段再运行命令。需要秘密时才弹出小型遮蔽输入框，标签根据服务显示为授权码、应用专用密码或 IMAP 密码。已有账号只改普通设置时复用已有凭据；修改连接目标或 `--replace-secret true` 会要求重新输入。连接目标显示在弹框中。

需要输入时，响应为 `status: awaiting_input`，包含请求 `id` 和敏感字段描述，**不包含字段值**。用户在本机确认后，用 `mail input-status` 查询 `saved`，或查询 `cancelled` / `expired`。窗口保留 15 分钟，重启会丢弃未完成的输入请求。等待输入时不要重复创建请求或读取输入框、截图及剪贴板获取敏感值。

密码只从本地输入框进入 Windows 凭据管理器，不接受 `--password`、JSON 密码字段或 stdin 密码输入，也不会返回给 Agent。输入期间暂停 islo 自身的剪贴板记录，关闭窗口后恢复用户原有开关；已输入的敏感值会从 islo 自身的匹配历史条目中清理。账号文件仅保存邮箱、类型、启用状态和连接信息。

`saved` 表示配置已保存，后台会按现有邮件总开关开始或停止监控；不表示服务器连接已验证，应另外查询 `mail status`。保存账号文件失败时会回滚凭据更新。删除账号返回 `credential_removed`，旧连接目标清理返回 `old_credential_cleanup`；为 false 时不要声称凭据已完全清理。

Outlook／Hotmail 使用 `--provider outlook`，在微软官方网页登录授权，不输入密码／授权码。产品 Client ID 通过普通设置 `outlook_client_id` 配置，发行版也可内置公开构建变量 `ISLO_OUTLOOK_CLIENT_ID`。缺少该配置返回 `needs_metadata`；已开始网页授权返回 `awaiting_authorization`，复用 `mail input-status`／`mail input-cancel`。Agent 不读取登录页或回调 URL，不得到授权码或令牌。配置与验证边界见 [Outlook 研发说明](../docs/OUTLOOK.md)。

CLI 和桌面程序属于同一个 Cargo 项目，共享 `src/model.rs` 的设置键、校验、规范化与原子保存；`src/agent_control.rs` 负责运行中应用的命令交接。入口位于 `cli/src/main.rs`。

```powershell
cargo build --locked --bin islo-cli
.\target\debug\islo-cli.exe settings list
.\target\debug\islo-cli.exe settings get pet_expression
.\target\debug\islo-cli.exe settings set pet_expression laughing
.\target\debug\islo-cli.exe settings set language en
```

安装版将 CLI 放在 `%LOCALAPPDATA%\Programs\islo\islo-cli.exe`，与桌面应用一起升级。命令成功时输出单个 JSON 对象；失败时向 stderr 输出 `{"ok":false,"error":"..."}` 并返回非零退出码。

## 修改规则

- 先用 `settings list` 查看完整可用键及当前值。值沿用配置文件格式：布尔值使用 `0` / `1`，时长使用十进制整数字符串，列表使用紧凑 JSON。
- 无效键、越界值和不完整的列表会被拒绝，批量操作失败时不会保存其中一部分。
- 开启 `pet=1` 会退出极简模式，开启 `minimal=1` 会关闭伙伴，与界面开关保持一致。
- 固定表情：`pet_expression` 支持 `laughing`、`happy`、`curious`、`music` 等已有表情键；`auto` 恢复自动模式。伙伴显示还取决于 `pet=1`、`minimal=0`、`dock=top` 及自动避让状态。
- 应用正在运行时，由应用本身处理修改并返回最新状态，响应中的 `backend` 为 `running-app`。应用关闭时 CLI 直接原子保存，`backend` 为 `offline`，下次启动生效。
- 如果运行中的版本不支持 agent 命令，CLI 返回错误，不覆盖其配置。请求超时时结果未知，应先重新查询，再决定是否重试。
- 此接口覆盖 `settings list` 列出的持久化设置。邮箱/日历账号凭据、AI 登录文件和系统开机启动项不属于该接口。

## 批量修改

将字符串值写入 `patch.json`，例如：

```json
{"pet":"1","pet_expression":"laughing","language":"zh-CN","focus_minutes":"45"}
```

```powershell
.\target\debug\islo-cli.exe settings apply --file patch.json
```

## 隔离测试

```powershell
.\target\debug\islo-cli.exe --data-dir .\target\cli-sandbox settings set focus_minutes 45
```

`--data-dir` 的隔离目录按离线方式处理，适合测试；指向默认目录时仍与运行中的应用同步。日常操作直接使用默认目录。
