# Xenobot TodoWrite 状态对账

更新时间: 2026-02-24

状态定义:
- 完成: 已有可运行实现并通过当前构建/测试验证, 确保是质量和效果极致完美状态。
- 完成(简化): 有可用实现，但仍是降级/占位/本地回退实现，非目标最终形态, 需要最终补全至质量和效果极致完美状态。
- 进行中: 已有框架或部分实现，尚未闭环, 需要最终闭环于质量和效果极致完美状态。
- 未完成: 当前代码未实现, 需要最终实现为质量和效果极致完美状态。
- 替换(合法安全): 原任务与合法安全边界冲突，改为合法安全替代项, 需要最终变为质量和效果极致完美状态。

## 1. 项目设置和基础设施
- 1.1 检查并补全所有空文件和空目录: 完成(并清理前端`public/images`中未被引用的历史遗留素材`chatlab.svg`、`intro_en.png`、`intro_zh.png`，降低参考项目痕迹与仓库体积)
- 1.2 配置Cargo.toml文件，确保所有依赖项正确: 完成
- 1.3 设置构建脚本和开发环境: 进行中(已强化沙箱受限环境运行路径：`api start`支持TCP失败自动回退UDS；当TCP+UDS都被拒绝时自动切换到文件网关IPC模式，无需端口监听；`api smoke`可用于纯进程内健康校验；新增`api gateway-stress`并发压测入口用于文件网关稳定性验证；已执行安全构建垃圾清理：删除`target/debug/{deps,incremental,build}`历史缓存后从约30GB降到<1GB，再在重编译回归后稳定在约4.6GB；已统一Rust crates许可证为`AGPL-3.0-only`并补齐仓库地址`https://github.com/AuroraMackenzie/Xenobot.git`)
- 1.4 配置Metal MPS GPU加速支持: 进行中

## 2. 数据库层和模式迁移
- 2.1 设计数据库模式（基于ChatLab的schema）: 完成
- 2.2 实现SQLx迁移系统: 完成
- 2.3 实现连接池和仓库模式: 完成
- 2.4 实现数据模型（消息、联系人、群聊、会话等）: 完成

## 3. 数据提取和实时监控（chatlog功能）
    // WhatsApp、LINE、微信、QQ、Discord、Instagram、Telegram等
- 3.0 多平台合法安全数据源发现（source scan + 平台候选路径）: 完成(简化)
  覆盖清单(当前): WeChat(微信), WhatsApp, LINE, QQ, Discord, Instagram, Telegram, iMessage, Messenger, KakaoTalk, Slack, Teams, Signal, Skype, Google Chat, Zoom, Viber
- 3.1 进程内存访问密钥提取: 替换(合法安全)
- 3.2 多平台数据库解密（V3/V4）: 进行中(合法安全替代路径已补强：授权导出文件写库阶段新增`import_source_checkpoint`持久化检查点，支持按源文件指纹跳过未变化数据，减少重复写入与回归漂移；CLI `import --write-db`在中途失败时已补写`failed`检查点，保证失败可追踪；API平台识别短别名逻辑已修复为token匹配，避免`signal`被误判为`instagram`等串匹配错误)
- 3.3 文件监控（fsnotify）和自动增量解密: 完成(简化)(已将非微信平台`monitor --start`升级为“系统文件事件监听(`notify`) + 周期轮询兜底”混合监控；检测新/变更导出文件并触发解析器增量识别；新增`monitor --write-db`可选增量写库+去重+webhook回调；自动增量解密链路仍需与各平台授权导出/解析管线进一步闭环)
- 3.4 多账号管理: 完成(简化)
- 3.5 多媒体处理：图片解密(.dat)、语音转码(SILK->MP3): 进行中

## 4. 多平台解析器（ChatLab功能）
- 4.1 实现格式嗅探器: 完成(已新增API集成回归：17平台样本路径逐一校验`/detect-format`平台识别结果；`/detect-format`已优先尝试analysis解析器并返回`parserSource=analysis|builtin`可观测来源)
- 4.2 实现17个解析器: 完成(简化)(已增加注册完整性回归测试，确保17个平台解析器持续可见；API侧新增`/supported-formats`清单断言测试，防止17平台格式条目回退)
  平台覆盖清单(当前注册): WeChat(微信), WhatsApp, LINE, QQ, Telegram, Discord, Instagram, iMessage, Messenger, KakaoTalk, Slack, Teams, Signal, Skype, Google Chat, Zoom, Viber
- 4.3 实现流式导入和增量导入: 进行中(已提供CLI解析预览+可选写库闭环[api+analysis]，含基础增量去重与`import_progress`状态持久化: pending/importing/completed/failed；新增`import`与`monitor --write-db`共享的源文件检查点闭环：未变化文件自动增量跳过、变化文件继续去重写库并回写检查点统计；`monitor --write-db`已将检查点短路前移到“解析前”，减少无效解析开销；API侧`/sessions/:id/analyze-incremental-import`与`/sessions/:id/incremental-import`已接入同一检查点语义，并在失败路径回写checkpoint状态；CLI侧`import --write-db`失败路径已补写`failed`检查点；API导入解析已优先接入`xenobot-analysis` 17平台解析器并在失败时回退内置解析器；已新增WhatsApp原生TXT集成回归验证analysis解析路径（发送者身份提取不退化到text-importer）；新增API集成回归：17平台矩阵(WeChat/WhatsApp/LINE/QQ/Telegram/Discord/Instagram/iMessage/Messenger/KakaoTalk/Slack/Teams/Signal/Skype/GoogleChat/Zoom/Viber)下验证“导入->增量重复->checkpoint快跳->源变化增量写入->写库计数一致”) 
- 4.4 实现批量导入和合并导入: 进行中(目录批量解析已支持，`--merge` 合并写库基础版已接入；API侧已新增`/import-batch`端点支持separate/merged两种批量模式：separate模式已接入`api-import-batch-separate`检查点快跳、失败重试(`retryFailed/maxRetries`)与失败回写，并补全成功checkpoint的platform/chat_name元数据；merged模式已补齐“导入前checkpoint快跳 + 解析失败/空消息失败回写 + 跨文件去重 + `api-import-batch-merged`检查点回写”，并在“全部未变化输入”场景下返回checkpoint-only结果且不新建session；多聊天文件链路已加集成回归：`/scan-multi-chat-file`扫描 + `/import-with-options(chatIndex)`按索引导入并验证写库结果)

## 5. API服务器（HTTP）
- 5.1 设计Axum HTTP API端点: 完成(新增`/import-batch`端点用于批量/合并导入)
- 5.2 聊天记录查询（时间范围、对话方、关键词、分页）: 完成
- 5.3 联系人、群聊、会话列表端点: 完成
- 5.4 多媒体路由（图片、视频、文件、语音）: 完成(简化)
- 5.5 健康检查和状态端点: 完成(已在API server启动路径接入数据库初始化；已支持`api start --unix-socket`配置UDS监听；新增严格沙箱下可用的`api smoke`进程内健康检查，无需端口/Unix监听；新增TCP绑定失败自动回退UDS、UDS路径长度限制检查、socket文件权限模式应用；并新增TCP+UDS均被拒绝时自动进入文件网关IPC模式，支持`req_<id>.json`请求与`resp_<id>.json`响应；文件网关新增运行态指标落盘`gateway_metrics.json`并在`api status`中展示核心指标)

## 6. 分析引擎
- 6.1 基础统计（活跃度、时间分布）: 完成
- 6.2 高级分析（夜猫子/龙王/潜水/打卡）: 进行中
- 6.3 行为分析（表情包大战、@分析、笑声）: 进行中
- 6.4 社交分析（社交网络聚类图）: 完成
- 6.5 复读分析和口头禅分析: 进行中

## 7. AI代理系统（12个工具）
- 7.1 AI代理执行器（FC循环最多5轮）: 完成(简化)
- 7.2 12个AI工具: 进行中
- 7.3 集成LLM（OpenAI兼容、Gemini、DeepSeek、硅基流动(Silicon Flow)）: 完成(简化)
- 7.4 API密钥加密存储: 完成(简化)

## 8. RAG和语义搜索（GPU加速）
- 8.1 文本分块: 未完成
- 8.2 Embedding向量生成（Candle + Metal MPS）: 进行中
- 8.3 向量存储: 完成
- 8.4 语义搜索（余弦相似度）: 未完成
- 8.5 查询改写: 未完成

## 9. SQL Lab和自定义查询
- 9.1 安全SQL查询接口（仅SELECT）: 完成
- 9.2 Schema面板和结果表格: 完成
- 9.3 AI辅助SQL生成: 完成(简化)

## 10. MCP服务器（SSE和HTTP协议）
- 10.1 MCP协议（Streamable HTTP和SSE）: 完成(简化)
- 10.2 MCP工具（联系人、群聊、最近会话、聊天记录、当前时间）: 完成(简化)
- 10.3 集成Claude Desktop、ChatWise、Opencode等AI助手: 完成(简化)

## 11. TUI界面（ratatui）
- 11.1 终端UI（菜单、状态栏、表单）: 进行中
- 11.2 实时状态显示: 进行中
- 11.3 菜单操作（获取密钥、解密、服务控制）: 进行中

## 12. CLI命令行工具
- 12.1 CLI命令结构（clap）: 完成(已补齐`api start/status/stop/restart/smoke`可执行路径，含API状态文件与PID信号控制；`api start`新增`--unix-socket-mode`八进制权限参数与文件网关参数`--file-gateway-dir/--file-gateway-poll-ms/--file-gateway-response-ttl-seconds`；新增`api gateway-stress`并发压测命令；已修复`monitor`子命令与自动`--version`参数冲突导致的运行时panic，统一改为`--wechat-version`)
  可执行入口: 已提供 `xenobot-cli` binary (`crates/cli/src/bin/xenobot-cli.rs`)
- 12.2 密钥提取、解密、监控命令: 进行中(已支持多平台format参数、source scan、非微信平台`monitor --start`文件事件监听+轮询兜底增量解析，以及`monitor --write-db`增量写库)
- 12.3 数据查询和导出命令: 完成(简化)(已支持query: search/sql/semantic回退；export: jsonl/text/csv/json/html 基础导出)

## 13. 多媒体处理
- 13.1 .dat图片解密（XOR + AES）: 完成(简化)
- 13.2 SILK语音转MP3: 完成(简化)
- 13.3 实时解密/转码（不落盘）: 未完成

## 14. Webhook和实时通知
- 14.1 Webhook回调: 完成(简化)(已在CLI与API的import写库路径接入“新消息到达->HTTP POST”回调执行，含过滤与失败重试；非微信`monitor --write-db`路径亦接入)
- 14.2 过滤配置: 完成(简化)(event_type/sender/keyword 过滤规则已支持并持久化，且已抽取到core共享匹配逻辑供CLI/API复用)
- 14.3 延迟优化: 完成(简化)(已在CLI+API接入基础重试退避 + webhook事件队列刷新 + 并发调度；CLI含独立后台worker；已接入dead-letter持久化与CLI list/retry/clear；API与web进程已接入可配置自动dead-letter重放后台调度)

## 15. 测试和性能优化
- 15.1 单元测试和集成测试: 进行中(新增core webhook匹配/dead-letter/统计单测 + API dead-letter重放选择逻辑单测 + CLI单测: SQL安全校验/过滤匹配；新增API回归测试：`import_source_checkpoint` upsert/unchanged判定、`message_exists`增量写库一致性；新增API集成测试`chat_incremental_test.rs`覆盖增量导入checkpoint快跳、源文件变化增量写入、失败写回、17平台矩阵回归、17平台detect-format识别、supported-formats条目完整性、多聊天scan+chatIndex导入闭环、`/import-batch`合并模式去重与检查点回写、`/import-batch`分离模式重试与checkpoint快跳、`/import-batch`合并模式checkpoint快跳且不新建session、WhatsApp原生TXT解析器身份提取验证（避免退化到text-importer）；新增CLI回归测试：源文件内容指纹变化检测与monitor检查点短路匹配；本轮对API/CLI相关改动已执行`cargo test -p xenobot-api -p xenobot-cli --features \"api,analysis\" --offline`通过；跨模块集成测试仍需扩展)
- 15.2 性能优化（索引/查询/缓存）: 进行中(新增迁移`003_import_performance_indexes.sql`：dedup查询与platform+chat_name定位索引；新增迁移`004_import_source_checkpoint.sql`：增量检查点表及状态/时间索引，支撑授权导出增量闭环；源文件指纹已从元数据模式升级为`v2`流式内容哈希，降低误跳过概率)
- 15.3 内存和CPU优化: 进行中

## 16. 文档和用户指南
- 16.1 README（中英西）: 完成(简化)(已新增三语README，覆盖项目定位、合法安全边界、Apple Silicon快速开始与当前能力概览；后续可继续扩展架构图、API示例与部署细节)
- 16.2 API文档: 未完成(已补充开源许可证基础文件`LICENSE`并完成crate元数据对齐，API接口文档正文仍待补全)
- 16.3 用户指南和教程: 未完成

## 17. GPU加速与Metal MPS集成
- 17.1 Metal MPS后端配置（candle-metal）: 进行中
- 17.2 GPU加速Embedding计算: 进行中
- 17.3 GPU加速矩阵运算: 完成
- 17.4 性能测试和基准测试: 未完成
