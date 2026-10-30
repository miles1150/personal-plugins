import React from 'react';
export function ConnectionRow({name,detail,status,onManage}) {return <div className="row" style={{padding:'var(--space-4)',borderBottom:'1px solid var(--border)'}}><div><strong>{name}</strong><p className="small" style={{margin:0}}>{detail}</p></div><span className="tag">{status}</span><button type="button" className="secondary" onClick={onManage}>管理</button></div>;}
