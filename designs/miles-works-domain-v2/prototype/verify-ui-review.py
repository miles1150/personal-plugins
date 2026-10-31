from pathlib import Path
from playwright.sync_api import sync_playwright
import json
root=Path(__file__).parent;checks=[];errors=[]
with sync_playwright() as p:
 b=p.chromium.launch(executable_path='/usr/bin/chromium',args=['--no-sandbox']);pg=b.new_page(viewport={'width':1440,'height':1000});pg.on('pageerror',lambda e:errors.append(str(e)))
 pg.goto('http://localhost:4313');pg.wait_for_selector('h1')
 def click(x):pg.get_by_role('button',name=x,exact=True).first.click()
 def check(label,condition):assert condition,label;checks.append(label)
 def tools():pg.locator('.prototype-tools > summary').click()
 def outside_demo_buttons():return pg.locator('main section button').all_text_contents()
 check('Navigation has no independent connect-app tab',pg.locator('nav').inner_text()=='助手\n收件箱\n发消息\n后台接收')
 check('Account control is in top-right header',pg.locator('.topline').get_by_role('button',name='账户 Miles').is_visible())
 check('Prototype tools collapsed initially',not pg.locator('.prototype-tools').evaluate('(e)=>e.open'))
 pg.screenshot(path=str(root/'screenshots/assistants-zh.png'),full_page=True)
 click('查看助手')
 check('Assistant detail contains attachment entry',pg.get_by_role('button',name='挂载客户端',exact=True).is_visible())
 check('No lifecycle simulation in normal session UI',not any('模拟' in s or '注册会话' in s for s in outside_demo_buttons()))
 click('挂载客户端');check('Attachment stays in assistant detail',pg.get_by_role('heading',name='开发助手',exact=True).is_visible() and pg.get_by_role('button',name='确认挂载',exact=True).is_visible());click('确认挂载');check('Attachment confirmation renders in place',pg.get_by_role('button',name='已挂载',exact=True).is_disabled());click('关闭')
 pg.screenshot(path=str(root/'screenshots/sessions-zh-desktop.png'),full_page=True)
 click('编辑权限');check('Secret scope remains visible',pg.get_by_text('密钥范围：全部现有及未来密钥。',exact=False).is_visible());pg.screenshot(path=str(root/'screenshots/permissions-zh.png'),full_page=True)
 click('助手');click('查看助手');tools();click('模拟客户端登记');pg.locator('#session-name').fill('自动登记示例');click('注册（演示）');check('Demo registration adds session to correct assistant',pg.get_by_role('heading',name='自动登记示例',exact=True).is_visible());pg.locator('.prototype-tools > summary').click()
 click('移除会话');check('Removal impact still explicit',pg.get_by_text('只移除 Miles Works 中的注册，不删除 Codex／Claude 原始对话。',exact=True).is_visible());check('Default replacement remains editable',pg.get_by_label('替换默认会话',exact=True).is_visible());click('保留会话')
 click('助手');pg.get_by_role('button',name='查看助手',exact=True).nth(1).click();check('Research detail has no developer attachments',pg.get_by_text('尚未挂载客户端',exact=True).is_visible() and pg.get_by_role('button',name='解除挂载').count()==0)
 click('挂载客户端');check('Research attachment inherits research permissions',pg.get_by_text('继承权限：收发消息',exact=True).is_visible());click('确认挂载');click('关闭');check('Research attachment visible in research detail',pg.get_by_text('已挂载 · 设备待确认',exact=True).is_visible())
 click('后台接收');check('No simulator in ordinary bridge controls',not any('模拟' in s or '推进' in s or '重置' in s for s in outside_demo_buttons()));tools();click('模拟通知结果未知');check('Unknown is visible as a diagnosis',pg.get_by_text('通知请求超时，结果未知。',exact=False).is_visible());check('Unknown blocks demonstration replay',pg.get_by_role('button',name='推进演示进度').is_disabled());pg.locator('.prototype-tools > summary').click();pg.screenshot(path=str(root/'screenshots/bridge-zh.png'),full_page=True)
 click('发消息');pg.get_by_label('消息',exact=True).fill('查看新的收件布局');click('发送');check('Normal inbox has no simulated claims',not any('模拟' in s for s in outside_demo_buttons()));tools();click('模拟会话领取');check('Claim simulation remains available in tools',pg.get_by_text('已领取',exact=True).is_visible())
 click('账户 Miles');click('退出登录');check('Sign-in is standalone without navigation',pg.locator('nav').count()==0 and pg.get_by_role('button',name='使用 Google 登录').is_visible());pg.screenshot(path=str(root/'screenshots/login-zh.png'),full_page=True);click('使用 Google 登录');check('Sign-in returns to app',pg.get_by_role('heading',name='助手',exact=True).is_visible())
 # Only modified layouts, two sizes and languages; not the previous full matrix.
 for lang in ['zh','en']:
  if lang=='en':click('English')
  click('查看助手' if lang=='zh' else 'View assistant');pg.set_viewport_size({'width':390,'height':950});check(lang+' revised mobile detail fits',pg.evaluate('document.documentElement.scrollWidth <= innerWidth'));pg.screenshot(path=str(root/f'screenshots/sessions-{lang}-mobile.png'),full_page=True)
  if lang=='zh':click('助手')
  else:click('Assistants')
 check('No runtime errors',not errors);b.close()
(root/'ui-review-validation.json').write_text(json.dumps({'passed':len(checks),'checks':checks,'browserErrors':errors,'scope':'Targeted navigation, control separation, attachment and revised layout checks'},ensure_ascii=False,indent=2));print(json.dumps({'passed':len(checks),'errors':errors}))
