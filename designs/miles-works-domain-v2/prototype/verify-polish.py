from pathlib import Path
from playwright.sync_api import sync_playwright
import json
root=Path(__file__).parent
checks=[]
with sync_playwright() as p:
 browser=p.chromium.launch(executable_path='/usr/bin/chromium',headless=True,args=['--no-sandbox'])
 page=browser.new_page(viewport={'width':1440,'height':1050}); errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
 page.goto('http://127.0.0.1:4313');page.wait_for_selector('h1')
 page.get_by_role('button',name='编辑权限',exact=True).click()
 rows=page.locator('.form label.check')
 assert rows.count()==4
 for row in rows.all():
  rb=row.bounding_box();ib=row.locator('input').bounding_box();tb=row.locator('span').bounding_box()
  assert rb['height']>=44 and tb['x']>ib['x'] and abs((ib['y']+ib['height']/2)-(tb['y']+tb['height']/2))<3
 checks.append('Four permission rows: checkbox left, text right, aligned centers, >=44px targets')
 first=rows.first;before=first.locator('input').is_checked();first.click(position={'x':first.bounding_box()['width']-8,'y':22});assert first.locator('input').is_checked()!=before;first.click(position={'x':first.bounding_box()['width']-8,'y':22})
 checks.append('Clicking blank right side of permission row toggles checkbox')
 page.screenshot(path=str(root/'screenshots/permissions-zh.png'),full_page=True)
 page.get_by_role('button',name='助手',exact=True).click();page.get_by_role('button',name='管理会话',exact=True).click();page.set_viewport_size({'width':390,'height':950})
 assert page.get_by_role('button',name='移除会话',exact=True).count()==2
 page.screenshot(path=str(root/'screenshots/sessions-zh-mobile.png'),full_page=True)
 page.get_by_role('button',name='移除会话',exact=True).first.click();assert page.get_by_text('只移除 Miles Works 中的注册，不删除 Codex／Claude 原始对话。',exact=True).is_visible();page.get_by_role('button',name='保留会话',exact=True).click()
 checks.append('Chinese remove action opens registration-only confirmation')
 page.get_by_role('button',name='English',exact=True).click();assert page.get_by_role('button',name='Remove session',exact=True).count()==2
 page.screenshot(path=str(root/'screenshots/sessions-en-mobile.png'),full_page=True)
 checks.append('English action consistently says Remove session')
 assert not errors;checks.append('No runtime errors in targeted polish checks')
 browser.close()
(root/'polish-validation.json').write_text(json.dumps({'passed':len(checks),'checks':checks,'browserErrors':errors,'scope':'Targeted follow-up only; original 71-check matrix not repeated'},ensure_ascii=False,indent=2))
print(json.dumps({'passed':len(checks),'errors':errors}))
