from playwright.sync_api import sync_playwright
from pathlib import Path
import json
root=Path(__file__).parent
results=[]
with sync_playwright() as p:
 b=p.chromium.launch(executable_path='/usr/bin/chromium',headless=True,args=['--no-sandbox']); page=b.new_page(viewport={'width':1440,'height':1050})
 errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
 page.goto('http://127.0.0.1:4313');page.wait_for_selector('h1')
 def check(name,ok):
  assert ok,name
  results.append(name)
 def click(label): page.get_by_role('button',name=label,exact=True).first.click()
 check('Assistant overview renders',page.get_by_role('heading',name='谁来接手这件事').is_visible())
 page.screenshot(path=str(root/'screenshots/assistants-zh.png'),full_page=True)
 click('创建助手');click('确认并创建');check('Empty assistant name rejected',page.get_by_text('给助手起一个名字。',exact=True).is_visible())
 page.locator('#assistant-name').fill('审阅助手');page.get_by_label('新建密钥',exact=True).uncheck();click('确认并创建');check('Assistant created with visible selected grants',page.get_by_role('heading',name='审阅助手').is_visible())
 click('管理会话');page.get_by_label('默认工作会话',exact=True).select_option('');click('发消息');page.get_by_label('消息',exact=True).fill('测试未指定会话的积压');click('发送（演示）');check('No default queues assistant message',page.get_by_text('等待分配',exact=True).is_visible())
 click('管理接收位置');page.get_by_label('默认工作会话',exact=True).select_option('daily');click('收件箱');check('Adding default assigns queued assistant message',page.get_by_text('已分配 · 尚未领取',exact=True).is_visible())
 click('模拟会话领取');check('Claim differs from notification',page.get_by_text('已领取',exact=True).is_visible());click('模拟完成回执');check('Processing receipt exposed',page.get_by_text('已处理',exact=True).is_visible())
 click('助手');click('管理会话');click('模拟离线');check('Offline preserves default',page.get_by_label('默认工作会话',exact=True).input_value()=='daily')
 page.get_by_role('button',name='移除会话',exact=True).first.click();click('确认移除');check('Deleted default replaced',page.get_by_label('默认工作会话',exact=True).input_value()=='login')
 click('移除会话');click('确认移除');check('Last session deletion clears default',page.get_by_label('默认工作会话',exact=True).input_value()=='');click('收件箱');check('Completed history survives session deletion',page.get_by_text('已处理',exact=True).is_visible());click('助手');click('管理会话')
 click('注册会话');page.get_by_label('会话名称',exact=True).fill('新工作会话');click('注册（演示）');check('Registration defaults to listed proposal',page.get_by_label('在其他助手的收件人列表中展示',exact=True).is_checked())
 page.get_by_label('在其他助手的收件人列表中展示',exact=True).check();click('发消息');page.get_by_label('指定工作会话（可选）').select_option(label='新工作会话');page.get_by_label('消息',exact=True).fill('指定给新会话');click('发送（演示）');check('Explicit target preserved',page.get_by_text('发件人选择：指定会话').is_visible())
 click('助手');click('管理会话');click('移除会话');click('确认移除');click('收件箱');check('Deleted explicit target blocked not rerouted',page.get_by_text('指定会话已删除',exact=True).is_visible())
 click('后台接收');click('模拟身份不一致');check('Identity mismatch blocks wake',page.get_by_role('button',name='推进演示进度').is_disabled());click('恢复正确身份');click('暂停接收');check('Paused bridge blocks wake',page.get_by_role('button',name='推进演示进度').is_disabled());click('恢复接收');click('推进演示进度');page.screenshot(path=str(root/'screenshots/bridge-zh.png'),full_page=True)
 click('模拟通知结果未知');check('Unknown notification blocks automatic retry',page.get_by_role('button',name='推进演示进度').is_disabled());click('模拟核对：已领取');check('Verified claim resolves unknown',page.get_by_text('同一任务只保留一个当前执行者',exact=True).is_visible())
 click('接入应用');check('Sign in does not request assistant',page.get_by_role('heading',name='先登录，再开始工作').is_visible());click('使用 Google 登录（演示）');click('确认挂载');check('Attachment inherits assistant grant',page.get_by_role('button',name='已挂载（演示）').is_disabled())
 click('发消息');page.get_by_label('收件助手').select_option('research');page.get_by_label('消息',exact=True).fill('研究助手待分配消息');click('发送（演示）');page.get_by_role('button',name='管理接收位置',exact=True).first.click();check('Research message opens research settings',page.get_by_text('助手 / 研究助手',exact=True).is_visible());click('注册会话');page.get_by_label('会话名称',exact=True).fill('研究工作会话');click('注册（演示）');page.get_by_label('默认工作会话',exact=True).select_option(label='研究工作会话');click('收件箱');check('Research queued message assigned within research assistant',page.get_by_text('实际分配：研究工作会话',exact=False).is_visible())
 click('助手');click('编辑权限');page.get_by_label('修改密钥',exact=True).uncheck();click('保存助手权限（演示）');click('编辑权限');check('Assistant permission edit persists independently',not page.get_by_label('修改密钥',exact=True).is_checked());page.screenshot(path=str(root/'screenshots/permissions-zh.png'),full_page=True)
 click('助手');click('管理会话');click('解除挂载');click('确认解除');check('Client detachment is visible',page.get_by_text('已解除挂载 · 访问已停止',exact=True).is_visible());click('后台接收');check('Detached receiver blocks wake despite online bridge',page.get_by_role('button',name='推进演示进度').is_disabled())
 click('助手');click('管理会话');click('重新挂载（演示）');click('注册会话');page.get_by_label('会话名称',exact=True).fill('接手会话');click('注册（演示）');click('收件箱');page.get_by_label('交接给工作会话').select_option(label='接手会话');click('确认交接（演示）');check('Explicit handoff preserves sender target',page.get_by_text('发件人选择：指定会话',exact=False).is_visible());check('Explicit handoff changes execution assignment',page.get_by_text('实际分配：接手会话',exact=False).is_visible())
 # Fresh state: bilingual and viewport checks across seven pages.
 page.reload();page.wait_for_selector('h1')
 for language in ['zh','en']:
  if language=='en':click('English')
  labels=['助手','收件箱','发消息','后台接收','接入应用'] if language=='zh' else ['Assistants','Inbox','Compose','Background receiver','Connect an app']
  for w in [390,768,1440]:
   page.set_viewport_size({'width':w,'height':950})
   for label in labels:
    click(label)
    check(f'{language} {w} {label}: no horizontal overflow',page.evaluate('document.documentElement.scrollWidth <= innerWidth'))
   click(labels[0]);click('创建助手' if language=='zh' else 'Create assistant');check(f'{language} {w} create fits',page.evaluate('document.documentElement.scrollWidth <= innerWidth'))
   click(labels[0]);click('管理会话' if language=='zh' else 'Manage sessions');check(f'{language} {w} sessions fit',page.evaluate('document.documentElement.scrollWidth <= innerWidth'))
   if w==390:page.screenshot(path=str(root/f'screenshots/sessions-{language}-mobile.png'),full_page=True)
 check('No browser runtime errors',not errors)
 # accessibility scan if existing offline axe is available
 if Path('/workspace/scratch/axe.min.js').exists():
  page.add_script_tag(path='/workspace/scratch/axe.min.js')
  axe=page.evaluate("async()=>await axe.run(document,{runOnly:{type:'tag',values:['wcag2a','wcag2aa','wcag21aa']}})")
  (root/'axe-results.json').write_text(json.dumps({'violations':axe['violations']},ensure_ascii=False,indent=2))
  check('Current sessions screen axe WCAG A/AA has no violations',not axe['violations'])
 b.close()
(root/'validation.json').write_text(json.dumps({'passed':len(results),'checks':results,'browserErrors':errors,'scope':'Synthetic local Chromium only; no real OAuth, host or production validation'},ensure_ascii=False,indent=2))
print(json.dumps({'passed':len(results),'errors':errors}))
