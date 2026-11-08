# Outlook / Hotmail 接入研发说明

islo 支持个人 Microsoft 账号的 Outlook / Hotmail 邮箱，使用原有 IMAP 收信与通知服务，认证改为 OAuth2 + SASL XOAUTH2。当前不包含组织租户、共享邮箱、POP 或 SMTP 发信。

## 产品方配置

产品方需要注册属于 islo 的 Microsoft 应用。Client ID 是公开标识，可在发行构建中配置，不使用其他邮件客户端的应用 ID，也不创建或收集客户端密钥。

1. 在 [Microsoft Entra 应用注册](https://learn.microsoft.com/en-us/entra/identity-platform/quickstart-register-app)中创建桌面应用，支持个人 Microsoft 账号（也可注册支持组织和个人账号的应用，但当前 islo 仅使用个人账号入口）。
2. 添加「移动和桌面应用程序」平台，登记重定向 URI **`http://localhost/oauth/callback`**。islo 绑定随机本机端口，微软对 localhost 重定向忽略端口，路径必须一致。不要登记为 Web／SPA，不需要 client secret。
3. 使用委托权限 `https://outlook.office.com/IMAP.AccessAsUser.All` 和 `offline_access`。不申请 SMTP.Send、应用级邮箱权限或租户管理权限。IMAP 的该权限由微软定义为完整 IMAP 邮件访问，islo 的接入仅沿用现有收信逻辑。
4. 为正式发行构建设置公开变量 `ISLO_OUTLOOK_CLIENT_ID`；或在开发版「设置 → 邮件 → Outlook → Microsoft 应用配置（开发者）」中填写 Client ID。CLI 可使用 `islo-cli settings set outlook_client_id CLIENT_ID`。空／无效 ID 返回普通参数要求，不弹密码框。

相关协议：[授权码 + PKCE](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow)、[localhost 重定向](https://learn.microsoft.com/en-us/entra/identity-platform/reply-url#localhost-exceptions)、[IMAP OAuth2](https://learn.microsoft.com/en-us/exchange/client-developer/legacy-protocols/how-to-authenticate-an-imap-pop-smtp-application-by-using-oauth)。

## 用户与 Agent 流程

用户先在 Outlook 网页设置中启用 IMAP，然后在 islo 选择 Outlook 并填写邮箱。点击「登录并连接 Outlook」后，只在微软官方网页登录并授权。取消 islo 的授权等待窗口、关闭或按 Esc 会中止本次接入；已有账号、凭据不变。

```powershell
islo-cli mail configure --email user@outlook.com --provider outlook
islo-cli mail input-status --id REQUEST_ID
islo-cli mail input-cancel --id REQUEST_ID
islo-cli mail status
```

`needs_metadata` 表示需要先设置产品的公开 Client ID；`awaiting_authorization` 表示正在等待微软网页授权，Agent 只得到请求 ID 与状态。授权回调不返回 CLI，禁止读取浏览器密码、授权页、回调 URL 或剪贴板来提取秘密。授权成功后，先用所得访问令牌登录目标邮箱并打开 INBOX，再原子保存账号和刷新令牌；密码不会出现在 islo 的 Outlook 表单中。

已有相同 Client ID 的凭据可复用。显式重新授权使用界面的「重新授权」，或 `mail configure --email user@outlook.com --provider outlook --replace-secret true`。更换 Client ID 后需要重新授权。`saved` 表示本机已保存，`connection_status: verified` 表示初始邮箱验证通过；后台监控仍遵循邮件总开关，实时状态以 `mail status` 为准。

访问令牌在连接时通过刷新令牌获取；轮换后的刷新令牌写入同一系统凭据。凭据更新使用比较后替换，删除账号或新的授权会使旧的刷新结果失效。微软撤销授权或刷新失败时会显示需要重新登录，不会回退到密码登录。

## 验证边界

本地测试覆盖 PKCE、回调 state／Host／路径与重复参数校验、取消请求不落盘、响应脱敏、刷新轮换和旧结果保护。真实 Microsoft 授权需要产品方注册的有效 Client ID、可访问微软的网络和测试账号；没有这些条件时，不应宣称已完成线上授权或真实收信验证。
