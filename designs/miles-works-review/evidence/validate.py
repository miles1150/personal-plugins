from playwright.sync_api import sync_playwright,expect
from pathlib import Path
import json,time,os,shutil
R=Path(__file__).resolve().parents[1]
BASE='http://127.0.0.1:4311/'
results=[];errors=[];external=[];axe_results={};overflow=[]
def record(n):results.append(n)
with sync_playwright() as p:
 b=p.chromium.launch(executable_path=os.environ.get('CHROMIUM_PATH',shutil.which('chromium')),args=['--no-sandbox'])
 g=b.new_page(viewport={'width':1440,'height':1050},device_scale_factor=1)
 g.on('pageerror',lambda e:errors.append(str(e)))
 def route(req):
  if not req.request.url.startswith(BASE):external.append(req.request.url);req.abort()
  else:req.continue_()
 g.route('**/*',route)
 def fresh(v):g.goto(BASE+'?review='+str(time.time())+'#'+v);g.wait_for_selector('main')
 def action(a):g.locator('[data-action="'+a+'"]').first.click()
 fresh('inbox');expect(g.get_by_role('heading',name='收件箱',exact=True)).to_be_visible();record('主页面加载，合成数据标识与状态语义可见')
 action('filter-attention');expect(g.get_by_role('heading',name='本机接收已暂停，消息仍然保存')).to_be_visible();record('需处理筛选与详情匹配')
 action('filter-all');action('reply');expect(g.locator('#send-to')).to_have_value('Claude · 调研');action('send');expect(g.locator('#send-error')).to_contain_text('请填写');g.locator('#send-body').fill('合成回复');action('send');expect(g.get_by_role('heading',name='消息已存入云端')).to_be_visible();action('close');record('回复明确收件人、空值校验、保存与通知状态分离')
 fresh('inbox');action('compose');g.locator('#send-space').select_option('旅行计划');expect(g.locator('#send-to')).to_have_value('行程调研助手');assert g.locator('#send-to').inner_text().find('Codex')<0;record('发送切换 Space 后收件人重新解析，旧目标不残留');action('close')
 fresh('connect');action('client-Claude');action('step-2');action('step-3');expect(g.locator('#origin-error')).to_contain_text('请先确认');g.locator('#approve-origin').check();action('step-3');expect(g.get_by_role('heading',name='消息连接已准备好')).to_be_visible();action('roundtrip');expect(g.get_by_text('收发均已确认 · 合成测试')).to_be_visible();record('Claude 首次接入完整点击链；防误批准、合成往返与后台未开启说明')
 fresh('grok');expect(g.get_by_text('https://waypost.miles1150.top/mcp',exact=True)).to_be_visible();expect(g.get_by_text('无需插件，直接添加远程 MCP')).to_be_visible();record('Grok 无插件路径与实际宿主待验收边界')
 fresh('keys-consent');assert g.locator('.onboard-card input:checked').count()==0;action('approve-keys');expect(g.locator('#keys-error')).to_contain_text('至少选择');g.locator('#key-maps-read').check();action('approve-keys');expect(g.locator('#modal')).to_contain_text('maps/travel · 读取');assert 'github/research · 读取' not in g.locator('#modal').inner_text();action('keys-granted');expect(g.get_by_role('heading',name='密钥',exact=True)).to_be_visible();expect(g.locator('.list-row').filter(has_text='maps/travel')).to_contain_text('Claude 可读取');expect(g.locator('.list-row').filter(has_text='github/research')).to_contain_text('未向当前助手授权');record('密钥初始无选中、具体条目摘要、保存后准确显示获批条目')
 fresh('keys-consent');g.locator('summary').click();g.locator('#all-read').check();action('approve-keys');expect(g.locator('#modal')).to_contain_text('读取全部现有与未来条目');action('close');record('全量读取明确包含未来条目，不伪装为所选单项')
 fresh('recovery');action('bridge-setup');action('bridge-run');expect(g.locator('#bridge-error')).to_contain_text('持续运行');g.locator('#bridge-start').check();action('bridge-run');expect(g.get_by_text('接收运行中',exact=True).first).to_be_visible();expect(g.get_by_text('1 条结果不确定',exact=True)).to_be_visible();record('本机持续运行需明确确认；恢复接收不清空未知状态')
 action('reconcile');action('unknown-remains');action('retry-unknown');expect(g.locator('#retry-error')).to_contain_text('独立确认');action('close');expect(g.get_by_text('1 条结果不确定',exact=True)).to_be_visible();record('找不到标记仍保留 unknown，未经独立确认无法重试')
 action('reconcile');action('known-accepted');expect(g.get_by_text('已完成核对',exact=True)).to_be_visible();record('找到原通知记录后确认，不重复发送')
 fresh('spaces');action('create-space');g.locator('#new-space-name').fill('<demo>');action('save-space');expect(g.get_by_role('heading',name='<demo>',exact=True)).to_be_visible();assert g.locator('demo').count()==0;action('open-space-2');expect(g.get_by_role('heading',name='这个 Space 还没有消息')).to_be_visible();record('创建提案可交互、文本被转义；新 Space 不显示个人空间消息')
 action('compose');expect(g.locator('#send-to')).to_have_value('自己');assert g.locator('#send-to option').count()==1;action('close');record('新 Space 没有虚构助手收件人')
 fresh('spaces');action('space-intent');expect(g.locator('#modal')).to_contain_text('尚未实现');action('intent-approved');expect(g.get_by_role('heading',name='旅行研究',exact=True)).to_be_visible();record('MCP 管理意图批准示意明确未实现，无凭据输出')
 fresh('security');action('revoke');expect(g.locator('#modal')).to_contain_text('另一份凭据');action('revoke-confirm');expect(g.get_by_role('button',name='撤销消息连接',exact=True)).to_be_disabled();record('撤销明确不同凭据边界，结果在原型内保留')
 # Native dialog focus constraints and Escape return.
 fresh('inbox');trigger=g.locator('[data-action="compose"]');trigger.click();g.keyboard.press('Escape');expect(g.locator('#modal')).not_to_be_visible();expect(trigger).to_be_focused();record('原生 dialog 支持 Esc，并返回触发控件焦点');trigger.click();
 for _ in range(18):
  g.keyboard.press('Tab');assert g.evaluate('document.activeElement===document.body || document.getElementById("modal").contains(document.activeElement)')
 g.keyboard.press('Escape');record('18 次 Tab 未进入背景控件；允许浏览器 chrome 取焦')
 # Relevant static views and accessibility scan (synthetic local UI only).
 for view in ['inbox','connect','keys-consent','recovery','connections','spaces','security','materials']:
  fresh(view);g.add_script_tag(path=os.environ.get('AXE_CORE_PATH','/workspace/scratch/axe.min.js'));res=g.evaluate('async()=>{const r=await axe.run(document,{runOnly:{type:"tag",values:["wcag2a","wcag2aa","wcag21aa","wcag22aa"]}});return {violations:r.violations.map(v=>({id:v.id,impact:v.impact,help:v.help,nodes:v.nodes.map(n=>n.target)})),incomplete:r.incomplete.map(v=>v.id)}}');axe_results[view]=res
  g.screenshot(path=str(R/'screenshots'/(view+'-desktop.png')),full_page=True)
 # Multiple widths and every core view, check actual document overflow.
 for width in [320,390,768,1024,1440]:
  g.set_viewport_size({'width':width,'height':900})
  for view in ['inbox','connect','keys-consent','recovery','connections','spaces','security','materials']:
   fresh(view);dims=g.evaluate('({scroll:document.documentElement.scrollWidth,view:innerWidth})')
   if dims['scroll']>dims['view']+1:overflow.append({'width':width,'view':view,**dims})
 record('40 个视口／页面组合检查横向溢出')
 g.set_viewport_size({'width':390,'height':844});fresh('inbox');action('message-0');expect(g.locator('.detail-pane')).to_be_visible();g.screenshot(path=str(R/'screenshots/inbox-mobile.png'),full_page=True);action('back-list');expect(g.locator('.message-list')).to_be_visible();expect(g.get_by_role('button',name='安全与撤销',exact=True)).to_be_visible();record('手机消息详情返回与账户安全入口保留')
 fresh('keys-consent');g.screenshot(path=str(R/'screenshots/keys-mobile.png'),full_page=True)
 # Offline standalone and generated DS preview must not require any remote requests.
 g.set_viewport_size({'width':1440,'height':1050});g.goto(BASE+'prototype-standalone.html#inbox');expect(g.get_by_role('heading',name='收件箱',exact=True)).to_be_visible();record('单文件产品原型可载入，未请求任何外部资源')
 g.goto(BASE+'system/preview.html');g.wait_for_timeout(1000);assert g.locator('button').count()>5;assert g.evaluate('Array.from(document.querySelectorAll("*")).filter(x=>x.shadowRoot).length')==24;g.get_by_role('button',name='打开撤销对话框',exact=True).click();expect(g.get_by_role('dialog')).to_be_visible();g.keyboard.press('Escape');expect(g.get_by_role('dialog')).not_to_be_visible();g.get_by_role('button',name='打开撤销对话框',exact=True).click();expect(g.get_by_role('dialog')).to_be_visible();g.keyboard.press('Escape');record('设计系统 24 个隔离卡挂载；Dialog 可 Esc 关闭并再次打开');g.screenshot(path=str(R/'screenshots/design-system-desktop.png'),full_page=True);record('生成的设计系统预览加载；实际 React 组件卡挂载')
 # Journey map scenario switching.
 g.goto(BASE+'journeys.html');g.get_by_role('button',name='后台接收与异常',exact=True).click();expect(g.get_by_role('heading',name='后台接收与异常',exact=True)).to_be_visible();assert g.locator('.node').count()==6;record('7 条可切换旅程图，每条 6 步并有恢复路径')
 g.goto(BASE+'review.html');expect(g.get_by_role('heading',name='判断',exact=True)).to_be_visible();record('完整报告 HTML 可阅读')
 b.close()
summary={'checks_passed':len(results),'checks':results,'page_errors':errors,'external_requests':external,'overflow':overflow,'axe':axe_results}
(R/'evidence/validation.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2))
print(json.dumps({'checks_passed':len(results),'page_errors':errors,'external_requests':external,'overflow':overflow,'axe_violations':{k:v['violations'] for k,v in axe_results.items()}},ensure_ascii=False,indent=2))
assert not errors and not external and not overflow
assert not any(v['violations'] for v in axe_results.values())
