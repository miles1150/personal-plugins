from pathlib import Path
from playwright.sync_api import sync_playwright
import json
root=Path(__file__).parent; checks=[]
with sync_playwright() as p:
 b=p.chromium.launch(executable_path='/usr/bin/chromium',args=['--no-sandbox']);pg=b.new_page(viewport={'width':1440,'height':1050});errors=[];pg.on('pageerror',lambda e:errors.append(str(e)))
 pg.goto('http://localhost:4313');pg.wait_for_selector('h1')
 def click(s):pg.get_by_role('button',name=s,exact=True).first.click()
 def send(text):
  click('发消息');pg.get_by_label('消息',exact=True).fill(text);click('发送（演示）')
 send('已经完成的任务');click('模拟会话领取');click('模拟完成回执');send('等待领取的任务')
 click('后台接收');click('模拟通知结果未知')
 click('助手');click('管理会话');click('解除挂载');click('确认解除')
 assert pg.get_by_text('已解除挂载 · 不再接收',exact=True).is_visible();checks.append('Detached client sessions marked ineligible')
 assert pg.get_by_label('默认工作会话',exact=True).input_value()=='login';checks.append('Default falls back to remaining client session')
 click('收件箱');assert pg.get_by_text('已处理',exact=True).is_visible();assert pg.get_by_text('待交接 · 不自动重跑',exact=True).is_visible();assert pg.get_by_role('button',name='模拟会话领取',exact=True).count()==0;checks.append('Pending task requires handoff, completed task retained, no claim action for detached session')
 click('发消息');options=pg.get_by_label('指定工作会话（可选）').locator('option').all_text_contents();assert not any('日常开发' in x for x in options);checks.append('Detached session removed from new recipient options')
 click('后台接收');assert pg.get_by_text('通知请求超时，结果未知。先核对目标会话与领取记录，不自动重复唤醒。',exact=False).is_visible();assert pg.get_by_role('button',name='推进演示进度').is_disabled();checks.append('Unknown notification preserved; detachment does not replay it')
 pg.screenshot(path=str(root/'screenshots/bridge-detached-zh.png'),full_page=True)
 # Separate fresh instance to check editable removal candidate with identity and capability.
 pg.reload();pg.wait_for_selector('h1');click('管理会话');pg.get_by_role('button',name='移除会话',exact=True).first.click();select=pg.get_by_label('替换默认会话',exact=True)
 label=select.locator('option:checked').inner_text();assert all(x in label for x in ['Claude Code','MacBook','离线','需主动查收']);checks.append('Replacement preview includes client, online state and receiving capability')
 pg.screenshot(path=str(root/'screenshots/remove-default-zh.png'),full_page=True)
 select.select_option('');click('确认移除');assert pg.get_by_label('默认工作会话',exact=True).input_value()=='';checks.append('User can explicitly choose no replacement default')
 pg.set_viewport_size({'width':390,'height':950});pg.reload();pg.wait_for_selector('h1');click('管理会话');pg.screenshot(path=str(root/'screenshots/sessions-zh-mobile.png'),full_page=True);click('English');pg.screenshot(path=str(root/'screenshots/sessions-en-mobile.png'),full_page=True)
 assert not errors;checks.append('No runtime errors in targeted routing checks');b.close()
(root/'detachment-validation.json').write_text(json.dumps({'passed':len(checks),'checks':checks,'browserErrors':errors,'scope':'Targeted detachment and replacement checks only'},ensure_ascii=False,indent=2));print(json.dumps({'passed':len(checks),'errors':errors}))
