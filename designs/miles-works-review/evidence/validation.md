# 本轮恢复与复跑记录

2026-10-30（新加坡）。从设计聊天记录恢复源码并应用后续修正，不声称与原 ZIP 完全相同。

- baoyu-design 0fd52fc：44 tokens、12 组件、24 卡片，checker clean。
- 原方案重跑：24 项交互；40 组视口无横向溢出；8 页 axe 所测规则无违规；0 pageerror、0 外部请求。见 validation.json。
- 推荐方案：全库/任务预设互斥切换、摘要、默认身份折叠后登录、接收身份不匹配诊断实际点击通过；390/1440 下三页无横向溢出或所测 axe 违规。见 proposal-validation.json。
- 全部为合成数据；没有真实 Google 登录、服务调用或设备控制。
- 浏览器：当前 Chromium/Playwright。未做 Safari、读屏器及真机测试。自动 axe 无违规不是完整无障碍认证。
- localhost:4311 是当前 cloud 环境内的静态预览，不能当作用户 Mac 本地地址或公开部署。
