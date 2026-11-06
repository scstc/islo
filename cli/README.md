# islo CLI

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
