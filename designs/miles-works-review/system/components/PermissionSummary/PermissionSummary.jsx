import React from 'react';
export function PermissionSummary({space,sensitive=false}) {return <div className={'notice '+(sensitive?'warning':'')}><strong>{sensitive?'密钥读取':'Space 成员访问'} · {space}</strong><p className="small" style={{margin:'var(--space-2) 0 0'}}>{sensitive?'所选条目的值会返回给助手，进入宿主与模型上下文。':'可发送消息、读取该身份的收发消息，并读写本 Space 的共享资料和封存附件。'}</p></div>;}
